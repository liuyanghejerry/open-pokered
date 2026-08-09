// ───────────────────────────────────────────────────────────────────────────
// image.ts — the pixel substrate for the sprite-sheet pipeline.
//
// Port of Go's `*image.NRGBA` usage to a plain `{ width, height, data }` struct
// where `data` is straight (non-premultiplied) RGBA, row-major — identical in
// layout to a browser `ImageData` and to what `pngjs` decodes. All the ported
// algorithms (chroma matting, segmentation, extraction, quantization) operate on
// this type.
//
// Also provides the one non-trivial primitive the Go code leaned on the standard
// library for: a high-quality resampler. Go used `golang.org/x/image/draw`'s
// CatmullRom scaler; we reimplement an equivalent separable Catmull-Rom filter
// that works in PREMULTIPLIED alpha so transparent edges don't bleed a dark/bright
// halo when downscaling.
// ───────────────────────────────────────────────────────────────────────────
import { PNG } from 'pngjs'

/** Straight-alpha RGBA image, row-major. `data.length === width * height * 4`. */
export interface Img {
  width: number
  height: number
  data: Uint8ClampedArray
}

/** Alpha at or below this is treated as an empty (transparent) pixel. */
export const alphaThreshold = 10

/** Allocate a fully-transparent image. */
export function newImg(width: number, height: number): Img {
  return { width, height, data: new Uint8ClampedArray(width * height * 4) }
}

/** Byte offset of pixel (x, y) — equivalent to Go's `PixOffset`. */
export function idx(width: number, x: number, y: number): number {
  return (y * width + x) * 4
}

export function cloneImg(img: Img): Img {
  return { width: img.width, height: img.height, data: new Uint8ClampedArray(img.data) }
}

/** Round-half-up clamp to a byte — mirrors Go's `u8(v float64)`. */
export function u8(v: number): number {
  if (v <= 0) return 0
  if (v >= 255) return 255
  return Math.floor(v + 0.5)
}

// ── PNG codec (pngjs, pure JS) ──────────────────────────────────────────────

/** Decode a PNG buffer into a straight-alpha RGBA image. */
export function decodePNG(buf: Buffer): Img {
  const png = PNG.sync.read(buf)
  return { width: png.width, height: png.height, data: new Uint8ClampedArray(png.data) }
}

/** Encode a straight-alpha RGBA image to a PNG buffer. */
export function encodePNG(img: Img): Buffer {
  const png = new PNG({ width: img.width, height: img.height })
  png.data = Buffer.from(img.data) // copy; pngjs reads this during write
  return PNG.sync.write(png)
}

// ── Compositing ─────────────────────────────────────────────────────────────

/**
 * Copy `src` onto `dst` at (dstX, dstY), clipped to dst bounds. The Go pipeline
 * always blits onto a freshly-allocated transparent cell, so a straight copy is
 * equivalent to source-over here; we copy every source pixel (incl. transparent)
 * so the placement is exact.
 */
export function blit(dst: Img, dstX: number, dstY: number, src: Img): void {
  for (let sy = 0; sy < src.height; sy++) {
    const dy = dstY + sy
    if (dy < 0 || dy >= dst.height) continue
    for (let sx = 0; sx < src.width; sx++) {
      const dx = dstX + sx
      if (dx < 0 || dx >= dst.width) continue
      const si = (sy * src.width + sx) * 4
      const di = (dy * dst.width + dx) * 4
      dst.data[di] = src.data[si]
      dst.data[di + 1] = src.data[si + 1]
      dst.data[di + 2] = src.data[si + 2]
      dst.data[di + 3] = src.data[si + 3]
    }
  }
}

/** Horizontal mirror — used to derive west/SW/NW directions from east/SE/NE. */
export function flipH(src: Img): Img {
  const out = newImg(src.width, src.height)
  for (let y = 0; y < src.height; y++) {
    for (let x = 0; x < src.width; x++) {
      const si = (y * src.width + x) * 4
      const di = (y * src.width + (src.width - 1 - x)) * 4
      out.data[di] = src.data[si]
      out.data[di + 1] = src.data[si + 1]
      out.data[di + 2] = src.data[si + 2]
      out.data[di + 3] = src.data[si + 3]
    }
  }
  return out
}

// ── Resampling (premultiplied Catmull-Rom, separable) ───────────────────────

function catmullRom(t: number): number {
  t = Math.abs(t)
  if (t < 1) return 1.5 * t * t * t - 2.5 * t * t + 1
  if (t < 2) return -0.5 * t * t * t + 2.5 * t * t - 4 * t + 2
  return 0
}

/** Per-output-pixel filter taps for one axis (source indices + normalized weights). */
interface Taps {
  starts: Int32Array // first source index contributing to each dst pixel
  counts: Int32Array // number of taps for each dst pixel
  weights: Float64Array // flattened weights, grouped by dst pixel
  offsets: Int32Array // offset into `weights` for each dst pixel
}

function buildTaps(srcSize: number, dstSize: number): Taps {
  const ratio = srcSize / dstSize
  const filterScale = Math.max(ratio, 1) // widen kernel when minifying (low-pass)
  const support = 2 * filterScale
  const starts = new Int32Array(dstSize)
  const counts = new Int32Array(dstSize)
  const offsets = new Int32Array(dstSize)
  const flat: number[] = []
  for (let i = 0; i < dstSize; i++) {
    const center = (i + 0.5) * ratio - 0.5
    let left = Math.ceil(center - support)
    let right = Math.floor(center + support)
    if (left < 0) left = 0
    if (right > srcSize - 1) right = srcSize - 1
    offsets[i] = flat.length
    starts[i] = left
    counts[i] = right - left + 1
    let sum = 0
    const w: number[] = []
    for (let j = left; j <= right; j++) {
      const weight = catmullRom((j - center) / filterScale)
      w.push(weight)
      sum += weight
    }
    if (sum === 0) sum = 1
    for (const weight of w) flat.push(weight / sum)
  }
  return { starts, counts, weights: Float64Array.from(flat), offsets }
}

/**
 * Resample `src` to `dstW × dstH` with a separable Catmull-Rom filter in
 * premultiplied alpha. Quality-equivalent to `xdraw.CatmullRom.Scale`.
 */
export function resample(src: Img, dstW: number, dstH: number): Img {
  if (dstW === src.width && dstH === src.height) return cloneImg(src)
  const { width: sw, height: sh } = src

  // Straight RGBA → premultiplied float planes.
  const n = sw * sh
  const pr = new Float64Array(n)
  const pg = new Float64Array(n)
  const pb = new Float64Array(n)
  const pa = new Float64Array(n)
  for (let p = 0; p < n; p++) {
    const a = src.data[p * 4 + 3]
    const f = a / 255
    pr[p] = src.data[p * 4] * f
    pg[p] = src.data[p * 4 + 1] * f
    pb[p] = src.data[p * 4 + 2] * f
    pa[p] = a
  }

  // Horizontal pass: (sw × sh) → (dstW × sh).
  const tx = buildTaps(sw, dstW)
  const hr = new Float64Array(dstW * sh)
  const hg = new Float64Array(dstW * sh)
  const hb = new Float64Array(dstW * sh)
  const ha = new Float64Array(dstW * sh)
  for (let y = 0; y < sh; y++) {
    const rowBase = y * sw
    for (let x = 0; x < dstW; x++) {
      let ar = 0, ag = 0, ab = 0, aa = 0
      const start = tx.starts[x]
      const cnt = tx.counts[x]
      const off = tx.offsets[x]
      for (let k = 0; k < cnt; k++) {
        const w = tx.weights[off + k]
        const si = rowBase + start + k
        ar += pr[si] * w
        ag += pg[si] * w
        ab += pb[si] * w
        aa += pa[si] * w
      }
      const di = y * dstW + x
      hr[di] = ar; hg[di] = ag; hb[di] = ab; ha[di] = aa
    }
  }

  // Vertical pass: (dstW × sh) → (dstW × dstH).
  const ty = buildTaps(sh, dstH)
  const out = newImg(dstW, dstH)
  for (let x = 0; x < dstW; x++) {
    for (let y = 0; y < dstH; y++) {
      let ar = 0, ag = 0, ab = 0, aa = 0
      const start = ty.starts[y]
      const cnt = ty.counts[y]
      const off = ty.offsets[y]
      for (let k = 0; k < cnt; k++) {
        const w = ty.weights[off + k]
        const si = (start + k) * dstW + x
        ar += hr[si] * w
        ag += hg[si] * w
        ab += hb[si] * w
        aa += ha[si] * w
      }
      // Un-premultiply.
      const di = (y * dstW + x) * 4
      const a = aa <= 0 ? 0 : aa >= 255 ? 255 : aa
      if (a <= 0) {
        out.data[di] = 0; out.data[di + 1] = 0; out.data[di + 2] = 0; out.data[di + 3] = 0
      } else {
        const inv = 255 / a
        out.data[di] = u8(ar * inv)
        out.data[di + 1] = u8(ag * inv)
        out.data[di + 2] = u8(ab * inv)
        out.data[di + 3] = Math.round(a)
      }
    }
  }
  return out
}
