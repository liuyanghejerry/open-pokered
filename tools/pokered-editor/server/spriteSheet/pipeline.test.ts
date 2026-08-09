// ───────────────────────────────────────────────────────────────────────────
// Fidelity tests for the ported sprite pipeline — mirrors PerfectPixel's Go
// tests (segment_test.go, extract_anchor_test.go, extract_bodyextent_test.go,
// segment_overlap_test.go) on equivalent synthetic strips.
// ───────────────────────────────────────────────────────────────────────────
import { describe, expect, it } from 'vitest'
import { type Img, alphaThreshold, idx, newImg, resample } from './image'
import { detectBackground, removeBackground } from './chroma'
import { segmentStrip } from './segment'
import { extractFrames } from './extract'

// fillBox — fill an inclusive opaque rectangle (mirrors Go's fillBox).
function fillBox(img: Img, x0: number, y0: number, x1: number, y1: number, r: number, g: number, b: number): void {
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const i = idx(img.width, x, y)
      img.data[i] = r; img.data[i + 1] = g; img.data[i + 2] = b; img.data[i + 3] = 255
    }
  }
}

function magentaFill(img: Img): void {
  for (let i = 0; i + 3 < img.data.length; i += 4) {
    img.data[i] = 255; img.data[i + 1] = 0; img.data[i + 2] = 255; img.data[i + 3] = 255
  }
}

function opaqueCount(f: Img): number {
  let n = 0
  for (let p = 3; p < f.data.length; p += 4) if (f.data[p] > alphaThreshold) n++
  return n
}

// torsoCenterX — center of the tallest opaque columns (≥ minCount).
function torsoCenterX(f: Img, minCount: number): number {
  let first = -1, last = -1
  for (let x = 0; x < f.width; x++) {
    let cnt = 0
    for (let y = 0; y < f.height; y++) if (f.data[idx(f.width, x, y) + 3] > alphaThreshold) cnt++
    if (cnt >= minCount) { if (first < 0) first = x; last = x }
  }
  return (first + last) / 2
}

// maxOpaqueColHeight — height of the tallest opaque column.
function maxOpaqueColHeight(f: Img): number {
  let best = 0
  for (let x = 0; x < f.width; x++) {
    let top = -1, bottom = -1
    for (let y = 0; y < f.height; y++) {
      if (f.data[idx(f.width, x, y) + 3] > alphaThreshold) { if (top < 0) top = y; bottom = y }
    }
    if (top >= 0 && bottom - top + 1 > best) best = bottom - top + 1
  }
  return best
}

describe('segmentation', () => {
  it('counts poses with clean gutters', () => {
    const strip = newImg(600, 100)
    fillBox(strip, 20, 20, 79, 79, 200, 100, 50)
    fillBox(strip, 220, 20, 279, 79, 200, 100, 50)
    fillBox(strip, 420, 20, 479, 79, 200, 100, 50)
    const { segs, natural } = segmentStrip(strip, 3)
    expect(natural).toBe(3)
    expect(segs.length).toBe(3)
  })

  it('force-splits overlapping poses to the expected count', () => {
    const strip = newImg(400, 100)
    fillBox(strip, 40, 20, 140, 79, 200, 100, 50)
    fillBox(strip, 120, 20, 220, 79, 200, 100, 50)
    const { natural } = segmentStrip(strip, 2)
    expect(natural).toBe(2)
  })

  it('separates two touching poses via prominence + DP', () => {
    const strip = newImg(400, 100)
    fillBox(strip, 40, 10, 110, 89, 200, 100, 50) // torso A
    fillBox(strip, 110, 48, 250, 55, 200, 100, 50) // thin connector
    fillBox(strip, 250, 10, 330, 89, 200, 100, 50) // torso B
    const res = extractFrames(strip, 2, 100, 100, 8)
    expect(res.frames.length).toBe(2)
    for (const f of res.frames) expect(opaqueCount(f)).toBeGreaterThan(500)
  })
})

describe('chroma matting', () => {
  it('keys out magenta and keeps a green subject (YCbCr)', () => {
    const src = newImg(64, 64)
    magentaFill(src)
    fillBox(src, 20, 20, 43, 43, 40, 200, 60) // green box
    const out = removeBackground(src)
    const ci = idx(out.width, 32, 32)
    expect(out.data[ci + 3]).toBeGreaterThanOrEqual(200) // green subject opaque
    const ei = idx(out.width, 2, 2)
    expect(out.data[ei + 3]).toBeLessThanOrEqual(alphaThreshold) // magenta bg transparent
  })

  it('detects a magenta background key', () => {
    const src = newImg(64, 64)
    magentaFill(src)
    fillBox(src, 20, 20, 43, 43, 40, 200, 60)
    const key = detectBackground(src)
    expect(key[0]).toBeGreaterThan(150)
    expect(key[2]).toBeGreaterThan(150)
    expect(key[1]).toBeLessThan(120)
  })
})

describe('extraction & alignment', () => {
  it('anchors poses by alpha-weighted centroid (no torso jitter)', () => {
    const strip = newImg(480, 100)
    fillBox(strip, 30, 20, 49, 79, 200, 100, 50)
    fillBox(strip, 270, 20, 289, 79, 200, 100, 50)
    fillBox(strip, 390, 20, 409, 79, 200, 100, 50)
    // Frame 2: same torso + a long thin arm reaching right (extends bbox 40px).
    fillBox(strip, 150, 20, 169, 79, 200, 100, 50)
    fillBox(strip, 170, 30, 209, 33, 200, 100, 50)

    const res = extractFrames(strip, 4, 100, 100, 8)
    expect(res.found).toBe(4)
    const centers = res.frames.map((f) => torsoCenterX(f, 40))
    const spread = Math.max(...centers) - Math.min(...centers)
    expect(spread).toBeLessThan(6) // bbox-center would give ~20
    for (const i of [0, 2, 3]) expect(Math.abs(centers[i] - 50)).toBeLessThanOrEqual(2)
  })

  it('does not merge a distant residual blob into a frame', () => {
    const strip = newImg(600, 100)
    fillBox(strip, 20, 20, 79, 79, 200, 100, 50) // pose 1 (60×60)
    fillBox(strip, 120, 20, 179, 79, 200, 100, 50) // pose 2 (60×60)
    fillBox(strip, 560, 10, 579, 29, 90, 200, 90) // distant blob (20×20)
    const res = extractFrames(strip, 2, 256, 256, 16)
    expect(res.found).toBe(2)
    for (const f of res.frames) expect(opaqueCount(f)).toBe(3600) // exactly the 60×60 body
  })

  it('scales by body extent, not outlier limbs', () => {
    const strip = newImg(1000, 100)
    fillBox(strip, 95, 20, 154, 79, 200, 100, 50)
    fillBox(strip, 595, 20, 654, 79, 200, 100, 50)
    fillBox(strip, 845, 20, 904, 79, 200, 100, 50)
    // outlier (slot 2): 60×60 torso + a thin 4px arm reaching right within the slot.
    fillBox(strip, 345, 20, 404, 79, 200, 100, 50)
    fillBox(strip, 405, 47, 490, 50, 200, 100, 50)
    const res = extractFrames(strip, 4, 100, 100, 8)
    expect(res.found).toBe(4)
    for (const i of [0, 2, 3]) expect(maxOpaqueColHeight(res.frames[i])).toBeGreaterThanOrEqual(45)
  })
})

describe('resample (premultiplied Catmull-Rom)', () => {
  it('downscales an opaque box keeping it opaque and transparent border clear', () => {
    const src = newImg(80, 80)
    fillBox(src, 20, 20, 59, 59, 200, 100, 50) // centered 40×40 opaque
    const out = resample(src, 40, 40)
    expect(out.width).toBe(40)
    expect(out.height).toBe(40)
    // center stays opaque
    expect(out.data[idx(40, 20, 20) + 3]).toBeGreaterThan(200)
    // corner stays transparent (no halo bleed)
    expect(out.data[idx(40, 0, 0) + 3]).toBeLessThanOrEqual(alphaThreshold)
  })
})
