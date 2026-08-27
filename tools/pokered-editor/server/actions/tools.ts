// ───────────────────────────────────────────────────────────────────────────
// Agent tool surface, backed by ProjectContext + a ChangeSet.
//
//   READ tools     — execute immediately, return project data to the model.
//   PROPOSE tools  — do NOT mutate; stage a Proposal into the ChangeSet and emit
//                    a `proposal` event. This split is the trust model: the agent
//                    can inspect freely and suggest edits, but only the human
//                    applies them.
//
// The pure impls (readToolImpls / proposeToolImpls) are exported so they can be
// unit-tested without the AI SDK; buildReadTools / buildProposeTools wrap them in
// the SDK `tool()` shape for the chat agent (streamChat).
// ───────────────────────────────────────────────────────────────────────────
import fs from 'fs'
import path from 'path'
import type { ActionContext } from './types'
import { ChangeSet, type Proposal } from './changeSet'
import { appendMemory } from './memory'
import { lintScene } from '../sceneLint'
import { lintGui } from '../guiLint'
import { checkScene } from '../sceneCheck'
import { listSkills, readSkillByName } from './skills'
import { PROJECT_TEMPLATES, activitiesFor, slugify } from '../scaffold'

const CAP = 9000
const j = (v: unknown) => JSON.stringify(v, null, 2).slice(0, CAP)

/** Render lint findings as a compact, model-readable report. */
function fmtFindings(findings: Array<{ line: number; severity: string; message: string }>, okMsg: string): string {
  if (!findings.length) return okMsg
  return findings.map(f => `[${f.severity}] line ${f.line}: ${f.message}`).join('\n').slice(0, CAP)
}

// ── READ ────────────────────────────────────────────────────────────────────

export function readToolImpls(ctx: ActionContext) {
  const p = ctx.project
  return {
    list_characters: async () => j(p.listCharacters().map(c => ({ id: c.id, name: c.name }))),
    read_character: async ({ id }: { id: string }) => { const r = p.readStoryRecord('characters', id); return r ? j(r) : 'ERROR: not found' },
    list_quests: async () => j(p.listQuests().map(q => ({ id: q.id, name: q.name }))),
    read_quest: async ({ id }: { id: string }) => { const r = p.readStoryRecord('quests', id); return r ? j(r) : 'ERROR: not found' },
    list_scenes: async () => j(p.listScenes()),
    read_scene: async ({ path }: { path: string }) => { try { return p.readScene(path).slice(0, CAP) } catch (e) { return 'ERROR: ' + (e as Error).message } },
    scan_flags: async () => p.scanFlags().join('\n') || '(none)',
    list_tables: async () => j(p.listTables().map(t => ({ id: t.id }))),
    list_records: async ({ table }: { table: string }) => j(p.listRecords(table)),
    read_record: async ({ table, id }: { table: string; id: string }) => { const r = p.readRecord(table, id); return r ? j(r) : 'ERROR: not found' },
    list_gui: async () => p.listGui().join('\n') || '(none)',
    read_gui: async ({ name }: { name: string }) => { try { return p.readGui(name).slice(0, CAP) } catch (e) { return 'ERROR: ' + (e as Error).message } },
    list_maps: async () => p.listMaps().join('\n') || '(none)',
    read_file: async ({ path }: { path: string }) => { try { return p.readFileSandboxed(path).slice(0, CAP) } catch (e) { return 'ERROR: ' + (e as Error).message } },
    // ── Validation oracles: let the agent self-check a draft BEFORE proposing it,
    //    instead of proposing `.scene`/`.gui` blind. ──
    validate_scene: async ({ content }: { content: string }) => {
      if (typeof content !== 'string') return 'ERROR: content must be a string'
      return fmtFindings(lintScene(p, content), 'OK: no flag/API issues found (note: this is a lint, not a full DSL compile).')
    },
    check_scene: async ({ scene, content }: { scene?: string; content: string }) => {
      if (typeof content !== 'string' || !content.trim()) return 'ERROR: content must be a non-empty string'
      const r = await checkScene(p, scene ?? '', content)
      return `[${r.source}] ${r.ok ? 'PASS' : 'FAIL'}\n${r.output}`.slice(0, CAP)
    },
    compile_gui: async ({ content }: { content: string }) => {
      if (typeof content !== 'string') return 'ERROR: content must be a string'
      return fmtFindings(lintGui(content), 'OK: structure looks valid (note: structural pre-check only; the editor runs the full WASM compile on apply).')
    },
    // ── Skills: loadable task playbooks (see server/actions/skills.ts). ──
    list_skills: async () =>
      j(listSkills(ctx.project).map(s => ({ name: s.name, description: s.description, source: s.source }))),
    read_skill: async ({ name }: { name: string }) => {
      const doc = readSkillByName(String(name ?? ''), ctx.project)
      return doc ? `# Skill: ${doc.name}\n\n${doc.body}`.slice(0, CAP * 3) : 'ERROR: unknown skill — call list_skills for the available names'
    },
  }
}

// ── PROPOSE ───────────────────────────────────────────────────────────────────

/** Map a story kind to its on-disk dir name, tolerating the singular form the
 *  model often passes (mentions are singular: "character" → dir "characters"). */
function storyKindDir(kind: string): string {
  const k = String(kind).toLowerCase()
  if (k === 'character' || k === 'characters') return 'characters'
  if (k === 'quest' || k === 'quests') return 'quests'
  if (k === 'arc' || k === 'arcs') return 'arcs'
  return k // pass through unknown / custom story kinds
}

/** Coerce a tool's `content` arg to a pretty JSON string, validating it parses. */
function normalizeJson(content: unknown): { text: string; error?: string } {
  if (typeof content === 'string') {
    try { JSON.parse(content); return { text: content } } catch (e) { return { text: '', error: 'content is not valid JSON: ' + (e as Error).message } }
  }
  if (content && typeof content === 'object') return { text: JSON.stringify(content, null, 2) }
  return { text: '', error: 'content must be a JSON object or a JSON string' }
}

export function proposeToolImpls(ctx: ActionContext, cs: ChangeSet) {
  const p = ctx.project
  // `after` rides along so the client review tray can apply the proposal (the
  // server-side ChangeSet is per-run and gone once the stream ends).
  const emit = (pr: Proposal) =>
    ctx.emit('proposal', { id: pr.id, target: pr.target, title: pr.title, rationale: pr.rationale, diff: pr.diff, before: pr.before, after: pr.after })

  return {
    propose_story_edit: async ({ kind, id, content, rationale }: { kind: string; id: string; content: unknown; rationale?: string }) => {
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const storyKind = storyKindDir(kind)
      const cur = p.readStoryRecord(storyKind, id)
      const before = cur ? JSON.stringify(cur, null, 2) : null
      const pr = cs.add({ target: { kind: 'story', storyKind, id, path: `${storyKind}/${id}` }, title: `${before ? 'Edit' : 'Create'} ${storyKind} "${id}"`, rationale, before, after: norm.text })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    propose_data_edit: async ({ table, id, content, rationale }: { table: string; id: string; content: unknown; rationale?: string }) => {
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const cur = p.readRecord(table, id)
      const before = cur ? JSON.stringify(cur, null, 2) : null
      const pr = cs.add({ target: { kind: 'data', table, id, path: `${table}/${id}` }, title: `${before ? 'Edit' : 'Create'} ${table} "${id}"`, rationale, before, after: norm.text })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    propose_scene_write: async ({ scene, content, rationale }: { scene: string; content: string; rationale?: string }) => {
      if (typeof content !== 'string' || !content.trim()) return 'ERROR: content must be a non-empty string'
      // resolveSceneRel (not sceneTargetRel): tolerate the stem, the list_scenes
      // `path`, "<stem>/script", "<stem>.scene", etc. — resolve an existing scene
      // to its real file so revising it is an Edit, not a stray Create.
      const targetRel = p.resolveSceneRel(scene)
      const before = p.readDataFileOrNull(targetRel)
      const pr = cs.add({ target: { kind: 'scene', scene, path: targetRel }, title: `${before ? 'Edit' : 'Create'} scene "${scene}"`, rationale, before, after: content })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    propose_gui_write: async ({ name, content, rationale }: { name: string; content: string; rationale?: string }) => {
      if (typeof content !== 'string' || !content.trim()) return 'ERROR: content must be a non-empty string'
      let before: string | null = null
      try { before = p.readGui(name) } catch { /* new file */ }
      const pr = cs.add({ target: { kind: 'gui', name, path: name }, title: `${before ? 'Edit' : 'Create'} gui "${name}"`, rationale, before, after: content })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    propose_map_edit: async ({ map, content, rationale }: { map: string; content: unknown; rationale?: string }) => {
      if (!map || typeof map !== 'string') return 'ERROR: map (the map directory name) is required'
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      let before: string | null = null
      try { before = p.readMapObjectsOrNull(map) } catch (e) { return 'ERROR: ' + (e as Error).message }
      const pr = cs.add({ target: { kind: 'map', map, path: `maps/${map}/objects.json` }, title: `${before ? 'Edit' : 'Create'} objects.json for map "${map}"`, rationale, before, after: norm.text })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    propose_map_file: async ({ map, file, content, rationale }: { map: string; file: string; content: unknown; rationale?: string }) => {
      if (!map || typeof map !== 'string') return 'ERROR: map (the map directory name, e.g. "PalletTown") is required'
      if (file !== 'map.json' && file !== 'script_config.json') return 'ERROR: file must be "map.json" or "script_config.json"'
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const mapsDir = ((p.activity('map')?.config ?? {}) as { mapsDir?: string }).mapsDir ?? 'maps'
      const before = p.readDataFileOrNull(path.join(mapsDir, map, file))
      if (file === 'map.json' && before === null) return `ERROR: map "${map}" has no map.json — create the map first with propose_map_create`
      const pr = cs.add({ target: { kind: 'map-file', map, file, path: `${mapsDir}/${map}/${file}` }, title: `${before ? 'Edit' : 'Create'} ${file} for map "${map}"`, rationale, before, after: norm.text })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    // ── Project-level proposals (project creation / config / new maps) ──
    draft_project_scaffold: async ({ name, dir, templateId, summary }: { name: string; dir?: string; templateId: string; summary?: string }) => {
      if (!name || !String(name).trim()) return 'ERROR: name is required'
      const tpl = PROJECT_TEMPLATES.find(t => t.id === templateId)
      if (!tpl) return `ERROR: unknown templateId "${templateId}" — use one of: ${PROJECT_TEMPLATES.map(t => t.id).join(', ')}`
      const dirName = dir && String(dir).trim() ? String(dir).trim() : slugify(String(name).trim())
      if (!path.isAbsolute(dirName) && !/^[a-z0-9][a-z0-9-]*$/.test(dirName)) {
        return `ERROR: invalid dir "${dirName}" — use a lowercase slug (letters, digits, dashes) or an absolute path`
      }
      // No disk writes here: the structured payload rides in `after`; applying
      // the proposal runs the real scaffold. `activities` is review-tray metadata.
      const payload = {
        name: String(name).trim(), dir: dirName, templateId,
        dataRoot: './data', gfxRoot: './gfx',
        activities: activitiesFor(tpl).map(a => ({ id: a.id, label: a.label })),
      }
      const after = JSON.stringify(payload, null, 2)
      const pr = cs.add({ target: { kind: 'project-scaffold', dir: dirName, name: payload.name, path: dirName }, title: `Create project "${payload.name}"`, rationale: summary, before: null, after })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    propose_project_config: async ({ config, rationale }: { config: unknown; rationale?: string }) => {
      if (!p) return 'ERROR: no project is open'
      const norm = normalizeJson(config)
      if (norm.error) return 'ERROR: ' + norm.error
      const parsed = JSON.parse(norm.text)
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed) || !Array.isArray(parsed.activities)) {
        return 'ERROR: config must be a JSON object with an `activities` array (the COMPLETE new .dotzuki-editor.json)'
      }
      const file = p.configFile()
      const before = fs.existsSync(file) ? fs.readFileSync(file, 'utf-8') : null
      const after = JSON.stringify(parsed, null, 2)
      const pr = cs.add({ target: { kind: 'project-config', path: '.dotzuki-editor.json' }, title: `${before ? 'Edit' : 'Create'} project config (.dotzuki-editor.json)`, rationale, before, after })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
    propose_map_create: async (args: { name: string; tileset?: string; width?: number; height?: number; music?: string; borderBlock?: number; displayName?: string; townMap?: { x: number; y: number }; rationale?: string }) => {
      if (!p) return 'ERROR: no project is open'
      const { name, rationale, ...rest } = args
      if (!name || !/^[A-Za-z0-9_-]+$/.test(String(name))) return 'ERROR: a valid map name (A–Z, 0–9, _-) is required'
      const mapsDir = ((p.activity('map')?.config ?? {}) as { mapsDir?: string }).mapsDir ?? 'maps'
      if (p.readDataFileOrNull(path.join(mapsDir, name, 'map.json'))) {
        return `ERROR: map "${name}" already exists — use propose_map_file to edit its map.json / script_config.json`
      }
      // The creation params ride in `after`; applyChange hands them to createMap.
      const payload: Record<string, unknown> = { name }
      for (const k of ['tileset', 'width', 'height', 'music', 'borderBlock', 'displayName', 'townMap'] as const) {
        if (rest[k] !== undefined) payload[k] = rest[k]
      }
      const after = JSON.stringify(payload, null, 2)
      const pr = cs.add({ target: { kind: 'map-create', map: name, path: `${mapsDir}/${name}/` }, title: `Create map "${name}"`, rationale, before: null, after })
      emit(pr); return { ok: true, proposalId: pr.id }
    },
  }
}

// ── PLAN (UI-only: publishes the agent's working checklist, mutates nothing) ──

export async function buildPlanTools(ctx: ActionContext): Promise<Record<string, any>> {
  const { tool } = await import('ai')
  const { z } = await import('zod')
  return {
    update_plan: tool({
      description: 'Publish or update your working plan for a MULTI-STEP task so the user can watch progress. Call it once when you begin a task with several steps, and AGAIN each time a step starts or finishes. Keep 2–6 concise steps; mark exactly the one in progress as "active", finished ones "done". Mutates nothing.',
      inputSchema: z.object({
        steps: z.array(z.object({
          title: z.string(),
          status: z.enum(['pending', 'active', 'done']).optional(),
        })).min(1).max(12),
      }),
      execute: async ({ steps }: { steps: Array<{ title: string; status?: string }> }) => {
        const norm = steps.map(s => ({ title: String(s.title), status: s.status ?? 'pending' }))
        ctx.emit('plan', { steps: norm })
        return { ok: true, steps: norm.length }
      },
    }),
  }
}

// ── MEMORY (auto-executing, like the READ tools) ──────────────────────────────
// The assistant's own memory is NOT project content, so it never goes through
// the proposal tray: remember_fact writes immediately. Registered in BOTH tool
// sets (in-project and creation mode).

export function memoryToolImpl(ctx: ActionContext, homeDir?: string) {
  return {
    remember_fact: async ({ fact, scope }: { fact: string; scope?: 'project' | 'global' }) => {
      try {
        const r = appendMemory(ctx.project ?? null, scope ?? 'project', fact, homeDir)
        return `OK: saved to ${r.scope} memory (${r.file})`
      } catch (e) { return 'ERROR: ' + (e as Error).message }
    },
  }
}

export async function buildMemoryTools(ctx: ActionContext): Promise<Record<string, any>> {
  const { tool } = await import('ai')
  const { z } = await import('zod')
  const impl = memoryToolImpl(ctx)
  return {
    remember_fact: tool({
      description: 'Save one durable fact to your assistant memory (read back into your system prompt on later turns). Call this when the user reveals a lasting preference — genre/setting tastes, naming style, workflow habits — or explicitly asks you to remember something. `scope`: "project" (default; this game only) or "global" (all projects; forced when no project is open). Executes immediately — this is your own memory, NOT a project edit, so it is never staged for review.',
      inputSchema: z.object({ fact: z.string().max(500), scope: z.enum(['project', 'global']).optional() }),
      execute: impl.remember_fact,
    }),
  }
}

// ── SDK tool() wrappers (for the chat agent) ──────────────────────────────────

export async function buildReadTools(ctx: ActionContext): Promise<Record<string, any>> {
  const { tool } = await import('ai')
  const { z } = await import('zod')
  const impl = readToolImpls(ctx)
  const tools: Record<string, any> = {
    list_characters: tool({ description: 'List all characters (id + name).', inputSchema: z.object({}), execute: impl.list_characters }),
    read_character: tool({ description: 'Read a full character record by id.', inputSchema: z.object({ id: z.string() }), execute: impl.read_character }),
    list_quests: tool({ description: 'List all quests (id + name).', inputSchema: z.object({}), execute: impl.list_quests }),
    read_quest: tool({ description: 'Read a full quest record by id.', inputSchema: z.object({ id: z.string() }), execute: impl.read_quest }),
    list_scenes: tool({ description: 'List `.scene` files: { stem, names, path }.', inputSchema: z.object({}), execute: impl.list_scenes }),
    read_scene: tool({ description: 'Read a scene file by its scenesDir-relative path.', inputSchema: z.object({ path: z.string() }), execute: impl.read_scene }),
    scan_flags: tool({ description: 'List the EVENT_ flags known in this game.', inputSchema: z.object({}), execute: impl.scan_flags }),
    list_tables: tool({ description: 'List data table ids.', inputSchema: z.object({}), execute: impl.list_tables }),
    list_records: tool({ description: 'List all records in a data table.', inputSchema: z.object({ table: z.string() }), execute: impl.list_records }),
    read_record: tool({ description: 'Read a single data record by table + id.', inputSchema: z.object({ table: z.string(), id: z.string() }), execute: impl.read_record }),
    list_gui: tool({ description: 'List `.gui` layout files.', inputSchema: z.object({}), execute: impl.list_gui }),
    read_gui: tool({ description: 'Read a `.gui` layout by name.', inputSchema: z.object({ name: z.string() }), execute: impl.read_gui }),
    list_maps: tool({ description: 'List map directory names.', inputSchema: z.object({}), execute: impl.list_maps }),
    read_file: tool({ description: 'Read a UTF-8 project file (path relative to project root).', inputSchema: z.object({ path: z.string() }), execute: impl.read_file }),
    validate_scene: tool({ description: 'Lint a draft `.scene` buffer: dangling/orphan EVENT_ flags and hallucinated game.* calls. A quick pre-check; for the full gate use check_scene.', inputSchema: z.object({ content: z.string() }), execute: impl.validate_scene }),
    check_scene: tool({ description: 'Compile-check a DRAFT `.scene` buffer WITHOUT saving it: runs the project scene compiler when configured (real errors), else the deterministic lint. The response starts with `[compile] PASS/FAIL` or `[lint] PASS/FAIL`. Run this on your draft and FIX every FAIL/error, iterating until it PASSES, BEFORE calling propose_scene_write. `scene` = the target stem (used only for messages).', inputSchema: z.object({ scene: z.string().optional(), content: z.string() }), execute: impl.check_scene }),
    compile_gui: tool({ description: 'Structural pre-check of a draft `.gui` buffer: unbalanced { } [ ] ( ) and missing top-level block. Run this on your draft BEFORE propose_gui_write to catch truncated/broken output.', inputSchema: z.object({ content: z.string() }), execute: impl.compile_gui }),
    list_skills: tool({ description: 'List the loadable skill playbooks (name + description) available for this project — e.g. authoring maps, trainers, Pokémon, saves.', inputSchema: z.object({}), execute: impl.list_skills }),
    read_skill: tool({ description: 'Load the full playbook of one skill by name (from list_skills or the system-prompt skill index). Call this BEFORE starting a task that matches a skill, and follow its workflow.', inputSchema: z.object({ name: z.string() }), execute: impl.read_skill }),
  }
  // Story tools only exist when the project declares a story activity —
  // otherwise their impls throw "No story activity / storiesDir configured",
  // and the model may reach for them when a DATA table is what it wants.
  if (!ctx.project?.storyConfig()?.storiesDir) {
    delete tools.list_characters
    delete tools.read_character
    delete tools.list_quests
    delete tools.read_quest
  }
  return tools
}

export async function buildProposeTools(ctx: ActionContext, cs: ChangeSet): Promise<Record<string, any>> {
  const { tool } = await import('ai')
  const { z } = await import('zod')
  const impl = proposeToolImpls(ctx, cs)
  const note = ' Does NOT apply — it stages the edit for human review. Never claim you applied it.'
  const tools: Record<string, any> = {
    propose_story_edit: tool({
      description: 'Propose creating/replacing a story record. `kind` is one of "characters" | "quests" | "arcs"; `content` = the COMPLETE new record as a JSON string.' + note,
      inputSchema: z.object({ kind: z.string(), id: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: impl.propose_story_edit,
    }),
    propose_data_edit: tool({
      description: 'Propose creating/replacing a data-table record. `content` = the COMPLETE new record as a JSON string.' + note,
      inputSchema: z.object({ table: z.string(), id: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: impl.propose_data_edit,
    }),
    propose_scene_write: tool({
      description: 'Propose writing a `.scene` DSL file for a scene/map. `scene` = the scene STEM from list_scenes (e.g. "ChenManor"), NOT its `path`/filename. To REVISE an existing scene, pass its existing stem and the file is replaced IN PLACE (an Edit); a new stem creates a new scene. `content` = the COMPLETE file text.' + note,
      inputSchema: z.object({
        scene: z.string().describe('Scene stem from list_scenes (e.g. "ChenManor") — not the path or filename. Pass an existing stem to edit that scene in place; a new stem creates a new scene.'),
        content: z.string(),
        rationale: z.string().optional(),
      }),
      execute: impl.propose_scene_write,
    }),
    propose_gui_write: tool({
      description: 'Propose writing a `.gui` layout file. `content` = the COMPLETE file text.' + note,
      inputSchema: z.object({ name: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: impl.propose_gui_write,
    }),
    propose_map_edit: tool({
      description: 'Propose editing a map\'s `objects.json` (NPC placements, warps, collision) — for generic dotzuki projects. NOTE: pokered maps keep NPCs/warps/signs/wild encounters in `map.json` instead — use propose_map_file for those.' + note,
      inputSchema: z.object({ map: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: impl.propose_map_edit,
    }),
    propose_map_file: tool({
      description: 'Propose writing an allowlisted file inside a map directory: `file` = "map.json" (header/connections/warps/npcs/signs/text/wild — the COMPLETE new JSON) or "script_config.json" (npc/sign talk-handler bindings + coordEvents). `map` = the map directory name from list_maps. The map must already exist (use propose_map_create first for a new one).' + note,
      inputSchema: z.object({
        map: z.string().describe('Map directory name, e.g. "PalletTown"'),
        file: z.enum(['map.json', 'script_config.json']),
        content: z.string().describe('The COMPLETE new file content as a JSON string'),
        rationale: z.string().optional(),
      }),
      execute: impl.propose_map_file,
    }),
    draft_project_scaffold: scaffoldTool(tool, z, impl, note),
    propose_project_config: tool({
      description: 'Propose replacing the project config `.dotzuki-editor.json`. `config` = the COMPLETE new config as a JSON string: { name: string, dataRoot: string (e.g. "./data"), gfxRoot?: string (e.g. "./gfx"), activities: [{ id, type, label?, icon?, enabled?, config }] }. Preserve unrelated fields from the current config (read it first with read_file).' + note,
      inputSchema: z.object({ config: z.string(), rationale: z.string().optional() }),
      execute: impl.propose_project_config,
    }),
    propose_map_create: tool({
      description: 'Propose creating a NEW map directory. In a pokered project this scaffolds the full game shape (map.json with the next free id + header, map.blk, script_config.json, empty script.scene); pass `tileset`, `width`, `height` (in 4x4-tile blocks), `music`, `borderBlock` and optionally `townMap` {x,y} + `displayName` to control it. For an existing map use propose_map_file instead.' + note,
      inputSchema: z.object({
        name: z.string().describe('New map directory name (A–Z, 0–9, _-; PascalCase by convention, e.g. "CinnabarLab")'),
        tileset: z.string().optional(),
        width: z.number().int().min(1).max(255).optional(),
        height: z.number().int().min(1).max(255).optional(),
        music: z.string().optional(),
        borderBlock: z.number().int().min(0).max(255).optional(),
        displayName: z.string().optional(),
        townMap: z.object({ x: z.number().int(), y: z.number().int() }).optional(),
        rationale: z.string().optional(),
      }),
      execute: impl.propose_map_create,
    }),
  }
  // Same story-activity gate as buildReadTools: without a story activity there
  // is nowhere for a story record to land, so hide the tool entirely.
  if (!ctx.project?.storyConfig()?.storiesDir) delete tools.propose_story_edit
  return tools
}

/** Shared draft_project_scaffold tool definition (in-project + creation mode). */
function scaffoldTool(tool: any, z: any, impl: ReturnType<typeof proposeToolImpls>, note: string) {
  return tool({
    description: 'Draft scaffolding a NEW dotzuki-editor project (used in creation mode when no project is open, or to spin up a sibling project). `name` = display name, `dir` = folder slug (lowercase letters/digits/dashes; derived from the name when omitted) or absolute path, `templateId` = one of "empty" | "wuxia" | "jrpg", `summary` = a short rationale for the review card. The result is pure editor content: .dotzuki-editor.json + data/ (maps, tables, tiles) + gfx/ + assets/scenes/main.scene — no Rust workspace, no build step.' + note,
    inputSchema: z.object({
      name: z.string(),
      dir: z.string().optional(),
      templateId: z.enum(['empty', 'wuxia', 'jrpg']),
      summary: z.string().optional(),
    }),
    execute: impl.draft_project_scaffold,
  })
}

/**
 * Creation-mode tool surface (no project open): the agent can only draft a
 * project scaffold and publish a plan — every read/propose tool that needs a
 * ProjectContext is excluded. The caller adds buildPlanTools.
 */
export async function buildScaffoldTools(ctx: ActionContext, cs: ChangeSet): Promise<Record<string, any>> {
  const { tool } = await import('ai')
  const { z } = await import('zod')
  const impl = proposeToolImpls(ctx, cs)
  const note = ' Does NOT apply — it stages the edit for human review. Never claim you applied it.'
  return { draft_project_scaffold: scaffoldTool(tool, z, impl, note) }
}
