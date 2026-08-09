// Assistant memory: read/write of the global + project memory files, read
// truncation, the no-project scope fallback, and the remember_fact tool impl.
// Everything runs against temp dirs (homeDir injection) — the real ~/ is untouched.
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { createProjectContext } from '../context/projectContext'
import {
  appendMemory, globalMemoryFile, projectMemoryFile, readMemories, readMemoryFile,
} from './memory'
import { memoryToolImpl } from './tools'
import type { ActionContext } from './types'

let HOME = ''   // fake home (holds the global memory file)
let ROOT = ''   // fake project root (holds the project memory file)
const EXTRA_DIRS: string[] = []

beforeAll(() => {
  HOME = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-mem-home-'))
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-mem-proj-'))
})
afterAll(() => {
  for (const d of [HOME, ROOT, ...EXTRA_DIRS]) { try { fs.rmSync(d, { recursive: true, force: true }) } catch { /* ignore */ } }
})

const project = () => createProjectContext(ROOT)
const globalFile = () => globalMemoryFile(HOME)
const projectFile = () => projectMemoryFile(project())

describe('readMemories', () => {
  it('returns empty strings when no memory files exist', () => {
    expect(readMemories(project(), HOME)).toEqual({ global: '', project: '' })
    expect(readMemories(null, HOME)).toEqual({ global: '', project: '' })
  })

  it('reads both files; with no project only the global one', () => {
    fs.mkdirSync(path.dirname(globalFile()), { recursive: true })
    fs.writeFileSync(globalFile(), 'likes wuxia settings\n', 'utf-8')
    fs.writeFileSync(projectFile(), 'hero names are two-char\n', 'utf-8')
    expect(readMemories(project(), HOME)).toEqual({ global: 'likes wuxia settings\n', project: 'hero names are two-char\n' })
    expect(readMemories(null, HOME)).toEqual({ global: 'likes wuxia settings\n', project: '' })
  })

  it('truncates each file to ~4KB', () => {
    fs.writeFileSync(globalFile(), 'x'.repeat(5000), 'utf-8')
    expect(readMemoryFile(globalFile())).toHaveLength(4096)
  })
})

describe('appendMemory', () => {
  // Each case gets a fresh project root / home so appended lines never mix.
  const fresh = () => {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-mem-home-'))
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-mem-proj-'))
    EXTRA_DIRS.push(home, root)
    return { home, root, proj: createProjectContext(root) }
  }

  it('creates the project file with a dated line on first write', () => {
    const { home, root, proj } = fresh()
    const r = appendMemory(proj, 'project', 'uses pinyin ids', home)
    expect(r.scope).toBe('project')
    expect(r.file).toBe(path.join(root, '.jrpg-editor.memory.md'))
    expect(fs.readFileSync(r.file, 'utf-8')).toMatch(/^- \[\d{4}-\d{2}-\d{2}\] uses pinyin ids\n$/)
  })

  it('appends below existing content, collapsing whitespace and capping at 500 chars', () => {
    const { home, proj } = fresh()
    const r1 = appendMemory(proj, 'project', 'first', home)
    const r2 = appendMemory(proj, 'project', `  multi\nline\t${'y'.repeat(600)}  `, home)
    const lines = fs.readFileSync(r2.file, 'utf-8').split('\n')
    expect(lines[0]).toContain('first')
    const last = lines[1]
    expect(last).toContain('multi line ')
    // "- [yyyy-mm-dd] " prefix (15 chars) + at most 500 chars of fact
    expect(last.length).toBeLessThanOrEqual(15 + 500)
    expect(r1.file).toBe(r2.file)
  })

  it('starts a new line when the existing file lacks a trailing newline', () => {
    const { home, proj } = fresh()
    const file = projectMemoryFile(proj)
    fs.writeFileSync(file, 'hand-written note without newline', 'utf-8')
    appendMemory(proj, 'project', 'second entry', home)
    const lines = fs.readFileSync(file, 'utf-8').split('\n')
    expect(lines[0]).toBe('hand-written note without newline')
    expect(lines[1]).toMatch(/^- \[\d{4}-\d{2}-\d{2}\] second entry$/)
  })

  it('falls back to GLOBAL scope when no project is open', () => {
    const { home } = fresh()
    const r = appendMemory(null, 'project', 'no project yet', home)
    expect(r.scope).toBe('global')
    expect(r.file).toBe(globalMemoryFile(home))
    expect(fs.readFileSync(r.file, 'utf-8')).toContain('no project yet')
  })

  it('rejects an empty fact without writing anything', () => {
    const { home, proj } = fresh()
    expect(() => appendMemory(proj, 'project', '   ', home)).toThrow(/empty/)
    expect(fs.existsSync(projectMemoryFile(proj))).toBe(false)
  })
})

describe('remember_fact tool impl', () => {
  const ctxWithProject = () => ({
    actionId: 'assistant', input: {}, profile: {} as any, apiKey: 'k',
    project: createProjectContext(ROOT), emit: () => {},
  }) as ActionContext
  const ctxNoProject = () => ({
    actionId: 'assistant', input: {}, profile: {} as any, apiKey: 'k',
    project: null, emit: () => {},
  }) as unknown as ActionContext

  it('writes to the PROJECT memory by default when a project is open', async () => {
    const res = await memoryToolImpl(ctxWithProject(), HOME).remember_fact({ fact: 'bilingual en/zh text' })
    expect(String(res)).toMatch(/^OK: saved to project memory/)
    expect(fs.readFileSync(projectFile(), 'utf-8')).toContain('bilingual en/zh text')
  })

  it('honours an explicit global scope', async () => {
    const res = await memoryToolImpl(ctxWithProject(), HOME).remember_fact({ fact: 'prefers terse answers', scope: 'global' })
    expect(String(res)).toMatch(/^OK: saved to global memory/)
    expect(fs.readFileSync(globalFile(), 'utf-8')).toContain('prefers terse answers')
  })

  it('forces global scope in creation mode (no project open)', async () => {
    const res = await memoryToolImpl(ctxNoProject(), HOME).remember_fact({ fact: 'likes dark themes', scope: 'project' })
    expect(String(res)).toMatch(/^OK: saved to global memory/)
    expect(fs.readFileSync(globalFile(), 'utf-8')).toContain('likes dark themes')
  })

  it('reports an ERROR for an empty fact', async () => {
    const res = await memoryToolImpl(ctxWithProject(), HOME).remember_fact({ fact: ' ' })
    expect(String(res)).toMatch(/^ERROR/)
  })
})
