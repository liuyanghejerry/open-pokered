// ───────────────────────────────────────────────────────────────────────────
// ProjectContext — the editor-agent framework's read/retrieval layer.
//
// One module that knows how to read a jrpg-engine project (stories, scenes,
// flags, data tables, gui, maps, the DSL guide) entirely from its
// `.jrpg-editor.json` config — no game-specific knowledge. It generalizes the
// helpers that were previously closures inside vite.config.ts
// (storiesRoot / readStoryRecord / scanFlags / listScenes / assembleAiContext)
// so every AI action (refine-character, generate-scene, the chat assistant, …)
// shares one source of truth for "what's in this project".
//
// Two responsibilities:
//   1. Plain reads     — list/read records, scenes, flags, tables, gui, maps.
//   2. Structured      — resolve @mentions and follow references (a quest pulls
//      retrieval          its characters + flags + scenes; a character pulls its
//                         relationships + the quests it appears in), plus
//                         assembleContext() which grounds the model in the real
//                         engine API even when no `ai` block is configured.
//
// Pure Node (fs/path); no AI deps. Safe to import from the Vite dev middleware
// and from vitest. Parameterized by a project root so it is fully testable.
// ───────────────────────────────────────────────────────────────────────────
import fs from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'

// This module's real directory under Vite 8 / vitest (and the bundle's own
// directory inside the esbuild-built Electron api-server).
const moduleDir = path.dirname(fileURLToPath(import.meta.url))

export interface TableDef {
  id: string
  dir: string
  idField?: string
  label?: unknown
  fields?: unknown[]
  [k: string]: unknown
}

export interface ActivityDef {
  id: string
  type: 'map' | 'script' | 'data' | 'assets' | 'story' | 'ui' | 'tiles' | 'settings' | string
  label?: string
  icon?: string
  enabled?: boolean
  config: Record<string, unknown>
}

export interface ProjectConfig {
  name: string
  dataRoot: string
  gfxRoot?: string
  activities: ActivityDef[]
}

/** A `.scene` file discovered under the story scenesDir. */
export interface SceneEntry {
  /** Stable identifier: path minus extension, with a trailing `/script` collapsed. */
  stem: string
  /** @storyline / game_scene handler names declared in the file. */
  names: string[]
  /** Path relative to the scenesDir. */
  path: string
}

export type StoryKind = 'characters' | 'quests' | 'arcs' | string

export interface StoryRecord {
  id?: string
  name?: unknown
  [k: string]: unknown
}

/** A resolvable reference the chat surface can @-mention. */
export interface MentionTarget {
  kind: 'character' | 'quest' | 'arc' | 'scene' | 'data' | 'gui' | 'map'
  /** Cross-reference key (record id, scene stem, gui name, map name). */
  id: string
  /** Human label for the picker (display name when available). */
  label: string
  /** For data records, which table they came from. */
  table?: string
}

export interface AssembleContextOptions {
  /** Auto-sample real scene files when no explicit exampleScenes are configured. Default true. */
  autoExampleScenes?: boolean
  /** How many scenes to auto-sample. Default 3. */
  exampleSceneLimit?: number
  /** Byte cap per auto-sampled scene. Default 3000. */
  exampleSceneBytes?: number
}

const SCENE_DEFAULT_DIR = 'maps'

/**
 * A read/retrieval view over one jrpg-engine project. Construct with
 * `createProjectContext(root)`; use `getProjectContext()` for the default
 * (env / cwd) project shared across the dev server.
 */
export class ProjectContext {
  readonly root: string
  private _config: ProjectConfig | null = null

  constructor(root: string) {
    this.root = path.resolve(root)
  }

  // ── Config & roots ────────────────────────────────────────────────────────

  configFile(): string {
    return path.join(this.root, '.jrpg-editor.json')
  }

  /** Loaded `.jrpg-editor.json` (cached). Throws if missing — same as the server. */
  config(): ProjectConfig {
    if (this._config) return this._config
    const file = this.configFile()
    if (!fs.existsSync(file)) {
      throw new Error(`No .jrpg-editor.json found in ${this.root}. Run 'jrpg-editor init' first.`)
    }
    this._config = JSON.parse(fs.readFileSync(file, 'utf-8')) as ProjectConfig
    return this._config
  }

  /** Drop the cached config so the next config() re-reads `.jrpg-editor.json`. */
  resetConfigCache(): void {
    this._config = null
  }

  /** Resolve a path relative to the project's dataRoot. */
  resolveData(rel: string): string {
    return path.resolve(this.root, this.config().dataRoot, rel)
  }

  /** Resolve a path relative to the project's gfxRoot. */
  resolveGfx(rel: string): string {
    return path.resolve(this.root, this.config().gfxRoot ?? 'gfx', rel)
  }

  /** Read a dataRoot-relative file, or null if it is absent / not a file. */
  readDataFileOrNull(rel: string): string | null {
    const abs = this.resolveData(rel)
    return fs.existsSync(abs) && fs.statSync(abs).isFile() ? fs.readFileSync(abs, 'utf-8') : null
  }

  /** First enabled activity of a given type, or null. */
  activity(type: ActivityDef['type']): ActivityDef | null {
    return this.config().activities.find(a => a.type === type && a.enabled !== false) ?? null
  }

  /** The story activity's config block, or null if the project has no story activity. */
  storyConfig(): Record<string, any> | null {
    return (this.activity('story')?.config as Record<string, any>) ?? null
  }

  /** All data tables declared across data activities. */
  dataTables(): TableDef[] {
    return this.config().activities.flatMap(a =>
      a.type === 'data' ? ((a.config as { tables?: TableDef[] }).tables ?? []) : [],
    )
  }

  // ── Sandboxed file read ───────────────────────────────────────────────────

  /** Read a UTF-8 file, refusing anything that escapes the project root. */
  readFileSandboxed(rel: string): string {
    const abs = path.resolve(this.root, rel)
    if (abs !== this.root && !abs.startsWith(this.root + path.sep)) throw new Error('access denied')
    if (!fs.existsSync(abs) || !fs.statSync(abs).isFile()) throw new Error('not found: ' + rel)
    return fs.readFileSync(abs, 'utf-8')
  }

  // ── Story records (characters / quests / arcs / …) ────────────────────────

  private storiesRoot(): string {
    const sc = this.storyConfig()
    if (!sc?.storiesDir) throw new Error('No story activity / storiesDir configured')
    return this.resolveData(sc.storiesDir)
  }

  /** Kebab-ASCII slug for a record id (mirrors the server's filename rule). */
  static storySlug(id: string): string {
    const s = String(id).toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '')
    return s || path.basename(String(id)) || 'record'
  }

  /**
   * Map a record id to its on-disk file. Story files are named by a kebab slug
   * while the record `id` is a display name, so resolve by MATCHING an existing
   * record's `id` — not by assuming filename === id.
   */
  private resolveStoryFile(dir: string, id: string): string {
    if (fs.existsSync(dir)) {
      for (const f of fs.readdirSync(dir)) {
        if (!f.endsWith('.json')) continue
        try {
          if (JSON.parse(fs.readFileSync(path.join(dir, f), 'utf-8'))?.id === id) return path.join(dir, f)
        } catch { /* skip unreadable */ }
      }
    }
    return path.join(dir, `${ProjectContext.storySlug(id)}.json`)
  }

  /** Read one story record by kind + id, or null if absent/unreadable. */
  readStoryRecord(kind: StoryKind, id: string): StoryRecord | null {
    let dir: string
    try { dir = path.join(this.storiesRoot(), kind) } catch { return null }
    const file = this.resolveStoryFile(dir, id)
    if (!fs.existsSync(file)) return null
    try { return JSON.parse(fs.readFileSync(file, 'utf-8')) as StoryRecord } catch { return null }
  }

  /** All records of a given story kind. */
  listStoryRecords(kind: StoryKind): StoryRecord[] {
    let dir: string
    try { dir = path.join(this.storiesRoot(), kind) } catch { return [] }
    if (!fs.existsSync(dir)) return []
    const out: StoryRecord[] = []
    for (const f of fs.readdirSync(dir)) {
      if (!f.endsWith('.json')) continue
      try { out.push(JSON.parse(fs.readFileSync(path.join(dir, f), 'utf-8'))) } catch { /* skip */ }
    }
    return out
  }

  listCharacters(): StoryRecord[] { return this.listStoryRecords('characters') }
  listQuests(): StoryRecord[] { return this.listStoryRecords('quests') }
  listArcs(): StoryRecord[] { return this.listStoryRecords('arcs') }

  // ── Scenes (.scene DSL) ───────────────────────────────────────────────────

  private scenesDir(): string {
    const sc = this.storyConfig() ?? {}
    return this.resolveData(sc.scenesDir ?? SCENE_DEFAULT_DIR)
  }

  private sceneExt(): string {
    const sc = this.storyConfig() ?? {}
    return sc.scene?.ext ?? '.scene'
  }

  /**
   * List `.scene` files under the scenesDir as { stem, names, path }. Mirrors
   * GET /api/scenes: the stem collapses a trailing `/script` (the per-map
   * `<Map>/script.scene` convention) so a scene links as "Wangjiang".
   */
  listScenes(): SceneEntry[] {
    const root = this.scenesDir()
    const ext = this.sceneExt()
    if (!fs.existsSync(root)) return []
    const out: SceneEntry[] = []
    const walk = (dir: string, rel: string) => {
      for (const name of fs.readdirSync(dir).sort()) {
        const full = path.join(dir, name)
        const childRel = rel ? `${rel}/${name}` : name
        if (fs.statSync(full).isDirectory()) { walk(full, childRel); continue }
        if (!name.endsWith(ext)) continue
        const stem = childRel.slice(0, -ext.length).replace(/\/script$/, '')
        let names: string[] = []
        try {
          const text = fs.readFileSync(full, 'utf-8')
          names = [...text.matchAll(/@storyline\("([^"]+)"\)/g)].map(m => m[1])
          if (!names.length) names = [...text.matchAll(/game_scene\s+(\w+)/g)].map(m => m[1])
        } catch { /* unreadable — leave names empty */ }
        out.push({ stem, names, path: childRel })
      }
    }
    walk(root, '')
    return out
  }

  /** Read a scene file by its scenesDir-relative path. */
  readScene(relPath: string): string {
    const abs = path.join(this.scenesDir(), relPath)
    return this.readFileSandboxed(path.relative(this.root, abs))
  }

  /** Top-level entries of the scenesDir (the per-map `<Map>/` dirs). Mirrors the
   *  old listSceneNames — used by the scene-writer agent's list_scenes tool. */
  listSceneDirs(): string[] {
    const dir = this.scenesDir()
    if (!fs.existsSync(dir)) return []
    return fs.readdirSync(dir, { withFileTypes: true }).map(e => e.name)
  }

  /** The dataRoot-relative target file for a scene name, honoring the story
   *  activity's `scene.pathTemplate` / `scene.ext` (default `<Map>/script.scene`). */
  sceneTargetRel(sceneName: string): string {
    const sc = this.storyConfig() ?? {}
    const ext = this.sceneExt()
    const tmpl = sc.scene?.pathTemplate
    if (tmpl) return String(tmpl).replace(/\{scene\}/g, sceneName).replace(/\{ext\}/g, ext)
    return path.join(sc.scenesDir ?? SCENE_DEFAULT_DIR, sceneName, 'script' + ext)
  }

  /**
   * Normalize a scene identifier — the model passes it in several shapes — down
   * to the bare stem. Accepts a stem ("ChenManor"), the list_scenes `path`
   * ("ChenManor/script.scene"), a "<stem>/script" form, a "<stem>.scene" form,
   * or a dataRoot-relative path ("data/maps/ChenManor/script.scene").
   */
  private sceneStem(scene: string): string {
    const ext = this.sceneExt()
    let s = String(scene).trim().replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
    const scenesDirRel = String(this.storyConfig()?.scenesDir ?? SCENE_DEFAULT_DIR).replace(/^\/+|\/+$/g, '')
    if (scenesDirRel && s.startsWith(scenesDirRel + '/')) s = s.slice(scenesDirRel.length + 1)
    if (ext && s.endsWith(ext)) s = s.slice(0, -ext.length)
    s = s.replace(/\/script$/, '')
    return s
  }

  /**
   * The dataRoot-relative target file for a scene the model referenced. Unlike
   * the raw `sceneTargetRel`, this first RESOLVES the identifier against the
   * existing scenes (matching list_scenes by path / stem / handler name) so
   * "revise scene X" round-trips to the real file — an Edit, not a stray Create
   * at a mangled nested path (`.../ChenManor/script.scene/script.scene`) when the
   * model reuses the `read_scene` path instead of the bare stem. Falls back to
   * `sceneTargetRel(stem)` for a genuinely new scene. Every write path
   * (propose_scene_write, generate-scene, apply-scene, applyChange) routes
   * through this so the proposed path always equals the applied one.
   */
  resolveSceneRel(scene: string): string {
    const raw = String(scene).trim().replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
    const stem = this.sceneStem(raw)
    const hit = this.listScenes().find(s =>
      s.path === raw || s.stem === raw || s.stem === stem ||
      s.names.includes(raw) || s.names.includes(stem),
    )
    if (hit) return path.join(String(this.storyConfig()?.scenesDir ?? SCENE_DEFAULT_DIR), hit.path)
    return this.sceneTargetRel(stem)
  }

  // ── Event flags ───────────────────────────────────────────────────────────

  /**
   * Discover EVENT_ flags by scanning the scenesDir for getFlag/setFlag calls
   * (plus an optional data-table id column). Mirrors the server's scanFlags.
   */
  scanFlags(): string[] {
    const sc = this.storyConfig() ?? {}
    const flags = new Set<string>()
    const scan = sc.flagSource?.scan
    if (scan !== null) {
      const dirRel = scan?.dir ?? sc.scenesDir ?? SCENE_DEFAULT_DIR
      const fns: string[] = scan?.fns ?? ['getFlag', 'setFlag']
      const recursive = scan?.recursive !== false
      const scanRoot = this.resolveData(dirRel)
      const re = new RegExp(`(?:${fns.join('|')})\\s*\\(\\s*["']([A-Za-z_][A-Za-z0-9_]*)["']`, 'g')
      const walk = (d: string) => {
        if (!fs.existsSync(d)) return
        for (const entry of fs.readdirSync(d, { withFileTypes: true })) {
          const full = path.join(d, entry.name)
          if (entry.isDirectory()) { if (recursive) walk(full); continue }
          if (!/\.(js|ts|scene|json)$/.test(entry.name)) continue
          const text = fs.readFileSync(full, 'utf-8')
          let m: RegExpExecArray | null
          while ((m = re.exec(text)) !== null) flags.add(m[1])
        }
      }
      walk(scanRoot)
    }
    const tableId = sc.flagSource?.table
    if (tableId) {
      const table = this.dataTables().find(t => t.id === tableId)
      if (table) {
        const tdir = this.resolveData(table.dir)
        const idField = table.idField ?? 'id'
        if (fs.existsSync(tdir)) {
          for (const f of fs.readdirSync(tdir).filter(x => x.endsWith('.json'))) {
            try {
              const rec = JSON.parse(fs.readFileSync(path.join(tdir, f), 'utf-8'))
              if (rec[idField]) flags.add(String(rec[idField]))
            } catch { /* ignore */ }
          }
        }
      }
    }
    return [...flags].sort()
  }

  /**
   * Project-wide flag usage, split into reads (getFlag) and writes (setFlag), so a
   * lint can spot flags read-but-never-set (likely typos) or set-but-never-read.
   */
  scanFlagUsage(): { get: Set<string>; set: Set<string> } {
    const sc = this.storyConfig() ?? {}
    const get = new Set<string>()
    const set = new Set<string>()
    const scan = sc.flagSource?.scan
    if (scan === null) return { get, set }
    const dirRel = scan?.dir ?? sc.scenesDir ?? SCENE_DEFAULT_DIR
    const recursive = scan?.recursive !== false
    const scanRoot = this.resolveData(dirRel)
    const walk = (d: string) => {
      if (!fs.existsSync(d)) return
      for (const entry of fs.readdirSync(d, { withFileTypes: true })) {
        const full = path.join(d, entry.name)
        if (entry.isDirectory()) { if (recursive) walk(full); continue }
        if (!/\.(js|ts|scene|json)$/.test(entry.name)) continue
        const text = fs.readFileSync(full, 'utf-8')
        for (const [re, bag] of [[/getFlag\s*\(\s*["']([A-Za-z_][A-Za-z0-9_]*)["']/g, get], [/setFlag\s*\(\s*["']([A-Za-z_][A-Za-z0-9_]*)["']/g, set]] as const) {
          let m: RegExpExecArray | null
          while ((m = re.exec(text)) !== null) bag.add(m[1])
        }
      }
    }
    walk(scanRoot)
    return { get, set }
  }

  // ── Data tables ───────────────────────────────────────────────────────────

  listTables(): TableDef[] { return this.dataTables() }

  /** All records in a data table (each tagged with its `_file`). */
  listRecords(tableId: string): Array<Record<string, unknown>> {
    const table = this.dataTables().find(t => t.id === tableId)
    if (!table) return []
    const dir = this.resolveData(table.dir)
    if (!fs.existsSync(dir)) return []
    return fs.readdirSync(dir).filter(f => f.endsWith('.json')).map(f => {
      const raw = fs.readFileSync(path.join(dir, f), 'utf-8')
      try { return { _file: f, ...JSON.parse(raw) } } catch { return { _file: f, _error: 'parse error' } }
    })
  }

  /** Read one data record by table id + its idField value. */
  readRecord(tableId: string, id: string): Record<string, unknown> | null {
    const table = this.dataTables().find(t => t.id === tableId)
    if (!table) return null
    const idField = table.idField ?? 'id'
    return this.listRecords(tableId).find(r => String(r[idField]) === String(id)) ?? null
  }

  // ── GUI layouts (.gui) ────────────────────────────────────────────────────

  private guiRoot(): string | null {
    const gc = this.activity('ui')?.config as { guiRoot?: string } | undefined
    return gc?.guiRoot ? path.resolve(this.root, gc.guiRoot) : null
  }

  private guiExt(): string {
    return (this.activity('ui')?.config as { extension?: string } | undefined)?.extension ?? '.gui'
  }

  /** List `.gui` layout files (root-relative paths) under the ui activity's guiRoot. */
  listGui(): string[] {
    const root = this.guiRoot()
    const ext = this.guiExt()
    if (!root || !fs.existsSync(root)) return []
    const out: string[] = []
    const walk = (dir: string, rel: string) => {
      for (const name of fs.readdirSync(dir).sort()) {
        const full = path.join(dir, name)
        const childRel = rel ? `${rel}/${name}` : name
        if (fs.statSync(full).isDirectory()) { walk(full, childRel); continue }
        if (name.endsWith(ext)) out.push(childRel)
      }
    }
    walk(root, '')
    return out
  }

  /** Read a `.gui` file by its guiRoot-relative name. */
  readGui(name: string): string {
    const root = this.guiRoot()
    if (!root) throw new Error('No ui activity / guiRoot configured')
    const abs = path.resolve(root, name)
    if (abs !== root && !abs.startsWith(root + path.sep)) throw new Error('access denied')
    if (!fs.existsSync(abs)) throw new Error('not found: ' + name)
    return fs.readFileSync(abs, 'utf-8')
  }

  // ── Maps ──────────────────────────────────────────────────────────────────

  private mapsDir(): string | null {
    const mc = this.activity('map')?.config as { mapsDir?: string } | undefined
    return mc?.mapsDir ? this.resolveData(mc.mapsDir) : null
  }

  /** List map directory names under the map activity's mapsDir. */
  listMaps(): string[] {
    const dir = this.mapsDir()
    if (!dir || !fs.existsSync(dir)) return []
    return fs.readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name).sort()
  }

  /** Absolute path to a map's `objects.json` sidecar (NPCs / warps / collision).
   *  `path.basename` guards against traversal in the supplied map name. */
  mapObjectsPath(name: string): string {
    const dir = this.mapsDir()
    if (!dir) throw new Error('project has no map activity / mapsDir configured')
    return path.join(dir, path.basename(String(name)), 'objects.json')
  }

  /** Current `objects.json` text for a map, or null if the file does not exist. */
  readMapObjectsOrNull(name: string): string | null {
    const f = this.mapObjectsPath(name)
    return fs.existsSync(f) && fs.statSync(f).isFile() ? fs.readFileSync(f, 'utf-8') : null
  }

  // ── DSL guide / API typings (from the story `ai` config block) ────────────

  getDslGuide(): string | null { return this.readAiFile(this.storyConfig()?.ai?.dslGuide) }
  getApiTypes(): string | null { return this.readAiFile(this.storyConfig()?.ai?.apiTypes) }
  getExampleScenes(): string[] {
    const list = this.storyConfig()?.ai?.exampleScenes
    return Array.isArray(list) ? list.map((r: string) => this.readAiFile(r)).filter((x): x is string => !!x) : []
  }

  private readAiFile(rel: string | null | undefined): string | null {
    if (!rel) return null
    const f = path.resolve(this.root, rel)
    if (fs.existsSync(f) && fs.statSync(f).isFile()) return fs.readFileSync(f, 'utf-8')
    return null
  }

  // ── Context assembly (the empty-context fix) ──────────────────────────────

  /**
   * Build the project-context string injected into AI prompts. Replaces the old
   * assembleAiContext(sc.ai) — which returned '' whenever the project had no
   * `ai` block (the live wuxia case), leaving the model to invent APIs.
   *
   * Strategy:
   *   1. Include any explicitly-configured DSL guide / API typings / example scenes.
   *   2. If NO example scenes are explicitly configured, auto-sample real `.scene`
   *      files from the project so the model always sees the actual engine API and
   *      house conventions — config-driven, game-agnostic, works with `ai: null`.
   */
  assembleContext(opts: AssembleContextOptions = {}): string {
    const sc = this.storyConfig()
    const ai = sc?.ai
    const parts: string[] = []
    const seen = new Set<string>()
    const addFile = (label: string, rel: string | null | undefined, max = 6000) => {
      if (!rel) return
      const abs = path.resolve(this.root, rel)
      if (seen.has(abs)) return
      if (fs.existsSync(abs) && fs.statSync(abs).isFile()) {
        seen.add(abs)
        parts.push(`# ${label} (${rel})\n` + fs.readFileSync(abs, 'utf-8').slice(0, max))
      }
    }

    if (ai) {
      addFile('DSL guide', ai.dslGuide)
      addFile('Game API types', ai.apiTypes)
      for (const s of (ai.exampleScenes || [])) addFile('Example scene', s, 3000)
    }

    const haveExplicitExamples = Array.isArray(ai?.exampleScenes) && ai.exampleScenes.length > 0
    if ((opts.autoExampleScenes ?? true) && !haveExplicitExamples) {
      const limit = opts.exampleSceneLimit ?? 3
      const bytes = opts.exampleSceneBytes ?? 3000
      for (const rel of this.sampleExampleScenes(limit)) {
        addFile('Example scene (auto)', rel, bytes)
      }
    }

    return parts.join('\n\n')
  }

  /**
   * Pick up to `limit` real scene files as few-shot examples, as paths relative
   * to the project root. Prefers small-but-non-trivial files (cleaner exemplars).
   */
  private sampleExampleScenes(limit: number): string[] {
    const root = this.scenesDir()
    const ext = this.sceneExt()
    if (!fs.existsSync(root)) return []
    const found: Array<{ rel: string; size: number }> = []
    const walk = (dir: string) => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name)
        if (entry.isDirectory()) { walk(full); continue }
        if (!entry.name.endsWith(ext)) continue
        const size = fs.statSync(full).size
        if (size < 40) continue // skip empty/stub scenes
        found.push({ rel: path.relative(this.root, full), size })
      }
    }
    walk(root)
    return found.sort((a, b) => a.size - b.size).slice(0, limit).map(f => f.rel)
  }

  // ── Write-target path resolution ──────────────────────────────────────────
  // (Pure path computation — ProjectContext stays read-only; the actual writes
  //  live in server/actions/apply.ts, which calls these to find the target file.)

  /** Absolute file for a story record (resolves by matching id, like reads). */
  storyRecordPath(kind: string, id: string): string {
    return this.resolveStoryFile(path.join(this.storiesRoot(), kind), id)
  }

  /** Absolute file for a data record: the existing file matching `id`, else `<id>.json`. */
  dataRecordPath(tableId: string, id: string): string {
    const table = this.dataTables().find(t => t.id === tableId)
    if (!table) throw new Error('Table not found: ' + tableId)
    const dir = this.resolveData(table.dir)
    const idField = table.idField ?? 'id'
    if (fs.existsSync(dir)) {
      for (const f of fs.readdirSync(dir)) {
        if (!f.endsWith('.json')) continue
        try {
          if (String(JSON.parse(fs.readFileSync(path.join(dir, f), 'utf-8'))[idField]) === String(id)) return path.join(dir, f)
        } catch { /* skip */ }
      }
    }
    return path.join(dir, `${id}.json`)
  }

  /** Absolute path for a scene name. Resolves to the EXISTING scene file when
   *  the identifier matches one (so apply edits in place), else the default
   *  target (honors scene.pathTemplate / ext). Mirrors resolveSceneRel so the
   *  applied path equals the path the proposal diff was computed against. */
  sceneAbsPath(scene: string): string {
    return this.resolveData(this.resolveSceneRel(scene))
  }

  /** Absolute path for a `.gui` layout, refusing anything escaping guiRoot. */
  guiAbsPath(name: string): string {
    const root = this.guiRoot()
    if (!root) throw new Error('No ui activity / guiRoot configured')
    const abs = path.resolve(root, name)
    if (abs !== root && !abs.startsWith(root + path.sep)) throw new Error('access denied')
    return abs
  }

  // ── Structured retrieval ──────────────────────────────────────────────────

  /**
   * Gather everything needed to author/understand a quest: the quest record, the
   * resolved character profiles it involves (giver + characters), the project's
   * known flags, and any scenes already implementing it. Generalizes the
   * gathering inline in /api/ai/generate-scene.
   */
  gatherForQuest(questId: string): {
    quest: StoryRecord | null
    characters: StoryRecord[]
    flags: string[]
    scenes: SceneEntry[]
  } {
    const quest = this.readStoryRecord('quests', questId)
    if (!quest) return { quest: null, characters: [], flags: this.scanFlags(), scenes: [] }
    const ids = [...((quest.characters as string[]) || []), quest.giver as string]
      .filter((v, i, a) => v && a.indexOf(v) === i)
    const characters = ids.map(id => this.readStoryRecord('characters', id)).filter((c): c is StoryRecord => !!c)
    const refs = new Set<string>([
      ...((quest.implementedBy as Array<{ scene?: string }> | undefined)?.map(x => x?.scene).filter(Boolean) as string[] || []),
      ...((quest.maps as string[]) || []),
    ])
    const scenes = this.listScenes().filter(s => refs.has(s.stem) || s.names.some(n => refs.has(n)))
    return { quest, characters, flags: this.scanFlags(), scenes }
  }

  /**
   * Gather a character's neighbourhood: the record, its resolved relationship
   * targets, and the quests it gives or appears in.
   */
  gatherForCharacter(id: string): {
    character: StoryRecord | null
    related: Array<{ to: string; kind: string; record: StoryRecord | null }>
    questsInvolving: StoryRecord[]
  } {
    const character = this.readStoryRecord('characters', id)
    if (!character) return { character: null, related: [], questsInvolving: [] }
    const rels = (character.relationships as Array<{ to: string; kind: string }> | undefined) || []
    const related = rels.map(r => ({ to: r.to, kind: r.kind, record: this.readStoryRecord('characters', r.to) }))
    const questsInvolving = this.listQuests().filter(q =>
      q.giver === id || ((q.characters as string[]) || []).includes(id))
    return { character, related, questsInvolving }
  }

  /**
   * Resolve an @-mention token to a concrete target. Accepts `kind:id`
   * (e.g. `quest:rescue-the-elder`) or a bare id/stem searched across kinds.
   */
  resolveMention(token: string): MentionTarget | null {
    const raw = token.replace(/^@/, '').trim()
    const colon = raw.indexOf(':')
    const kindHint = colon > 0 ? raw.slice(0, colon) : null
    const key = colon > 0 ? raw.slice(colon + 1) : raw

    const tryKinds: Array<MentionTarget['kind']> = kindHint
      ? [normalizeKind(kindHint)]
      : ['character', 'quest', 'arc', 'scene', 'data', 'gui', 'map']

    for (const kind of tryKinds) {
      const hit = this.findMention(kind, key)
      if (hit) return hit
    }
    return null
  }

  private findMention(kind: MentionTarget['kind'], key: string): MentionTarget | null {
    switch (kind) {
      case 'character': case 'quest': case 'arc': {
        const storyKind = kind === 'character' ? 'characters' : kind === 'quest' ? 'quests' : 'arcs'
        const rec = this.readStoryRecord(storyKind, key)
        return rec ? { kind, id: String(rec.id ?? key), label: localLabel(rec, key) } : null
      }
      case 'scene': {
        const s = this.listScenes().find(x => x.stem === key || x.names.includes(key) || x.path === key)
        return s ? { kind, id: s.stem, label: s.stem } : null
      }
      case 'gui': {
        const g = this.listGui().find(x => x === key || x.replace(this.guiExt(), '') === key)
        return g ? { kind, id: g, label: g } : null
      }
      case 'map': {
        const m = this.listMaps().find(x => x === key)
        return m ? { kind, id: m, label: m } : null
      }
      case 'data': {
        for (const t of this.dataTables()) {
          const rec = this.readRecord(t.id, key)
          if (rec) return { kind, id: key, label: localLabel(rec, key), table: t.id }
        }
        return null
      }
    }
  }

  /**
   * Every mentionable target in the project, for @-autocomplete in the chat
   * surface. Cheap to compute; reads ids/names only.
   */
  mentionIndex(): MentionTarget[] {
    const out: MentionTarget[] = []
    for (const c of this.listCharacters()) out.push({ kind: 'character', id: String(c.id ?? ''), label: localLabel(c, String(c.id ?? '')) })
    for (const q of this.listQuests()) out.push({ kind: 'quest', id: String(q.id ?? ''), label: localLabel(q, String(q.id ?? '')) })
    for (const a of this.listArcs()) out.push({ kind: 'arc', id: String(a.id ?? ''), label: localLabel(a, String(a.id ?? '')) })
    for (const s of this.listScenes()) out.push({ kind: 'scene', id: s.stem, label: s.stem })
    for (const t of this.dataTables()) {
      const idField = t.idField ?? 'id'
      for (const r of this.listRecords(t.id)) {
        const id = String(r[idField] ?? '')
        if (id) out.push({ kind: 'data', id, label: localLabel(r, id), table: t.id })
      }
    }
    for (const g of this.listGui()) out.push({ kind: 'gui', id: g, label: g })
    for (const m of this.listMaps()) out.push({ kind: 'map', id: m, label: m })
    return out.filter(t => t.id)
  }
}

function normalizeKind(hint: string): MentionTarget['kind'] {
  const h = hint.toLowerCase()
  if (h.startsWith('char')) return 'character'
  if (h.startsWith('quest')) return 'quest'
  if (h.startsWith('arc')) return 'arc'
  if (h.startsWith('scene')) return 'scene'
  if (h.startsWith('gui') || h.startsWith('ui')) return 'gui'
  if (h.startsWith('map')) return 'map'
  return 'data'
}

/** Best human label for a record: localized name → plain name → id. */
function localLabel(rec: Record<string, unknown>, fallback: string): string {
  const name = rec.name
  if (typeof name === 'string' && name) return name
  if (name && typeof name === 'object') {
    const o = name as Record<string, unknown>
    const v = o.en ?? o.zh ?? Object.values(o)[0]
    if (typeof v === 'string' && v) return v
  }
  return fallback
}

// ── Shared default-project instance ──────────────────────────────────────────

let _current: ProjectContext | null = null

export function createProjectContext(root: string): ProjectContext {
  return new ProjectContext(root)
}

/**
 * Re-point the shared ProjectContext at a project root. Call this whenever the
 * dev server switches projects (POST /api/project/open) so getProjectContext()
 * stays in sync with the server's PROJECT_ROOT.
 */
export function setProjectRoot(root: string): ProjectContext {
  _current = new ProjectContext(root)
  return _current
}

/**
 * The shared ProjectContext for the dev server's current project. Defaults to
 * JRPG_PROJECT_ROOT / the workspace root, matching the default in
 * server/api/projectConfig.ts (pokered-editor: `import.meta.url` is this
 * module's real file URL — server/context — so the workspace root is four
 * levels up; import.meta.url rather than `__dirname` because the bundled
 * Electron api-server is pure ESM); updated by setProjectRoot on a project
 * switch.
 */
export function getProjectContext(): ProjectContext {
  if (!_current) _current = new ProjectContext(process.env.JRPG_PROJECT_ROOT || path.resolve(moduleDir, '../../../..'))
  return _current
}
