// ───────────────────────────────────────────────────────────────────────────
// score.ts — 0–1 quality scoring of a frame set (port of score.go).
//   overall = 0.5·identity + 0.3·motion + 0.2·contact
// ───────────────────────────────────────────────────────────────────────────
import { type Img, alphaThreshold, idx } from './image'
import { motionPresence } from './inspect'

export interface ScoreResult {
  identity: number // mean perceptual similarity between adjacent frames (0–1)
  motion: number // MotionPresence 0–1
  contact: number // baseline/edge consistency 0–1
  overall: number // 0–1 composite
}

export function emptyScore(): ScoreResult {
  return { identity: 0, motion: 0, contact: 0, overall: 0 }
}

/** scoreFrames — completeness score of a frame set. */
export function scoreFrames(frames: Img[]): ScoreResult {
  const r = emptyScore()
  if (frames.length < 2) return r
  r.motion = motionPresence(frames)
  r.identity = pairwiseIdentity(frames)
  r.contact = contactScore(frames)
  r.overall = 0.5 * r.identity + 0.3 * r.motion + 0.2 * r.contact
  return r
}

/** pairwiseIdentity — weighted color/alpha similarity between adjacent frames. */
function pairwiseIdentity(frames: Img[]): number {
  let total = 0
  let pairs = 0
  for (let i = 1; i < frames.length; i++) {
    const a = frames[i - 1], b = frames[i]
    if (a.width !== b.width || a.height !== b.height) continue
    let diff = 0
    let n = 0
    const ad = a.data, bd = b.data
    for (let p = 0; p + 3 < ad.length && p + 3 < bd.length; p += 4) {
      const dr = ad[p] - bd[p]
      const dg = ad[p + 1] - bd[p + 1]
      const db = ad[p + 2] - bd[p + 2]
      const da = ad[p + 3] - bd[p + 3]
      let d = Math.sqrt(0.299 * dr * dr + 0.587 * dg * dg + 0.114 * db * db)
      d += 0.5 * Math.abs(da)
      if (ad[p + 3] > alphaThreshold || bd[p + 3] > alphaThreshold) {
        diff += Math.min(d / (255.0 * 1.5), 1.0)
        n++
      }
    }
    if (n > 0) { total += 1.0 - diff / n; pairs++ }
  }
  return pairs === 0 ? 0 : total / pairs
}

/** contactScore — vertical consistency of baseline/top contact across frames. */
function contactScore(frames: Img[]): number {
  interface B { top: number; bottom: number; h: number; has: boolean }
  const bbs: B[] = []
  for (const f of frames) {
    const w = f.width, h = f.height
    let top = -1, bottom = -1
    for (let y = 0; y < h; y++) {
      let rowOpaque = false
      for (let x = 0; x < w; x++) {
        if (f.data[idx(w, x, y) + 3] > alphaThreshold) { rowOpaque = true; break }
      }
      if (rowOpaque) { if (top < 0) top = y; bottom = y }
    }
    bbs.push({ top, bottom, h, has: top >= 0 })
  }
  let n = 0
  let meanBottom = 0, meanTop = 0, maxH = 1
  for (const b of bbs) {
    if (b.h > maxH) maxH = b.h
    if (b.has) { meanBottom += b.bottom; meanTop += b.top; n++ }
  }
  if (n === 0) return 0
  meanBottom /= n
  meanTop /= n
  let bottomVar = 0, topVar = 0
  for (const b of bbs) {
    if (b.has) { bottomVar += Math.abs(b.bottom - meanBottom); topVar += Math.abs(b.top - meanTop) }
  }
  const bottomMAE = bottomVar / n
  const topMAE = topVar / n
  // Tolerance vs height: top (head) may move 28%, bottom (feet) 10%.
  const tolBottom = Math.max(maxH * 0.1, 2.0)
  const tolTop = Math.max(maxH * 0.28, 2.0)
  const bottomScore = 1.0 - Math.min(bottomMAE / tolBottom, 1.0)
  const topScore = 1.0 - Math.min(topMAE / tolTop, 1.0)
  return 0.75 * bottomScore + 0.25 * topScore
}
