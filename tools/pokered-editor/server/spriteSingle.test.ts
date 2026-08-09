// ───────────────────────────────────────────────────────────────────────────
// spriteSingle tests — the pokered one-shot static-sprite flow, with a mock AI
// image source (magenta field + dark subject box, like the pipeline tests).
// ───────────────────────────────────────────────────────────────────────────
import { describe, expect, it } from 'vitest'
import { type Img, decodePNG, idx, newImg } from './spriteSheet/image'
import type { GenerateImageFn } from './spriteSheet/pipeline'
import { flattenOverWhite, generateSingleSprite } from './spriteSingle'
import type { ImageProviderProfile } from './ai'

const profile: ImageProviderProfile = { id: 't', kind: 'openai', baseURL: '', model: 'img' }

/** 64×64 magenta field with a centered dark box (the "subject"). */
function fakeAiImage(): Img {
  const img = newImg(64, 64)
  for (let i = 0; i + 3 < img.data.length; i += 4) {
    img.data[i] = 255; img.data[i + 1] = 0; img.data[i + 2] = 255; img.data[i + 3] = 255
  }
  for (let y = 16; y < 48; y++) {
    for (let x = 16; x < 48; x++) {
      const i = idx(img.width, x, y)
      img.data[i] = 40; img.data[i + 1] = 40; img.data[i + 2] = 60; img.data[i + 3] = 255
    }
  }
  return img
}

function px(img: Img, x: number, y: number): [number, number, number, number] {
  const i = idx(img.width, x, y)
  return [img.data[i], img.data[i + 1], img.data[i + 2], img.data[i + 3]]
}

describe('flattenOverWhite', () => {
  it('keeps opaque pixels and flattens transparent ones to white', () => {
    const img = newImg(2, 1)
    // pixel 0: opaque dark; pixel 1: fully transparent red (color should not leak)
    img.data.set([40, 40, 60, 255, 200, 0, 0, 0])
    const out = flattenOverWhite(img)
    expect(px(out, 0, 0)).toEqual([40, 40, 60, 255])
    expect(px(out, 1, 0)).toEqual([255, 255, 255, 255])
  })

  it('blends half-alpha pixels toward white', () => {
    const img = newImg(1, 1)
    img.data.set([0, 0, 0, 128])
    const [r, g, b, a] = px(flattenOverWhite(img), 0, 0)
    expect(a).toBe(255)
    expect(r).toBeGreaterThan(120)
    expect(r).toBeLessThan(136)
    expect(g).toBe(r)
    expect(b).toBe(r)
  })
})

describe('generateSingleSprite', () => {
  const genImage: GenerateImageFn = async () => fakeAiImage()

  it('mattes, downscales and returns a white-background PNG of the target size', async () => {
    const res = await generateSingleSprite(
      // paletteSize 0: skip quantization so the flatten/matte assertions are exact.
      { profile, apiKey: 'k', prompt: 'a small bird creature', width: 16, height: 16, paletteSize: 0 },
      genImage,
    )
    expect(res.mediaType).toBe('image/png')
    expect(res.width).toBe(16)
    expect(res.height).toBe(16)
    const img = decodePNG(Buffer.from(res.base64, 'base64'))
    expect(img.width).toBe(16)
    expect(img.height).toBe(16)
    // magenta background is gone → flattened white corner
    expect(px(img, 0, 0)).toEqual([255, 255, 255, 255])
    // the subject survives in the middle (darkish, far from white)
    const [r, g, b, a] = px(img, 8, 8)
    expect(a).toBe(255)
    expect(255 - r + (255 - g) + (255 - b)).toBeGreaterThan(200)
  })

  it('applies the default palette cap without losing subject/background separation', async () => {
    const res = await generateSingleSprite(
      { profile, apiKey: 'k', prompt: 'a small bird creature', width: 16, height: 16 },
      genImage,
    )
    const img = decodePNG(Buffer.from(res.base64, 'base64'))
    const [cr, cg, cb] = px(img, 0, 0)
    expect(Math.min(cr, cg, cb)).toBeGreaterThan(230) // corner still (near-)white
    const [r, g, b] = px(img, 8, 8)
    expect(255 - r + (255 - g) + (255 - b)).toBeGreaterThan(200) // center still dark
  })

  it('clamps target size into 8–512', async () => {
    const res = await generateSingleSprite(
      { profile, apiKey: 'k', prompt: 'x', width: 9999, height: 1 },
      genImage,
    )
    expect(res.width).toBe(512)
    expect(res.height).toBe(8)
  })

  it('rejects a blank prompt before calling the provider', async () => {
    let called = false
    await expect(
      generateSingleSprite({ profile, apiKey: 'k', prompt: '   ', width: 16, height: 16 }, async () => {
        called = true
        return fakeAiImage()
      }),
    ).rejects.toThrow('prompt')
    expect(called).toBe(false)
  })
})
