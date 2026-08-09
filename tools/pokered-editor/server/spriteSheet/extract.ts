// ───────────────────────────────────────────────────────────────────────────
// extract.ts — frame extraction & alignment (port of extract.go).
//
// Each segmented pose is cropped to its bbox, then placed in a cell aligned by
// its ALPHA-WEIGHTED CENTROID (center of mass) rather than its bbox center — so a
// pose with an outstretched arm/weapon doesn't shove the torso sideways and the
// character doesn't jitter between frames. A shared scale unifies size and a
// common baseline preserves jump/fall arcs.
// ───────────────────────────────────────────────────────────────────────────
import { type Img, alphaThreshold, blit, idx, newImg, resample } from './image'
import type { ColSpan } from './segment'
import { segmentStrip } from './segment'
import type { ExtractResult } from './types'

/** A pose's content in strip coordinates. */
interface FrameContent {
  img: Img | null
  minX: number
  cx: number // alpha-weighted centroid X (strip coords)
  bottom: number // baseline (lowest content row, strip coords)
}

/** Go's `int(f + 0.5)` — round-half-up via truncation toward zero. */
function roundGo(x: number): number {
  return Math.trunc(x + 0.5)
}

/**
 * extractContent — collect every opaque pixel inside a column span into a bbox
 * crop (no connected-component ownership, so split limbs stay one pose) and
 * compute the alpha-weighted centroid.
 */
function extractContent(strip: Img, span: ColSpan, h: number): FrameContent {
  const w = strip.width
  const data = strip.data
  let minX = span.end, minY = h, maxX = span.start - 1, maxY = -1
  let sumWX = 0, sumW = 0
  for (let x = span.start; x < span.end; x++) {
    for (let y = 0; y < h; y++) {
      const a = data[idx(w, x, y) + 3]
      if (a <= alphaThreshold) continue
      if (x < minX) minX = x
      if (x > maxX) maxX = x
      if (y < minY) minY = y
      if (y > maxY) maxY = y
      sumWX += x * a
      sumW += a
    }
  }
  if (maxX < minX || maxY < minY) return { img: null, minX: 0, cx: 0, bottom: 0 }
  const gw = maxX - minX + 1, gh = maxY - minY + 1
  const dst = newImg(gw, gh)
  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      const si = idx(w, x, y)
      if (data[si + 3] <= alphaThreshold) continue
      const di = idx(gw, x - minX, y - minY)
      dst.data[di] = data[si]
      dst.data[di + 1] = data[si + 1]
      dst.data[di + 2] = data[si + 2]
      dst.data[di + 3] = data[si + 3]
    }
  }
  let cx = (minX + maxX + 1) / 2
  if (sumW > 0) cx = sumWX / sumW
  return { img: dst, minX, cx, bottom: maxY }
}

/**
 * extractFrames — detect poses in a transparent-background strip and render each
 * into a cellW × cellH frame with a shared scale, centroid-centered horizontally,
 * and a common baseline (vertical offsets — e.g. jump arcs — preserved).
 */
export function extractFrames(strip: Img, expected: number, cellW: number, cellH: number, margin: number): ExtractResult {
  const res: ExtractResult = { frames: [], found: 0, expected, warnings: [] }
  const { segs, natural } = segmentStrip(strip, expected)
  if (segs.length === 0) {
    res.warnings.push('No character found in the image. Please regenerate.')
    return res
  }
  const h = strip.height

  const fcs: FrameContent[] = []
  for (const s of segs) {
    const fc = extractContent(strip, s, h)
    if (fc.img) fcs.push(fc)
  }
  if (fcs.length === 0) {
    res.warnings.push('No valid pose found. Please regenerate.')
    return res
  }

  // Common baseline + shared scale.
  let baseline = 0
  for (const g of fcs) if (g.bottom > baseline) baseline = g.bottom
  let availW = cellW - margin * 2
  let availH = cellH - margin * 2
  if (availW < 8 || availH < 8) { availW = cellW; availH = cellH }
  let maxBodyW = 1, maxBodyH = 1
  for (const g of fcs) {
    const [bw, bh] = bodyExtent(g.img!)
    if (bw > maxBodyW) maxBodyW = bw
    if (bh > maxBodyH) maxBodyH = bh
  }
  let scale = Math.min(availW / maxBodyW, availH / maxBodyH)
  if (scale > 1) scale = 1

  for (const g of fcs) {
    const gi = g.img!
    // scale is body-extent-based; clamp again to the full bbox so a sparse bbox
    // still fits the available space.
    let boxScale = Math.min(scale, Math.min(availW / gi.width, availH / gi.height))
    if (boxScale > 1) boxScale = 1
    let sw = roundGo(gi.width * boxScale)
    let sh = roundGo(gi.height * boxScale)
    if (sw < 1) sw = 1
    if (sh < 1) sh = 1
    let scaled = gi
    if (sw !== gi.width || sh !== gi.height) scaled = resample(gi, sw, sh)
    // Scale the strip-baseline offset so jump/fall arcs survive.
    const contentBaseline = roundGo((baseline - g.bottom) * boxScale)

    const cell = newImg(cellW, cellH)
    // Place so the centroid lands at cell center (the large torso dominates, so
    // limbs swing without shifting the body).
    let left = roundGo(cellW / 2 - (g.cx - g.minX) * boxScale)
    if (left < 0) left = 0
    if (left + sw > cellW) left = cellW - sw
    let top = cellH - margin - contentBaseline - sh
    if (top < 0) top = 0
    blit(cell, left, top, scaled)
    res.frames.push(cell)
  }

  res.found = natural
  if (natural !== expected) {
    res.warnings.push(
      `Detected ${natural} poses instead of the expected ${expected}. Poses may have overlapped or be missing — regenerating is recommended.`,
    )
  }
  return res
}

/**
 * bodyExtent — the smallest size covering 80% of alpha mass, used as the "real
 * body" extent so a long outstretched limb doesn't over-inflate the scale.
 */
export function bodyExtent(img: Img): [number, number] {
  const w = img.width, h = img.height
  if (w === 0 || h === 0) return [1, 1]
  const alphaX = new Float64Array(w)
  const alphaY = new Float64Array(h)
  const data = img.data
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const a = data[idx(w, x, y) + 3]
      alphaX[x] += a
      alphaY[y] += a
    }
  }
  let cutX = cumulativeExtent(alphaX, 0.8)
  let cutY = cumulativeExtent(alphaY, 0.8)
  if (cutX < 1) cutX = 1
  if (cutY < 1) cutY = 1
  return [cutX, cutY]
}

/** cumulativeExtent — length of the narrowest contiguous span covering massFrac. */
function cumulativeExtent(mass: Float64Array, massFrac: number): number {
  let total = 0
  for (const v of mass) total += v
  if (total === 0) return 0
  const target = total * massFrac
  const n = mass.length
  let best = n
  let left = 0
  let cur = 0
  for (let right = 0; right < n; right++) {
    cur += mass[right]
    while (cur >= target) {
      const span = right - left + 1
      if (span < best) best = span
      cur -= mass[left]
      left++
    }
  }
  return best
}
