import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { createProjectContext } from './context/projectContext'
import { lintScene } from './sceneLint'

let ROOT = ''
function write(rel: string, content: string) {
  const abs = path.join(ROOT, rel)
  fs.mkdirSync(path.dirname(abs), { recursive: true })
  fs.writeFileSync(abs, content, 'utf-8')
}

beforeAll(() => {
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-lint-'))
  write('.dotzuki-editor.json', JSON.stringify({
    name: 'F', dataRoot: '.', activities: [
      { id: 'story', type: 'story', config: { storiesDir: 'data/story', scenesDir: 'data/maps', scene: { ext: '.scene' } } },
    ],
  }))
  // Project corpus: EVENT_A is both set and read; EVENT_SET_ONLY is only set.
  write('data/maps/Town/script.scene', 'setFlag("EVENT_A")\ngetFlag("EVENT_A")\nsetFlag("EVENT_SET_ONLY")\n')
})
afterAll(() => { try { fs.rmSync(ROOT, { recursive: true, force: true }) } catch { /* ignore */ } })

describe('lintScene', () => {
  it('flags reads of a flag that is never set anywhere', () => {
    const f = lintScene(createProjectContext(ROOT), 'getFlag("EVENT_NEVER_SET")\n')
    expect(f.some(x => x.flag === 'EVENT_NEVER_SET' && x.severity === 'warn')).toBe(true)
  })
  it('does not flag a read of a project-known flag', () => {
    const f = lintScene(createProjectContext(ROOT), 'getFlag("EVENT_A")\n')
    expect(f.some(x => x.flag === 'EVENT_A')).toBe(false)
  })
  it('flags a set of a flag that is never read anywhere (info)', () => {
    const f = lintScene(createProjectContext(ROOT), 'setFlag("EVENT_SET_ONLY")\n')
    expect(f.some(x => x.flag === 'EVENT_SET_ONLY' && x.severity === 'info')).toBe(true)
  })
  it('reports the 1-based line number', () => {
    const f = lintScene(createProjectContext(ROOT), '// line1\ngetFlag("EVENT_NOPE")\n')
    expect(f.find(x => x.flag === 'EVENT_NOPE')?.line).toBe(2)
  })
})
