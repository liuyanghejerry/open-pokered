// Skills loader tests — frontmatter parsing, builtin+project discovery and
// merge, read-by-name, the system-prompt index, and a sanity pass over the
// skills shipped under tools/pokered-editor/skills/.
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { createProjectContext } from '../context/projectContext'
import { builtinSkillsDir, listSkills, listSkillsInDir, parseSkillFile, readSkillByName, skillsPromptSection } from './skills'

let ROOT = ''
function write(rel: string, content: string) {
  const abs = path.join(ROOT, rel)
  fs.mkdirSync(path.dirname(abs), { recursive: true })
  fs.writeFileSync(abs, content, 'utf-8')
}

beforeAll(() => {
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'pokered-skills-'))
  write('.dotzuki-editor.json', JSON.stringify({
    name: 'F', dataRoot: '.', activities: [
      { id: 'map', type: 'map', config: { mapsDir: 'maps' } },
    ],
  }))
  // A project-local skill, plus one that overrides a builtin by name.
  write('skills/fest-quests/SKILL.md', '---\nname: fest-quests\ndescription: Author festival quest lines.\n---\n\n# Fest quests\n\nBody here.\n')
  write('skills/pokered-new-map/SKILL.md', '---\nname: pokered-new-map\ndescription: Project-local override of the map playbook.\n---\n\nOverride body.\n')
})
afterAll(() => { try { fs.rmSync(ROOT, { recursive: true, force: true }) } catch { /* ignore */ } })

describe('parseSkillFile', () => {
  it('parses frontmatter name/description and strips it from the body', () => {
    const p = parseSkillFile('---\nname: demo\ndescription: Does demos.\n---\n\n# Demo\n\nDo the thing.\n', 'fallback')
    expect(p.name).toBe('demo')
    expect(p.description).toBe('Does demos.')
    expect(p.body).toBe('# Demo\n\nDo the thing.')
  })

  it('falls back to the directory name and tolerates missing frontmatter', () => {
    const p = parseSkillFile('# No frontmatter\n', 'my-skill')
    expect(p.name).toBe('my-skill')
    expect(p.description).toBe('')
    expect(p.body).toContain('No frontmatter')
  })
})

describe('discovery', () => {
  it('lists <dir>/<name>/SKILL.md entries, skipping non-skill dirs', () => {
    const list = listSkillsInDir(path.join(ROOT, 'skills'), 'project')
    expect(list.map(s => s.name).sort()).toEqual(['fest-quests', 'pokered-new-map'])
    expect(list[0].source).toBe('project')
  })

  it('returns [] for a missing directory', () => {
    expect(listSkillsInDir(path.join(ROOT, 'nope'), 'project')).toEqual([])
  })

  it('merges builtin + project skills, project winning on a name clash', () => {
    const project = createProjectContext(ROOT)
    const list = listSkills(project)
    // The four shipped pokered skills are present…
    for (const name of ['pokered-new-map', 'pokered-new-trainer', 'pokered-new-pokemon', 'pokered-save-construction']) {
      expect(list.some(s => s.name === name)).toBe(true)
    }
    // …plus the project-local one…
    expect(list.some(s => s.name === 'fest-quests' && s.source === 'project')).toBe(true)
    // …and the project override shadows the builtin map skill.
    const map = list.find(s => s.name === 'pokered-new-map')!
    expect(map.source).toBe('project')
    expect(map.description).toContain('override')
  })

  it('reads a skill body by name and returns null for unknown names', () => {
    const project = createProjectContext(ROOT)
    const doc = readSkillByName('fest-quests', project)!
    expect(doc.body).toContain('Fest quests')
    expect(readSkillByName('no-such-skill', project)).toBeNull()
  })
})

describe('skillsPromptSection', () => {
  it('renders the index with read_skill usage instructions', () => {
    const section = skillsPromptSection(createProjectContext(ROOT))
    expect(section).toContain('read_skill')
    expect(section).toContain('pokered-new-map')
    expect(section).toContain('fest-quests')
  })
})

describe('shipped builtin skills', () => {
  it('the four pokered playbooks exist with descriptions and bodies', () => {
    const list = listSkillsInDir(builtinSkillsDir(), 'builtin')
    expect(list.map(s => s.name).sort()).toEqual([
      'pokered-new-map', 'pokered-new-pokemon', 'pokered-new-trainer', 'pokered-save-construction',
    ])
    for (const s of list) {
      const doc = readSkillByName(s.name, null)!
      expect(s.description.length, s.name).toBeGreaterThan(20)
      expect(doc.body.length, s.name).toBeGreaterThan(500)
    }
  })
})
