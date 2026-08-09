// ───────────────────────────────────────────────────────────────────────────
// segment.ts — strip → frame segmentation (port of segment.go).
//
// Not connected-components: a vertical alpha projection profile counts natural
// poses by the gutters (valleys) between them, and when poses touch (no gutter)
// a dynamic program finds the `expected-1` cuts that slice through the least
// content. This is OCR's projection-profile + optimal-cut technique.
// ───────────────────────────────────────────────────────────────────────────
import { type Img, idx } from './image'

/** A column span [start, end) in strip coordinates. */
export interface ColSpan {
  start: number
  end: number
}

/** projectAlpha — per-column alpha mass P[x] = Σ_y α(x,y). */
export function projectAlpha(img: Img): number[] {
  const w = img.width, h = img.height
  const p = new Array<number>(w).fill(0)
  const data = img.data
  for (let x = 0; x < w; x++) {
    let sum = 0
    for (let y = 0; y < h; y++) sum += data[idx(w, x, y) + 3]
    p[x] = sum
  }
  return p
}

/** smoothProfile — box moving-average (suppresses compression noise / thin gaps). */
export function smoothProfile(p: number[], win: number): number[] {
  if (win < 1 || p.length === 0) return p
  const out = new Array<number>(p.length)
  const half = Math.floor(win / 2)
  for (let i = 0; i < p.length; i++) {
    let sum = 0, n = 0
    for (let j = i - half; j <= i + half; j++) {
      if (j >= 0 && j < p.length) { sum += p[j]; n++ }
    }
    out[i] = sum / n
  }
  return out
}

function maxOf(p: number[]): number {
  let m = 0
  for (const v of p) if (v > m) m = v
  return m
}

/**
 * contentRuns — continuous runs where P > eps (poses). Runs narrower than minW
 * or whose peak is below peakMin are dropped as grit.
 */
export function contentRuns(p: number[], eps: number, peakMin: number, minW: number): ColSpan[] {
  const runs: ColSpan[] = []
  let i = 0
  const n = p.length
  while (i < n) {
    if (p[i] <= eps) { i++; continue }
    let j = i
    let peak = 0
    for (; j < n && p[j] > eps; j++) if (p[j] > peak) peak = p[j]
    if (j - i >= minW && peak >= peakMin) runs.push({ start: i, end: j })
    i = j
  }
  return runs
}

function runMass(p: number[], s: ColSpan): number {
  let m = 0
  for (let x = s.start; x < s.end && x < p.length; x++) m += p[x]
  return m
}

/** dropMinorRuns — drop runs whose mass is below `frac` of the largest run. */
function dropMinorRuns(p: number[], runs: ColSpan[], frac: number): ColSpan[] {
  if (runs.length <= 1) return runs
  let maxM = 0
  for (const r of runs) { const m = runMass(p, r); if (m > maxM) maxM = m }
  const thr = maxM * frac
  return runs.filter((r) => runMass(p, r) >= thr)
}

/**
 * dpNCut — find the n-1 cut columns that split [x0,x1) into exactly n segments.
 * cost = Σ P[cut] (cheaper to cut low-mass columns) + λ·(width − ideal)².
 */
export function dpNCut(p: number[], x0: number, x1: number, n: number): number[] | null {
  if (n <= 1 || x1 - x0 < n) return null
  const width = x1 - x0
  const ideal = width / n
  let minW = Math.trunc(ideal * 0.45)
  if (minW < 2) minW = 2
  const lambda = 0.0015 // width-penalty weight (relative to mass cost)

  const cuts = n - 1
  const INF = 1e18
  const cost: Float64Array[] = []
  const prev: Int32Array[] = []
  for (let k = 0; k <= cuts; k++) {
    const c = new Float64Array(x1 + 1).fill(INF)
    const pr = new Int32Array(x1 + 1).fill(-1)
    cost.push(c)
    prev.push(pr)
  }
  cost[0][x0] = 0 // virtual start boundary
  for (let k = 1; k <= cuts; k++) {
    const lo = x0 + (k - 1) * minW
    for (let x = x0 + k * minW; x <= x1 - (cuts - k + 1) * minW; x++) {
      let best = INF
      let bestPrev = -1
      for (let xp = lo; xp <= x - minW; xp++) {
        if (cost[k - 1][xp] >= 1e17) continue
        const d = (x - xp) - ideal
        const c = cost[k - 1][xp] + p[x] + lambda * d * d
        if (c < best) { best = c; bestPrev = xp }
      }
      cost[k][x] = best
      prev[k][x] = bestPrev
    }
  }
  let bestEnd = -1, bestCost = INF
  for (let x = x0 + cuts * minW; x <= x1 - minW; x++) {
    const d = (x1 - x) - ideal
    const c = cost[cuts][x] + lambda * d * d
    if (c < bestCost) { bestCost = c; bestEnd = x }
  }
  if (bestEnd < 0) return null
  const out = new Array<number>(cuts)
  let x = bestEnd
  for (let k = cuts; k >= 1; k--) {
    out[k - 1] = x
    x = prev[k][x]
    if (x < 0) return null
  }
  return out
}

/** medianRunWidth — median run width (estimate of a typical single-pose width). */
function medianRunWidth(runs: ColSpan[]): number {
  if (runs.length === 0) return 0
  const ws = runs.map((r) => r.end - r.start).sort((a, b) => a - b)
  return ws[Math.floor(ws.length / 2)]
}

/**
 * posePeaks — strong prominence-based peaks (poses) in [s,e). A candidate is a
 * local max ≥ 45% of the run max; it survives only if the valley between it and
 * any taller peak drops below 62% of its own height (else it's part of one pose).
 */
export function posePeaks(p: number[], s: number, e: number): number[] {
  if (e - s < 3) return [Math.floor((s + e) / 2)]
  let runMax = 0
  for (let x = s; x < e; x++) if (p[x] > runMax) runMax = p[x]
  if (runMax <= 0) return [Math.floor((s + e) / 2)]
  const cand: number[] = []
  for (let x = s + 1; x < e - 1; x++) {
    if (p[x] >= p[x - 1] && p[x] > p[x + 1] && p[x] >= 0.45 * runMax) cand.push(x)
  }
  if (cand.length === 0) return [Math.floor((s + e) / 2)]
  const keep: number[] = []
  for (const m of cand) {
    let prominent = true
    for (const k of cand) {
      if (k === m || p[k] < p[m]) continue // only check valleys vs taller peaks
      let lo = m, hi = k
      if (lo > hi) { const t = lo; lo = hi; hi = t }
      let vmin = p[lo]
      for (let x = lo; x <= hi; x++) if (p[x] < vmin) vmin = p[x]
      if (vmin > 0.62 * p[m]) { prominent = false; break } // shallow valley → same pose
    }
    if (prominent) keep.push(m)
  }
  if (keep.length === 0) return [cand[0]]
  return keep
}

/** splitRange — split [s,e) into n segments via DP min-cut (falls back to even). */
export function splitRange(p: number[], s: number, e: number, n: number): ColSpan[] {
  if (n <= 1 || e - s < n) return [{ start: s, end: e }]
  const cuts = dpNCut(p, s, e, n)
  if (cuts && cuts.length === n - 1) {
    const out: ColSpan[] = []
    let prev = s
    for (const c of cuts) { out.push({ start: prev, end: c }); prev = c }
    out.push({ start: prev, end: e })
    return out
  }
  const out: ColSpan[] = []
  for (let i = 0; i < n; i++) {
    out.push({ start: s + Math.trunc(((e - s) * i) / n), end: s + Math.trunc(((e - s) * (i + 1)) / n) })
  }
  return out
}

/**
 * segmentStrip — split a strip into `expected` column segments and report the
 * detected natural pose count. If the natural count equals expected we cut at the
 * gutters; otherwise DP forces exactly `expected`.
 */
export function segmentStrip(img: Img, expected: number): { segs: ColSpan[]; natural: number } {
  const w = img.width
  if (w === 0 || expected < 1) return { segs: [], natural: 0 }
  const raw = projectAlpha(img)
  let win = Math.floor(w / 220)
  if (win < 3) win = 3
  const p = smoothProfile(raw, win)
  const mx = maxOf(p)
  if (mx <= 0) return { segs: [], natural: 0 }
  const eps = 0.045 * mx
  const peakMin = 0.18 * mx
  let minRun = Math.floor(w / 100)
  if (minRun < 4) minRun = 4
  let runs = contentRuns(p, eps, peakMin, minRun)
  runs = dropMinorRuns(p, runs, 0.2)
  if (runs.length === 0) return { segs: [], natural: 0 }

  // Per run, estimate pose count by torso-prominence peaks, capped by run width:
  // peaks decide "where to cut", width decides "how many". A kick whose torso +
  // extended leg makes two peaks but whose run width is one pose wide stays 1
  // (prevents over-splitting); only touched-wider runs split that much.
  const med = medianRunWidth(runs)
  let widthTotal = 0
  for (const r of runs) widthTotal += r.end - r.start
  const segs: ColSpan[] = []
  for (const r of runs) {
    let nPeaks = posePeaks(p, r.start, r.end).length
    if (runs.length > 1 && med > 0) {
      let maxByWidth = Math.round((r.end - r.start) / med)
      if (maxByWidth < 1) maxByWidth = 1
      if (nPeaks > maxByWidth) nPeaks = maxByWidth
    }
    // Overlapping poses with almost no gap give a single peak, but a run wider
    // than 1.45× the average pose width is suspected to hold 2.
    if (nPeaks === 1 && runs.length > 1 && med > 0) {
      if (r.end - r.start > med * 1.45) nPeaks = 2
    }
    if (nPeaks <= 1) segs.push(r)
    else segs.push(...splitRange(p, r.start, r.end, nPeaks))
  }

  // Forced recovery: if the detected count differs from expected and the total
  // content width can carry the minimum width for `expected`, split the whole
  // strip into `expected` (defends against the AI drawing poses with no gutter).
  if (segs.length !== expected && widthTotal / expected >= 16 && Math.floor(w / expected) >= 16) {
    const forced = splitRange(p, 0, w, expected)
    return { segs: forced, natural: forced.length }
  }

  // Emitted frame count = estimated pose count (honest report; DP did not force).
  return { segs, natural: segs.length }
}
