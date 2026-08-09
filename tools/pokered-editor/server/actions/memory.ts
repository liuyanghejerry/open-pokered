// ───────────────────────────────────────────────────────────────────────────
// Assistant memory — small markdown files the agent keeps about the user and
// the project. They are folded into the system prompt on every turn (see
// assistantSystem.ts) and appended to via the remember_fact tool.
//
//   project memory: <projectRoot>/.jrpg-editor.memory.md
//   global memory:  ~/.jrpg-editor/memory.md  (the only one in creation mode)
//
// `homeDir` parameters exist so tests can point the global file at a temp dir.
// ───────────────────────────────────────────────────────────────────────────
import fs from 'fs'
import os from 'os'
import path from 'path'
import type { ProjectContext } from '../context/projectContext'

/** Per-file read cap: keeps the system-prompt section bounded (~4KB each). */
const READ_CAP = 4096
/** A memory entry stays a one-liner. */
const FACT_CAP = 500

export interface Memories { global: string; project: string }
export type MemoryScope = 'project' | 'global'

export function globalMemoryFile(homeDir: string = os.homedir()): string {
  return path.join(homeDir, '.jrpg-editor', 'memory.md')
}
export function projectMemoryFile(project: ProjectContext): string {
  return path.join(project.root, '.jrpg-editor.memory.md')
}

/** Read a memory file (missing/unreadable → ''), truncated to READ_CAP. */
export function readMemoryFile(file: string): string {
  try {
    if (!fs.existsSync(file)) return ''
    return fs.readFileSync(file, 'utf-8').slice(0, READ_CAP)
  } catch { return '' }
}

/** Read both memory files. `project` = null in creation mode → project is ''. */
export function readMemories(project: ProjectContext | null, homeDir?: string): Memories {
  return {
    global: readMemoryFile(globalMemoryFile(homeDir)),
    project: project ? readMemoryFile(projectMemoryFile(project)) : '',
  }
}

/**
 * Append one dated line to a memory file (created on first write). With no
 * project open the scope falls back to global. Returns the file written and
 * the effective scope.
 */
export function appendMemory(
  project: ProjectContext | null, scope: MemoryScope, fact: string, homeDir?: string,
): { file: string; scope: MemoryScope } {
  const clean = String(fact).replace(/\s+/g, ' ').trim().slice(0, FACT_CAP)
  if (!clean) throw new Error('fact is empty')
  const effective: MemoryScope = project && scope === 'project' ? 'project' : 'global'
  const file = effective === 'project' ? projectMemoryFile(project!) : globalMemoryFile(homeDir)
  fs.mkdirSync(path.dirname(file), { recursive: true })
  const date = new Date().toISOString().slice(0, 10)
  // A hand-edited file may lack a trailing newline — don't glue onto its last line.
  fs.appendFileSync(file, `${endsWithNewline(file) ? '' : '\n'}- [${date}] ${clean}\n`, 'utf-8')
  return { file, scope: effective }
}

/** Whether an existing non-empty file ends with '\n' (reads just the last byte). */
function endsWithNewline(file: string): boolean {
  try {
    const size = fs.statSync(file).size
    if (!size) return true
    const fd = fs.openSync(file, 'r')
    try {
      const buf = Buffer.alloc(1)
      fs.readSync(fd, buf, 0, 1, size - 1)
      return buf.toString('utf-8') === '\n'
    } finally { fs.closeSync(fd) }
  } catch { return true } // missing/unreadable → appendFileSync creates it
}
