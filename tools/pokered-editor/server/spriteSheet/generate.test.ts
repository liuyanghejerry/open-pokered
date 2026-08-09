// ───────────────────────────────────────────────────────────────────────────
// Phase-4 tests: the integration layer — multi-state generation, 8-direction
// mirroring, and atlas composition (with a mock AI image source).
// ───────────────────────────────────────────────────────────────────────────
import { describe, expect, it } from 'vitest'
import { type Img, flipH, idx, newImg } from './image'
import { type GenerateImageFn } from './pipeline'
import { type AnimatedState, generateAnimatedSprite } from './generate'
import type { ImageProviderProfile } from '../ai'

function magentaFill(img: Img): void {
  for (let i = 0; i + 3 < img.data.length; i += 4) { img.data[i] = 255; img.data[i + 1] = 0; img.data[i + 2] = 255; img.data[i + 3] = 255 }
}
function fillBox(img: Img, x0: number, y0: number, x1: number, y1: number, r: number, g: number, b: number): void {
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) { const i = idx(img.width, x, y); img.data[i] = r; img.data[i + 1] = g; img.data[i + 2] = b; img.data[i + 3] = 255 }
}
// Asymmetric poses (torso + a right-side arm), so a horizontal mirror is detectably different.
function asymStrip(cols: number): Img {
  const colW = 200, h = 200
  const img = newImg(cols * colW, h)
  magentaFill(img)
  for (let i = 0; i < cols; i++) {
    const cx = i * colW + colW / 2
    fillBox(img, Math.round(cx - 22), 55, Math.round(cx + 22), 144, 60, 120, 200) // torso
    fillBox(img, Math.round(cx + 22), 70, Math.round(cx + 60), 80, 60, 120, 200) // right arm
  }
  return img
}

const profile: ImageProviderProfile = { id: 't', kind: 'openai', baseURL: '', model: 'img' }

describe('animated generation', () => {
  it('generates a state and mirrors its pair via horizontal flip', async () => {
    const genImage: GenerateImageFn = async () => asymStrip(4)
    const states: AnimatedState[] = [
      { name: 'walk-east', frames: 4, fps: 10, loop: true, action: '', facing: 'east' },
      { name: 'walk-west', frames: 4, fps: 10, loop: true, action: '', facing: 'west', mirrorOf: 'walk-east' },
    ]
    const { states: results, sheet, manifest } = await generateAnimatedSprite(
      { profile, apiKey: 'k', character: 'hero', description: 'a knight', styleKey: 'pixel', states, cellSize: 64 },
      genImage,
    )
    const east = results[0], west = results[1]
    expect(east.found).toBe(4)
    expect(west.frames.length).toBe(4)
    // west is exactly the horizontal mirror of east…
    expect(Array.from(west.frames[0].data)).toEqual(Array.from(flipH(east.frames[0]).data))
    // …and the asymmetric pose makes it genuinely different from east.
    expect(Array.from(west.frames[0].data)).not.toEqual(Array.from(east.frames[0].data))
    // atlas: 2 rows (states) × maxFrames cols, manifest has both
    expect(sheet.height).toBe(2 * 64)
    expect(sheet.width).toBe(4 * 64)
    expect(Object.keys(manifest.animations).sort()).toEqual(['walk-east', 'walk-west'])
  })

  it('reports per-state progress with state index', async () => {
    const seen: Array<{ i: number; total: number }> = []
    const genImage: GenerateImageFn = async () => asymStrip(2)
    await generateAnimatedSprite(
      {
        profile, apiKey: 'k', character: 'hero', description: 'a knight', styleKey: 'pixel', cellSize: 64,
        states: [{ name: 'idle', frames: 2, fps: 6, loop: true, action: '', facing: '' }],
        onProgress: (e) => seen.push({ i: e.stateIndex, total: e.totalStates }),
      },
      genImage,
    )
    expect(seen.length).toBeGreaterThan(0)
    expect(seen[0].total).toBe(1)
  })

  it('accepts a gemini image provider (kind branch)', async () => {
    // The mock genImage is injected, so we only exercise the kind/model validation
    // and orchestration — not the real Gemini call.
    const gem: ImageProviderProfile = { id: 'g', kind: 'gemini', baseURL: '', model: 'gemini-2.5-flash-image' }
    const { states } = await generateAnimatedSprite(
      { profile: gem, apiKey: 'k', character: 'h', description: 'd', styleKey: 'pixel', cellSize: 64, states: [{ name: 'idle', frames: 2, fps: 6, loop: true, action: '', facing: '' }] },
      async () => asymStrip(2),
    )
    expect(states[0].found).toBe(2)
  })

  it('rejects an image provider with no model', async () => {
    await expect(generateAnimatedSprite(
      { profile: { id: 'x', kind: 'openai', baseURL: '', model: '' }, apiKey: 'k', character: 'h', description: 'd', styleKey: 'pixel', cellSize: 64, states: [{ name: 'idle', frames: 2, fps: 6, loop: true, action: '', facing: '' }] },
      async () => asymStrip(2),
    )).rejects.toThrow(/no model/)
  })
})
