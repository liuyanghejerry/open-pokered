// ───────────────────────────────────────────────────────────────────────────
// image.ts tests: PNG codec round-trip (the endpoint writes real PNGs via this)
// and the horizontal mirror.
// ───────────────────────────────────────────────────────────────────────────
import { describe, expect, it } from 'vitest'
import { type Img, decodePNG, encodePNG, flipH, idx, newImg } from './image'

function fillBox(img: Img, x0: number, y0: number, x1: number, y1: number, r: number, g: number, b: number, a = 255): void {
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
    const i = idx(img.width, x, y); img.data[i] = r; img.data[i + 1] = g; img.data[i + 2] = b; img.data[i + 3] = a
  }
}

describe('PNG codec', () => {
  it('round-trips an RGBA image through encode → decode', () => {
    const img = newImg(20, 12)
    fillBox(img, 2, 2, 9, 9, 200, 30, 30, 255) // opaque red
    fillBox(img, 12, 2, 17, 9, 30, 30, 200, 128) // semi-transparent blue
    const buf = encodePNG(img)
    expect(buf[0]).toBe(0x89) // PNG signature
    const back = decodePNG(buf)
    expect(back.width).toBe(20)
    expect(back.height).toBe(12)
    expect(Array.from(back.data)).toEqual(Array.from(img.data)) // straight alpha preserved
  })
})

describe('flipH', () => {
  it('is its own inverse', () => {
    const img = newImg(16, 8)
    fillBox(img, 0, 0, 3, 7, 10, 20, 30) // left stripe only → asymmetric
    const twice = flipH(flipH(img))
    expect(Array.from(twice.data)).toEqual(Array.from(img.data))
    expect(Array.from(flipH(img).data)).not.toEqual(Array.from(img.data))
  })
})
