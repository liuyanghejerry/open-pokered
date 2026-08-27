// ───────────────────────────────────────────────────────────────────────────
// Skills — loadable task playbooks for the chat assistant.
//
// A skill is a directory containing a `SKILL.md` with a tiny YAML frontmatter
// (`name` + `description`) followed by the playbook body — the same convention
// as this repo's `.claude/skills/`. Discovery is by directory scan; no index
// file to keep in sync.
//
// Two sources, merged by name (project wins):
//   builtin — `<pokered-editor>/skills/` (shipped with the editor; the pokered-*
//             playbooks for maps / trainers / Pokémon / saves);
//   project — `<projectRoot>/skills/` (a game's own playbooks, if any).
//
// The chat agent sees only the name+description index in its system prompt and
// pulls the full body on demand via the `read_skill` tool (progressive
// disclosure — the index stays cheap when no skill matches the request).
//
// Pure Node (fs/path); no AI deps. Safe to import from the Vite dev middleware
// and from vitest.
// ───────────────────────────────────────────────────────────────────────────
import fs from 'fs'
import path from 'path'
import { fileURLToPath } from 'url'
import type { ProjectContext } from '../context/projectContext'

const moduleDir = path.dirname(fileURLToPath(import.meta.url))

export interface SkillSummary {
  name: string
  description: string
  /** 'builtin' = shipped with the editor; 'project' = from the opened project. */
  source: 'builtin' | 'project'
  /** Absolute path of the SKILL.md (for debugging/troubleshooting). */
  path: string
}

export interface SkillDoc extends SkillSummary {
  /** The playbook body (markdown after the frontmatter). */
  body: string
}

/** Directory the editor's bundled skills live in (server/actions → ../../skills).
 *  DOTZUKI_SKILLS_DIR overrides — e.g. inside a packaged Electron app, where the
 *  bundled api-server's moduleDir no longer sits next to the skills tree. */
export function builtinSkillsDir(): string {
  return process.env.DOTZUKI_SKILLS_DIR || path.resolve(moduleDir, '../../skills')
}

/**
 * Parse a SKILL.md into name/description/body. Frontmatter is a `---`-fenced
 * block of `key: value` lines at the very top; everything after it is the body.
 * Tolerant by design: a missing block yields the fallback name + empty
 * description, and unknown keys are ignored.
 */
export function parseSkillFile(text: string, fallbackName: string): { name: string; description: string; body: string } {
  let name = fallbackName
  let description = ''
  let body = text
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/)
  if (m) {
    body = text.slice(m[0].length)
    for (const line of m[1].split(/\r?\n/)) {
      const kv = line.match(/^([A-Za-z_-]+):\s*(.*)$/)
      if (!kv) continue
      const key = kv[1].toLowerCase()
      const value = kv[2].trim().replace(/^["']|["']$/g, '')
      if (key === 'name' && value) name = value
      if (key === 'description') description = value
    }
  }
  return { name, description, body: body.trim() }
}

/** List the skills in ONE directory (`<dir>/<skill>/SKILL.md` entries). */
export function listSkillsInDir(dir: string, source: SkillSummary['source']): SkillSummary[] {
  let entries: fs.Dirent[]
  try {
    if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) return []
    entries = fs.readdirSync(dir, { withFileTypes: true })
  } catch {
    return []
  }
  const out: SkillSummary[] = []
  for (const e of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    if (!e.isDirectory()) continue
    const file = path.join(dir, e.name, 'SKILL.md')
    if (!fs.existsSync(file) || !fs.statSync(file).isFile()) continue
    try {
      const parsed = parseSkillFile(fs.readFileSync(file, 'utf-8'), e.name)
      out.push({ name: parsed.name, description: parsed.description, source, path: file })
    } catch { /* skip unreadable */ }
  }
  return out
}

/** All skills visible to the assistant: builtin first, project skills overriding by name. */
export function listSkills(project?: ProjectContext | null): SkillSummary[] {
  const out = new Map<string, SkillSummary>()
  for (const s of listSkillsInDir(builtinSkillsDir(), 'builtin')) out.set(s.name, s)
  if (project) {
    for (const s of listSkillsInDir(path.join(project.root, 'skills'), 'project')) out.set(s.name, s)
  }
  return [...out.values()].sort((a, b) => a.name.localeCompare(b.name))
}

/** Read one skill's full doc by name (same resolution order as listSkills). */
export function readSkillByName(name: string, project?: ProjectContext | null): SkillDoc | null {
  const hit = listSkills(project).find(s => s.name === name)
  if (!hit) return null
  try {
    const parsed = parseSkillFile(fs.readFileSync(hit.path, 'utf-8'), path.basename(path.dirname(hit.path)))
    return { ...hit, name: parsed.name, description: parsed.description, body: parsed.body }
  } catch {
    return null
  }
}

/**
 * Render the system-prompt section listing the available skills ('' when none).
 * Kept short on purpose — the body is pulled via read_skill only when a task
 * actually matches.
 */
export function skillsPromptSection(project?: ProjectContext | null): string {
  const skills = listSkills(project)
  if (!skills.length) return ''
  return [
    '## Skills — loadable task playbooks',
    'This project ships skill playbooks for recurring authoring tasks. When the request matches one, call the read_skill tool with its name BEFORE acting, then follow its workflow end-to-end (it encodes the project\'s file formats, invariants, and verification steps). Skills available here:',
    ...skills.map(s => `- "${s.name}" — ${s.description}`),
  ].join('\n')
}
