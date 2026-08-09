// checkScene tests — the draft verification loop. Covers the lint fallback (no
// command configured) and the real-command path (a portable shell command that
// fails when the draft contains a FAILMARK), proving both pass/fail branches and
// that the project's files are never touched.
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { createProjectContext } from './context/projectContext'
import { checkScene } from './sceneCheck'

let ROOT = ''
function writeConfig(sceneBlock: Record<string, unknown>) {
  fs.writeFileSync(path.join(ROOT, '.jrpg-editor.json'), JSON.stringify({
    name: 'F', dataRoot: '.', activities: [
      { id: 'story', type: 'story', config: { storiesDir: 'data/story', scenesDir: 'data/maps', scene: sceneBlock } },
    ],
  }), 'utf-8')
}

beforeAll(() => {
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-scenecheck-t-'))
  fs.mkdirSync(path.join(ROOT, 'data/maps/Town'), { recursive: true })
  fs.writeFileSync(path.join(ROOT, 'data/maps/Town/script.scene'), 'setFlag("EVENT_TOWN_DONE")\ngetFlag("EVENT_TOWN_DONE")\n', 'utf-8')
})
afterAll(() => { try { fs.rmSync(ROOT, { recursive: true, force: true }) } catch { /* ignore */ } })

describe('checkScene', () => {
  it('falls back to lint when no scene.checkCmd is configured, and labels it', async () => {
    writeConfig({ ext: '.scene' })
    const p = createProjectContext(ROOT)
    // A flag read but never set → lint FAIL.
    const bad = await checkScene(p, 'Town', 'getFlag("EVENT_NEVER")\n')
    expect(bad.source).toBe('lint')
    expect(bad.ok).toBe(false)
    expect(bad.output).toMatch(/lint only/)
    // A clean buffer (reads a flag the project sets) → lint PASS.
    const good = await checkScene(p, 'Town', 'getFlag("EVENT_TOWN_DONE")\n')
    expect(good.ok).toBe(true)
  })

  it('runs a configured scene.checkCmd against the draft (real compile path)', async () => {
    // Portable stand-in for a compiler: fail iff the draft contains FAILMARK.
    writeConfig({ ext: '.scene', checkCmd: "sh -c '! grep -q FAILMARK {file}'" })
    const p = createProjectContext(ROOT)

    const pass = await checkScene(p, 'Town', '@storyline("ok")\n')
    expect(pass.source).toBe('compile')
    expect(pass.ok).toBe(true)

    const fail = await checkScene(p, 'Town', '@storyline("bad")\nFAILMARK\n')
    expect(fail.source).toBe('compile')
    expect(fail.ok).toBe(false)
  })

  it('never writes the draft into the project', async () => {
    writeConfig({ ext: '.scene', checkCmd: "sh -c 'true'" })
    const p = createProjectContext(ROOT)
    const before = fs.readFileSync(path.join(ROOT, 'data/maps/Town/script.scene'), 'utf-8')
    await checkScene(p, 'Town', '@storyline("draft only")\n// should never hit disk\n')
    expect(fs.readFileSync(path.join(ROOT, 'data/maps/Town/script.scene'), 'utf-8')).toBe(before)
  })
})
