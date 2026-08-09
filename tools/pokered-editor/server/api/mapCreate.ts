// ───────────────────────────────────────────────────────────────────────────
// createMap — lay out a new map directory with a minimal map.json. Shared by
// the /api/maps-create route and the assistant's map-create apply path so the
// on-disk shape never forks.
// ───────────────────────────────────────────────────────────────────────────
import path from 'path'
import fs from 'fs'
import type { ProjectContext } from '../context/projectContext'

export interface CreateMapParams {
  /** Map directory name under the map activity's mapsDir. */
  name: string
}

/** Create `<mapsDir>/<name>/` + a minimal map.json; returns the map dir. */
export function createMap(project: ProjectContext, params: CreateMapParams): { name: string; dir: string } {
  const mapActivity = project.config().activities.find(a => a.type === 'map')
  if (!mapActivity) throw new Error('No map activity configured')
  const mc = mapActivity.config as { mapsDir: string }

  const name = String(params.name)
  const dir = project.resolveData(path.join(mc.mapsDir, name))
  fs.mkdirSync(dir, { recursive: true })
  const mapJson = {
    name,
    width: 20, height: 18,
    tileset: '',
    music: '',
    warps: [], signs: [], npcs: [],
  }
  fs.writeFileSync(path.join(dir, 'map.json'), JSON.stringify(mapJson, null, 2), 'utf-8')
  return { name, dir }
}
