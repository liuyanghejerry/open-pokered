// ───────────────────────────────────────────────────────────────────────────
// applyChange — the single write surface for accepted proposals.
//
// The review tray (client) calls POST /api/ai/apply-change once per accepted
// proposal. The server resolves the target file (per kind) via ProjectContext
// and writes the new content, returning the previous content as `backup` so the
// client can offer Revert (re-apply `before`, or delete a freshly-created file).
//
// Beyond single-file targets, three PROJECT-level kinds are handled here:
//   project-config   — overwrite `.dotzuki-editor.json` (caches reset after write)
//   project-scaffold — lay out a whole new project via scaffoldProject and
//                      switch the editor root to it (revert deletes the new dir)
//   map-create       — create a map directory via the shared createMap helper
//                      (revert deletes the new map dir)
// Directory deletes are guarded: only ever recursive-delete a directory this
// same apply path could have created (marker-file + name checks), never a
// generic path.
// ───────────────────────────────────────────────────────────────────────────
import fs from 'fs'
import path from 'path'
import type { ProjectContext } from '../context/projectContext'
import type { ChangeTarget } from './changeSet'
import { scaffoldProject } from '../scaffold'
import { createMap } from '../api/mapCreate'
import { getProjectRoot, setProjectRootDir, resetConfigCache } from '../api/projectConfig'

export interface ApplyChangeRequest {
  target: ChangeTarget
  /** 'write' (default) applies `after`; 'delete' removes the target file (revert of a create). */
  op?: 'write' | 'delete'
  after?: string
  /**
   * The file content the proposal's diff was computed against (`before`): a
   * string for an edit, `null` when the proposal expected to CREATE the file.
   * When provided, applyChange refuses to write if the file has since drifted
   * from it (stale-proposal guard). `undefined` skips the check (e.g. revert).
   */
  expect?: string | null
  /** Bypass the drift check and overwrite anyway. */
  force?: boolean
}

export interface ApplyChangeResult {
  ok: boolean
  /** Previous file content (null if the file did not exist), for Revert. */
  backup: string | null
  /** The applied target file, relative to the project root. */
  path: string
  /** True when the write was refused because the file drifted from `expect`. */
  conflict?: boolean
}

/** The structured payload a project-scaffold proposal carries in `after`. */
interface ScaffoldPayload {
  name: string
  /** Folder slug under the editor root, or an absolute path. */
  dir: string
  templateId: string
  dataRoot?: string
  gfxRoot?: string
}

/** Resolve a scaffold target dir the same way the project create route does. */
function scaffoldTargetAbs(dir: string): string {
  const d = String(dir ?? '').trim()
  if (!d) throw new Error('project-scaffold target needs dir')
  if (path.isAbsolute(d)) return path.normalize(d)
  if (!/^[a-z0-9][a-z0-9-]*$/.test(d)) throw new Error(`Invalid directory name: ${d}`)
  return path.join(getProjectRoot(), d)
}

/**
 * Resolve a scaffold target for DELETE (revert-of-create). Applying a scaffold
 * switches the editor root INTO the new project, so a slug would now resolve
 * one level too deep — in that case the scaffolded dir is the root itself.
 */
function scaffoldDeleteAbs(t: ChangeTarget): string {
  const primary = scaffoldTargetAbs(t.dir ?? '')
  if (fs.existsSync(primary)) return primary
  const root = getProjectRoot()
  if (!path.isAbsolute(String(t.dir ?? '')) && path.basename(root) === t.dir) return root
  return primary
}

/** Absolute path of map `<mapsDir>/<name>`, clamped so it cannot escape mapsDir. */
function mapDirAbs(project: ProjectContext, name: string): string {
  if (!name || !/^[A-Za-z0-9_-]+$/.test(name)) throw new Error('map-create target needs a valid map name (A–Z, 0–9, _-)')
  const mc = (project.activity('map')?.config ?? {}) as { mapsDir?: string }
  const base = project.resolveData(mc.mapsDir ?? 'maps')
  const abs = path.resolve(base, path.basename(name))
  if (abs !== base && !abs.startsWith(base + path.sep)) throw new Error('access denied')
  return abs
}

/**
 * Recursively delete a directory THIS apply path created (revert of a
 * scaffold/map-create). Guards against becoming a generic rm -rf: the dir must
 * contain the marker file its creator wrote, and for scaffolds the recorded
 * project name must match when the target carries one.
 */
function removeCreatedDir(abs: string, t: ChangeTarget): void {
  if (!fs.existsSync(abs)) return
  if (!fs.statSync(abs).isDirectory()) throw new Error('not a directory: ' + abs)
  const marker = t.kind === 'project-scaffold'
    ? path.join(abs, '.dotzuki-editor.json')
    : path.join(abs, 'map.json')
  if (!fs.existsSync(marker)) throw new Error('refusing to delete: directory was not created by this proposal')
  if (t.kind === 'project-scaffold' && t.name) {
    let cfgName: unknown
    try { cfgName = JSON.parse(fs.readFileSync(marker, 'utf-8'))?.name }
    catch { throw new Error('refusing to delete: unreadable project config') }
    if (cfgName !== t.name) throw new Error('refusing to delete: project name mismatch')
  }
  fs.rmSync(abs, { recursive: true, force: true })
}

function resolveTargetPath(project: ProjectContext, t: ChangeTarget): string {
  switch (t.kind) {
    case 'story':
      if (!t.storyKind || !t.id) throw new Error('story target needs storyKind + id')
      return project.storyRecordPath(t.storyKind, t.id)
    case 'data':
      if (!t.table || !t.id) throw new Error('data target needs table + id')
      return project.dataRecordPath(t.table, t.id)
    case 'scene':
      if (!t.scene) throw new Error('scene target needs scene')
      return project.sceneAbsPath(t.scene)
    case 'gui':
      if (!t.name) throw new Error('gui target needs name')
      return project.guiAbsPath(t.name)
    case 'map':
      if (!t.map) throw new Error('map target needs map')
      return project.mapObjectsPath(t.map)
    case 'project-config':
      return project.configFile()
    case 'project-scaffold':
      return scaffoldTargetAbs(t.dir ?? '')
    case 'map-create':
      return mapDirAbs(project, t.map ?? '')
    default:
      throw new Error('unknown target kind: ' + (t as ChangeTarget).kind)
  }
}

export function applyChange(project: ProjectContext, req: ApplyChangeRequest): ApplyChangeResult {
  // Revert of a scaffold: resolve against the pre-switch location (the root
  // moved INTO the new project when the scaffold was applied).
  if (req.op === 'delete' && req.target.kind === 'project-scaffold') {
    const abs = scaffoldDeleteAbs(req.target)
    removeCreatedDir(abs, req.target)
    if (getProjectRoot() === abs) setProjectRootDir(path.dirname(abs))
    return { ok: true, backup: null, path: abs }
  }

  const abs = resolveTargetPath(project, req.target)

  if (req.op === 'delete') {
    if (req.target.kind === 'map-create') {
      // Revert of a create: recursively remove the new map dir (guarded).
      removeCreatedDir(abs, req.target)
      return { ok: true, backup: null, path: path.relative(project.root, abs) }
    }
    const backup = fs.existsSync(abs) && fs.statSync(abs).isFile() ? fs.readFileSync(abs, 'utf-8') : null
    if (fs.existsSync(abs)) fs.unlinkSync(abs)
    if (req.target.kind === 'project-config') { resetConfigCache(); project.resetConfigCache() }
    return { ok: true, backup, path: path.relative(project.root, abs) }
  }

  if (typeof req.after !== 'string') throw new Error('after content is required for a write')

  if (req.target.kind === 'project-scaffold') return applyScaffold(abs, req)
  if (req.target.kind === 'map-create') return applyMapCreate(project, abs, req)

  const backup = fs.existsSync(abs) && fs.statSync(abs).isFile() ? fs.readFileSync(abs, 'utf-8') : null
  const relPath = path.relative(project.root, abs)

  // Stale-proposal guard: if the caller told us what the file looked like when
  // the diff was built (`expect`) and it has since drifted, refuse rather than
  // silently clobber the intervening change. `force` overrides.
  if (!req.force && req.expect !== undefined && backup !== req.expect) {
    return { ok: false, conflict: true, backup, path: relPath }
  }

  fs.mkdirSync(path.dirname(abs), { recursive: true })
  fs.writeFileSync(abs, req.after, 'utf-8')
  if (req.target.kind === 'project-config') { resetConfigCache(); project.resetConfigCache() }
  return { ok: true, backup, path: relPath }
}

/** Apply a project-scaffold proposal: scaffold the project, switch root to it. */
function applyScaffold(abs: string, req: ApplyChangeRequest): ApplyChangeResult {
  let payload: ScaffoldPayload
  try { payload = JSON.parse(req.after!) } catch { throw new Error('project-scaffold needs a JSON payload in after') }
  if (!payload?.name || !payload?.templateId) throw new Error('project-scaffold payload needs name + templateId')

  // Same rule as the create route: never scaffold into a non-empty directory.
  if (fs.existsSync(abs) && fs.statSync(abs).isDirectory() && fs.readdirSync(abs).length > 0) {
    return { ok: false, conflict: true, backup: null, path: abs }
  }

  scaffoldProject(abs, {
    name: String(payload.name),
    templateId: String(payload.templateId),
    dataRoot: payload.dataRoot,
    gfxRoot: payload.gfxRoot,
  })
  setProjectRootDir(abs)
  return { ok: true, backup: null, path: abs }
}

/** Apply a map-create proposal: create the map dir via the shared helper. */
function applyMapCreate(project: ProjectContext, abs: string, req: ApplyChangeRequest): ApplyChangeResult {
  // Stale-proposal guard: the proposal expected to CREATE the map (before=null),
  // so an existing map.json is a collision, not something to overwrite.
  if (!req.force && req.expect === null && fs.existsSync(path.join(abs, 'map.json'))) {
    return { ok: false, conflict: true, backup: null, path: path.relative(project.root, abs) }
  }
  createMap(project, { name: path.basename(abs) })
  return { ok: true, backup: null, path: path.relative(project.root, abs) }
}
