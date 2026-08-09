// ───────────────────────────────────────────────────────────────────────────
// Phase-3 tests: prompts, presets, atlas, and the end-to-end self-correcting
// orchestration loop (with a mock AI that draws a synthetic magenta filmstrip).
// ───────────────────────────────────────────────────────────────────────────
import { describe, expect, it } from 'vitest'
import { type Img, idx, newImg } from './image'
import { buildStripPrompt, resolveStyle } from './prompt'
import { motionHint, listPresets } from './presets'
import { composeAtlas } from './atlas'
import { buildAsepriteJSON } from './aseprite'
import { generateState, type GenerateImageFn } from './pipeline'
import type { StateFrames, StateSpec } from './types'

function magentaFill(img: Img): void {
  for (let i = 0; i + 3 < img.data.length; i += 4) {
    img.data[i] = 255; img.data[i + 1] = 0; img.data[i + 2] = 255; img.data[i + 3] = 255
  }
}
function fillBox(img: Img, x0: number, y0: number, x1: number, y1: number, r: number, g: number, b: number): void {
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
    const i = idx(img.width, x, y); img.data[i] = r; img.data[i + 1] = g; img.data[i + 2] = b; img.data[i + 3] = 255
  }
}
// Draw `cols` evenly-spaced poses on a magenta background, with a slight per-frame
// vertical bob so the frames aren't byte-identical (avoids the static warning).
function posesStrip(cols: number): Img {
  const colW = 200, h = 200, boxW = 44, boxH = 90
  const img = newImg(cols * colW, h)
  magentaFill(img)
  for (let i = 0; i < cols; i++) {
    const cx = i * colW + colW / 2
    const x0 = Math.round(cx - boxW / 2)
    const y0 = Math.round((h - boxH) / 2) + i * 3
    fillBox(img, x0, y0, x0 + boxW - 1, y0 + boxH - 1, 60, 120, 200)
  }
  return img
}

const walk: StateSpec = { name: 'walk', frames: 4, fps: 10, loop: true, action: '', facing: '' }

describe('prompts & presets', () => {
  it('buildStripPrompt locks the exact pose count, facing and choreography', () => {
    const spec: StateSpec = { name: 'attack', frames: 5, fps: 12, loop: false, action: '', facing: 'east' }
    const p = buildStripPrompt('a knight', resolveStyle('pixel', ''), spec, '')
    expect(p).toContain('exactly 5 game-sprite poses')
    expect(p).toContain('right-side profile view') // facing=east lock
    expect(p).toContain('Choreography:') // motion hint injected
    expect(p).toContain('pixel-art game sprite') // resolved style
  })

  it('motionHint strips a direction suffix to find the base keyword', () => {
    expect(motionHint('attack')).toContain('Melee attack')
    expect(motionHint('attack-south-east')).toBe(motionHint('attack'))
    expect(motionHint('walk-north')).toBe(motionHint('walk'))
  })

  it('listPresets returns the catalog without the backend-only hint', () => {
    const list = listPresets()
    expect(list.length).toBe(100)
    expect(list.every((p) => !('hint' in p))).toBe(true)
  })

  it('buildStripPrompt appends artist feedback last', () => {
    const p = buildStripPrompt('a knight', resolveStyle('pixel', ''), walk, 'make the cape longer')
    expect(p).toContain('Artist revision (apply over everything above): make the cape longer')
  })
})

describe('atlas composition', () => {
  it('lays out rows and builds a v2 manifest with a foot pivot', () => {
    const f = () => { const c = newImg(64, 64); fillBox(c, 24, 10, 39, 55, 60, 120, 200); return c }
    const states: StateFrames[] = [
      { spec: { name: 'idle', frames: 2, fps: 6, loop: true, action: '', facing: '' }, frames: [f(), f()] },
      { spec: { name: 'walk', frames: 3, fps: 10, loop: true, action: '', facing: '' }, frames: [f(), f(), f()] },
    ]
    const { sheet, manifest } = composeAtlas('hero', states, 64, 64)
    expect(sheet.width).toBe(3 * 64) // max frames across states
    expect(sheet.height).toBe(2 * 64)
    expect(Object.keys(manifest.animations)).toEqual(['idle', 'walk'])
    expect(manifest.animations.walk.frames).toBe(3)
    expect(manifest.animations.idle.pivot.x).toBe(32) // cell center
    expect(manifest.version).toBe(2)
  })

  it('exports Aseprite-compatible JSON with a frameTag per state', () => {
    const f = () => { const c = newImg(64, 64); fillBox(c, 24, 10, 39, 55, 60, 120, 200); return c }
    const states: StateFrames[] = [
      { spec: { name: 'idle', frames: 2, fps: 6, loop: true, action: '', facing: '' }, frames: [f(), f()] },
      { spec: { name: 'attack', frames: 3, fps: 12, loop: false, action: '', facing: '' }, frames: [f(), f(), f()] },
    ]
    const { manifest } = composeAtlas('hero', states, 64, 64)
    const ase = JSON.parse(buildAsepriteJSON(manifest))
    expect(ase.frames.length).toBe(5) // 2 + 3
    expect(ase.meta.frameTags.map((t: any) => t.name)).toEqual(['idle', 'attack'])
    const attackTag = ase.meta.frameTags.find((t: any) => t.name === 'attack')
    expect(attackTag.from).toBe(2)
    expect(attackTag.to).toBe(4)
    expect(attackTag.repeat).toBe('1') // non-looping → play once
    expect(ase.frames[0].duration).toBe(Math.trunc(1000 / 6))
  })
})

describe('self-correcting orchestration', () => {
  it('extracts exactly the requested frames from a clean filmstrip', async () => {
    const genImage: GenerateImageFn = async () => posesStrip(4)
    const res = await generateState(
      { description: 'a knight', styleKey: 'pixel', state: walk, base: null },
      genImage,
    )
    expect(res.found).toBe(4)
    expect(res.frames.length).toBe(4)
    expect(res.warnings.some((w) => /could not|cancelled/i.test(w))).toBe(false)
  })

  it('retries with a measurement-driven hint when the count is wrong', async () => {
    const prompts: string[] = []
    let call = 0
    const genImage: GenerateImageFn = async (prompt) => {
      prompts.push(prompt)
      call++
      return call === 1 ? newImgMagenta(800, 200) /* blank → 0 poses */ : posesStrip(4)
    }
    const res = await generateState(
      { description: 'a knight', styleKey: 'pixel', state: walk, base: null },
      genImage,
    )
    expect(call).toBe(2)
    expect(res.found).toBe(4)
    expect(prompts[1]).toContain('EXACTLY 4 are required') // correction injected into retry
  })

  it('reports progress phases', async () => {
    const phases: string[] = []
    const genImage: GenerateImageFn = async () => posesStrip(4)
    await generateState(
      { description: 'a knight', styleKey: 'pixel', state: walk, base: null },
      genImage,
      { onProgress: (e) => phases.push(e.phase) },
    )
    expect(phases).toContain('generate')
    expect(phases).toContain('extract')
  })
})

function newImgMagenta(w: number, h: number): Img {
  const img = newImg(w, h); magentaFill(img); return img
}
