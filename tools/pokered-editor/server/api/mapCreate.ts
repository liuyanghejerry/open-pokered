// ───────────────────────────────────────────────────────────────────────────
// createMap — lay out a new map directory. Shared by the /api/maps-create route
// and the assistant's map-create apply path so the on-disk shape never forks.
//
// Two shapes, chosen by sniffing the project's existing maps:
//   pokered  — an existing maps/*/map.json carries a `header` block: write the
//              full game shape (id = next free, header{}, connections, warps,
//              npcs, signs, text, wild) + map.blk + script_config.json + an
//              empty script.scene, mirroring POST /api/maps in pokeredRoutes.
//   generic  — any other dotzuki-editor project: the legacy minimal map.json
//              (name/width/height/tileset/music + empty lists).
// ───────────────────────────────────────────────────────────────────────────
import path from 'path'
import fs from 'fs'
import type { ProjectContext } from '../context/projectContext'

export interface CreateMapParams {
  /** Map directory name under the map activity's mapsDir. */
  name: string
  /** Tileset name (pokered shape; default 'Overworld'). */
  tileset?: string
  /** Map width in blocks (default 10). */
  width?: number
  /** Map height in blocks (default 9). */
  height?: number
  /** Music track id (pokered shape; default 'PalletTown'). */
  music?: string
  /** Border block id — also the map.blk fill (default 0). */
  borderBlock?: number
  /** Display name for the town-map dot (defaults to uppercased `name`). */
  displayName?: string
  /** Optional town-map placement; recorded into town_map_extras.json. */
  townMap?: { x: number; y: number }
}

/** True when the project's existing maps use the pokered `header` shape. */
function isPokeredMapProject(mapsDirAbs: string): boolean {
  if (!fs.existsSync(mapsDirAbs)) return false
  for (const d of fs.readdirSync(mapsDirAbs, { withFileTypes: true })) {
    if (!d.isDirectory()) continue
    const p = path.join(mapsDirAbs, d.name, 'map.json')
    if (!fs.existsSync(p)) continue
    try {
      if (JSON.parse(fs.readFileSync(p, 'utf-8'))?.header) return true
    } catch { /* ignore unreadable */ }
  }
  return false
}

/** Next free numeric map id (max existing + 1, 0 when there are none). */
function nextMapId(mapsDirAbs: string): number {
  let maxId = -1
  if (!fs.existsSync(mapsDirAbs)) return 0
  for (const d of fs.readdirSync(mapsDirAbs, { withFileTypes: true })) {
    if (!d.isDirectory()) continue
    const p = path.join(mapsDirAbs, d.name, 'map.json')
    if (!fs.existsSync(p)) continue
    try {
      const j = JSON.parse(fs.readFileSync(p, 'utf-8'))
      if (typeof j.id === 'number' && j.id > maxId) maxId = j.id
    } catch { /* ignore */ }
  }
  return maxId + 1
}

/** Create `<mapsDir>/<name>/` + map files; returns the map dir. */
export function createMap(project: ProjectContext, params: CreateMapParams): { name: string; dir: string } {
  const mapActivity = project.config().activities.find(a => a.type === 'map')
  if (!mapActivity) throw new Error('No map activity configured')
  const mc = mapActivity.config as { mapsDir: string }

  const name = String(params.name)
  const mapsDirAbs = project.resolveData(mc.mapsDir)
  const dir = path.join(mapsDirAbs, name)
  fs.mkdirSync(dir, { recursive: true })

  if (isPokeredMapProject(mapsDirAbs)) {
    createPokeredMap(project, dir, name, params)
  } else {
    const mapJson = {
      name,
      width: 20, height: 18,
      tileset: '',
      music: '',
      warps: [], signs: [], npcs: [],
    }
    fs.writeFileSync(path.join(dir, 'map.json'), JSON.stringify(mapJson, null, 2), 'utf-8')
  }
  return { name, dir }
}

/** The pokered map shape — mirrors the POST /api/maps route in pokeredRoutes. */
function createPokeredMap(project: ProjectContext, dir: string, name: string, params: CreateMapParams): void {
  const mapsDirAbs = path.dirname(dir)
  const tileset = params.tileset ?? 'Overworld'
  const width = Math.max(1, Math.min(255, Math.floor(params.width ?? 10)))
  const height = Math.max(1, Math.min(255, Math.floor(params.height ?? 9)))
  const music = params.music ?? 'PalletTown'
  const borderBlock = Math.max(0, Math.min(255, Math.floor(params.borderBlock ?? 0)))

  const mapJson = {
    id: nextMapId(mapsDirAbs),
    name,
    header: { tileset, music, connectionFlags: 0, width, height, borderBlock },
    connections: {},
    warps: [],
    npcs: [],
    signs: [],
    text: { npc: {}, sign: {} },
    wild: null,
  }
  fs.writeFileSync(path.join(dir, 'map.json'), JSON.stringify(mapJson, null, 2) + '\n')
  fs.writeFileSync(path.join(dir, 'map.blk'), Buffer.alloc(width * height, borderBlock))
  fs.writeFileSync(
    path.join(dir, 'script_config.json'),
    JSON.stringify({ npcs: [], signs: [], coordEvents: [] }, null, 2) + '\n',
  )
  // Empty scene: compiles to a no-op; the author fills it via the script
  // editor or the assistant's propose_scene_write.
  fs.writeFileSync(path.join(dir, 'script.scene'), '')

  // Optional town-map placement (same sidecar the /api/maps route maintains).
  if (params.townMap && Number.isFinite(params.townMap.x) && Number.isFinite(params.townMap.y)) {
    const extrasFile = project.resolveData('town_map_extras.json')
    let extras: Record<string, { x: number; y: number; displayName: string }> = {}
    if (fs.existsSync(extrasFile)) {
      try { extras = JSON.parse(fs.readFileSync(extrasFile, 'utf-8')) } catch { /* ignore */ }
    }
    extras[name] = {
      x: Math.max(0, Math.min(15, Math.floor(params.townMap.x))),
      y: Math.max(0, Math.min(15, Math.floor(params.townMap.y))),
      displayName: (params.displayName ?? name).toUpperCase(),
    }
    fs.writeFileSync(extrasFile, JSON.stringify(extras, null, 2) + '\n')
  }
}
