// ───────────────────────────────────────────────────────────────────────────
// quantize.ts — shared-palette median-cut quantization (port of quantize.go).
//
// A SINGLE palette is extracted across ALL frames of a state (per-frame palettes
// would flicker), via median-cut with a perceptually-weighted color distance.
// Near colors are merged so cross-frame micro-drift collapses to one color.
// ───────────────────────────────────────────────────────────────────────────
import { type Img, alphaThreshold } from './image'

export interface RGB { r: number; g: number; b: number }

function packRGB(r: number, g: number, b: number): number {
  return (r << 16) | (g << 8) | b
}

/** Perceptually-weighted squared color distance (green-sensitive). */
export function colorDist2(a: RGB, b: RGB): number {
  const dr = a.r - b.r, dg = a.g - b.g, db = a.b - b.b
  return 2 * dr * dr + 4 * dg * dg + 3 * db * db
}

/** collectOpaque — gather opaque pixel colors across frames (downsampled). */
function collectOpaque(frames: Img[], maxSamples: number): RGB[] {
  let total = 0
  for (const f of frames) total += f.data.length / 4
  let step = 1
  if (maxSamples > 0 && total > maxSamples) step = Math.trunc(total / maxSamples) + 1
  const out: RGB[] = []
  let idx = 0
  for (const f of frames) {
    const d = f.data
    for (let i = 0; i + 3 < d.length; i += 4) {
      if (d[i + 3] <= alphaThreshold) continue
      if (idx % step === 0) out.push({ r: d[i], g: d[i + 1], b: d[i + 2] })
      idx++
    }
  }
  return out
}

/**
 * buildSharedPalette — median-cut palette across all frames. Forcing one palette
 * on every frame greatly improves cross-frame color consistency.
 */
export function buildSharedPalette(frames: Img[], maxColors: number): RGB[] | null {
  if (maxColors < 2) maxColors = 2
  const samples = collectOpaque(frames, 1 << 16)
  if (samples.length === 0) return null
  let buckets: RGB[][] = [samples]
  while (buckets.length < maxColors) {
    // pick the bucket with the largest channel range
    let bestIdx = -1, bestRange = 0, bestCh = 0
    for (let bi = 0; bi < buckets.length; bi++) {
      const b = buckets[bi]
      if (b.length < 2) continue
      const minC = [255, 255, 255]
      const maxC = [0, 0, 0]
      for (const c of b) {
        const ch = [c.r, c.g, c.b]
        for (let k = 0; k < 3; k++) {
          if (ch[k] < minC[k]) minC[k] = ch[k]
          if (ch[k] > maxC[k]) maxC[k] = ch[k]
        }
      }
      for (let k = 0; k < 3; k++) {
        const r = maxC[k] - minC[k]
        if (r > bestRange) { bestIdx = bi; bestRange = r; bestCh = k }
      }
    }
    if (bestIdx < 0 || bestRange === 0) break // can't split further
    const b = buckets[bestIdx]
    b.sort((x, y) => (bestCh === 0 ? x.r - y.r : bestCh === 1 ? x.g - y.g : x.b - y.b))
    const mid = Math.floor(b.length / 2)
    buckets[bestIdx] = b.slice(0, mid)
    buckets.push(b.slice(mid))
  }

  interface Entry { c: RGB; n: number }
  let entries: Entry[] = []
  for (const b of buckets) {
    if (b.length === 0) continue
    let sr = 0, sg = 0, sb = 0
    for (const c of b) { sr += c.r; sg += c.g; sb += c.b }
    const n = b.length
    entries.push({ c: { r: Math.trunc(sr / n), g: Math.trunc(sg / n), b: Math.trunc(sb / n) }, n })
  }

  // Near-color merge: collapse cross-frame micro-drift (≤ ~8 per channel).
  const mergeThresh = 600 // colorDist2 units ≈ 8/channel
  entries.sort((a, b) => b.n - a.n)
  const merged: Entry[] = []
  for (const e of entries) {
    let absorbed = false
    for (const m of merged) {
      if (colorDist2(e.c, m.c) < mergeThresh) {
        const tot = m.n + e.n
        m.c = {
          r: Math.trunc((m.c.r * m.n + e.c.r * e.n) / tot),
          g: Math.trunc((m.c.g * m.n + e.c.g * e.n) / tot),
          b: Math.trunc((m.c.b * m.n + e.c.b * e.n) / tot),
        }
        m.n = tot
        absorbed = true
        break
      }
    }
    if (!absorbed) merged.push(e)
  }
  if (merged.length < 2) {
    if (merged.length === 0) {
      merged.push({ c: { r: 0, g: 0, b: 0 }, n: 1 }, { c: { r: 255, g: 255, b: 255 }, n: 1 })
    } else {
      merged.push({ c: { r: 255, g: 255, b: 255 }, n: 1 })
    }
  }
  return merged.map((e) => e.c)
}

function nearestColor(c: RGB, palette: RGB[], cache: Map<number, RGB>): RGB {
  const key = packRGB(c.r, c.g, c.b)
  const hit = cache.get(key)
  if (hit) return hit
  let best = palette[0]
  let bestD = Number.MAX_SAFE_INTEGER
  for (const p of palette) {
    const d = colorDist2(c, p)
    if (d < bestD) { best = p; bestD = d }
  }
  cache.set(key, best)
  return best
}

/**
 * applyPalette — snap every opaque pixel to the nearest palette color and
 * binarize alpha to 0/255 (pixel art uses no partial transparency). In place.
 */
export function applyPalette(img: Img, palette: RGB[]): void {
  if (palette.length === 0) return
  const cache = new Map<number, RGB>()
  const d = img.data
  for (let i = 0; i + 3 < d.length; i += 4) {
    if (d[i + 3] < 128) {
      d[i] = 0; d[i + 1] = 0; d[i + 2] = 0; d[i + 3] = 0
      continue
    }
    d[i + 3] = 255
    const c = nearestColor({ r: d[i], g: d[i + 1], b: d[i + 2] }, palette, cache)
    d[i] = c.r; d[i + 1] = c.g; d[i + 2] = c.b
  }
}
