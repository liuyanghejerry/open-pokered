// ──────────────────────────────────────────────────────────────────────────
// Pokered data routes — the editor's /api data surface plus the /gfx and
// /wasm asset handlers.
//
// Extracted from vite.config.ts, where these lived as inline dev-server
// middleware, so the exact same handlers can be mounted on any connect-style
// app: the Vite dev server (vite.config.ts) or the Electron production
// api-server (electron/api-server.ts). Both hand over a `{ middlewares }`
// object, the same shape the jrpg-editor route modules (server/api/routes/*)
// are written against.
//
// All filesystem roots derive from getProjectRoot() (server/api/projectConfig)
// and are re-evaluated on EVERY request (never cached at module level), so
// POST /api/project/open re-points every route at the newly opened repo
// without a server restart. With the default project root (the workspace
// root) the resolved paths are identical to the old inline constants.
// ──────────────────────────────────────────────────────────────────────────
import path from 'path'
import fs from 'fs'
import type { IncomingMessage, ServerResponse } from 'http'
import { sendJson, sendError, readBody } from './api/http'
import { getProjectRoot } from './api/projectConfig'
import { validateNewRecordName, pokemonTemplate, moveTemplate, itemTemplate, formatItemList } from './api/pokeredDataCreate'

// Filesystem roots of the pokered repo, relative to the current project root.
// Computed per request — cheap (pure path joins) and always in sync with
// projectConfig.
function roots() {
  const projectRoot = getProjectRoot()
  const dataRoot = path.join(projectRoot, 'examples/pokered/crates/pokered-data')
  return {
    // Game assets: <workspace>/examples/pokered/gfx (populated by
    // scripts/fetch-gfx.sh; gitignored). The pre-extraction vite.config.ts
    // pointed this at <repo-root>/gfx, which never existed — fixed here so
    // /gfx serving, blockset reads and PUT writes hit the real asset tree.
    gfxRoot: path.join(projectRoot, 'examples/pokered/gfx'),
    mapsRoot: path.join(dataRoot, 'maps'),
    dataRoot,
    uiLayoutsRoot: path.join(dataRoot, 'ui_layouts'),
    // WASM layout-preview pkg. JRPG_WASM_ROOT overrides (a packaged Electron
    // app ships the pkg as an extraResource outside any repo checkout).
    wasmRoot: process.env.JRPG_WASM_ROOT
      ? path.resolve(process.env.JRPG_WASM_ROOT)
      : path.resolve(projectRoot, 'crates/jrpg-web/pkg'),
    // WYSIWYG game-preview pkg (pokered-runner-web). In a packaged app the
    // two pkgs are merged into JRPG_WASM_ROOT, so the fallback collapses to
    // the same directory.
    wasmPokeredRoot: process.env.JRPG_WASM_ROOT
      ? path.resolve(process.env.JRPG_WASM_ROOT)
      : path.resolve(projectRoot, 'crates/pokered-runner-web/pkg'),
  }
}

const TILESET_BST_FILES: Record<string, string> = {
  Overworld: 'overworld.bst',
  RedsHouse1: 'reds_house.bst',
  Mart: 'pokecenter.bst',
  Forest: 'forest.bst',
  RedsHouse2: 'reds_house.bst',
  Dojo: 'gym.bst',
  Pokecenter: 'pokecenter.bst',
  Gym: 'gym.bst',
  House: 'house.bst',
  ForestGate: 'gate.bst',
  Museum: 'gate.bst',
  Underground: 'underground.bst',
  Gate: 'gate.bst',
  Ship: 'ship.bst',
  ShipPort: 'ship_port.bst',
  Cemetery: 'cemetery.bst',
  Interior: 'interior.bst',
  Cavern: 'cavern.bst',
  Lobby: 'lobby.bst',
  Mansion: 'mansion.bst',
  Lab: 'lab.bst',
  Club: 'club.bst',
  Facility: 'facility.bst',
  Plateau: 'plateau.bst',
}

const TILESET_PASSABLE_TILES: Record<string, number[]> = {
  Overworld: [0x00, 0x10, 0x1B, 0x20, 0x21, 0x23, 0x2C, 0x2D, 0x2E, 0x30, 0x31, 0x33, 0x39, 0x3C, 0x3E, 0x52, 0x54, 0x58, 0x5B],
  RedsHouse1: [0x01, 0x02, 0x03, 0x11, 0x12, 0x13, 0x14, 0x1C, 0x1A],
  Mart: [0x11, 0x1A, 0x1C, 0x3C, 0x5E],
  Forest: [0x1E, 0x20, 0x2E, 0x30, 0x34, 0x37, 0x39, 0x3A, 0x40, 0x51, 0x52, 0x5A, 0x5C, 0x5E, 0x5F],
  RedsHouse2: [0x01, 0x02, 0x03, 0x11, 0x12, 0x13, 0x14, 0x1C, 0x1A],
  Dojo: [0x11, 0x16, 0x19, 0x2B, 0x3C, 0x3D, 0x3F, 0x4A, 0x4C, 0x4D, 0x03],
  Pokecenter: [0x11, 0x1A, 0x1C, 0x3C, 0x5E],
  Gym: [0x11, 0x16, 0x19, 0x2B, 0x3C, 0x3D, 0x3F, 0x4A, 0x4C, 0x4D, 0x03],
  House: [0x01, 0x12, 0x14, 0x28, 0x32, 0x37, 0x44, 0x54, 0x5C],
  ForestGate: [0x01, 0x12, 0x14, 0x1A, 0x1C, 0x37, 0x38, 0x3B, 0x3C, 0x5E],
  Museum: [0x01, 0x12, 0x14, 0x1A, 0x1C, 0x37, 0x38, 0x3B, 0x3C, 0x5E],
  Underground: [0x0B, 0x0C, 0x13, 0x15, 0x18],
  Gate: [0x01, 0x12, 0x14, 0x1A, 0x1C, 0x37, 0x38, 0x3B, 0x3C, 0x5E],
  Ship: [0x04, 0x0D, 0x17, 0x1D, 0x1E, 0x23, 0x34, 0x37, 0x39, 0x4A],
  ShipPort: [0x0A, 0x1A, 0x32, 0x3B],
  Cemetery: [0x01, 0x10, 0x13, 0x1B, 0x22, 0x42, 0x52],
  Interior: [0x04, 0x0F, 0x15, 0x1F, 0x3B, 0x45, 0x47, 0x55, 0x56],
  Cavern: [0x05, 0x15, 0x18, 0x1A, 0x20, 0x21, 0x22, 0x2A, 0x2D, 0x30],
  Lobby: [0x14, 0x17, 0x1A, 0x1C, 0x20, 0x38, 0x45],
  Mansion: [0x01, 0x05, 0x11, 0x12, 0x14, 0x1A, 0x1C, 0x2C, 0x53],
  Lab: [0x0C, 0x26, 0x16, 0x1E, 0x34, 0x37],
  Club: [0x0F, 0x1A, 0x1F, 0x26, 0x28, 0x29, 0x2C, 0x2D, 0x2E, 0x2F, 0x41],
  Facility: [0x01, 0x10, 0x11, 0x13, 0x1B, 0x20, 0x21, 0x22, 0x30, 0x31, 0x32, 0x42, 0x43, 0x48, 0x52, 0x55, 0x58, 0x5E],
  Plateau: [0x1B, 0x23, 0x2C, 0x2D, 0x3B, 0x45],
}

const BLOCK_SIZE = 16

function parseBst(bstPath: string): Record<number, number[]> {
  const buf = fs.readFileSync(bstPath)
  const numBlocks = Math.floor(buf.length / BLOCK_SIZE)
  const blocks: Record<number, number[]> = {}
  for (let i = 0; i < numBlocks; i++) {
    const tiles: number[] = []
    for (let j = 0; j < BLOCK_SIZE; j++) {
      tiles.push(buf[i * BLOCK_SIZE + j])
    }
    blocks[i] = tiles
  }
  return blocks
}

interface TilesetExtra {
  base: string
  bstFile: string
  pngFile: string
  category: 'outdoor' | 'indoor' | 'cave'
  displayName: string
}

function readTilesetExtras(extrasFile: string): Record<string, TilesetExtra> {
  if (!fs.existsSync(extrasFile)) return {}
  try {
    return JSON.parse(fs.readFileSync(extrasFile, 'utf-8'))
  } catch {
    return {}
  }
}

function readPassableOverrides(overridesFile: string): Record<string, number[]> {
  if (!fs.existsSync(overridesFile)) return {}
  try {
    return JSON.parse(fs.readFileSync(overridesFile, 'utf-8'))
  } catch {
    return {}
  }
}

function resolveBstFileForTileset(extrasFile: string, name: string): string | undefined {
  if (TILESET_BST_FILES[name]) return TILESET_BST_FILES[name]
  const extras = readTilesetExtras(extrasFile)
  return extras[name]?.bstFile
}

const TILESET_OUTDOOR_NAMES = new Set(['Overworld', 'Plateau'])
const TILESET_CAVE_NAMES = new Set([
  'Forest', 'Museum', 'Ship', 'Cavern', 'Lobby',
  'Mansion', 'Gate', 'Lab', 'Facility', 'Cemetery', 'Gym',
])

function inferCategory(name: string): 'outdoor' | 'indoor' | 'cave' {
  if (TILESET_OUTDOOR_NAMES.has(name)) return 'outdoor'
  if (TILESET_CAVE_NAMES.has(name)) return 'cave'
  return 'indoor'
}

type Next = (err?: unknown) => void

/**
 * Mount the pokered data routes. The registration order below reproduces the
 * original vite.config.ts plugin sequence exactly: /gfx write → /gfx static →
 * /wasm static → maps/blocksets/tilesets → trainers → ui-layouts → pokemon →
 * moves → items → categories/effect-registry/shops.
 */
export function registerPokeredRoutes(server: { middlewares: any }) {
  // ── /gfx write API (PUT) — was the `gfx-write-api` plugin ──
  server.middlewares.use('/gfx', (req: IncomingMessage, res: ServerResponse, next: Next) => {
    if (req.method !== 'PUT') {
      next()
      return
    }

    const { gfxRoot } = roots()
    const requestUrl = decodeURIComponent(req.url || '')

    // Tileset tile extraction — skeleton (501 Not Implemented)
    const tilesetTileMatch = requestUrl.match(/^\/tilesets\/([A-Za-z][A-Za-z0-9_]*)\/tile\/(\d+)\.png$/)
    if (tilesetTileMatch) {
      res.writeHead(501, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ error: 'Not implemented yet' }))
      return
    }

    // Map the URL to a relative gfx path
    let relativePath: string | null = null
    const patterns = [
      { regex: /^\/pokemon\/front\/(.+)\.png$/, subdir: 'pokemon/front' },
      { regex: /^\/pokemon\/back\/(.+)\.png$/, subdir: 'pokemon/back' },
      { regex: /^\/trainers\/(.+)\.png$/, subdir: 'trainers' },
      { regex: /^\/sprites\/(.+)\.png$/, subdir: 'sprites' },
      { regex: /^\/tilesets\/(.+)\.png$/, subdir: 'tilesets' },
    ]

    for (const { regex, subdir } of patterns) {
      const m = requestUrl.match(regex)
      if (m) {
        relativePath = `${subdir}/${m[1]}.png`
        break
      }
    }

    if (!relativePath) {
      // Generic fallback for UI / Effects / any gfx subdirectory
      const genericMatch = requestUrl.match(/^\/([a-z0-9_\/-]+\.png)$/)
      if (genericMatch) {
        relativePath = genericMatch[1]
      } else {
        res.statusCode = 400
        res.end('Bad Request')
        return
      }
    }

    // Path traversal protection
    const resolved = path.resolve(gfxRoot, relativePath)
    if (!resolved.startsWith(gfxRoot)) {
      res.statusCode = 400
      res.end('Bad Request')
      return
    }

    // Read body as binary buffer
    const chunks: Buffer[] = []
    req.on('data', (chunk: Buffer) => chunks.push(chunk))
    req.on('end', () => {
      const buffer = Buffer.concat(chunks)
      if (buffer.length === 0) {
        res.statusCode = 400
        res.end('Empty body')
        return
      }
      fs.writeFileSync(resolved, buffer)
      console.log(`[gfx] Saved ${relativePath} (${buffer.length} bytes)`)
      res.end('OK')
    })
    req.on('error', () => {
      res.statusCode = 500
      res.end('Internal Server Error')
    })
  })

  // ── /gfx static (GET) — was the `serve-gfx` plugin ──
  server.middlewares.use('/gfx', (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const { gfxRoot } = roots()
    const filePath = path.join(gfxRoot, decodeURIComponent(req.url || ''))
    if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
      res.setHeader('Content-Type', 'image/png')
      fs.createReadStream(filePath).pipe(res)
    } else {
      next()
    }
  })

  // ── /wasm static — was the `serve-wasm` plugin ──
  // Primary root is the jrpg-web layout-preview pkg; files not found there
  // fall back to the pokered-runner-web pkg (the WYSIWYG game preview). A
  // packaged Electron app overrides both via JRPG_WASM_ROOT (extraResource
  // dir holding the merged pkgs — see electron/stage-resources.mjs).
  server.middlewares.use('/wasm', (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const { wasmRoot, wasmPokeredRoot } = roots()
    const relative = decodeURIComponent(req.url || '')
    const candidates = [path.join(wasmRoot, relative), path.join(wasmPokeredRoot, relative)]
    for (const filePath of candidates) {
      if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
        const ext = path.extname(filePath).toLowerCase()
        if (ext === '.wasm') {
          res.setHeader('Content-Type', 'application/wasm')
        } else if (ext === '.js') {
          res.setHeader('Content-Type', 'application/javascript')
        }
        fs.createReadStream(filePath).pipe(res)
        return
      }
    }
    next()
  })

  // ── maps / town-map-extras / blocksets / passable-tiles / tileset-extras /
  //    tilesets — was the `map-data-api` plugin ──
  server.middlewares.use('/api', async (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const { gfxRoot, mapsRoot, dataRoot } = roots()
    const TOWN_MAP_EXTRAS_FILE = path.join(dataRoot, 'town_map_extras.json')
    // Tilesets created by the user (editor side). Keyed by tileset name. Stored
    // alongside town_map_extras.json so both the editor and the Rust runtime can
    // optionally read it without forking the closed `enum TilesetId`.
    const TILESET_EXTRAS_FILE = path.join(dataRoot, 'tileset_extras.json')
    // Per-tileset collision overrides written by the editor's tile-collision tool.
    // Keyed by tileset name → array of passable tile ids. Persisted as a single
    // JSON file rather than rewriting the binary `.tilecoll` files so that the
    // changes are easy to review and revert.
    const PASSABLE_OVERRIDES_FILE = path.join(dataRoot, 'tileset_passable_overrides.json')

    const url = decodeURIComponent(req.url || '')
    const method = req.method || 'GET'

    if (url === '/maps' && method === 'GET') {
      const dirs = fs.readdirSync(mapsRoot, { withFileTypes: true })
        .filter(d => d.isDirectory() && fs.existsSync(path.join(mapsRoot, d.name, 'map.json')))
        .map(d => d.name)
        .sort()
      sendJson(res, dirs)
      return
    }

    // POST /maps -> create a new map directory with default files
    // Body: { name, displayName?, tileset, width, height, music?, borderBlock?, townMap?: { x, y } }
    if (url === '/maps' && method === 'POST') {
      try {
        const body = await readBody(req)
        const opts = JSON.parse(body) as {
          name?: string
          displayName?: string
          tileset?: string
          width?: number
          height?: number
          music?: string
          borderBlock?: number
          townMap?: { x: number; y: number }
        }

        const name = (opts.name ?? '').trim()
        if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) {
          sendError(res, 'Invalid map name (must match /^[A-Za-z][A-Za-z0-9_]*$/)', 400)
          return
        }
        const tileset = opts.tileset ?? 'Overworld'
        const width = Math.max(1, Math.min(255, Math.floor(opts.width ?? 10)))
        const height = Math.max(1, Math.min(255, Math.floor(opts.height ?? 9)))
        const music = opts.music ?? 'PalletTown'
        const borderBlock = Math.max(0, Math.min(255, Math.floor(opts.borderBlock ?? 0)))

        const newDir = path.join(mapsRoot, name)
        if (fs.existsSync(newDir)) {
          sendError(res, `Map "${name}" already exists`, 409)
          return
        }

        // Compute next free map id (max existing + 1)
        let maxId = -1
        const existing = fs.readdirSync(mapsRoot, { withFileTypes: true })
          .filter(d => d.isDirectory())
        for (const d of existing) {
          const p = path.join(mapsRoot, d.name, 'map.json')
          if (fs.existsSync(p)) {
            try {
              const j = JSON.parse(fs.readFileSync(p, 'utf-8'))
              if (typeof j.id === 'number' && j.id > maxId) maxId = j.id
            } catch { /* ignore */ }
          }
        }
        const newId = maxId + 1

        const mapJson = {
          id: newId,
          name,
          header: {
            tileset,
            music,
            connectionFlags: 0,
            width,
            height,
            borderBlock,
          },
          connections: {},
          warps: [],
          npcs: [],
          signs: [],
          text: { npc: {}, sign: {} },
          wild: null,
        }

        fs.mkdirSync(newDir, { recursive: true })
        fs.writeFileSync(path.join(newDir, 'map.json'), JSON.stringify(mapJson, null, 2) + '\n')
        fs.writeFileSync(
          path.join(newDir, 'map.blk'),
          Buffer.alloc(width * height, borderBlock),
        )
        fs.writeFileSync(
          path.join(newDir, 'script_config.json'),
          JSON.stringify({ npcs: [], signs: [], coordEvents: [] }, null, 2) + '\n',
        )
        fs.writeFileSync(path.join(newDir, 'script.js'), '')
        fs.writeFileSync(path.join(newDir, 'script.scene'), '')

        // Optionally record town-map placement
        if (opts.townMap
          && Number.isFinite(opts.townMap.x)
          && Number.isFinite(opts.townMap.y)) {
          let extras: Record<string, { x: number; y: number; displayName: string }> = {}
          if (fs.existsSync(TOWN_MAP_EXTRAS_FILE)) {
            try { extras = JSON.parse(fs.readFileSync(TOWN_MAP_EXTRAS_FILE, 'utf-8')) } catch { /* ignore */ }
          }
          extras[name] = {
            x: Math.max(0, Math.min(15, Math.floor(opts.townMap.x))),
            y: Math.max(0, Math.min(15, Math.floor(opts.townMap.y))),
            displayName: (opts.displayName ?? name).toUpperCase(),
          }
          fs.writeFileSync(TOWN_MAP_EXTRAS_FILE, JSON.stringify(extras, null, 2) + '\n')
        }

        sendJson(res, { ok: true, name, id: newId })
        return
      } catch (err) {
        sendError(res, `Failed to create map: ${(err as Error).message}`, 500)
        return
      }
    }

    // GET / PUT town map extras
    if (url === '/town-map-extras' && method === 'GET') {
      if (!fs.existsSync(TOWN_MAP_EXTRAS_FILE)) {
        sendJson(res, {})
        return
      }
      try {
        const content = fs.readFileSync(TOWN_MAP_EXTRAS_FILE, 'utf-8')
        sendJson(res, JSON.parse(content))
      } catch {
        sendJson(res, {})
      }
      return
    }
    if (url === '/town-map-extras' && method === 'PUT') {
      const body = await readBody(req)
      const parsed = JSON.parse(body)
      fs.writeFileSync(TOWN_MAP_EXTRAS_FILE, JSON.stringify(parsed, null, 2) + '\n')
      sendJson(res, { ok: true })
      return
    }

    const mapFileMatch = url.match(/^\/maps\/([^/]+)\/(.+)$/)
    if (mapFileMatch) {
      const mapName = mapFileMatch[1]
      const fileName = mapFileMatch[2]
      const mapDir = path.join(mapsRoot, mapName)

      if (!fs.existsSync(mapDir)) {
        sendError(res, `Map "${mapName}" not found`)
        return
      }

      if (method === 'GET') {
        if (fileName === 'map.json' || fileName === 'script_config.json') {
          const filePath = path.join(mapDir, fileName)
          if (!fs.existsSync(filePath)) {
            sendError(res, `${fileName} not found for ${mapName}`)
            return
          }
          const content = fs.readFileSync(filePath, 'utf-8')
          sendJson(res, JSON.parse(content))
          return
        }
        if (fileName === 'map.blk') {
          const filePath = path.join(mapDir, 'map.blk')
          if (!fs.existsSync(filePath)) {
            sendJson(res, [])
            return
          }
          const buf = fs.readFileSync(filePath)
          sendJson(res, Array.from(buf))
          return
        }
        if (fileName === 'script.js') {
          const filePath = path.join(mapDir, 'script.js')
          if (!fs.existsSync(filePath)) {
            res.writeHead(200, { 'Content-Type': 'text/plain' })
            res.end('')
            return
          }
          const content = fs.readFileSync(filePath, 'utf-8')
          res.writeHead(200, { 'Content-Type': 'text/plain' })
          res.end(content)
          return
        }
        if (fileName === 'script.scene') {
          const filePath = path.join(mapDir, 'script.scene')
          if (!fs.existsSync(filePath)) {
            res.writeHead(200, { 'Content-Type': 'text/plain' })
            res.end('')
            return
          }
          const content = fs.readFileSync(filePath, 'utf-8')
          res.writeHead(200, { 'Content-Type': 'text/plain' })
          res.end(content)
          return
        }
      }

      if (method === 'PUT') {
        if (fileName === 'map.json' || fileName === 'script_config.json') {
          const body = await readBody(req)
          const parsed = JSON.parse(body)
          const filePath = path.join(mapDir, fileName)
          fs.writeFileSync(filePath, JSON.stringify(parsed, null, 2) + '\n')
          sendJson(res, { ok: true })
          return
        }
        if (fileName === 'map.blk') {
          const body = await readBody(req)
          const arr: number[] = JSON.parse(body)
          const buf = Buffer.from(arr)
          fs.writeFileSync(path.join(mapDir, 'map.blk'), buf)
          sendJson(res, { ok: true })
          return
        }
        if (fileName === 'script.js') {
          const body = await readBody(req)
          const filePath = path.join(mapDir, 'script.js')
          fs.writeFileSync(filePath, body)
          sendJson(res, { ok: true })
          return
        }
        if (fileName === 'script.scene') {
          const body = await readBody(req)
          const filePath = path.join(mapDir, 'script.scene')
          fs.writeFileSync(filePath, body)
          sendJson(res, { ok: true })
          return
        }
      }
    }

    if (url === '/blocksets' && method === 'GET') {
      const blocksets: Record<string, Record<number, number[]>> = {}
      // built-in blocksets
      for (const [name, file] of Object.entries(TILESET_BST_FILES)) {
        const bstPath = path.join(gfxRoot, 'blocksets', file)
        if (fs.existsSync(bstPath)) {
          blocksets[name] = parseBst(bstPath)
        }
      }
      // user-created tileset blocksets — read from tileset_extras.json
      const extras = readTilesetExtras(TILESET_EXTRAS_FILE)
      for (const [name, info] of Object.entries(extras)) {
        const file = info.bstFile ?? `${name.toLowerCase()}.bst`
        const bstPath = path.join(gfxRoot, 'blocksets', file)
        if (fs.existsSync(bstPath)) {
          blocksets[name] = parseBst(bstPath)
        }
      }
      sendJson(res, blocksets)
      return
    }

    // PUT /api/blocksets/:name — overwrite the .bst for this tileset.
    // Body: { blocks: { [id]: number[16] } } (sparse — keys we send are
    // written; missing ids keep their existing value)
    const blocksetMatch = url.match(/^\/blocksets\/([A-Za-z][A-Za-z0-9_]*)$/)
    if (blocksetMatch && method === 'PUT') {
      try {
        const tilesetName = blocksetMatch[1]
        const file = resolveBstFileForTileset(TILESET_EXTRAS_FILE, tilesetName)
        if (!file) {
          sendError(res, `Unknown tileset "${tilesetName}"`, 404)
          return
        }
        const bstPath = path.join(gfxRoot, 'blocksets', file)
        if (!fs.existsSync(bstPath)) {
          sendError(res, `Blockset file ${file} not found`, 404)
          return
        }
        const body = await readBody(req)
        const parsed = JSON.parse(body) as { blocks?: Record<string, number[]> }
        const updates = parsed.blocks ?? {}
        const buf = Buffer.from(fs.readFileSync(bstPath))
        for (const [k, tiles] of Object.entries(updates)) {
          const id = parseInt(k, 10)
          if (!Number.isFinite(id) || id < 0 || id > 255) continue
          if (!Array.isArray(tiles) || tiles.length !== BLOCK_SIZE) continue
          const offset = id * BLOCK_SIZE
          if (offset + BLOCK_SIZE > buf.length) continue
          for (let j = 0; j < BLOCK_SIZE; j++) {
            const v = tiles[j]
            buf[offset + j] = Math.max(0, Math.min(255, Math.floor(v)))
          }
        }
        fs.writeFileSync(bstPath, buf)
        sendJson(res, { ok: true })
        return
      } catch (err) {
        sendError(res, `Failed to write blockset: ${(err as Error).message}`, 500)
        return
      }
    }

    if (url === '/passable-tiles' && method === 'GET') {
      // Merge built-in defaults with persisted overrides so the editor
      // sees the user's edits, and brand-new tilesets pick up an empty
      // (fully-blocked) default that they can extend.
      const merged: Record<string, number[]> = { ...TILESET_PASSABLE_TILES }
      const overrides = readPassableOverrides(PASSABLE_OVERRIDES_FILE)
      for (const [k, v] of Object.entries(overrides)) {
        if (Array.isArray(v)) merged[k] = v.slice()
      }
      // Surface custom tilesets without overrides as empty lists so the
      // UI knows they exist.
      for (const name of Object.keys(readTilesetExtras(TILESET_EXTRAS_FILE))) {
        if (!(name in merged)) merged[name] = []
      }
      sendJson(res, merged)
      return
    }

    // PUT /api/passable-tiles/:name — overwrite the passable-tile list
    // for one tileset. Body: { tiles: number[] }
    const passableMatch = url.match(/^\/passable-tiles\/([A-Za-z][A-Za-z0-9_]*)$/)
    if (passableMatch && method === 'PUT') {
      try {
        const tilesetName = passableMatch[1]
        const body = await readBody(req)
        const parsed = JSON.parse(body) as { tiles?: number[] }
        const tiles = (parsed.tiles ?? [])
          .map((t) => Math.max(0, Math.min(255, Math.floor(t))))
          .filter((t, i, arr) => arr.indexOf(t) === i)
          .sort((a, b) => a - b)
        const overrides = readPassableOverrides(PASSABLE_OVERRIDES_FILE)
        overrides[tilesetName] = tiles
        fs.writeFileSync(
          PASSABLE_OVERRIDES_FILE,
          JSON.stringify(overrides, null, 2) + '\n',
        )
        sendJson(res, { ok: true })
        return
      } catch (err) {
        sendError(res, `Failed to write passable tiles: ${(err as Error).message}`, 500)
        return
      }
    }

    // GET /api/tileset-extras — list user-created tilesets.
    if (url === '/tileset-extras' && method === 'GET') {
      sendJson(res, readTilesetExtras(TILESET_EXTRAS_FILE))
      return
    }

    // POST /api/tilesets — create a new tileset by cloning a base
    // tileset's blockset/PNG/passable-list. Body: { name, base, category? }
    if (url === '/tilesets' && method === 'POST') {
      try {
        const body = await readBody(req)
        const opts = JSON.parse(body) as {
          name?: string
          base?: string
          category?: 'outdoor' | 'indoor' | 'cave'
          displayName?: string
        }
        const name = (opts.name ?? '').trim()
        if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) {
          sendError(res, 'Invalid tileset name (must match /^[A-Za-z][A-Za-z0-9_]*$/)', 400)
          return
        }
        const extras = readTilesetExtras(TILESET_EXTRAS_FILE)
        if (extras[name] || name in TILESET_BST_FILES) {
          sendError(res, `Tileset "${name}" already exists`, 409)
          return
        }
        const base = opts.base ?? 'Overworld'
        const baseBstFile = TILESET_BST_FILES[base]
        if (!baseBstFile) {
          sendError(res, `Unknown base tileset "${base}"`, 400)
          return
        }
        // File names use snake_case of the tileset name.
        // (Manual loop — the regex /([A-Z])/g callback receives the
        // index *within the match*, not within the source, so we can't
        // rely on it for "is this the first character" checks.)
        let stem = ''
        for (let i = 0; i < name.length; i++) {
          const ch = name[i]
          if (i > 0 && ch >= 'A' && ch <= 'Z') stem += '_'
          stem += ch.toLowerCase()
        }
        const bstFile = `${stem}.bst`
        const pngFile = `${stem}.png`
        const newBstPath = path.join(gfxRoot, 'blocksets', bstFile)
        const newPngPath = path.join(gfxRoot, 'tilesets', pngFile)
        const baseBstPath = path.join(gfxRoot, 'blocksets', baseBstFile)
        // Use the *base PNG's* filename (the existing TILESET_FILES on
        // the editor side maps tileset name → png), looked up via the
        // editor's own table on the client; but on the server we know
        // most names share the same stem as the bst sans extension.
        const basePngStem = baseBstFile.replace(/\.bst$/, '')
        const basePngPath = path.join(gfxRoot, 'tilesets', `${basePngStem}.png`)

        if (!fs.existsSync(baseBstPath)) {
          sendError(res, `Base blockset ${baseBstFile} missing`, 500)
          return
        }
        fs.copyFileSync(baseBstPath, newBstPath)
        if (fs.existsSync(basePngPath)) {
          fs.copyFileSync(basePngPath, newPngPath)
        }

        extras[name] = {
          base,
          bstFile,
          pngFile,
          category: opts.category ?? inferCategory(base),
          displayName: opts.displayName ?? name,
        }
        fs.writeFileSync(TILESET_EXTRAS_FILE, JSON.stringify(extras, null, 2) + '\n')

        // Seed empty collision override so editor surfaces it
        const overrides = readPassableOverrides(PASSABLE_OVERRIDES_FILE)
        if (!(name in overrides)) {
          overrides[name] = (TILESET_PASSABLE_TILES[base] ?? []).slice()
          fs.writeFileSync(
            PASSABLE_OVERRIDES_FILE,
            JSON.stringify(overrides, null, 2) + '\n',
          )
        }

        sendJson(res, { ok: true, name, base, bstFile, pngFile })
        return
      } catch (err) {
        sendError(res, `Failed to create tileset: ${(err as Error).message}`, 500)
        return
      }
    }

    next()
  })

  // ── /api/trainers — was the `trainer-data-api` plugin ──
  server.middlewares.use('/api/trainers', async (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const trainersRoot = path.join(roots().dataRoot, 'trainers')
    const url = decodeURIComponent(req.url || '')
    const method = req.method || 'GET'

    if (!fs.existsSync(trainersRoot)) {
      sendError(res, `Trainer data directory not found at ${trainersRoot}. Expected JSON files under crates/pokered-data/trainers/.`, 500)
      return
    }

    if ((url === '' || url === '/') && method === 'GET') {
      const files = fs.readdirSync(trainersRoot)
        .filter(f => f.endsWith('.json'))
        .map(f => f.replace(/\.json$/, ''))
        .sort()
      sendJson(res, files)
      return
    }

    const fileMatch = url.match(/^\/([A-Za-z][A-Za-z0-9]*)$/)
    if (fileMatch) {
      const className = fileMatch[1]
      const filePath = path.join(trainersRoot, `${className}.json`)

      if (method === 'GET') {
        if (!fs.existsSync(filePath)) {
          sendError(res, `Trainer class "${className}" not found`)
          return
        }
        try {
          sendJson(res, JSON.parse(fs.readFileSync(filePath, 'utf-8')))
        } catch (err) {
          sendError(res, `Failed to read ${className}.json: ${(err as Error).message}`, 500)
        }
        return
      }

      if (method === 'PUT') {
        try {
          const body = await readBody(req)
          const parsed = JSON.parse(body)
          fs.writeFileSync(filePath, JSON.stringify(parsed, null, 2) + '\n')
          sendJson(res, { ok: true })
        } catch (err) {
          sendError(res, `Failed to write ${className}.json: ${(err as Error).message}`, 500)
        }
        return
      }
    }

    next()
  })

  // ── /api/ui-layouts — was the `ui-layouts-api` plugin ──
  server.middlewares.use('/api/ui-layouts', async (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const { uiLayoutsRoot } = roots()
    const url = decodeURIComponent(req.url || '')
    const method = req.method || 'GET'

    if (!fs.existsSync(uiLayoutsRoot)) {
      sendError(res, `UI layouts directory not found at ${uiLayoutsRoot}. Expected files under crates/pokered-data/ui_layouts/.`, 500)
      return
    }

    if ((url === '' || url === '/') && method === 'GET') {
      // Collect names from both .gui (root) and .json (v1/) — deduplicate
      const rootEntries = fs.readdirSync(uiLayoutsRoot)
      const guiNames = rootEntries.filter(f => f.endsWith('.gui')).map(f => f.replace(/\.gui$/, ''))
      const v1Dir = path.join(uiLayoutsRoot, 'v1')
      const jsonNames = fs.existsSync(v1Dir)
        ? fs.readdirSync(v1Dir).filter(f => f.endsWith('.json')).map(f => f.replace(/\.json$/, ''))
        : []
      const allNames = [...new Set([...guiNames, ...jsonNames])].sort()
      sendJson(res, allNames)
      return
    }

    // Serve compiled JSON from v1/ subdirectory
    const v1Match = url.match(/^\/v1\/([A-Za-z][A-Za-z0-9_]*)$/)
    if (v1Match) {
      const layoutName = v1Match[1]
      const jsonPath = path.join(uiLayoutsRoot, 'v1', `${layoutName}.json`)
      if (fs.existsSync(jsonPath)) {
        sendJson(res, JSON.parse(fs.readFileSync(jsonPath, 'utf-8')))
      } else {
        sendError(res, `Layout "${layoutName}" not found in v1/`)
      }
      return
    }

    const fileMatch = url.match(/^\/([A-Za-z][A-Za-z0-9_]*)$/)
    if (fileMatch) {
      const layoutName = fileMatch[1]
      const guiPath = path.join(uiLayoutsRoot, `${layoutName}.gui`)
      const jsonPath = path.join(uiLayoutsRoot, 'v1', `${layoutName}.json`)

      if (method === 'GET') {
        // Try .gui first, then .json in v1/
        if (fs.existsSync(guiPath)) {
          res.writeHead(200, { 'Content-Type': 'text/plain' })
          res.end(fs.readFileSync(guiPath, 'utf-8'))
          return
        }
        if (fs.existsSync(jsonPath)) {
          sendJson(res, JSON.parse(fs.readFileSync(jsonPath, 'utf-8')))
          return
        }
        sendError(res, `Layout "${layoutName}" not found`)
        return
      }

      if (method === 'PUT') {
        try {
          const body = await readBody(req)
          const contentType = req.headers['content-type'] || ''
          if (contentType.includes('text/plain') || fs.existsSync(guiPath)) {
            fs.writeFileSync(guiPath, body, 'utf-8')
          } else {
            const parsed = JSON.parse(body)
            fs.mkdirSync(path.dirname(jsonPath), { recursive: true })
            fs.writeFileSync(jsonPath, JSON.stringify(parsed, null, 2) + '\n')
          }
          sendJson(res, { ok: true })
        } catch (err) {
          if (err instanceof SyntaxError) {
            sendError(res, `Invalid JSON: ${err.message}`, 400)
          } else {
            sendError(res, `Failed to write ${layoutName}: ${(err as Error).message}`, 500)
          }
        }
        return
      }
    }

    next()
  })

  // ── /api/pokemon — was the `pokemon-data-api` plugin ──
  server.middlewares.use('/api/pokemon', async (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const pokemonRoot = path.join(roots().dataRoot, 'pokemon')
    const url = decodeURIComponent(req.url || '')
    const method = req.method || 'GET'

    if (!fs.existsSync(pokemonRoot)) {
      sendError(res, `Pokemon data directory not found at ${pokemonRoot}. Run \`cargo run --example dump_pokemon_and_moves -p pokered-data\` to seed it.`, 500)
      return
    }

    if ((url === '' || url === '/') && method === 'GET') {
      const files = fs.readdirSync(pokemonRoot)
        .filter(f => f.endsWith('.json'))
        .map(f => f.replace(/\.json$/, ''))
        .sort()
      sendJson(res, files)
      return
    }

    // POST /api/pokemon -> create a new species from a template.
    // Body: { name } — the name becomes a `Species` enum variant on the next
    // `cargo build` (see pokered-data/build.rs::generate_species_enum).
    if ((url === '' || url === '/') && method === 'POST') {
      try {
        const body = await readBody(req)
        const opts = JSON.parse(body) as { name?: string }
        const name = (opts.name ?? '').trim()
        const existing = fs.readdirSync(pokemonRoot)
          .filter(f => f.endsWith('.json'))
          .map(f => f.replace(/\.json$/, ''))
        const nameError = validateNewRecordName(name, existing)
        if (nameError) {
          sendError(res, nameError.error, nameError.status)
          return
        }
        const json = pokemonTemplate(name)
        fs.writeFileSync(path.join(pokemonRoot, `${name}.json`), JSON.stringify(json, null, 2) + '\n')
        sendJson(res, json)
      } catch (err) {
        sendError(res, `Failed to create pokemon: ${(err as Error).message}`, 500)
      }
      return
    }

    const fileMatch = url.match(/^\/([A-Za-z][A-Za-z0-9]*)$/)
    if (fileMatch) {
      const species = fileMatch[1]
      const filePath = path.join(pokemonRoot, `${species}.json`)

      if (method === 'GET') {
        if (!fs.existsSync(filePath)) {
          sendError(res, `Pokemon species "${species}" not found`)
          return
        }
        try {
          sendJson(res, JSON.parse(fs.readFileSync(filePath, 'utf-8')))
        } catch (err) {
          sendError(res, `Failed to read ${species}.json: ${(err as Error).message}`, 500)
        }
        return
      }

      if (method === 'PUT') {
        try {
          const body = await readBody(req)
          const parsed = JSON.parse(body)
          // Guard: the record's `species` field must match the filename —
          // build.rs asserts on this and would fail the next build otherwise.
          if (!parsed || typeof parsed !== 'object' || (parsed as { species?: unknown }).species !== species) {
            sendError(res, `Species field must match the filename ("${species}")`, 400)
            return
          }
          fs.writeFileSync(filePath, JSON.stringify(parsed, null, 2) + '\n')
          sendJson(res, { ok: true })
        } catch (err) {
          sendError(res, `Failed to write ${species}.json: ${(err as Error).message}`, 500)
        }
        return
      }
    }

    next()
  })

  // ── /api/moves — was the `move-data-api` plugin ──
  server.middlewares.use('/api/moves', async (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const movesRoot = path.join(roots().dataRoot, 'moves')
    const url = decodeURIComponent(req.url || '')
    const method = req.method || 'GET'

    if (!fs.existsSync(movesRoot)) {
      sendError(res, `Move data directory not found at ${movesRoot}. Run \`cargo run --example dump_pokemon_and_moves -p pokered-data\` to seed it.`, 500)
      return
    }

    if ((url === '' || url === '/') && method === 'GET') {
      const files = fs.readdirSync(movesRoot)
        .filter(f => f.endsWith('.json'))
        .map(f => f.replace(/\.json$/, ''))
        .sort()
      sendJson(res, files)
      return
    }

    // POST /api/moves -> create a new move from a template.
    // Body: { name } — the name becomes a `MoveId` enum variant on the next
    // `cargo build` (see pokered-data/build.rs::generate_moves_enum).
    if ((url === '' || url === '/') && method === 'POST') {
      try {
        const body = await readBody(req)
        const opts = JSON.parse(body) as { name?: string }
        const name = (opts.name ?? '').trim()
        const existing = fs.readdirSync(movesRoot)
          .filter(f => f.endsWith('.json'))
          .map(f => f.replace(/\.json$/, ''))
        const nameError = validateNewRecordName(name, existing)
        if (nameError) {
          sendError(res, nameError.error, nameError.status)
          return
        }
        const json = moveTemplate(name)
        fs.writeFileSync(path.join(movesRoot, `${name}.json`), JSON.stringify(json, null, 2) + '\n')
        sendJson(res, json)
      } catch (err) {
        sendError(res, `Failed to create move: ${(err as Error).message}`, 500)
      }
      return
    }

    const fileMatch = url.match(/^\/([A-Za-z][A-Za-z0-9]*)$/)
    if (fileMatch) {
      const moveId = fileMatch[1]
      const filePath = path.join(movesRoot, `${moveId}.json`)

      if (method === 'GET') {
        if (!fs.existsSync(filePath)) {
          sendError(res, `Move "${moveId}" not found`)
          return
        }
        try {
          sendJson(res, JSON.parse(fs.readFileSync(filePath, 'utf-8')))
        } catch (err) {
          sendError(res, `Failed to read ${moveId}.json: ${(err as Error).message}`, 500)
        }
        return
      }

      if (method === 'PUT') {
        try {
          const body = await readBody(req)
          const parsed = JSON.parse(body)
          // Guard: the record's `id` field must match the filename —
          // build.rs asserts on this and would fail the next build otherwise.
          if (!parsed || typeof parsed !== 'object' || (parsed as { id?: unknown }).id !== moveId) {
            sendError(res, `Move id field must match the filename ("${moveId}")`, 400)
            return
          }
          fs.writeFileSync(filePath, JSON.stringify(parsed, null, 2) + '\n')
          sendJson(res, { ok: true })
        } catch (err) {
          sendError(res, `Failed to write ${moveId}.json: ${(err as Error).message}`, 500)
        }
        return
      }
    }

    next()
  })

  // ── /api/items — was the `item-data-api` plugin ──
  server.middlewares.use('/api/items', async (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const itemsDir = path.join(roots().dataRoot, 'data/items')
    const url = decodeURIComponent(req.url || '')
    const method = req.method || 'GET'

    if (!fs.existsSync(itemsDir)) {
      sendError(res, `Item data directory not found at ${itemsDir}.`, 500)
      return
    }

    if ((url === '' || url === '/') && method === 'GET') {
      try {
        const listPath = path.join(itemsDir, 'item_list.json')
        if (fs.existsSync(listPath)) {
          const content = fs.readFileSync(listPath, 'utf-8')
          sendJson(res, JSON.parse(content))
        } else {
          const files = fs.readdirSync(itemsDir)
            .filter(f => f.endsWith('.json') && f !== 'item_list.json')
            .map(f => f.replace(/\.json$/, ''))
            .sort()
          sendJson(res, { items: files, count: files.length })
        }
        return
      } catch (err) {
        sendError(res, `Failed to list items: ${(err as Error).message}`, 500)
        return
      }
    }

    // POST /api/items -> create a new item from a template. The name is
    // appended to item_list.json (the ItemId enum-order source) so the next
    // `cargo build` picks it up (build.rs::generate_item_enum).
    if ((url === '' || url === '/') && method === 'POST') {
      try {
        const body = await readBody(req)
        const opts = JSON.parse(body) as { name?: string }
        const name = (opts.name ?? '').trim()
        const existing = fs.readdirSync(itemsDir)
          .filter(f => f.endsWith('.json') && f !== 'item_list.json')
          .map(f => f.replace(/\.json$/, ''))
        const nameError = validateNewRecordName(name, existing)
        if (nameError) {
          sendError(res, nameError.error, nameError.status)
          return
        }
        const json = itemTemplate(name)
        fs.writeFileSync(path.join(itemsDir, `${name}.json`), JSON.stringify(json, null, 2) + '\n')

        // Register the item in item_list.json (create the file if missing).
        const listPath = path.join(itemsDir, 'item_list.json')
        let list: string[] = []
        if (fs.existsSync(listPath)) {
          try {
            const parsed = JSON.parse(fs.readFileSync(listPath, 'utf-8'))
            if (Array.isArray(parsed.items)) list = parsed.items.map(String)
          } catch { /* rebuild the list */ }
        }
        if (!list.includes(name)) {
          list.push(name)
          fs.writeFileSync(listPath, formatItemList(list))
        }

        sendJson(res, json)
      } catch (err) {
        sendError(res, `Failed to create item: ${(err as Error).message}`, 500)
      }
      return
    }

    const fileMatch = url.match(/^\/([A-Za-z][A-Za-z0-9]*)$/)
    if (fileMatch) {
      const itemId = fileMatch[1]
      const filePath = path.join(itemsDir, `${itemId}.json`)

      if (method === 'GET') {
        if (!fs.existsSync(filePath)) {
          sendError(res, `Item "${itemId}" not found`)
          return
        }
        try {
          sendJson(res, JSON.parse(fs.readFileSync(filePath, 'utf-8')))
        } catch (err) {
          sendError(res, `Failed to read ${itemId}.json: ${(err as Error).message}`, 500)
        }
        return
      }

      if (method === 'PUT') {
        try {
          const body = await readBody(req)
          const parsed = JSON.parse(body)
          // Guard: the record's `id` field must match the filename —
          // build.rs asserts on this and would fail the next build otherwise.
          if (!parsed || typeof parsed !== 'object' || (parsed as { id?: unknown }).id !== itemId) {
            sendError(res, `Item id field must match the filename ("${itemId}")`, 400)
            return
          }
          fs.writeFileSync(filePath, JSON.stringify(parsed, null, 2) + '\n')
          sendJson(res, { ok: true })
        } catch (err) {
          sendError(res, `Failed to write ${itemId}.json: ${(err as Error).message}`, 500)
        }
        return
      }
    }

    next()
  })

  // ── /api/categories, /api/effect-registry, /api/shops — was the
  //    `shop-catalog-api` plugin ──
  server.middlewares.use('/api', async (req: IncomingMessage, res: ServerResponse, next: Next) => {
    const dataRoot = roots().dataRoot
    const shopsDir = path.join(dataRoot, 'data/shops')
    const categoriesFile = path.join(dataRoot, 'data/categories.json')
    const effectRegistryFile = path.join(dataRoot, 'data/effect_registry.json')

    const url = decodeURIComponent(req.url || '')
    const method = req.method || 'GET'

    // GET /api/categories
    if (url === '/categories' && method === 'GET') {
      if (!fs.existsSync(categoriesFile)) {
        sendJson(res, [])
        return
      }
      try {
        sendJson(res, JSON.parse(fs.readFileSync(categoriesFile, 'utf-8')))
      } catch {
        sendJson(res, [])
      }
      return
    }

    // PUT /api/categories
    if (url === '/categories' && method === 'PUT') {
      try {
        const body = await readBody(req)
        const parsed = JSON.parse(body)
        fs.writeFileSync(categoriesFile, JSON.stringify(parsed, null, 2) + '\n')
        sendJson(res, { ok: true })
      } catch (err) {
        sendError(res, `Failed to save categories: ${(err as Error).message}`, 500)
      }
      return
    }

    // GET /api/effect-registry
    if (url === '/effect-registry' && method === 'GET') {
      if (!fs.existsSync(effectRegistryFile)) {
        sendError(res, 'Effect registry not found', 404)
        return
      }
      try {
        sendJson(res, JSON.parse(fs.readFileSync(effectRegistryFile, 'utf-8')))
      } catch (err) {
        sendError(res, `Failed to read effect registry: ${(err as Error).message}`, 500)
      }
      return
    }

    // GET /api/shops — list all shops
    if (url === '/shops' && method === 'GET') {
      if (!fs.existsSync(shopsDir)) {
        sendJson(res, [])
        return
      }
      try {
        const files = fs.readdirSync(shopsDir)
          .filter(f => f.endsWith('.json'))
          .map(f => f.replace(/\.json$/, ''))
          .sort()
        sendJson(res, files)
      } catch (err) {
        sendError(res, `Failed to list shops: ${(err as Error).message}`, 500)
      }
      return
    }

    // GET/PUT /api/shops/:id
    const shopMatch = url.match(/^\/shops\/([A-Za-z][A-Za-z0-9_]*)$/)
    if (shopMatch) {
      const shopId = shopMatch[1]
      const filePath = path.join(shopsDir, `${shopId}.json`)

      if (method === 'GET') {
        if (!fs.existsSync(filePath)) {
          sendError(res, `Shop "${shopId}" not found`)
          return
        }
        try {
          sendJson(res, JSON.parse(fs.readFileSync(filePath, 'utf-8')))
        } catch (err) {
          sendError(res, `Failed to read ${shopId}.json: ${(err as Error).message}`, 500)
        }
        return
      }

      if (method === 'PUT') {
        try {
          const body = await readBody(req)
          const parsed = JSON.parse(body)
          fs.writeFileSync(filePath, JSON.stringify(parsed, null, 2) + '\n')
          sendJson(res, { ok: true })
        } catch (err) {
          sendError(res, `Failed to write ${shopId}.json: ${(err as Error).message}`, 500)
        }
        return
      }
    }

    next()
  })
}
