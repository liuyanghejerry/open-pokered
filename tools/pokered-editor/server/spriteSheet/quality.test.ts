// ───────────────────────────────────────────────────────────────────────────
// Phase-2 tests: quantization, pixelization, inspection & scoring.
// ───────────────────────────────────────────────────────────────────────────
import { describe, expect, it } from 'vitest'
import { type Img, alphaThreshold, idx, newImg } from './image'
import { applyPalette, buildSharedPalette, colorDist2 } from './quantize'
import { detectPixelScale, paletteSizeForStyle, pixelize, pixelPostProcess } from './pixelize'
import { inspectFrames, motionPresence } from './inspect'
import { scoreFrames } from './score'

function fillAll(img: Img, r: number, g: number, b: number, a = 255): void {
  for (let i = 0; i + 3 < img.data.length; i += 4) {
    img.data[i] = r; img.data[i + 1] = g; img.data[i + 2] = b; img.data[i + 3] = a
  }
}

function fillBox(img: Img, x0: number, y0: number, x1: number, y1: number, r: number, g: number, b: number, a = 255): void {
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
    const i = idx(img.width, x, y)
    img.data[i] = r; img.data[i + 1] = g; img.data[i + 2] = b; img.data[i + 3] = a
  }
}

function colorCount(img: Img): number {
  const set = new Set<number>()
  for (let i = 0; i + 3 < img.data.length; i += 4) {
    if (img.data[i + 3] <= alphaThreshold) continue
    set.add((img.data[i] << 16) | (img.data[i + 1] << 8) | img.data[i + 2])
  }
  return set.size
}

describe('shared palette', () => {
  it('extracts a 2-color palette from red & blue frames', () => {
    const a = newImg(32, 32); fillAll(a, 200, 30, 30)
    const b = newImg(32, 32); fillAll(b, 30, 30, 200)
    const pal = buildSharedPalette([a, b], 4)!
    expect(pal).not.toBeNull()
    expect(pal.length).toBe(2)
    const hasRed = pal.some((c) => colorDist2(c, { r: 200, g: 30, b: 30 }) < 600)
    const hasBlue = pal.some((c) => colorDist2(c, { r: 30, g: 30, b: 200 }) < 600)
    expect(hasRed && hasBlue).toBe(true)
  })

  it('applyPalette snaps colors and binarizes alpha', () => {
    const img = newImg(4, 4)
    fillBox(img, 0, 0, 1, 3, 198, 28, 33, 200) // near-red, alpha 200 → 255
    fillBox(img, 2, 0, 3, 3, 33, 33, 205, 100) // near-blue, alpha 100 → 0 (cleared)
    applyPalette(img, [{ r: 200, g: 30, b: 30 }, { r: 30, g: 30, b: 200 }])
    // red column snapped to palette red, opaque
    const r0 = idx(4, 0, 0)
    expect([img.data[r0], img.data[r0 + 1], img.data[r0 + 2]]).toEqual([200, 30, 30])
    expect(img.data[r0 + 3]).toBe(255)
    // blue column had alpha 100 (<128) → cleared transparent
    const b0 = idx(4, 2, 0)
    expect(img.data[b0 + 3]).toBe(0)
  })
})

describe('pixelization', () => {
  it('detects a 4× fake-pixel block size', () => {
    const img = newImg(64, 64)
    for (let by = 0; by < 64; by += 4) {
      for (let bx = 0; bx < 64; bx += 4) {
        const on = ((bx / 4 + by / 4) % 2) === 0
        const v = on ? 0 : 255
        fillBox(img, bx, by, bx + 3, by + 3, v, v, v)
      }
    }
    expect(detectPixelScale(img)).toBe(4)
  })

  it('pixelize snaps a block to its dominant color', () => {
    const img = newImg(8, 8)
    fillAll(img, 10, 20, 30)
    // make one pixel different inside the first 8×8 block; dominant should win
    const i = idx(8, 1, 1)
    img.data[i] = 200; img.data[i + 1] = 200; img.data[i + 2] = 200
    const out = pixelize(img, 8)
    const o = idx(8, 1, 1)
    expect([out.data[o], out.data[o + 1], out.data[o + 2]]).toEqual([10, 20, 30])
  })

  it('pixelPostProcess reduces color count for the pixel style', () => {
    // a smooth horizontal gradient (many colors) over 2 frames
    const mk = () => {
      const f = newImg(48, 48)
      for (let y = 0; y < 48; y++) for (let x = 0; x < 48; x++) {
        const i = idx(48, x, y)
        f.data[i] = x * 5 % 256; f.data[i + 1] = y * 5 % 256; f.data[i + 2] = 128; f.data[i + 3] = 255
      }
      return f
    }
    const frames = [mk(), mk()]
    const before = colorCount(frames[0])
    pixelPostProcess(frames, paletteSizeForStyle('pixel'))
    const after = colorCount(frames[0])
    expect(after).toBeLessThanOrEqual(32)
    expect(after).toBeLessThan(before)
  })
})

describe('motion & scoring', () => {
  it('identical frames have zero motion; shifted frames have motion', () => {
    const a = newImg(40, 40); fillBox(a, 10, 10, 25, 25, 80, 160, 90)
    const same = newImg(40, 40); fillBox(same, 10, 10, 25, 25, 80, 160, 90)
    const shifted = newImg(40, 40); fillBox(shifted, 16, 10, 31, 25, 80, 160, 90)
    expect(motionPresence([a, same])).toBe(0)
    expect(motionPresence([a, shifted])).toBeGreaterThan(0)
  })

  it('scoreFrames rewards a stable baseline (contact)', () => {
    // two frames, same foot baseline, slight upper motion
    const a = newImg(40, 40); fillBox(a, 14, 6, 25, 34, 80, 160, 90)
    const b = newImg(40, 40); fillBox(b, 14, 8, 25, 34, 80, 160, 90)
    const s = scoreFrames([a, b])
    expect(s.contact).toBeGreaterThan(0.8)
    expect(s.overall).toBeGreaterThan(0)
  })
})

describe('inspection', () => {
  it('passes a clean, consistent frame set', () => {
    const mk = () => { const f = newImg(64, 64); fillBox(f, 20, 12, 43, 51, 60, 120, 200); return f }
    const res = inspectFrames([mk(), mk(), mk()], [255, 0, 255], null)
    expect(res.ok).toBe(true)
    expect(res.errors.length).toBe(0)
  })

  it('flags an empty frame with a retry hint', () => {
    const good = () => { const f = newImg(64, 64); fillBox(f, 20, 12, 43, 51, 60, 120, 200); return f }
    const empty = newImg(64, 64) // fully transparent
    const res = inspectFrames([good(), empty, good()], [255, 0, 255], null)
    expect(res.ok).toBe(false)
    expect(res.errors.length).toBeGreaterThan(0)
    expect(res.retryHints.some((h) => /every column/i.test(h))).toBe(true)
  })
})
