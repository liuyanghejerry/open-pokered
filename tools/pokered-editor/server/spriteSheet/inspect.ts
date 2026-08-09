// ───────────────────────────────────────────────────────────────────────────
// inspect.ts — frame quality inspection (port of inspect.go).
//
// Measures each frame (content size, edge clipping, residual key chroma) and the
// set (size consistency, identity drift via a 64-bin RGB histogram, base-character
// drift). Defects become precise English RETRY HINTS injected into the next
// generation prompt — the engine of the self-correcting loop. Runs BEFORE
// quantization so palette reduction doesn't dull drift detection.
// ───────────────────────────────────────────────────────────────────────────
import { type Img, alphaThreshold, idx } from './image'
import { colorDist } from './chroma'
import { type ScoreResult, emptyScore, scoreFrames } from './score'

// Quality-inspection parameters.
const inspectEdgeMargin = 2 // edge band width (px)
const inspectEdgeMax = 24 // more edge pixels than this risks clipping
const inspectKeyDist = 70.0 // within this distance of the key → residual-chroma candidate
const inspectKeyMax = 120 // residual-chroma pixel allowance
const inspectSmallRatio = 0.35 // below this ratio of the median → abnormally small frame
const inspectLargeRatio = 2.75 // above this ratio of the median → abnormally large frame
const inspectMinContentAbs = 400 // minimum content pixels per frame (absolute)
const inspectContentMinAlpha = 0.25 // below this fraction of the mean → much sparser frame
const driftWarnSim = 0.65 // color-composition similarity below this → drift warning
const driftErrorSim = 0.45 // below this → severe drift, regenerate
const baseWarnSim = 0.6 // mean similarity vs base character warning
const baseErrorSim = 0.4 // below this → all frames are a different character

const histBins = 64 // 4×4×4 RGB quantization bins

/** keyTinted — does the pixel carry the background key tint (residual chroma/halo)? */
export function keyTinted(r: number, g: number, b: number, key: [number, number, number]): boolean {
  const px = [r, g, b]
  for (let c = 0; c < 3; c++) {
    if (key[c] > 192) {
      if (px[c] <= 150) return false
    } else if (key[c] < 64) {
      if (px[c] >= 110) return false
    }
  }
  return true
}

export interface FrameReport {
  index: number
  contentPixels: number
  edgePixels: number
  keyResidue: number
  paletteSim: number // color-composition similarity vs other frames (0–1)
}

export interface InspectResult {
  reports: FrameReport[]
  errors: string[] // serious problems needing regeneration (user-facing)
  warnings: string[] // advisory warnings (user-facing)
  retryHints: string[] // correction instructions to inject into the regen prompt (English)
  score: ScoreResult
  ok: boolean // no errors
}

function absDiff(a: number, b: number): number {
  return a > b ? a - b : b - a
}

/** colorHistogram — normalized coarse RGB histogram of opaque pixels. */
function colorHistogram(f: Img): Float64Array {
  const hist = new Float64Array(histBins)
  let total = 0
  const d = f.data
  for (let i = 0; i + 3 < d.length; i += 4) {
    if (d[i + 3] <= alphaThreshold) continue
    const bin = ((d[i] >> 6) << 4) | ((d[i + 1] >> 6) << 2) | (d[i + 2] >> 6)
    hist[bin]++
    total++
  }
  if (total > 0) for (let k = 0; k < histBins; k++) hist[k] /= total
  return hist
}

/** hasTransparency — does the image have a meaningful transparent region (≥5%)? */
export function hasTransparency(img: Img): boolean {
  let total = 0, transparent = 0
  const d = img.data
  for (let i = 3; i < d.length; i += 4) {
    total++
    if (d[i] <= alphaThreshold) transparent++
  }
  return total > 0 && transparent / total >= 0.05
}

/**
 * motionPresence — mean inter-frame change rate (0–1). Near 0 means an effectively
 * static "animation" (the opposite defect from identity drift).
 */
export function motionPresence(frames: Img[]): number {
  if (frames.length < 2) return 0
  let total = 0
  let pairs = 0
  for (let i = 1; i < frames.length; i++) {
    const a = frames[i - 1], b = frames[i]
    if (a.width !== b.width || a.height !== b.height) continue
    let diffSum = 0
    let count = 0
    const ad = a.data, bd = b.data
    for (let p = 0; p + 3 < ad.length && p + 3 < bd.length; p += 4) {
      const aa = ad[p + 3], ba = bd[p + 3]
      if (aa <= alphaThreshold && ba <= alphaThreshold) continue
      const d = absDiff(ad[p], bd[p]) + absDiff(ad[p + 1], bd[p + 1]) +
        absDiff(ad[p + 2], bd[p + 2]) + absDiff(aa, ba)
      diffSum += d / (255.0 * 4.0)
      count++
    }
    if (count > 0) { total += diffSum / count; pairs++ }
  }
  return pairs === 0 ? 0 : total / pairs
}

/**
 * inspectFrames — inspect extracted frames. `key` is the chroma background color
 * (for residual-chroma detection). If `base` is given, also checks identity
 * against the base character (catches batch drift the leave-one-out check misses).
 */
export function inspectFrames(frames: Img[], key: [number, number, number], base: Img | null): InspectResult {
  const res: InspectResult = { reports: [], errors: [], warnings: [], retryHints: [], score: emptyScore(), ok: true }
  if (frames.length === 0) return res

  const hintSet = new Set<string>()
  const addHint = (h: string) => { if (!hintSet.has(h)) { hintSet.add(h); res.retryHints.push(h) } }

  const areas: number[] = []
  let opaqueTotal = 0
  for (const f of frames) opaqueTotal += f.width * f.height
  const contentAlphaCutoff = Math.trunc(Math.floor(opaqueTotal / frames.length) * inspectContentMinAlpha)
  for (let i = 0; i < frames.length; i++) {
    const f = frames[i]
    const rep: FrameReport = { index: i, contentPixels: 0, edgePixels: 0, keyResidue: 0, paletteSim: 0 }
    const w = f.width, h = f.height
    const d = f.data
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const pi = idx(w, x, y)
        if (d[pi + 3] <= alphaThreshold) continue
        rep.contentPixels++
        if (x < inspectEdgeMargin || x >= w - inspectEdgeMargin || y < inspectEdgeMargin || y >= h - inspectEdgeMargin) {
          rep.edgePixels++
        }
        const pr = d[pi], pg = d[pi + 1], pb = d[pi + 2]
        if (colorDist(pr, pg, pb, key) <= inspectKeyDist && keyTinted(pr, pg, pb, key)) rep.keyResidue++
      }
    }

    let minContent = inspectMinContentAbs
    const rel = Math.trunc((w * h) / 100)
    if (rel > minContent) minContent = rel
    if (rep.contentPixels < minContent) {
      res.errors.push(`Frame ${i + 1} is empty or too faint (${rep.contentPixels}px).`)
      addHint('Every column must hold one complete, fully drawn full-body character. Leave no column empty or faint.')
    }
    if (rep.edgePixels > inspectEdgeMax) {
      res.warnings.push(`Frame ${i + 1} touches the edge and may be clipped (${rep.edgePixels}px).`)
      addHint('Keep every pose fully inside its column with clear padding on all sides; no body part may touch or cross a column edge.')
    }
    if (rep.keyResidue > inspectKeyMax) {
      res.errors.push(`Frame ${i + 1} has leftover background-key residue (${rep.keyResidue}px).`)
      addHint('The character must not contain magenta or magenta-adjacent colors anywhere (clothes, effects, highlights). Keep the background a perfectly flat pure magenta #FF00FF and keep all character colors far from magenta.')
    }
    if (contentAlphaCutoff > 0 && rep.contentPixels < contentAlphaCutoff) {
      res.warnings.push(`Frame ${i + 1} has noticeably less content than the others (${Math.trunc((1 - rep.contentPixels / contentAlphaCutoff) * 100)}% off).`)
      addHint('Draw the character at a consistent size across the strip; no pose may be much smaller or partially erased.')
    }

    res.reports.push(rep)
    areas.push(rep.contentPixels)
  }

  // Size consistency: detect under/oversized frames vs the median.
  if (areas.length >= 3) {
    const sorted = [...areas].sort((a, b) => a - b)
    const median = sorted[Math.floor(sorted.length / 2)]
    if (median > 0) {
      for (let i = 0; i < areas.length; i++) {
        const ratio = areas[i] / median
        if (ratio < inspectSmallRatio) {
          res.warnings.push(`Frame ${i + 1} is abnormally smaller than the others.`)
          addHint('Draw the character at the same scale in every frame; no pose may be much smaller or larger than the others.')
        } else if (ratio > inspectLargeRatio) {
          res.warnings.push(`Frame ${i + 1} is abnormally larger than the others (poses may have merged).`)
          addHint('Each pose must be completely separate with clear magenta gaps between poses; poses must never touch, overlap, or merge.')
        }
      }
    }
  }

  // Identity drift: pose-independent color composition (histogram) differing a lot
  // across frames means the AI redrew the character's identity (hair/outfit).
  if (frames.length >= 2) {
    const hists = frames.map((f) => colorHistogram(f))
    for (let i = 0; i < frames.length; i++) {
      const avg = new Float64Array(histBins)
      for (let j = 0; j < frames.length; j++) {
        if (j === i) continue
        for (let k = 0; k < histBins; k++) avg[k] += hists[j][k]
      }
      const n = frames.length - 1
      let sim = 0
      for (let k = 0; k < histBins; k++) sim += Math.min(hists[i][k], avg[k] / n)
      res.reports[i].paletteSim = sim
      if (sim < driftErrorSim) {
        res.errors.push(`Frame ${i + 1}'s color composition differs greatly from the others (character drift suspected, ${Math.round(sim * 100)}% similar).`)
        addHint('CRITICAL: keep the exact same character identity in every frame — identical hair color, skin tone, outfit colors and proportions. Only the pose may change between frames.')
      } else if (sim < driftWarnSim) {
        res.warnings.push(`Frame ${i + 1}'s color composition differs somewhat from the others (${Math.round(sim * 100)}% similar).`)
        addHint("Keep the character's colors and details consistent across all frames; do not change hair, skin or outfit colors between poses.")
      }
    }
  }

  // Base-character identity: mean frame composition far from the base means the
  // whole strip drew a different character (batch drift).
  if (base && frames.length > 0 && hasTransparency(base)) {
    const baseHist = colorHistogram(base)
    let totalSim = 0
    for (const f of frames) {
      const hh = colorHistogram(f)
      let sim = 0
      for (let k = 0; k < histBins; k++) sim += Math.min(hh[k], baseHist[k])
      totalSim += sim
    }
    const avg = totalSim / frames.length
    if (avg < baseErrorSim) {
      res.errors.push(`The generated frames differ greatly from the base character (${Math.round(avg * 100)}% similar).`)
      res.retryHints.push("CRITICAL: the previous attempt drew a different-looking character. Copy the attached reference image's identity exactly — identical hair color, skin tone, outfit colors, proportions and accessories in every frame.")
    } else if (avg < baseWarnSim) {
      res.warnings.push(`The generated frames' color composition differs somewhat from the base character (${Math.round(avg * 100)}% similar).`)
    }
  }

  res.score = scoreFrames(frames)
  res.ok = res.errors.length === 0
  return res
}
