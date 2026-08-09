// ───────────────────────────────────────────────────────────────────────────
// sceneLint — deterministic lint for a `.scene` buffer. Catches the high-signal,
// always-computable issues: flags read but never set anywhere (typos / dangling),
// flags set but never read (orphans), and — when the project configures API
// typings (story `ai.apiTypes`) — game.* calls that aren't in the engine API
// (the hallucinated-API bug). No AI. Pure, fast, unit-testable.
// ───────────────────────────────────────────────────────────────────────────
import type { ProjectContext } from './context/projectContext'

export interface LintFinding {
  line: number
  severity: 'warn' | 'info'
  message: string
  flag?: string
}

export function lintScene(project: ProjectContext, content: string): LintFinding[] {
  const usage = project.scanFlagUsage()
  const findings: LintFinding[] = []
  const lines = content.split('\n')

  lines.forEach((line, i) => {
    const ln = i + 1
    let m: RegExpExecArray | null

    const reGet = /getFlag\s*\(\s*["']([A-Za-z_][A-Za-z0-9_]*)["']/g
    while ((m = reGet.exec(line)) !== null) {
      if (!usage.set.has(m[1])) findings.push({ line: ln, severity: 'warn', message: `Flag "${m[1]}" is read but never set anywhere.`, flag: m[1] })
    }
    const reSet = /setFlag\s*\(\s*["']([A-Za-z_][A-Za-z0-9_]*)["']/g
    while ((m = reSet.exec(line)) !== null) {
      if (!usage.get.has(m[1])) findings.push({ line: ln, severity: 'info', message: `Flag "${m[1]}" is set but never read anywhere.`, flag: m[1] })
    }
  })

  // game.* API check — only when the project ships API typings to validate against.
  const api = project.getApiTypes()
  if (api) {
    const known = new Set([...api.matchAll(/\bgame\.(\w+)/g)].map(x => x[1]))
    if (known.size) {
      lines.forEach((line, i) => {
        let m: RegExpExecArray | null
        const re = /\bgame\.(\w+)\s*\(/g
        while ((m = re.exec(line)) !== null) {
          if (!known.has(m[1])) findings.push({ line: i + 1, severity: 'warn', message: `game.${m[1]}(...) is not in the game API — possible typo or hallucinated call.` })
        }
      })
    }
  }

  return findings
}
