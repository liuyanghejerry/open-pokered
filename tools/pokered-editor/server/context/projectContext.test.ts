// ───────────────────────────────────────────────────────────────────────────
// ProjectContext tests — run against a hermetic temp fixture project that has
// NO `ai` block (mirroring the live wuxia config), so the empty-context fix is
// exercised directly: assembleContext() must still ground the model in real
// scene files even with `ai` unset.
// ───────────────────────────────────────────────────────────────────────────
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { ProjectContext, createProjectContext } from './projectContext'

let ROOT = ''
let ctx: ProjectContext

function write(rel: string, content: string) {
  const abs = path.join(ROOT, rel)
  fs.mkdirSync(path.dirname(abs), { recursive: true })
  fs.writeFileSync(abs, content, 'utf-8')
}

beforeAll(() => {
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-ctx-'))

  // Config with story / script / data / map / ui — and intentionally NO `ai`.
  write('.dotzuki-editor.json', JSON.stringify({
    name: 'Fixture Game',
    dataRoot: '.',
    gfxRoot: '.',
    activities: [
      {
        id: 'story', type: 'story', enabled: true, config: {
          storiesDir: 'data/story',
          scenesDir: 'data/maps',
          locales: ['en', 'zh'],
          flagSource: { scan: { dir: 'data/maps', fns: ['getFlag', 'setFlag'], recursive: true }, table: null },
          scene: { ext: '.scene' },
        },
      },
      { id: 'scripts', type: 'script', enabled: true, config: { scriptsDir: 'data/maps', extension: '.scene' } },
      {
        id: 'data', type: 'data', enabled: true, config: {
          tables: [{ id: 'skills', label: 'Skills', dir: 'data/gamedata/skills', idField: 'id', fields: [] }],
        },
      },
      { id: 'maps', type: 'map', enabled: true, config: { mapsDir: 'data/maps', tileSize: 16 } },
      { id: 'ui', type: 'ui', enabled: true, config: { guiRoot: 'ui_layouts', extension: '.gui' } },
    ],
  }, null, 2))

  // Story records. Note `elder` has a filename slug that differs from its `id`,
  // to exercise resolve-by-id (not by filename).
  write('data/story/characters/chen-yuan.json', JSON.stringify({
    id: 'chen-yuan', name: { en: 'Chen Yuan', zh: '陈渊' },
    relationships: [{ to: 'li-mu', kind: 'rival' }],
  }))
  write('data/story/characters/li-mu.json', JSON.stringify({ id: 'li-mu', name: 'Li Mu' }))
  write('data/story/characters/the-village-elder.json', JSON.stringify({ id: 'elder-wang', name: 'Elder Wang' }))
  write('data/story/quests/rescue.json', JSON.stringify({
    id: 'rescue-the-elder', name: 'Rescue the Elder',
    giver: 'chen-yuan', characters: ['li-mu'],
    requires: ['EVENT_INTRO_DONE'], sets: ['EVENT_ELDER_SAFE'],
    implementedBy: [{ scene: 'Wangjiang', storyline: 'rescue' }], maps: ['Wangjiang'],
  }))
  write('data/story/arcs/act1.json', JSON.stringify({ id: 'act-1', name: 'Act One', beats: ['rescue-the-elder'] }))

  // Scenes (the per-map <Map>/script.scene convention).
  write('data/maps/Wangjiang/script.scene', [
    '@storyline("rescue")',
    '@trigger when getFlag("EVENT_INTRO_DONE")',
    '  game.showText(@t("The elder is trapped!", "长老被困住了！"))',
    '  setFlag("EVENT_ELDER_SAFE")',
  ].join('\n'))
  write('data/maps/Other/script.scene', [
    '@storyline("other")',
    '  game.showText("hello")',
    '  if (getFlag("EVENT_OTHER")) { }',
  ].join('\n'))
  write('data/maps/Empty/script.scene', '') // stub — must be skipped by sampling

  // Data + gui.
  write('data/gamedata/skills/fireball.json', JSON.stringify({ id: 'fireball', name: 'Fireball', power: 30 }))
  write('ui_layouts/main_menu.gui', 'screen main_menu {\n  panel { text "Hi" }\n}\n')

  ctx = createProjectContext(ROOT)
})

afterAll(() => { try { fs.rmSync(ROOT, { recursive: true, force: true }) } catch { /* ignore */ } })

describe('config & roots', () => {
  it('loads config and tables', () => {
    expect(ctx.config().name).toBe('Fixture Game')
    expect(ctx.dataTables().map(t => t.id)).toContain('skills')
    expect(ctx.storyConfig()?.storiesDir).toBe('data/story')
  })
})

describe('story records', () => {
  it('lists characters / quests / arcs', () => {
    expect(ctx.listCharacters().length).toBe(3)
    expect(ctx.listQuests().map(q => q.id)).toContain('rescue-the-elder')
    expect(ctx.listArcs().map(a => a.id)).toContain('act-1')
  })
  it('reads a record by id even when the filename slug differs', () => {
    const elder = ctx.readStoryRecord('characters', 'elder-wang')
    expect(elder?.name).toBe('Elder Wang') // file is the-village-elder.json
  })
  it('returns null for unknown records and unknown story kinds gracefully', () => {
    expect(ctx.readStoryRecord('characters', 'nobody')).toBeNull()
  })
})

describe('scenes & flags', () => {
  it('lists scenes with collapsed /script stems and storyline names', () => {
    const scenes = ctx.listScenes()
    const wang = scenes.find(s => s.stem === 'Wangjiang')
    expect(wang).toBeTruthy()
    expect(wang!.names).toContain('rescue')
    expect(scenes.map(s => s.stem)).toContain('Other')
  })
  it('scans event flags from scene getFlag/setFlag calls', () => {
    const flags = ctx.scanFlags()
    expect(flags).toContain('EVENT_INTRO_DONE')
    expect(flags).toContain('EVENT_ELDER_SAFE')
    expect(flags).toContain('EVENT_OTHER')
  })
})

describe('data, gui, maps', () => {
  it('lists/reads data records', () => {
    expect(ctx.listRecords('skills').map(r => r.id)).toContain('fireball')
    expect(ctx.readRecord('skills', 'fireball')?.power).toBe(30)
  })
  it('lists/reads gui layouts', () => {
    expect(ctx.listGui()).toContain('main_menu.gui')
    expect(ctx.readGui('main_menu.gui')).toContain('screen main_menu')
  })
  it('lists map directories', () => {
    expect(ctx.listMaps()).toEqual(expect.arrayContaining(['Wangjiang', 'Other']))
  })
})

describe('assembleContext — the empty-context fix', () => {
  it('grounds the model in real scenes even with NO ai block configured', () => {
    expect(ctx.storyConfig()?.ai).toBeUndefined() // precondition: this is the wuxia case
    const context = ctx.assembleContext()
    expect(context.length).toBeGreaterThan(0)
    // auto-sampled real scenes appear, so the model sees the actual engine API
    expect(context).toContain('Example scene (auto)')
    expect(context).toContain('@storyline')
  })
  it('skips empty/stub scene files when sampling', () => {
    const context = ctx.assembleContext({ exampleSceneLimit: 10 })
    expect(context).not.toContain('data/maps/Empty/script.scene')
  })
})

describe('structured retrieval', () => {
  it('gatherForQuest resolves giver + characters + flags + implementing scenes', () => {
    const g = ctx.gatherForQuest('rescue-the-elder')
    expect(g.quest?.id).toBe('rescue-the-elder')
    expect(g.characters.map(c => c.id).sort()).toEqual(['chen-yuan', 'li-mu'])
    expect(g.flags).toContain('EVENT_ELDER_SAFE')
    expect(g.scenes.map(s => s.stem)).toContain('Wangjiang')
  })
  it('gatherForCharacter resolves relationships and quests involving them', () => {
    const g = ctx.gatherForCharacter('chen-yuan')
    expect(g.related.map(r => r.record?.id)).toContain('li-mu')
    expect(g.questsInvolving.map(q => q.id)).toContain('rescue-the-elder') // giver
  })
  it('resolves @mentions across kinds', () => {
    expect(ctx.resolveMention('chen-yuan')?.kind).toBe('character')
    expect(ctx.resolveMention('quest:rescue-the-elder')?.kind).toBe('quest')
    expect(ctx.resolveMention('@scene:Wangjiang')?.kind).toBe('scene')
    expect(ctx.resolveMention('fireball')).toMatchObject({ kind: 'data', table: 'skills' })
    expect(ctx.resolveMention('does-not-exist')).toBeNull()
  })
  it('builds a mention index spanning every surface', () => {
    const kinds = new Set(ctx.mentionIndex().map(m => m.kind))
    expect(kinds).toEqual(new Set(['character', 'quest', 'arc', 'scene', 'data', 'gui', 'map']))
  })
  it('uses localized names for labels', () => {
    const chen = ctx.mentionIndex().find(m => m.id === 'chen-yuan')
    expect(chen?.label).toBe('Chen Yuan') // name.en
  })
})
