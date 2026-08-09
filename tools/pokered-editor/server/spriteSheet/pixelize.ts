// ───────────────────────────────────────────────────────────────────────────
// pixelize.ts — pixel-scale detection + grid snap (port of pixelize.go).
//
// AI "pixel art" is really a high-res image with anti-aliasing. We estimate the
// real (fake) pixel block size from the mode of same-color run lengths, then snap
// each block to its dominant color on a shared grid — true dot art.
// ───────────────────────────────────────────────────────────────────────────
import { type Img, alphaThreshold, idx, newImg } from './image'
import { buildSharedPalette, applyPalette } from './quantize'

function nearRGB(r1: number, g1: number, b1: number, r2: number, g2: number, b2: number): boolean {
  const tol = 12
  return Math.abs(r1 - r2) <= tol && Math.abs(g1 - g2) <= tol && Math.abs(b1 - b2) <= tol
}

/**
 * detectPixelScale — estimate the AI's fake pixel block size from the mode of
 * horizontal/vertical same-color run lengths (the "unfake" technique). Returns 1
 * on failure or already-native resolution.
 */
export function detectPixelScale(img: Img): number {
  const w = img.width, h = img.height
  if (w < 32 || h < 32) return 1
  let maxScale = Math.floor(Math.min(w, h) / 8)
  if (maxScale > 32) maxScale = 32
  if (maxScale < 2) return 1
  const hist = new Array<number>(maxScale + 1).fill(0)
  const d = img.data

  // Scan runs along one axis; `horizontal` true scans each row left→right.
  const scan = (horizontal: boolean) => {
    const outer = horizontal ? h : w
    const inner = horizontal ? w : h
    for (let o = 0; o < outer; o++) {
      let runLen = 1
      let px = horizontal ? 0 : o
      let py = horizontal ? o : 0
      let pi = idx(w, px, py)
      let pr = d[pi], pg = d[pi + 1], pb = d[pi + 2], pa = d[pi + 3]
      for (let i = 1; i < inner; i++) {
        const x = horizontal ? i : o
        const y = horizontal ? o : i
        const ci = idx(w, x, y)
        const r = d[ci], g = d[ci + 1], b = d[ci + 2], al = d[ci + 3]
        const same = (al <= alphaThreshold && pa <= alphaThreshold) ||
          (al > alphaThreshold && pa > alphaThreshold && nearRGB(r, g, b, pr, pg, pb))
        if (same) {
          runLen++
        } else {
          if (runLen >= 2 && runLen <= maxScale) hist[runLen]++
          runLen = 1
        }
        pr = r; pg = g; pb = b; pa = al
      }
    }
  }
  scan(true)
  scan(false)

  let best = 1, bestCount = 0
  for (let s = 2; s <= maxScale; s++) {
    const weighted = hist[s] * s // weight by run length (short runs are always plentiful)
    if (weighted > bestCount) { best = s; bestCount = weighted }
  }
  return best < 2 ? 1 : best
}

/**
 * pixelize — snap the image to scale×scale blocks of their dominant color,
 * keeping the output size identical to the input.
 */
export function pixelize(img: Img, scale: number): Img {
  if (scale < 2) return img
  const w = img.width, h = img.height
  const out = newImg(w, h)
  const src = img.data
  const dst = out.data
  for (let by = 0; by < h; by += scale) {
    for (let bx = 0; bx < w; bx += scale) {
      const bw = Math.min(scale, w - bx), bh = Math.min(scale, h - by)
      let opaque = 0
      const counts = new Map<number, number>()
      for (let dy = 0; dy < bh; dy++) {
        for (let dx = 0; dx < bw; dx++) {
          const i = idx(w, bx + dx, by + dy)
          if (src[i + 3] <= alphaThreshold) continue
          opaque++
          const key = (src[i] << 16) | (src[i + 1] << 8) | src[i + 2]
          counts.set(key, (counts.get(key) ?? 0) + 1)
        }
      }
      if (opaque * 2 < bw * bh) continue // mostly transparent block → leave empty
      let domKey = 0, domN = 0
      for (const [k, n] of counts) if (n > domN) { domN = n; domKey = k }
      const dr = (domKey >> 16) & 0xff, dg = (domKey >> 8) & 0xff, dbb = domKey & 0xff
      for (let dy = 0; dy < bh; dy++) {
        for (let dx = 0; dx < bw; dx++) {
          const i = idx(w, bx + dx, by + dy)
          dst[i] = dr; dst[i + 1] = dg; dst[i + 2] = dbb; dst[i + 3] = 255
        }
      }
    }
  }
  return out
}

/** paletteSizeForStyle — recommended palette size per style (0 disables post-proc). */
export function paletteSizeForStyle(styleKey: string): number {
  switch (styleKey) {
    case 'retro16': return 16
    case 'pixel': return 32
    default: return 0 // chibi/cartoon/custom: don't force a pixel grid
  }
}

/**
 * pixelPostProcess — apply shared-palette quantization + pixel-grid snap to a
 * state's frames. Frames are modified/replaced in place (the array is mutated).
 */
export function pixelPostProcess(frames: Img[], paletteSize: number): void {
  if (paletteSize <= 0 || frames.length === 0) return
  const palette = buildSharedPalette(frames, paletteSize)
  if (!palette) return
  const scales: number[] = []
  for (const f of frames) {
    applyPalette(f, palette)
    scales.push(detectPixelScale(f))
  }
  // Share the median scale across frames for grid consistency.
  const sorted = [...scales].sort((a, b) => a - b)
  const scale = sorted[Math.floor(sorted.length / 2)]
  if (scale < 2) return
  for (let i = 0; i < frames.length; i++) frames[i] = pixelize(frames[i], scale)
}
