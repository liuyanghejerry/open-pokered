// ───────────────────────────────────────────────────────────────────────────
// pipeline.ts — the self-correcting generation loop (port of app.go GenerateState).
//
// generate prompt → AI filmstrip → matte → extract → inspect → (pass: quantize,
// done | fail: measurement-driven retry hint → regenerate, up to 3×). Keeps the
// best-scored candidate (score = found*100 − errors*10) and never returns empty.
//
// The AI call and progress emission are injected as callbacks so the server layer
// (Phase 4) can wire the Vercel AI SDK + SSE without this module knowing about
// providers, HTTP, or the filesystem.
// ───────────────────────────────────────────────────────────────────────────
import type { Img } from './image'
import { detectBackground, removeBackground } from './chroma'
import { extractFrames } from './extract'
import { inspectFrames, motionPresence } from './inspect'
import { paletteSizeForStyle, pixelPostProcess } from './pixelize'
import { type ScoreResult, emptyScore, scoreFrames } from './score'
import { aspectForFrames, buildStripPrompt, resolveStyle } from './prompt'
import { isBackFacing } from './direction'
import type { StateSpec } from './types'

export interface GenerateStateArgs {
  description: string
  styleKey: string
  styleCustom?: string
  state: StateSpec
  base: Img | null // base character image (identity reference)
  cellSize?: number
  margin?: number
  feedback?: string
}

/** The AI call: prompt + aspect + reference images → the raw filmstrip image. */
export type GenerateImageFn = (prompt: string, aspect: string, refs: Img[]) => Promise<Img>

export interface ProgressEvent {
  phase: 'generate' | 'extract'
  state: string
  message: string
  attempt: number
  maxAttempts: number
}

export interface StateResult {
  name: string
  frames: Img[]
  rawStrip: Img | null
  expected: number
  found: number
  warnings: string[]
  scores: ScoreResult
}

export interface GenerateStateOptions {
  onProgress?: (e: ProgressEvent) => void
  signal?: AbortSignal
  /** Reference images (base + optional front-view motion strip). Defaults to [base]. */
  refs?: Img[]
  maxAttempts?: number
}

const MAX_ATTEMPTS = 3

/** generateState — generate one animation state's strip and extract its frames. */
export async function generateState(
  args: GenerateStateArgs,
  genImage: GenerateImageFn,
  opts: GenerateStateOptions = {},
): Promise<StateResult> {
  const expected = args.state.frames
  if (expected < 1 || expected > 10) throw new Error('Frame count must be between 1 and 10.')

  const cellSize = args.cellSize && args.cellSize > 0 ? args.cellSize : 256
  const margin = args.margin && args.margin > 0 ? args.margin : Math.max(8, Math.trunc(cellSize / 12))
  const style = resolveStyle(args.styleKey, args.styleCustom ?? '')
  const aspect = aspectForFrames(expected)
  const maxAttempts = opts.maxAttempts ?? MAX_ATTEMPTS
  const emit = (phase: ProgressEvent['phase'], message: string, attempt: number) =>
    opts.onProgress?.({ phase, state: args.state.name, message, attempt, maxAttempts })

  // Base for identity check — skipped for back-facing directions (a front-view
  // base would false-positive). inspectFrames further gates on transparency.
  const baseN = isBackFacing(args.state.facing) ? null : args.base

  // Generation reference images: base + optional front-view motion strip.
  const refs = opts.refs && opts.refs.length ? opts.refs : args.base ? [args.base] : []
  const hasRefStrip = refs.length > 1

  let feedback = args.feedback ?? ''
  let best: StateResult | null = null
  let bestImgs: Img[] = []
  let bestScore = -1e30
  let lastErr: Error | null = null

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    if (opts.signal?.aborted) throw new Error('Generation cancelled.')
    let prompt = buildStripPrompt(args.description, style, args.state, feedback)
    if (hasRefStrip) {
      prompt += '\nMotion reference: the second attached image is the FRONT-view animation strip of this same character performing this exact action. Reproduce the same motion timing and pose phases frame by frame, but viewed from the required facing direction above.\n'
    }

    emit('generate', attempt > 1 ? `Regenerating to fix frame count… (${attempt}/${maxAttempts})` : 'Generating AI frames…', attempt)

    let strip: Img
    try {
      strip = await genImage(prompt, aspect, refs)
    } catch (e) {
      lastErr = e instanceof Error ? e : new Error(String(e))
      break // API error/cancel — retrying is pointless (the client retries itself)
    }

    emit('extract', 'Removing background and extracting frames…', attempt)
    const bgKey = detectBackground(strip)
    const clean = removeBackground(strip)

    const cand: StateResult = { name: args.state.name, frames: [], rawStrip: clean, expected, found: 0, warnings: [], scores: emptyScore() }
    const extracted = extractFrames(clean, expected, cellSize, cellSize, margin)
    // Inspect BEFORE quantization (palette reduction would dull drift detection).
    const insp = inspectFrames(extracted.frames, bgKey, baseN)
    pixelPostProcess(extracted.frames, paletteSizeForStyle(args.styleKey))
    cand.found = extracted.found
    cand.warnings = [...extracted.warnings, ...insp.errors, ...insp.warnings]
    cand.frames = extracted.frames
    cand.scores = scoreFrames(extracted.frames)
    if (cand.found >= 2 && motionPresence(extracted.frames) < 0.01) {
      cand.warnings.push('There is almost no movement between frames. Consider strengthening the action description and regenerating so the motion reads clearly.')
    }
    const errCount = insp.errors.length

    // Exact frame count + no serious quality issue → immediate success.
    if (cand.found === expected && insp.ok) return cand

    // Update best candidate: frame count first, then fewer errors.
    const score = cand.found * 100 - errCount * 10
    if (score > bestScore) { best = cand; bestScore = score; bestImgs = extracted.frames }
    lastErr = null

    // Correction feedback for the next attempt (keep user feedback + measured fixes).
    const fixes: string[] = []
    if (cand.found !== expected) {
      fixes.push(`IMPORTANT CORRECTION: the last attempt read as ${cand.found} poses but EXACTLY ${expected} are required. Redraw as one horizontal row of ${expected} equally sized poses, one clearly separated pose per column, each ringed by a clean magenta gap so none touch or overlap. Do not draw any frame, border, or film strip.`)
    }
    if (insp.retryHints.length > 0) {
      fixes.push('QUALITY CORRECTIONS detected by automated inspection (fix all of these):')
      fixes.push(...insp.retryHints)
    }
    const auto = fixes.join('\n')
    feedback = args.feedback ? `${args.feedback}\n${auto}` : auto
  }

  if (!best || best.found === 0) {
    if (lastErr) throw lastErr
    throw new Error('Could not extract sprite frames. Try a more specific character description.')
  }
  if (best.found !== expected) {
    best.warnings.push(`Frame count still differs after automatic retries (requested ${expected} → extracted ${best.found}). Showing the closest result.`)
  } else {
    best.warnings.push('Some quality issues remain after automatic retries. Review the frames and regenerate with feedback if needed.')
  }
  best.frames = bestImgs
  return best
}
