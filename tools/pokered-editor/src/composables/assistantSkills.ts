// ───────────────────────────────────────────────────────────────────────────
// assistantSkills — the static (browser-only) assistant's view of the skill
// playbooks. The dev server serves them from disk (server/actions/skills.ts);
// in the hosted static build there is no server, so Vite inlines the shipped
// `skills/<name>/SKILL.md` files into the bundle at build time instead.
//
// The index is name + description only; the assistant pulls the full body via
// its read_skill tool when a task actually matches (progressive disclosure).
// ───────────────────────────────────────────────────────────────────────────

export interface StaticSkillSummary {
  name: string
  description: string
}

export interface StaticSkillDoc extends StaticSkillSummary {
  body: string
}

// tools/pokered-editor/skills/<name>/SKILL.md, bundled as raw strings.
const modules = import.meta.glob('../../skills/*/SKILL.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

/** Parse frontmatter (`name` / `description`) off a SKILL.md; body = the rest. */
function parseSkill(text: string, fallbackName: string): StaticSkillDoc {
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

const SKILLS: StaticSkillDoc[] = Object.entries(modules)
  .map(([file, text]) => {
    const dir = file.split('/').slice(-2, -1)[0] ?? ''
    return parseSkill(text, dir)
  })
  .sort((a, b) => a.name.localeCompare(b.name))

/** All bundled skills (name + description). */
export function listStaticSkills(): StaticSkillSummary[] {
  return SKILLS.map(({ name, description }) => ({ name, description }))
}

/** One skill's full doc by name, or null. */
export function readStaticSkill(name: string): StaticSkillDoc | null {
  return SKILLS.find(s => s.name === name) ?? null
}

/** System-prompt section listing the skills ('' when none are bundled). */
export function staticSkillsPromptSection(): string {
  if (!SKILLS.length) return ''
  return [
    '## Skills — loadable task playbooks',
    'This editor ships skill playbooks for recurring authoring tasks. When the request matches one, call the read_skill tool with its name BEFORE acting, then follow its workflow end-to-end (it encodes the project\'s file formats, invariants, and verification steps). Skills available here:',
    ...SKILLS.map(s => `- "${s.name}" — ${s.description}`),
  ].join('\n')
}
