// ───────────────────────────────────────────────────────────────────────────
// guiLint — deterministic STRUCTURAL pre-check for a `.gui` buffer.
//
// The authoritative `.gui` compiler is the WASM `compileScreen`, which runs in
// the browser (GuiActivity) — the server can't reach it. But the most common
// agent-authoring failures are structural and catchable without a full compile:
// truncated output (unbalanced { } [ ] ( )), an empty buffer, or a missing
// top-level block. This catches those so the chat assistant stops proposing
// obviously-broken `.gui` blind; the client still runs the real WASM compile on
// apply. Pure, fast, unit-testable. NOT a full grammar check — absence of
// findings does not guarantee the file compiles.
// ───────────────────────────────────────────────────────────────────────────
import type { LintFinding } from './sceneLint'

/** Recognized top-level block keywords (GAME_UI_DSL): a valid `.gui` opens with
 *  one of these. */
const TOP_LEVEL = ['screen', 'panel', 'component', 'theme', 'style']

/** Strip `//` line comments and the CONTENTS of double-quoted strings so that
 *  delimiters inside them don't skew the balance count. Keeps the quotes/length
 *  roughly intact (replaces inner chars with spaces) so the scan stays line-local. */
function stripCommentsAndStrings(src: string): string {
  let out = ''
  let inString = false
  for (let i = 0; i < src.length; i++) {
    const c = src[i]
    if (inString) {
      if (c === '\\') { out += '  '; i++; continue } // skip escaped char
      if (c === '"') { inString = false; out += '"'; continue }
      out += c === '\n' ? '\n' : ' '
      continue
    }
    if (c === '"') { inString = true; out += '"'; continue }
    if (c === '/' && src[i + 1] === '/') { // line comment → blank to EOL
      while (i < src.length && src[i] !== '\n') i++
      out += '\n'
      continue
    }
    out += c
  }
  return out
}

export function lintGui(content: string): LintFinding[] {
  const findings: LintFinding[] = []
  if (!content || !content.trim()) {
    return [{ line: 1, severity: 'warn', message: 'GUI file is empty.' }]
  }

  const scrubbed = stripCommentsAndStrings(content)

  // ── Delimiter balance (the dominant failure: truncated/runaway output) ──
  const pairs: Array<[string, string, string]> = [
    ['{', '}', 'brace'],
    ['[', ']', 'bracket'],
    ['(', ')', 'paren'],
  ]
  for (const [open, close, label] of pairs) {
    let depth = 0
    let firstUnmatchedClose = 0
    const lines = scrubbed.split('\n')
    lines.forEach((line, i) => {
      for (const ch of line) {
        if (ch === open) depth++
        else if (ch === close) { depth--; if (depth < 0 && !firstUnmatchedClose) firstUnmatchedClose = i + 1 }
      }
    })
    if (depth > 0) findings.push({ line: lines.length, severity: 'warn', message: `Unbalanced ${label}s: ${depth} '${open}' never closed (truncated file?).` })
    else if (depth < 0) findings.push({ line: firstUnmatchedClose || 1, severity: 'warn', message: `Unbalanced ${label}s: ${-depth} extra '${close}'.` })
  }

  // ── At least one recognized top-level block ──
  const hasTopLevel = scrubbed
    .split('\n')
    .some(l => { const t = l.trim(); return TOP_LEVEL.some(k => t.startsWith(k + ' ') || t === k || t.startsWith(k + '{')) })
  if (!hasTopLevel) {
    findings.push({ line: 1, severity: 'warn', message: `No top-level block found — a .gui file must declare one of: ${TOP_LEVEL.join(', ')}.` })
  }

  return findings
}
