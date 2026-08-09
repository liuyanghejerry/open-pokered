#!/usr/bin/env node
// ───────────────────────────────────────────────────────────────────────────
// Static baseline generator for the pokered-editor GitHub Pages build.
//
// Run from the Cargo workspace root (the deploy workflow does
// `working-directory: workspace`):
//
//   node tools/pokered-editor/scripts/generate-static-data.mjs
//
// Reads:
//   crates/pokered-data/  — maps, trainers, pokemon, moves,
//                                             items, ui_layouts, town-map extras,
//                                             tileset extras, passable overrides
//   gfx/                  — blockset .bst files + every image
//                                             (copied verbatim so /gfx URLs work)
//
// Writes into tools/pokered-editor/dist/ (which exists after `npm run build`):
//   maps.json                              — warp/playtest map list (GET /api/maps)
//   data/list/*.json                       — family + layout + item name lists
//   data/maps/<Name>/*                     — map.json, map.blk.json, scripts
//   data/{trainers,pokemon,moves,items}/<Name>.json
//   data/ui_layouts/<Name>.gui             — layout DSL source (compiled in-browser)
//   data/blocksets.json                    — parsed .bst blocks (GET /api/blocksets)
//   data/passable_tiles.json               — builtin + overrides (GET /api/passable-tiles)
//   data/town_map_extras.json              — GET /api/town-map-extras
//   data/tileset_extras.json               — GET /api/tileset-extras
//   gfx/                                   — full gfx tree (tileset PNGs, sprites…)
//
// The layout mirrors what dataFetch's staticFetch() expects; the tables below
// MUST stay in sync with server/pokeredRoutes.ts.
// ───────────────────────────────────────────────────────────────────────────

import fs from 'node:fs'
import path from 'node:path'

const cwd = process.cwd()
const DATA_ROOT = path.join(cwd, 'crates/pokered-data')
const GFX_ROOT = path.join(cwd, 'gfx')
const OUT = path.join(cwd, 'tools/pokered-editor/dist')

if (!fs.existsSync(path.join(DATA_ROOT, 'maps'))) {
  console.error(`data root not found: ${DATA_ROOT} — run from the workspace root`)
  process.exit(1)
}
if (!fs.existsSync(GFX_ROOT)) {
  console.error(`gfx root not found: ${GFX_ROOT} — run scripts/fetch-gfx.sh first`)
  process.exit(1)
}

// ── Tileset tables (mirror server/pokeredRoutes.ts) ──────────────────────
const TILESET_BST_FILES = {
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

const TILESET_PASSABLE_TILES = {
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

function parseBst(bstPath) {
  const buf = fs.readFileSync(bstPath)
  const numBlocks = Math.floor(buf.length / BLOCK_SIZE)
  const blocks = {}
  for (let i = 0; i < numBlocks; i++) {
    blocks[i] = Array.from(buf.subarray(i * BLOCK_SIZE, (i + 1) * BLOCK_SIZE))
  }
  return blocks
}

function readJsonOr(file, fallback) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf-8'))
  } catch {
    return fallback
  }
}

const dataOut = path.join(OUT, 'data')
const listDir = path.join(dataOut, 'list')
fs.mkdirSync(listDir, { recursive: true })

// ── maps: list + per-map files (map.blk → JSON array like the dev server) ─
const mapsRoot = path.join(DATA_ROOT, 'maps')
const maps = fs.readdirSync(mapsRoot)
  .filter((d) => {
    try {
      return fs.statSync(path.join(mapsRoot, d)).isDirectory() &&
        fs.existsSync(path.join(mapsRoot, d, 'map.json'))
    } catch { return false }
  })
  .sort()
for (const name of maps) {
  const srcDir = path.join(mapsRoot, name)
  const dstDir = path.join(dataOut, 'maps', name)
  fs.mkdirSync(dstDir, { recursive: true })
  for (const f of ['map.json', 'script_config.json', 'script.scene', 'script.js']) {
    const p = path.join(srcDir, f)
    if (fs.existsSync(p)) fs.copyFileSync(p, path.join(dstDir, f))
  }
  const blkPath = path.join(srcDir, 'map.blk')
  if (fs.existsSync(blkPath)) {
    fs.writeFileSync(path.join(dstDir, 'map.blk.json'),
      JSON.stringify(Array.from(fs.readFileSync(blkPath))))
  }
}
fs.writeFileSync(path.join(OUT, 'maps.json'), JSON.stringify(maps))
fs.writeFileSync(path.join(listDir, 'maps.json'), JSON.stringify(maps))
console.log(`maps: ${maps.length}`)

// ── families: trainers / pokemon / moves (list = .json basenames) ────────
for (const fam of ['trainers', 'pokemon', 'moves']) {
  const src = path.join(DATA_ROOT, fam)
  const dst = path.join(dataOut, fam)
  fs.mkdirSync(dst, { recursive: true })
  const names = fs.readdirSync(src)
    .filter((f) => f.endsWith('.json'))
    .map((f) => f.replace(/\.json$/, ''))
    .sort()
  for (const n of names) fs.copyFileSync(path.join(src, `${n}.json`), path.join(dst, `${n}.json`))
  fs.writeFileSync(path.join(listDir, `${fam}.json`), JSON.stringify(names))
  console.log(`${fam}: ${names.length}`)
}

// ── items: exclude item_list.json (the list is generated here) ───────────
const itemsSrc = path.join(DATA_ROOT, 'data/items')
const itemsDst = path.join(dataOut, 'items')
fs.mkdirSync(itemsDst, { recursive: true })
const items = fs.readdirSync(itemsSrc)
  .filter((f) => f.endsWith('.json') && f !== 'item_list.json')
  .map((f) => f.replace(/\.json$/, ''))
  .sort()
for (const n of items) fs.copyFileSync(path.join(itemsSrc, `${n}.json`), path.join(itemsDst, `${n}.json`))
fs.writeFileSync(path.join(listDir, 'items.json'), JSON.stringify(items))
console.log(`items: ${items.length}`)

// ── shops: data/shops/*.json ───────────────────────────────────────────────
const shopsSrc = path.join(DATA_ROOT, 'data/shops')
const shopsDst = path.join(dataOut, 'shops')
const shops = fs.existsSync(shopsSrc)
  ? fs.readdirSync(shopsSrc)
      .filter((f) => f.endsWith('.json'))
      .map((f) => f.replace(/\.json$/, ''))
      .sort()
  : []
if (shops.length > 0) {
  fs.mkdirSync(shopsDst, { recursive: true })
  for (const n of shops) fs.copyFileSync(path.join(shopsSrc, `${n}.json`), path.join(shopsDst, `${n}.json`))
}
fs.writeFileSync(path.join(listDir, 'shops.json'), JSON.stringify(shops))
console.log(`shops: ${shops.length}`)

// ── ui layouts: the .gui DSL source is the source of truth → flat dir. The
// legacy compiled v1 JSON (ui_layouts/v1/) is kept in the repo for reference
// but no longer shipped: the editor opens .gui files (DSL) by default and
// compiles them in-browser via the WASM preview bridge, which the legacy
// variants-format JSON can't feed (the renderer expects schema-v2 JSON).
const layoutsGuiSrc = path.join(DATA_ROOT, 'ui_layouts')
const layoutsDst = path.join(dataOut, 'ui_layouts')
fs.mkdirSync(layoutsDst, { recursive: true })
const layouts = fs.existsSync(layoutsGuiSrc)
  ? fs.readdirSync(layoutsGuiSrc)
      .filter((f) => f.endsWith('.gui'))
      .map((f) => f.replace(/\.gui$/, ''))
      .sort()
  : []
for (const n of layouts) {
  fs.copyFileSync(path.join(layoutsGuiSrc, `${n}.gui`), path.join(layoutsDst, `${n}.gui`))
}
fs.writeFileSync(path.join(listDir, 'ui_layouts.json'), JSON.stringify(layouts))
console.log(`ui_layouts (.gui): ${layouts.length}`)

// ── blocksets: parse every .bst into { tileset: { blockId: number[16] } } ─
const bstRoot = path.join(GFX_ROOT, 'blocksets')
const tilesetExtras = readJsonOr(path.join(DATA_ROOT, 'tileset_extras.json'), {})
const blocksets = {}
for (const [name, file] of Object.entries(TILESET_BST_FILES)) {
  const p = path.join(bstRoot, file)
  if (fs.existsSync(p)) blocksets[name] = parseBst(p)
}
for (const [name, info] of Object.entries(tilesetExtras)) {
  const file = info.bstFile ?? `${name.toLowerCase()}.bst`
  const p = path.join(bstRoot, file)
  if (fs.existsSync(p)) blocksets[name] = parseBst(p)
}
fs.writeFileSync(path.join(dataOut, 'blocksets.json'), JSON.stringify(blocksets))
console.log(`blocksets: ${Object.keys(blocksets).length}`)

// ── passable tiles: builtin defaults + persisted overrides + custom empty ─
const passableOverrides = readJsonOr(path.join(DATA_ROOT, 'tileset_passable_overrides.json'), {})
const passable = { ...TILESET_PASSABLE_TILES }
for (const [k, v] of Object.entries(passableOverrides)) {
  if (Array.isArray(v)) passable[k] = v.slice()
}
for (const name of Object.keys(tilesetExtras)) {
  if (!(name in passable)) passable[name] = []
}
fs.writeFileSync(path.join(dataOut, 'passable_tiles.json'), JSON.stringify(passable))
console.log(`passable_tiles: ${Object.keys(passable).length}`)

// ── town-map extras + tileset extras (empty objects when absent) ──────────
const townMapExtras = readJsonOr(path.join(DATA_ROOT, 'town_map_extras.json'), {})
fs.writeFileSync(path.join(dataOut, 'town_map_extras.json'), JSON.stringify(townMapExtras))
fs.writeFileSync(path.join(dataOut, 'tileset_extras.json'), JSON.stringify(tilesetExtras))
console.log(`town_map_extras: ${Object.keys(townMapExtras).length}`)

// ── gfx: full copy so base-prefixed /gfx URLs resolve on static hosting ───
fs.cpSync(GFX_ROOT, path.join(OUT, 'gfx'), { recursive: true })
console.log(`gfx: copied ${GFX_ROOT} → ${path.join(OUT, 'gfx')}`)

console.log('static baseline ready')
