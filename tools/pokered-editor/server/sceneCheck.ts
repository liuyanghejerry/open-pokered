// ───────────────────────────────────────────────────────────────────────────
// sceneCheck — verify a DRAFT `.scene` buffer WITHOUT committing it, so the chat
// agent can close the loop: draft → check → fix → re-check → propose only once
// it passes (instead of proposing DSL blind and letting the human discover the
// compile error on apply).
//
// If the story activity configures `scene.checkCmd` (or legacy `scene.validateCmd`),
// the draft is written to a temp file and the command runs against it, giving the
// agent a REAL compiler's output. Otherwise it falls back to the deterministic
// lint (dangling flags / hallucinated game.* APIs) — clearly labeled so the agent
// (and the reader) know it is a lint, not a full compile.
// ───────────────────────────────────────────────────────────────────────────
import fs from 'fs'
import os from 'os'
import path from 'path'
import type { ProjectContext } from './context/projectContext'
import { lintScene } from './sceneLint'

export interface SceneCheckResult {
  ok: boolean
  /** 'compile' when a project check command ran; 'lint' for the built-in fallback. */
  source: 'compile' | 'lint'
  /** Compiler output or lint report, model/human-readable. */
  output: string
}

const CAP = 9000

/**
 * Run the project's scene check against a draft buffer, or lint if none is
 * configured. Never mutates project files — the draft goes to a temp file that
 * is removed afterward.
 */
export async function checkScene(project: ProjectContext, sceneName: string, content: string): Promise<SceneCheckResult> {
  const sc = (project.storyConfig() ?? {}) as any
  const cmdTmpl: string | undefined = sc.scene?.checkCmd ?? sc.scene?.validateCmd
  if (cmdTmpl) {
    const ext: string = sc.scene?.ext ?? '.scene'
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-scenecheck-'))
    const file = path.join(dir, `draft${ext}`)
    try {
      fs.writeFileSync(file, content, 'utf-8')
      const cmd = String(cmdTmpl).replace(/\{file\}/g, file).replace(/\{scene\}/g, sceneName || 'draft')
      const { execSync } = await import('child_process')
      try {
        const out = execSync(cmd, { cwd: project.root, encoding: 'utf-8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 180000 })
        return { ok: true, source: 'compile', output: (out || '').trim().slice(0, CAP) || 'OK: scene compiles.' }
      } catch (e: any) {
        const out = (String(e.stdout || '') + String(e.stderr || '') + (e.message ? '\n' + e.message : '')).trim()
        return { ok: false, source: 'compile', output: out.slice(0, CAP) || 'Scene failed to compile.' }
      }
    } finally {
      try { fs.rmSync(dir, { recursive: true, force: true }) } catch { /* ignore */ }
    }
  }

  // Fallback: deterministic lint (no compiler configured for this project).
  const findings = lintScene(project, content)
  const hasErr = findings.some(f => f.severity === 'warn')
  const report = findings.length
    ? findings.map(f => `[${f.severity}] line ${f.line}: ${f.message}`).join('\n')
    : 'OK: no flag/API issues found.'
  return {
    ok: !hasErr,
    source: 'lint',
    output: (report + '\n(note: lint only — no scene.checkCmd configured, so this is NOT a full compile.)').slice(0, CAP),
  }
}
