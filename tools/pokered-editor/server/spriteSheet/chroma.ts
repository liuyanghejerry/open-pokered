// ───────────────────────────────────────────────────────────────────────────
// chroma.ts — chrominance-plane (CbCr) background matting (port of chroma.go).
//
// Keys out the magenta background using only chrominance (Cb, Cr), ignoring luma
// (Y) — so shaded and bright magenta read as the same color, and it stays robust
// to JPEG 4:2:0 subsampling (which crushes chrominance but preserves luma). The
// key is the mode of a CbCr histogram sampled from the corners; matting is a soft
// Hermite ramp with despill, then a 4-connectivity flood-fill clears residual
// background while preserving isolated interior pixels.
// ───────────────────────────────────────────────────────────────────────────
import { type Img, alphaThreshold, idx, newImg, u8 } from './image'

// CbCr-plane adaptive matting parameters.
const chromaIn = 24.0 // CbCr distance ≤ this → fully transparent (key color)
const chromaOut = 72.0 // CbCr distance ≥ this → fully opaque (subject)
const despillBand = 100.0 // pixels within this distance get key-tint despill correction
const despillScale = 0.92 // despill strength (key-direction chroma suppression)
const floodTol = 88.0 // border-seed flood-fill background tolerance (lenient)

export interface YCC {
  y: number
  cb: number
  cr: number
}

/** BT.601 YCbCr (8-bit, 128-centered). */
export function toYCC(r: number, g: number, b: number): YCC {
  const y = 0.299 * r + 0.587 * g + 0.114 * b
  return { y, cb: (b - y) * 0.564 + 128, cr: (r - y) * 0.713 + 128 }
}

export function fromYCC(c: YCC): [number, number, number] {
  const r = c.y + 1.402 * (c.cr - 128)
  const g = c.y - 0.344136 * (c.cb - 128) - 0.714136 * (c.cr - 128)
  const b = c.y + 1.772 * (c.cb - 128)
  return [u8(r), u8(g), u8(b)]
}

/** Hermite smoothstep — a smooth 0→1 transition (edge feathering). */
export function smoothstep(edge0: number, edge1: number, x: number): number {
  if (edge1 <= edge0) return 0
  let t = (x - edge0) / (edge1 - edge0)
  if (t < 0) t = 0
  else if (t > 1) t = 1
  return t * t * (3 - 2 * t)
}

function hypot(a: number, b: number): number {
  return Math.sqrt(a * a + b * b)
}

/**
 * Estimate the background key color from the mode of a CbCr histogram over the
 * border/corner pixels (mode, not mean, so gradients/noise don't shift it).
 */
export function detectBackground(img: Img): [number, number, number] {
  const w = img.width
  const h = img.height
  if (w === 0 || h === 0) return [255, 0, 255]
  interface Acc { n: number; sr: number; sg: number; sb: number }
  const bins = new Map<number, Acc>()
  let total = 0
  let magN = 0, magR = 0, magG = 0, magB = 0 // magenta-family (strong R·B, weak G)
  const data = img.data
  const visit = (x: number, y: number) => {
    const i = idx(w, x, y)
    const r = data[i], g = data[i + 1], b = data[i + 2]
    total++
    if (r > 150 && b > 150 && g < 120) {
      magN++; magR += r; magG += g; magB += b
    }
    const c = toYCC(r, g, b)
    const key = ((Math.trunc(c.cb) >> 3) << 6) | (Math.trunc(c.cr) >> 3) // 8-unit CbCr quantization
    let a = bins.get(key)
    if (!a) { a = { n: 0, sr: 0, sg: 0, sb: 0 }; bins.set(key, a) }
    a.n++; a.sr += r; a.sg += g; a.sb += b
  }
  // Wide poses (walking, etc.) touch the whole border, contaminating the key
  // estimate with character color — so sample the corner patches, which are
  // almost always background.
  let cw = Math.floor(w / 5), ch = Math.floor(h / 5)
  if (cw < 2) cw = w
  if (ch < 2) ch = h
  const corner = (x0: number, y0: number, x1: number, y1: number) => {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) visit(x, y)
  }
  corner(0, 0, cw, ch)
  corner(w - cw, 0, w, ch)
  corner(0, h - ch, cw, h)
  corner(w - cw, h - ch, w, h)
  // Thin border too, as a fallback (rare case where corners are covered).
  for (let x = 0; x < w; x++) { visit(x, 0); visit(x, h - 1) }
  for (let y = 0; y < h; y++) { visit(0, y); visit(w - 1, y) }
  // Magenta bias: this pipeline always intends a magenta key, so if enough of
  // the border/corner samples (12%+) are magenta-family, lock the magenta
  // cluster as the key even if a wide pose made character color most-frequent.
  if (total > 0 && magN >= Math.floor((total * 12) / 100)) {
    return [Math.trunc(magR / magN), Math.trunc(magG / magN), Math.trunc(magB / magN)]
  }
  let best: Acc | null = null
  for (const a of bins.values()) if (!best || a.n > best.n) best = a
  if (!best || best.n === 0) return [255, 0, 255]
  return [Math.trunc(best.sr / best.n), Math.trunc(best.sg / best.n), Math.trunc(best.sb / best.n)]
}

/** isMagentaKey — is the key color magenta-family (strong R·B, weak G)? */
export function isMagentaKey(k: [number, number, number]): boolean {
  return k[0] > 150 && k[2] > 150 && k[1] < 120
}

/**
 * magentaResidueFrac — fraction of opaque pixels close to pure magenta
 * (CbCr < 55). A symptom indicator of whether matting cleared the magenta bg.
 */
export function magentaResidueFrac(img: Img): number {
  const mk = toYCC(255, 0, 255)
  let n = 0
  const data = img.data
  for (let i = 0; i + 3 < data.length; i += 4) {
    if (data[i + 3] <= alphaThreshold) continue
    const c = toYCC(data[i], data[i + 1], data[i + 2])
    if (hypot(c.cb - mk.cb, c.cr - mk.cr) < 55) n++
  }
  const total = data.length / 4
  if (total === 0) return 0
  return n / total
}

/**
 * matteWith — CbCr-plane matting + despill + flood-fill for a given key.
 * Returns the matted image and the opaque-pixel fraction.
 */
export function matteWith(img: Img, key: [number, number, number]): { out: Img; frac: number } {
  const kc = toYCC(key[0], key[1], key[2])
  const kvb = kc.cb - 128, kvr = kc.cr - 128
  const klen = hypot(kvb, kvr)
  const out = newImg(img.width, img.height)
  const src = img.data
  const dst = out.data

  for (let i = 0; i + 3 < src.length; i += 4) {
    let r = src[i], g = src[i + 1], b = src[i + 2]
    const a = src[i + 3]
    if (a === 0) continue
    const c = toYCC(r, g, b)
    const dist = hypot(c.cb - kc.cb, c.cr - kc.cr)
    const alpha = smoothstep(chromaIn, chromaOut, dist)
    if (alpha <= 0) continue
    if (klen > 1 && dist < despillBand) {
      const pcb = c.cb - 128, pcr = c.cr - 128
      const proj = (pcb * kvb + pcr * kvr) / klen
      if (proj > 0) {
        const wgt = smoothstep(0, 1, (despillBand - dist) / despillBand) * despillScale
        const ub = kvb / klen, ur = kvr / klen
        c.cb = 128 + (pcb - ub * proj * wgt)
        c.cr = 128 + (pcr - ur * proj * wgt)
        ;[r, g, b] = fromYCC(c)
      }
    }
    dst[i] = r
    dst[i + 1] = g
    dst[i + 2] = b
    dst[i + 3] = Math.trunc(a * alpha) // Go truncates uint8(float)
  }

  floodClearBackground(out, img, kc)

  let opaque = 0
  for (let i = 3; i < dst.length; i += 4) if (dst[i] > alphaThreshold) opaque++
  const frac = opaque / (dst.length / 4)
  return { out, frac }
}

/**
 * floodClearBackground — 4-connectivity flood-fill from the borders along
 * key-close pixels, zeroing alpha. Removes gradient/noise background that the
 * soft matte misses, while preserving interior character pixels (even key-colored
 * ones) that aren't border-connected.
 */
function floodClearBackground(out: Img, orig: Img, kc: YCC): void {
  const w = orig.width, h = orig.height
  if (w < 3 || h < 3) return
  const od = orig.data
  const isKey = (x: number, y: number): boolean => {
    const i = idx(w, x, y)
    const c = toYCC(od[i], od[i + 1], od[i + 2])
    return hypot(c.cb - kc.cb, c.cr - kc.cr) <= floodTol
  }
  const visited = new Uint8Array(w * h)
  const stack: number[] = []
  const push = (x: number, y: number) => {
    const p = y * w + x
    if (!visited[p] && isKey(x, y)) {
      visited[p] = 1
      stack.push(p)
    }
  }
  for (let x = 0; x < w; x++) { push(x, 0); push(x, h - 1) }
  for (let y = 0; y < h; y++) { push(0, y); push(w - 1, y) }
  const dst = out.data
  while (stack.length > 0) {
    const p = stack.pop()!
    const x = p % w, y = Math.floor(p / w)
    dst[p * 4 + 3] = 0 // background → transparent
    if (x > 0) push(x - 1, y)
    if (x < w - 1) push(x + 1, y)
    if (y > 0) push(x, y - 1)
    if (y < h - 1) push(x, y + 1)
  }
}

/**
 * cleanupAlpha — remove isolated opaque specks (JPEG-block grit) and fill 1px
 * pinholes. Only clearly isolated/surrounded pixels are touched, preserving soft
 * edges.
 */
function cleanupAlpha(img: Img): void {
  const w = img.width, h = img.height
  if (w < 3 || h < 3) return
  const data = img.data
  const orig = new Uint8ClampedArray(w * h)
  for (let p = 0; p < w * h; p++) orig[p] = data[p * 4 + 3]
  const opaque = (x: number, y: number): number => {
    if (x < 0 || y < 0 || x >= w || y >= h) return 0
    return orig[y * w + x] > alphaThreshold ? 1 : 0
  }
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4
      const nb = opaque(x - 1, y) + opaque(x + 1, y) + opaque(x, y - 1) + opaque(x, y + 1) +
        opaque(x - 1, y - 1) + opaque(x + 1, y - 1) + opaque(x - 1, y + 1) + opaque(x + 1, y + 1)
      if (orig[y * w + x] > alphaThreshold) {
        if (nb === 0) data[i + 3] = 0 // fully isolated speck → remove
      } else if (nb >= 7) {
        data[i + 3] = 255 // nearly surrounded pinhole → fill
      }
    }
  }
}

/**
 * removeBackground — auto-detect the key and matte it away, with self-diagnostic
 * fallbacks to pure magenta when the detected key looks wrong (opaque ratio spike
 * or magenta residue spike, or a non-magenta key on a magenta-intended pipeline).
 */
export function removeBackground(src: Img): Img {
  const img = src
  const key = detectBackground(img)
  let { out, frac } = matteWith(img, key)

  // Safety: if a wide pose filled the border and the key was mistaken for
  // character color, matting either erases the character (opaque ratio spikes)
  // or only partly clears magenta (residue spikes). Re-matte with pure magenta
  // and keep the better result.
  if (frac > 0.6 || magentaResidueFrac(out) > 0.025) {
    const r2 = matteWith(img, [255, 0, 255])
    const betterFrac = r2.frac < frac - 0.03 && r2.frac > 0.02
    const lessResidue = magentaResidueFrac(r2.out) < magentaResidueFrac(out)
    if ((betterFrac || lessResidue) && r2.frac > 0.02) {
      out = r2.out
      frac = r2.frac
    }
  }
  // Dark/achromatic key fallback: also try pure magenta and keep the one with
  // less magenta residue.
  if (!isMagentaKey(key)) {
    const r2 = matteWith(img, [255, 0, 255])
    if (r2.frac > 0.02 && magentaResidueFrac(r2.out) < magentaResidueFrac(out)) {
      out = r2.out
    }
  }
  cleanupAlpha(out)
  return out
}

/** colorDist — RGB euclidean distance (for inspect's residual-chroma check). */
export function colorDist(r: number, g: number, b: number, bg: [number, number, number]): number {
  const dr = r - bg[0], dg = g - bg[1], db = b - bg[2]
  return Math.sqrt(dr * dr + dg * dg + db * db)
}
