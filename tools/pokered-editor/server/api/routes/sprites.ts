// @ts-nocheck
import path from 'path'
import fs from 'fs'
import type { IncomingMessage, ServerResponse } from 'http'
import { sendJson, sendError, readBody, parseUrl } from '../http'
import { resolveGfxPath, getProjectRoot } from '../projectConfig'
import { spriteCategories, spriteCategory, resolveGfxSafe, storyActivityConfig, readStoryRecord } from '../storyPaths'
import { pngSize } from '../util'
import { generateSprite } from '../../sprite'
import { generateSingleSprite } from '../../spriteSingle'
import { generateAnimatedSprite, testImageProvider, makeGenImage } from '../../spriteSheet/generate'
import { buildAsepriteJSON } from '../../spriteSheet/aseprite'
import { encodePNG, decodePNG } from '../../spriteSheet/image'
import { listPresets, presetByNameLookup } from '../../spriteSheet/presets'
import { listDirections, directionByKey } from '../../spriteSheet/direction'

export function registerSprites(server: any) {
    // ── POST /api/ai/generate-sprite — appearance + spriteSpec → image → gfx ──
    function spritePromptFor(character: any): string {
      const spec = character.spriteSpec || {}
      return [
        'A single 2D video-game character sprite, pixel-art style, front-facing, full body, clean readable silhouette, plain or transparent background.',
        character.role ? `Role: ${character.role}.` : '',
        character.appearance ? `Appearance: ${character.appearance}.` : '',
        spec.style ? `Art style: ${spec.style}.` : '',
        Array.isArray(spec.palette) && spec.palette.length ? `Limited palette: ${spec.palette.join(', ')}.` : '',
        spec.notes ? `Notes: ${spec.notes}.` : '',
      ].filter(Boolean).join(' ')
    }
    server.middlewares.use('/api/ai/generate-sprite', async (req, res) => {
      if (req.method !== 'POST') return nextMiddleware(req, res)
      try {
        const sc = storyActivityConfig()
        const { characterId, profile, apiKey, size } = JSON.parse(await readBody(req))
        if (!profile || !apiKey) return sendError(res, 'profile and apiKey are required', 400)
        const character = readStoryRecord('characters', characterId)
        if (!character) return sendError(res, 'Character not found', 404)

        const { base64, mediaType } = await generateSprite({
          profile, apiKey,
          prompt: spritePromptFor(character),
          size: size || sc.sprite?.size,
        })
        const ext = mediaType.includes('jpeg') ? '.jpg' : mediaType.includes('webp') ? '.webp' : '.png'
        const rel = path.join(sc.sprite?.dir ?? 'sprites', `${characterId}${ext}`)
        const abs = resolveGfxPath(rel)
        fs.mkdirSync(path.dirname(abs), { recursive: true })
        fs.writeFileSync(abs, Buffer.from(base64, 'base64'))
        sendJson(res, { ok: true, path: rel, mediaType })
      } catch (e) {
        sendError(res, (e as Error).message, 500)
      }
    })

    // ── POST /api/sprites/generate-single — pokered Pixel activity: one static
    //    sprite PNG from a prompt. Reuses the spriteSheet primitives (provider
    //    image call → chroma matte → pixel-grid snap → resample → white flatten)
    //    and returns base64; the client loads it into the Pixel canvas and saving
    //    goes through the usual PUT /gfx/** path. Mounted via registerSprites in
    //    vite.config.ts.
    server.middlewares.use('/api/sprites/generate-single', async (req, res) => {
      if (req.method !== 'POST') return nextMiddleware(req, res)
      try {
        const { profile, apiKey, prompt, width, height, paletteSize } = JSON.parse(await readBody(req))
        if (!profile || !apiKey) return sendError(res, 'profile and apiKey are required', 400)
        if (!prompt || !String(prompt).trim()) return sendError(res, 'prompt is required', 400)
        const result = await generateSingleSprite({
          profile, apiKey, prompt: String(prompt),
          width: Number(width) || 56, height: Number(height) || 56,
          ...(paletteSize !== undefined ? { paletteSize: Number(paletteSize) } : {}),
        })
        sendJson(res, { ok: true, ...result })
      } catch (e) {
        sendError(res, (e as Error).message, 500)
      }
    })

    // ── GET /api/sprites/presets — motion preset catalog (hint stripped) ──
    server.middlewares.use('/api/sprites/presets', (req, res) => {
      if (req.method !== 'GET') return nextMiddleware(req, res)
      try { sendJson(res, listPresets()) }
      catch (e) { sendError(res, (e as Error).message, 500) }
    })

    // ── GET /api/sprites/directions — 8-direction metadata (3×3 grid order) ──
    server.middlewares.use('/api/sprites/directions', (req, res) => {
      if (req.method !== 'GET') return nextMiddleware(req, res)
      try { sendJson(res, listDirections()) }
      catch (e) { sendError(res, (e as Error).message, 500) }
    })

    // Where animated (PerfectPixel-style) sprite sets are stored, per project.
    function animatedDirRel(): string {
      let sc: any = {}
      try { sc = storyActivityConfig() } catch { sc = {} }
      return sc?.sprite?.animatedDir || 'data/gfx/animated'
    }

    // Build the character brief from the request or the story record.
    function animatedDescription(body: any, id: string): string {
      if (body.description && String(body.description).trim()) return String(body.description).trim()
      const ch = readStoryRecord('characters', id)
      if (ch) return [ch.appearance, ch?.spriteSpec?.style, ch?.spriteSpec?.notes].filter(Boolean).join(' ')
      return ''
    }

    // Load an existing base sprite (single reference) for the server-side identity
    // check, if one exists. Best-effort; returns null otherwise.
    function loadBaseSprite(id: string): any {
      let sc: any = {}
      try { sc = storyActivityConfig() } catch { sc = {} }
      for (const rel of [path.join(sc?.sprite?.dir ?? 'sprites', `${id}.png`)]) {
        try {
          const abs = resolveGfxSafe(rel)
          if (fs.existsSync(abs) && fs.statSync(abs).isFile()) return decodePNG(fs.readFileSync(abs))
        } catch { /* ignore */ }
      }
      return null
    }

    const sanitizeStateName = (s: string) => String(s).toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '') || 'state'

    // Resolve the request into the list of states to generate (explicit list, or
    // a preset optionally fanned out across an 8-direction set with mirroring).
    function resolveAnimatedStates(body: any): any[] {
      if (Array.isArray(body.states) && body.states.length) {
        return body.states.map((s: any) => ({
          name: sanitizeStateName(s.name || s.action || 'state'),
          frames: Math.max(1, Math.min(10, Number(s.frames) || 4)),
          fps: Number(s.fps) || 8, loop: s.loop !== false,
          action: String(s.action || ''), facing: String(s.facing || ''),
          ...(s.mirrorOf ? { mirrorOf: sanitizeStateName(s.mirrorOf) } : {}),
        }))
      }
      const presetKey = String(body.preset || '').trim()
      if (!presetKey) return []
      const preset = presetByNameLookup(presetKey)
      const frames = Math.max(1, Math.min(10, Number(body.frames) || preset?.frames || 4))
      const fps = Number(body.fps) || preset?.fps || 8
      const loop = body.loop ?? preset?.loop ?? true
      const action = String(body.action || preset?.action || presetKey)
      const dirs: string[] = Array.isArray(body.directions) ? body.directions.filter(Boolean) : []
      if (dirs.length) {
        const requested = new Set(dirs)
        return dirs.map((d: string) => {
          const di = directionByKey(d)
          const useMirror = di?.mirrorOf && requested.has(di.mirrorOf)
          return {
            name: sanitizeStateName(`${presetKey}-${d}`), frames, fps, loop, action, facing: d,
            ...(useMirror ? { mirrorOf: sanitizeStateName(`${presetKey}-${di!.mirrorOf}`) } : {}),
          }
        })
      }
      return [{ name: sanitizeStateName(presetKey), frames, fps, loop, action, facing: String(body.facing || '') }]
    }

    // ── POST /api/ai/generate-animated — SSE: text → animated sprite sheet ──
    //    Reuses the AI provider config; runs the ported PerfectPixel pipeline
    //    (matte → segment → align → quantize → score, self-correcting up to 3×)
    //    and writes sheet.png + per-frame PNGs + manifest.json under
    //    <gfxRoot>/<animatedDir>/<id>/. Progress streams as SSE events.
    server.middlewares.use('/api/ai/generate-animated', async (req, res) => {
      if (req.method !== 'POST') return nextMiddleware(req, res)
      let started = false
      try {
        const body = JSON.parse(await readBody(req))
        const { profile, apiKey } = body
        const id = path.basename(String(body.id ?? ''))
        if (!profile || !apiKey) return sendError(res, 'profile and apiKey are required', 400)
        if (!id) return sendError(res, 'id is required', 400)
        const states = resolveAnimatedStates(body)
        if (!states.length) return sendError(res, 'Provide `states` or a `preset`.', 400)
        const description = animatedDescription(body, id)
        if (!description) return sendError(res, 'A `description` or a known character id is required.', 400)
        const cellSize = Math.max(16, Math.min(512, Number(body.cellSize) || 64))
        const base = loadBaseSprite(id)

        res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache', Connection: 'keep-alive' })
        started = true
        const send = (event: string, data: unknown) => res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`)
        const ac = new AbortController()
        req.on('close', () => ac.abort())

        try {
          const result = await generateAnimatedSprite({
            profile, apiKey, character: id, description,
            styleKey: String(body.styleKey || 'pixel'), styleCustom: body.styleCustom ? String(body.styleCustom) : undefined,
            states, cellSize, base, feedback: body.feedback ? String(body.feedback) : undefined,
            signal: ac.signal,
            onProgress: (e) => send('progress', e),
          })

          const dirRel = path.posix.join(animatedDirRel(), id)
          const absDir = resolveGfxSafe(dirRel)
          fs.mkdirSync(absDir, { recursive: true })
          fs.writeFileSync(path.join(absDir, 'sheet.png'), encodePNG(result.sheet))
          fs.writeFileSync(path.join(absDir, 'manifest.json'), JSON.stringify(result.manifest, null, 2))
          fs.writeFileSync(path.join(absDir, 'sprite-sheet.json'), buildAsepriteJSON(result.manifest)) // Aseprite-compatible (Phaser/Unity/Godot)
          const frameNames: string[] = []
          for (const st of result.states) {
            st.frames.forEach((f: any, k: number) => {
              const name = `${sanitizeStateName(st.name)}_${String(k).padStart(2, '0')}.png`
              fs.writeFileSync(path.join(absDir, name), encodePNG(f))
              frameNames.push(name)
            })
          }
          send('done', {
            ok: true, dir: dirRel, manifest: result.manifest, frames: frameNames,
            states: result.states.map((s: any) => ({ name: s.name, found: s.found, expected: s.expected, warnings: s.warnings, scores: s.scores })),
          })
        } catch (e) {
          send('error', { message: (e as Error).message })
        }
        res.end()
      } catch (e) {
        if (started) { try { res.end() } catch { /* noop */ } }
        else sendError(res, (e as Error).message, 500)
      }
    })

    // ── GET /api/sprites/animated?id= — existing animated set (manifest + frames) ──
    server.middlewares.use('/api/sprites/animated', (req, res) => {
      if (req.method !== 'GET') return nextMiddleware(req, res)
      try {
        const id = path.basename(parseUrl(req).searchParams.get('id') ?? '')
        if (!id) return sendError(res, 'id is required', 400)
        const dirRel = path.posix.join(animatedDirRel(), id)
        const absDir = resolveGfxSafe(dirRel)
        const manifestPath = path.join(absDir, 'manifest.json')
        if (!fs.existsSync(manifestPath)) return sendJson(res, { exists: false, dir: dirRel })
        const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf-8'))
        const frames = fs.readdirSync(absDir).filter((f) => /\.png$/i.test(f) && f !== 'sheet.png').sort()
        sendJson(res, { exists: true, dir: dirRel, manifest, frames })
      } catch (e) { sendError(res, (e as Error).message, 500) }
    })

    // ── GET /api/sprites/categories — resolved sprite category defs ──
    server.middlewares.use('/api/sprites/categories', (req, res) => {
      if (req.method !== 'GET') return nextMiddleware(req, res)
      try { sendJson(res, spriteCategories()) }
      catch (e) { sendError(res, (e as Error).message, 500) }
    })

    // ── GET /api/sprites/meta?category=&id= — on-disk sprite-set metadata ──
    server.middlewares.use('/api/sprites/meta', (req, res) => {
      if (req.method !== 'GET') return nextMiddleware(req, res)
      try {
        const url = parseUrl(req)
        const catId = url.searchParams.get('category') ?? ''
        const id = path.basename(url.searchParams.get('id') ?? '')
        const cat = spriteCategory(catId)
        if (!cat || !id) return sendError(res, 'category and id are required', 400)
        let sc: any = {}
        try { sc = storyActivityConfig() } catch { sc = {} }
        const dirRel = path.posix.join(cat.dir, id)
        const absDir = resolveGfxSafe(dirRel)
        const meta: any = {
          category: catId, id, dir: dirRel, exists: fs.existsSync(absDir),
          rows: cat.rows, cols: cat.cols, cellW: cat.cellW, cellH: cat.cellH,
          rowNames: cat.rowNames ?? null, colNames: cat.colNames ?? null,
          animated: !!cat.animated, footAnchor: !!cat.footAnchor,
          standCol: cat.standCol ?? 0, walkCols: cat.walkCols ?? null, runCols: cat.runCols ?? null,
          generateConfigured: !!(sc?.sprite?.generateCmd),
          sheet: { exists: false, w: 0, h: 0 }, raw: { exists: false }, frames: [],
        }
        if (meta.exists) {
          const sheetAbs = path.join(absDir, 'sheet.png')
          if (fs.existsSync(sheetAbs)) {
            const dims = pngSize(fs.readFileSync(sheetAbs))
            meta.sheet = { exists: true, w: dims?.w ?? 0, h: dims?.h ?? 0 }
          }
          meta.raw = { exists: fs.existsSync(path.join(absDir, 'raw.png')) }
          meta.frames = fs.readdirSync(absDir)
            .filter(f => /\.png$/i.test(f) && f !== 'sheet.png' && f !== 'raw.png').sort()
        }
        sendJson(res, meta)
      } catch (e) { sendError(res, (e as Error).message, 500) }
    })

    server.middlewares.use('/api/sprites/file', (req, res) => {
      if (req.method !== 'GET') return nextMiddleware(req, res)
      try {
        const url = parseUrl(req)
        const cat = spriteCategory(url.searchParams.get('category') ?? '')
        const id = path.basename(url.searchParams.get('id') ?? '')
        const name = path.basename(url.searchParams.get('name') ?? 'sheet.png')
        if (!cat || !id) { res.writeHead(404); res.end(); return }
        const resolved = resolveGfxSafe(path.posix.join(cat.dir, id, name))
        if (!fs.existsSync(resolved) || fs.statSync(resolved).isDirectory()) { res.writeHead(404); res.end(); return }
        res.writeHead(200, { 'Content-Type': 'image/png', 'Cache-Control': 'no-cache' })
        res.end(fs.readFileSync(resolved))
      } catch { res.writeHead(404); res.end() }
    })

    // ── POST /api/sprites/save — write a sheet.png (+ pre-sliced frames) ──
    //    Slicing happens client-side (canvas); the server only writes bytes.
    server.middlewares.use('/api/sprites/save', async (req, res) => {
      if (req.method !== 'POST') return nextMiddleware(req, res)
      try {
        const { category, id, sheetBase64, frames } = JSON.parse(await readBody(req))
        const cat = spriteCategory(category)
        const safeId = path.basename(id ?? '')
        if (!cat || !safeId) return sendError(res, 'category and id are required', 400)
        const dirRel = path.posix.join(cat.dir, safeId)
        const absDir = resolveGfxSafe(dirRel)
        fs.mkdirSync(absDir, { recursive: true })
        const writePng = (name: string, b64: string) => {
          const clean = String(b64).replace(/^data:image\/\w+;base64,/, '')
          fs.writeFileSync(path.join(absDir, path.basename(name)), Buffer.from(clean, 'base64'))
        }
        if (sheetBase64) writePng('sheet.png', sheetBase64)
        if (Array.isArray(frames)) for (const f of frames) if (f?.name && f?.base64) writePng(f.name, f.base64)
        sendJson(res, { ok: true, dir: dirRel })
      } catch (e) { sendError(res, (e as Error).message, 500) }
    })

    // ── POST /api/sprites/generate — run the project's configured generate cmd ──
    //    Placeholders: {id} {category} {rows} {cols} {cell} {dir} {prompt}. The
    //    command (e.g. the wuxia Gemini character-sprite-gen skill) writes the
    //    sheet itself; we just return its output + the resulting frame list.
    server.middlewares.use('/api/sprites/generate', async (req, res) => {
      if (req.method !== 'POST') return nextMiddleware(req, res)
      try {
        let sc: any = {}
        try { sc = storyActivityConfig() } catch { sc = {} }
        const cmdTmpl = sc?.sprite?.generateCmd
        if (!cmdTmpl) return sendError(res, 'No sprite.generateCmd configured for this project.', 400)
        const { category, id, prompt, apiKey, proxyUrl, model } = JSON.parse(await readBody(req))
        const cat = spriteCategory(category)
        const safeId = path.basename(id ?? '')
        if (!cat || !safeId) return sendError(res, 'category and id are required', 400)
        const dirRel = path.posix.join(cat.dir, safeId)
        const absDir = resolveGfxSafe(dirRel)
        const finalCmd = String(cmdTmpl)
          .replace(/\{id\}/g, safeId)
          .replace(/\{category\}/g, String(category))
          .replace(/\{rows\}/g, String(cat.rows))
          .replace(/\{cols\}/g, String(cat.cols))
          .replace(/\{cell\}/g, `${cat.cellW}x${cat.cellH}`)
          .replace(/\{dir\}/g, dirRel)
          .replace(/\{prompt\}/g, String(prompt ?? '').replace(/"/g, '\\"'))
        const { execSync } = await import('child_process')
        // Bridge the UI-configured image-provider key/model/proxy into the command's
        // environment so the generate script (e.g. the wuxia Gemini skill, which reads
        // GEMINI_KEY) uses the key set in Settings instead of a separate env var. The
        // key travels via env, never the command string (so it isn't logged).
        const cmdEnv: Record<string, any> = { ...process.env }
        if (apiKey) { cmdEnv.GEMINI_KEY = String(apiKey); cmdEnv.GOOGLE_API_KEY = String(apiKey); cmdEnv.JRPG_AI_KEY = String(apiKey) }
        if (model) { cmdEnv.GEMINI_MODEL = String(model); cmdEnv.JRPG_AI_MODEL = String(model) }
        if (proxyUrl) { cmdEnv.HTTPS_PROXY = String(proxyUrl); cmdEnv.https_proxy = String(proxyUrl); cmdEnv.GEMINI_PROXY = String(proxyUrl); cmdEnv.JRPG_AI_PROXY = String(proxyUrl) }
        let result: any
        try {
          const out = execSync(finalCmd, { cwd: getProjectRoot(), env: cmdEnv, encoding: 'utf-8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 600000 })
          result = { ok: true, output: out }
        } catch (e: any) {
          result = { ok: false, output: String(e.stdout || '') + String(e.stderr || '') + (e.message ? '\n' + e.message : '') }
        }
        let frames: string[] = []
        if (fs.existsSync(absDir)) frames = fs.readdirSync(absDir).filter(f => /\.png$/i.test(f)).sort()
        sendJson(res, { ...result, dir: dirRel, frames })
      } catch (e) { sendError(res, (e as Error).message, 500) }
    })

    // ── Fallthrough ──
    function nextMiddleware(_req: IncomingMessage, res: ServerResponse) {
      res.writeHead(405); res.end('Method Not Allowed')
    }
}
