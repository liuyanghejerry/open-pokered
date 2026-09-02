// ───────────────────────────────────────────────────────────────────────────
// Game publish routes for the backend-hosted editor (Vite dev + Electron).
//
// The static-hosting publish path lives entirely in the browser
// (src/publish/publish.ts → single-file HTML download). When a backend IS
// available, publishing instead produces the standard multi-file web layout —
// the pokered counterpart of `dotzuki export --web`:
//
//   <projectRoot>/dist/publish/
//   ├── index.html                    # player page (rendered by the client
//   │                                 #   via renderWebDirPlayerHtml — the
//   │                                 #   template is a renderer concern, the
//   │                                 #   server stays template-free)
//   ├── data.json                     # FULL replayable data set from
//   │                                 #   crates/pokered-data (not just
//   │                                 #   deltas — the artifact matches the
//   │                                 #   edited repo even if the runner wasm
//   │                                 #   predates the edits)
//   └── wasm/pokered_runner_web.js    # the deploy's runner pkg (prebuilt)
//       wasm/pokered_runner_web_bg.wasm
//
// The directory is served under /published/ so the user can play immediately;
// it is also a plain static dir that any web server (or a zip of it) can host.
// gfx is compiled into the runner binary, so it is not exported (same
// limitation as the in-editor playtest).
// ───────────────────────────────────────────────────────────────────────────
import path from 'path'
import fs from 'fs'
import type { IncomingMessage, ServerResponse } from 'http'
import { sendJson, sendError, readBody } from './api/http'
import { getProjectRoot } from './api/projectConfig'

const PUBLISH_URL_PREFIX = '/published'
const WASM_FILES = ['pokered_runner_web.js', 'pokered_runner_web_bg.wasm'] as const

/** Output directory of the web export, under the current project root. */
export function publishOutDir(): string {
  return path.join(getProjectRoot(), 'dist', 'publish')
}

/** The runner wasm pkg dir. Keep in sync with pokeredRoutes.ts roots(): a
 *  packaged Electron app merges both pkgs into DOTZUKI_WASM_ROOT. */
function runnerPkgRoot(): string {
  return process.env.DOTZUKI_WASM_ROOT
    ? path.resolve(process.env.DOTZUKI_WASM_ROOT)
    : path.resolve(getProjectRoot(), 'crates/pokered-runner-web/pkg')
}

/**
 * Collect the full replayable data set from crates/pokered-data — the same
 * file paths the player's applyEdits() understands:
 *   maps/<Map>/map.json|map.blk|script.scene|script_config.json,
 *   trainers|moves|items|pokemon/<name>.json
 * All content is a string: JSON/scene files verbatim; the binary map.blk as
 * a number-array JSON string (the exact shape /api/maps/<map>/map.blk serves
 * and runtime_overrides::set_map_blk_override parses).
 */
export function collectGameDataFiles(): { path: string; content: string }[] {
  const dataRoot = path.join(getProjectRoot(), 'crates/pokered-data')
  const out: { path: string; content: string }[] = []
  const mapsRoot = path.join(dataRoot, 'maps')
  if (fs.existsSync(mapsRoot)) {
    for (const d of fs.readdirSync(mapsRoot, { withFileTypes: true })) {
      if (!d.isDirectory()) continue
      for (const f of ['map.json', 'map.blk', 'script.scene', 'script_config.json']) {
        const abs = path.join(mapsRoot, d.name, f)
        if (!fs.existsSync(abs)) continue
        const content = f === 'map.blk'
          ? JSON.stringify(Array.from(fs.readFileSync(abs)))
          : fs.readFileSync(abs, 'utf-8')
        out.push({ path: `maps/${d.name}/${f}`, content })
      }
    }
  }
  for (const dir of ['trainers', 'moves', 'items', 'pokemon']) {
    const absDir = path.join(dataRoot, dir)
    if (!fs.existsSync(absDir)) continue
    for (const f of fs.readdirSync(absDir)) {
      if (!f.endsWith('.json')) continue
      out.push({ path: `${dir}/${f}`, content: fs.readFileSync(path.join(absDir, f), 'utf-8') })
    }
  }
  return out.sort((a, b) => a.path.localeCompare(b.path))
}

const CONTENT_TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
}

/** Serve the export directory under /published/ (GET only; 404 — never the
 *  SPA fallback — for missing files, since this prefix belongs to us). */
function servePublished(req: IncomingMessage, res: ServerResponse, next: () => void): void {
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    next()
    return
  }
  const outDir = publishOutDir()
  const rel = decodeURIComponent((req.url || '/').split('?')[0])
  const filePath = path.resolve(outDir, `.${rel}`)
  // Path traversal guard: the resolved path must stay inside the export dir.
  if (!filePath.startsWith(outDir + path.sep) && filePath !== outDir) {
    sendError(res, `Not found: ${rel}`, 404)
    return
  }
  const serveFile = filePath === outDir ? path.join(outDir, 'index.html') : filePath
  if (!fs.existsSync(serveFile) || !fs.statSync(serveFile).isFile()) {
    sendError(res, `Not found: ${rel}`, 404)
    return
  }
  res.setHeader('Content-Type', CONTENT_TYPES[path.extname(serveFile).toLowerCase()] ?? 'application/octet-stream')
  fs.createReadStream(serveFile).pipe(res)
}

/**
 * Mount the publish routes:
 *   POST /api/publish  { title, indexHtml } → write <root>/dist/publish
 *   GET  /published/*               → serve the export directory
 * `indexHtml` is the player page rendered by the client from
 * renderWebDirPlayerHtml() (see the header comment for why the server does
 * not render it).
 */
export function registerPublishRoutes(server: { middlewares: any }) {
  server.middlewares.use('/api/publish', async (req: IncomingMessage, res: ServerResponse, next: () => void) => {
    if (req.method !== 'POST') {
      next()
      return
    }
    try {
      const body = JSON.parse(await readBody(req)) as { indexHtml?: string }
      if (typeof body.indexHtml !== 'string' || !body.indexHtml.includes('<canvas')) {
        sendError(res, 'indexHtml (the player page rendered by renderWebDirPlayerHtml) is required', 400)
        return
      }
      const files = collectGameDataFiles()
      if (files.length === 0) {
        sendError(res, 'no pokered-data files found — check the project root', 400)
        return
      }
      const pkg = runnerPkgRoot()
      for (const f of WASM_FILES) {
        if (!fs.existsSync(path.join(pkg, f))) {
          sendError(
            res,
            `runner pkg missing ${f} in ${pkg} — build it with \`pnpm build:wasm-pokered\``,
            500,
          )
          return
        }
      }

      const out = publishOutDir()
      fs.rmSync(out, { recursive: true, force: true })
      fs.mkdirSync(path.join(out, 'wasm'), { recursive: true })
      for (const f of WASM_FILES) {
        fs.copyFileSync(path.join(pkg, f), path.join(out, 'wasm', f))
      }
      fs.writeFileSync(path.join(out, 'data.json'), JSON.stringify(files))
      fs.writeFileSync(path.join(out, 'index.html'), body.indexHtml)

      const bytes = WASM_FILES.reduce((n, f) => n + fs.statSync(path.join(out, 'wasm', f)).size, 0) +
        body.indexHtml.length
      console.log(`[publish] web export → ${out} (${files.length} data files, ${(bytes / 1024 / 1024).toFixed(1)} MB)`)
      sendJson(res, { ok: true, out, url: `${PUBLISH_URL_PREFIX}/`, fileCount: files.length, bytes })
    } catch (e) {
      sendError(res, (e as Error).message, 500)
    }
  })

  server.middlewares.use(PUBLISH_URL_PREFIX, servePublished)
}
