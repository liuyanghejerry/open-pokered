// Tool surface tests — exercise the pure impls directly (no AI SDK) against a
// hermetic temp fixture, proving READ tools return project data and PROPOSE tools
// stage diffs + emit proposals WITHOUT ever writing to disk.
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { createProjectContext } from '../context/projectContext'
import { ChangeSet } from './changeSet'
import { proposeToolImpls, readToolImpls } from './tools'
import type { ActionContext } from './types'

let ROOT = ''
function write(rel: string, content: string) {
  const abs = path.join(ROOT, rel)
  fs.mkdirSync(path.dirname(abs), { recursive: true })
  fs.writeFileSync(abs, content, 'utf-8')
}

beforeAll(() => {
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-tools-'))
  write('.dotzuki-editor.json', JSON.stringify({
    name: 'F', dataRoot: '.', activities: [
      { id: 'story', type: 'story', config: { storiesDir: 'data/story', scenesDir: 'data/maps', scene: { ext: '.scene' } } },
      { id: 'data', type: 'data', config: { tables: [{ id: 'skills', dir: 'data/skills', idField: 'id' }] } },
      { id: 'map', type: 'map', config: { mapsDir: 'data/maps' } },
    ],
  }))
  write('data/story/characters/hero.json', JSON.stringify({ id: 'hero', name: 'Hero', motivation: 'unknown' }))
  write('data/skills/fireball.json', JSON.stringify({ id: 'fireball', power: 10 }))
  write('data/maps/Town/script.scene', '@storyline("town")\nsetFlag("EVENT_TOWN_DONE")\ngetFlag("EVENT_TOWN_DONE")\n')
  write('data/maps/Town/objects.json', JSON.stringify({ npcs: [], warps: [] }))
})
afterAll(() => { try { fs.rmSync(ROOT, { recursive: true, force: true }) } catch { /* ignore */ } })

function makeCtx() {
  const events: Array<{ type: string; payload: any }> = []
  const ctx = {
    actionId: 'assistant', input: {}, profile: {} as any, apiKey: 'k',
    project: createProjectContext(ROOT), emit: (t: any, p: any) => events.push({ type: t, payload: p }),
  } as ActionContext
  return { ctx, events }
}

describe('readToolImpls', () => {
  it('reads characters / records / scenes and errors on misses', async () => {
    const r = readToolImpls(makeCtx().ctx)
    expect(await r.read_character({ id: 'hero' })).toContain('"motivation"')
    expect(await r.read_record({ table: 'skills', id: 'fireball' })).toContain('"power"')
    expect(JSON.parse(await r.list_characters())[0].id).toBe('hero')
    expect(await r.read_character({ id: 'nobody' })).toMatch(/ERROR/)
  })

  it('validate_scene flags a dangling getFlag and passes a clean buffer', async () => {
    const r = readToolImpls(makeCtx().ctx)
    // A flag read but never set anywhere in the project → [warn].
    expect(await r.validate_scene({ content: 'getFlag("EVENT_NEVER")\n' })).toMatch(/EVENT_NEVER.*never set|\[warn\]/)
    // A flag both set and read (Town scene sets+reads EVENT_TOWN_DONE) → OK.
    expect(await r.validate_scene({ content: 'getFlag("EVENT_TOWN_DONE")\n' })).toMatch(/OK/)
  })

  it('compile_gui catches unbalanced braces / empty and passes a valid layout', async () => {
    const r = readToolImpls(makeCtx().ctx)
    expect(await r.compile_gui({ content: '' })).toMatch(/empty/i)
    expect(await r.compile_gui({ content: 'screen X {\n  text("hi") { rect = {tx:1} }\n' })).toMatch(/[Uu]nbalanced/)
    expect(await r.compile_gui({ content: 'screen X {\n  text("hi") {\n    rect = {tx: 1, ty: 1}\n  }\n}\n' })).toMatch(/OK/)
  })
})

describe('proposeToolImpls', () => {
  it('stages a story edit (before/after + proposal event) without writing disk', async () => {
    const { ctx, events } = makeCtx()
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).propose_story_edit({
      kind: 'characters', id: 'hero',
      content: JSON.stringify({ id: 'hero', name: 'Hero', motivation: 'avenge his master' }),
      rationale: 'sharper',
    })
    expect(res).toMatchObject({ ok: true })
    expect(cs.proposals.length).toBe(1)
    const prop = cs.proposals[0]
    expect(prop.before).toContain('unknown')           // current on-disk value
    expect(prop.after).toContain('avenge his master')  // proposed value
    expect(prop.target).toMatchObject({ kind: 'story', storyKind: 'characters', id: 'hero' })
    expect(events.some(e => e.type === 'proposal')).toBe(true)
    // disk is untouched — propose ≠ apply
    expect(JSON.parse(fs.readFileSync(path.join(ROOT, 'data/story/characters/hero.json'), 'utf-8')).motivation).toBe('unknown')
  })

  it('normalizes a singular story kind ("character") to the dir ("characters")', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    // the model often passes the singular mention kind; it must still resolve the
    // existing record (so this is an Edit, not a Create) and target the right dir.
    await proposeToolImpls(ctx, cs).propose_story_edit({
      kind: 'character', id: 'hero', content: JSON.stringify({ id: 'hero', name: 'Hero', motivation: 'x' }),
    })
    expect(cs.proposals[0].before).toContain('unknown')               // found the existing record
    expect(cs.proposals[0].target.storyKind).toBe('characters')        // normalized dir
    expect(cs.proposals[0].title.startsWith('Edit')).toBe(true)
  })

  it('rejects invalid JSON content without staging anything', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).propose_data_edit({ table: 'skills', id: 'fireball', content: '{bad json' })
    expect(String(res)).toMatch(/ERROR/)
    expect(cs.proposals.length).toBe(0)
  })

  it('proposes a brand-new scene file with before=null', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    await proposeToolImpls(ctx, cs).propose_scene_write({ scene: 'NewMap', content: '@storyline("new")\n' })
    expect(cs.proposals[0].before).toBeNull()
    expect(cs.proposals[0].target).toMatchObject({ kind: 'scene', scene: 'NewMap' })
    // …and it targets the clean default path, not a nested one.
    expect(cs.proposals[0].target.path).toBe('data/maps/NewMap/script.scene')
  })

  it('resolves an existing scene to an EDIT in place however the model spells the id', async () => {
    // The model may reuse the list_scenes `path` / a "<stem>/script" / "<stem>.scene"
    // / a dataRoot-relative form instead of the bare stem. All must resolve to the
    // real Town/script.scene as an Edit — never a stray Create at a mangled path.
    for (const scene of ['Town', 'Town/script.scene', 'Town/script', 'Town.scene', 'data/maps/Town/script.scene', 'town']) {
      const { ctx } = makeCtx()
      const cs = new ChangeSet()
      await proposeToolImpls(ctx, cs).propose_scene_write({ scene, content: '@storyline("town")\n// revised\n' })
      const prop = cs.proposals[0]
      expect(prop.before, `scene=${scene}`).toContain('@storyline("town")') // found the existing file
      expect(prop.title.startsWith('Edit'), `scene=${scene}`).toBe(true)
      expect(prop.target.path, `scene=${scene}`).toBe('data/maps/Town/script.scene') // real file, not nested
    }
  })

  it('stages a map objects.json edit (before = current, target kind "map")', async () => {
    const { ctx, events } = makeCtx()
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).propose_map_edit({
      map: 'Town',
      content: JSON.stringify({ npcs: [{ id: 1, x: 5, y: 5, sprite: 'hero' }], warps: [] }),
      rationale: 'place the story anchor NPC',
    })
    expect(res).toMatchObject({ ok: true })
    const prop = cs.proposals[0]
    expect(prop.before).toContain('"npcs"')                  // current objects.json
    expect(prop.after).toMatch(/"sprite"\s*:\s*"hero"/)      // proposed placement
    expect(prop.target).toMatchObject({ kind: 'map', map: 'Town', path: 'maps/Town/objects.json' })
    expect(events.some(e => e.type === 'proposal')).toBe(true)
    // disk untouched — propose ≠ apply
    expect(JSON.parse(fs.readFileSync(path.join(ROOT, 'data/maps/Town/objects.json'), 'utf-8')).npcs).toEqual([])
  })

  it('rejects a map edit with invalid JSON without staging anything', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).propose_map_edit({ map: 'Town', content: '{bad' })
    expect(String(res)).toMatch(/ERROR/)
    expect(cs.proposals.length).toBe(0)
  })
})

describe('project-level propose tools', () => {
  it('draft_project_scaffold stages a structured proposal WITHOUT touching disk', async () => {
    const { ctx, events } = makeCtx()
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).draft_project_scaffold({
      name: 'My Game', dir: 'my-game', templateId: 'jrpg', summary: 'classic JRPG start',
    })
    expect(res).toMatchObject({ ok: true })
    expect(cs.proposals.length).toBe(1)
    const prop = cs.proposals[0]
    expect(prop.target).toMatchObject({ kind: 'project-scaffold', dir: 'my-game', name: 'My Game' })
    expect(prop.title).toBe('Create project "My Game"')
    expect(prop.before).toBeNull()
    const payload = JSON.parse(prop.after)
    expect(payload).toMatchObject({ name: 'My Game', dir: 'my-game', templateId: 'jrpg', dataRoot: './data', gfxRoot: './gfx' })
    expect(payload.activities.map((a: any) => a.id)).toEqual(['maps', 'scripts', 'data', 'assets', 'tiles'])
    expect(events.some(e => e.type === 'proposal')).toBe(true)
    // propose ≠ apply: nothing was scaffolded on disk
    expect(fs.existsSync(path.join(ROOT, 'my-game'))).toBe(false)
  })

  it('draft_project_scaffold works with NO project (creation mode) and derives the slug', async () => {
    const events: Array<{ type: string; payload: any }> = []
    const ctx = {
      actionId: 'assistant', input: {}, profile: {} as any, apiKey: 'k',
      project: null, emit: (t: any, p: any) => events.push({ type: t, payload: p }),
    } as unknown as ActionContext
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).draft_project_scaffold({ name: 'Wuxia World', templateId: 'wuxia' })
    expect(res).toMatchObject({ ok: true })
    const payload = JSON.parse(cs.proposals[0].after)
    expect(payload.dir).toBe('wuxia-world')
    expect(payload.templateId).toBe('wuxia')
  })

  it('draft_project_scaffold rejects an unknown templateId without staging', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).draft_project_scaffold({ name: 'X', templateId: 'cyberpunk' })
    expect(String(res)).toMatch(/ERROR.*unknown templateId/)
    expect(cs.proposals.length).toBe(0)
  })

  it('propose_map_create stages a create (before=null) and refuses an existing map', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    await proposeToolImpls(ctx, cs).propose_map_create({ name: 'NewMap' })
    expect(cs.proposals[0].target).toMatchObject({ kind: 'map-create', map: 'NewMap' })
    expect(cs.proposals[0].before).toBeNull()

    write('data/maps/Existing/map.json', '{"name":"Existing"}')
    const dup = await proposeToolImpls(ctx, cs).propose_map_create({ name: 'Existing' })
    expect(String(dup)).toMatch(/ERROR.*already exists/)
    expect(cs.proposals.length).toBe(1)
  })

  it('propose_project_config stages the complete new config against the current file', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    const current = JSON.parse(fs.readFileSync(path.join(ROOT, '.dotzuki-editor.json'), 'utf-8'))
    const res = await proposeToolImpls(ctx, cs).propose_project_config({
      config: JSON.stringify({ ...current, name: 'Renamed' }),
    })
    expect(res).toMatchObject({ ok: true })
    const prop = cs.proposals[0]
    expect(prop.target).toMatchObject({ kind: 'project-config', path: '.dotzuki-editor.json' })
    expect(prop.before).toContain('"F"')
    expect(prop.after).toContain('"Renamed"')
    // disk untouched — propose ≠ apply
    expect(JSON.parse(fs.readFileSync(path.join(ROOT, '.dotzuki-editor.json'), 'utf-8')).name).toBe('F')
  })

  it('propose_project_config rejects a config without an activities array', async () => {
    const { ctx } = makeCtx()
    const cs = new ChangeSet()
    const res = await proposeToolImpls(ctx, cs).propose_project_config({ config: '{"name":"x"}' })
    expect(String(res)).toMatch(/ERROR/)
    expect(cs.proposals.length).toBe(0)
  })
})

describe('story-tool gating (no story activity configured)', () => {
  let bareRoot = ''
  beforeAll(() => {
    bareRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-tools-nostory-'))
    const bareWrite = (rel: string, content: string) => {
      const abs = path.join(bareRoot, rel)
      fs.mkdirSync(path.dirname(abs), { recursive: true })
      fs.writeFileSync(abs, content, 'utf-8')
    }
    // Same shape as a scaffolded wuxia/jrpg template project: data tables and
    // maps, but NO story activity — story tools must not be registered.
    bareWrite('.dotzuki-editor.json', JSON.stringify({
      name: 'B', dataRoot: '.', activities: [
        { id: 'data', type: 'data', config: { tables: [{ id: 'characters', dir: 'data/characters', idField: 'id' }] } },
        { id: 'map', type: 'map', config: { mapsDir: 'data/maps' } },
      ],
    }))
  })
  afterAll(() => { try { fs.rmSync(bareRoot, { recursive: true, force: true }) } catch { /* ignore */ } })

  function makeBareCtx() {
    return {
      actionId: 'assistant', input: {}, profile: {} as any, apiKey: 'k',
      project: createProjectContext(bareRoot), emit: () => {},
    } as ActionContext
  }

  it('buildReadTools drops character/quest tools without a story activity', async () => {
    const { buildReadTools } = await import('./tools')
    const tools = await buildReadTools(makeBareCtx())
    expect(tools.list_characters).toBeUndefined()
    expect(tools.read_character).toBeUndefined()
    expect(tools.list_quests).toBeUndefined()
    expect(tools.read_quest).toBeUndefined()
    expect(tools.list_tables).toBeDefined()
    expect(tools.list_scenes).toBeDefined()
  })

  it('buildProposeTools drops propose_story_edit without a story activity', async () => {
    const { buildProposeTools } = await import('./tools')
    const tools = await buildProposeTools(makeBareCtx(), new ChangeSet())
    expect(tools.propose_story_edit).toBeUndefined()
    expect(tools.propose_data_edit).toBeDefined()
  })

  it('buildReadTools/buildProposeTools keep story tools when a story activity exists', async () => {
    const { buildReadTools, buildProposeTools } = await import('./tools')
    const { ctx } = makeCtx()
    const read = await buildReadTools(ctx)
    expect(read.list_characters).toBeDefined()
    const propose = await buildProposeTools(ctx, new ChangeSet())
    expect(propose.propose_story_edit).toBeDefined()
  })
})
