// ───────────────────────────────────────────────────────────────────────────
// spriteSingle.ts — pokered: ONE static sprite PNG from a text prompt.
//
// Unlike the animated sheet pipeline (generate.ts/pipeline.ts, multi-state film
// strips + SSE) and unlike server/sprite.ts (bound to Story-Designer character
// records, OpenAI-only), this is a single-shot flow for the Pixel activity:
//
//   prompt → provider image → matte background (chroma) → pixel-grid snap →
//   resample to the canvas size → flatten over white → optional palette cap
//
// Every step reuses the ported spriteSheet primitives; the AI call is injectable
// so the module is testable without a provider.
// ───────────────────────────────────────────────────────────────────────────
import type { ImageProviderProfile } from './ai'
import { type Img, cloneImg, encodePNG, resample } from './spriteSheet/image'
import { removeBackground } from './spriteSheet/chroma'
import { detectPixelScale, pixelize } from './spriteSheet/pixelize'
import { buildSharedPalette, applyPalette } from './spriteSheet/quantize'
import { makeGenImage } from './spriteSheet/generate'
import type { GenerateImageFn } from './spriteSheet/pipeline'

export interface SingleSpriteParams {
  profile: ImageProviderProfile
  apiKey: string
  prompt: string
  /** Target canvas size in pixels (clamped to 8–512). */
  width: number
  height: number
  /** Median-cut palette cap applied after downscaling (≤1 disables). Default 16. */
  paletteSize?: number
}

export interface SingleSpriteResult {
  base64: string
  mediaType: 'image/png'
  width: number
  height: number
}

const MIN_SIZE = 8
const MAX_SIZE = 512

function clampSize(v: number, fallback: number): number {
  const n = Math.trunc(Number(v))
  if (!Number.isFinite(n) || n <= 0) return fallback
  return Math.max(MIN_SIZE, Math.min(MAX_SIZE, n))
}

/**
 * flattenOverWhite — composite transparency onto opaque white. pokered battle /
 * overworld sprites are white-background PNGs (the Pixel editor's eraser paints
 * white too), so generated sprites enter the canvas in the same shape.
 */
export function flattenOverWhite(img: Img): Img {
  const out = cloneImg(img)
  const d = out.data
  for (let i = 0; i + 3 < d.length; i += 4) {
    const a = d[i + 3]
    if (a >= 255) continue
    const f = a / 255
    d[i] = Math.round(d[i] * f + 255 * (1 - f))
    d[i + 1] = Math.round(d[i + 1] * f + 255 * (1 - f))
    d[i + 2] = Math.round(d[i + 2] * f + 255 * (1 - f))
    d[i + 3] = 255
  }
  return out
}

/**
 * generateSingleSprite — one prompt → one finished sprite PNG (base64).
 * The default `genImage` is the provider-backed call (OpenAI-compatible or
 * Gemini, same as the animated pipeline); tests inject a fake.
 */
export async function generateSingleSprite(
  p: SingleSpriteParams,
  genImage: GenerateImageFn = makeGenImage(p.profile, p.apiKey),
): Promise<SingleSpriteResult> {
  if (p.profile.kind !== 'openai' && p.profile.kind !== 'gemini') {
    throw new Error('Sprite generation needs an image provider (OpenAI-compatible or Gemini).')
  }
  if (!p.profile.model) throw new Error('This image provider has no model configured.')
  if (!p.prompt || !p.prompt.trim()) throw new Error('A prompt is required.')

  const width = clampSize(p.width, 56)
  const height = clampSize(p.height, 56)

  // Magenta background is what the chroma-matting step keys out best (its
  // fallbacks also handle white/other flat keys, so this is a hint, not a contract).
  const fullPrompt = [
    'A single 2D video-game sprite, pixel-art style, one subject only, centered, full body,',
    'clean readable silhouette, on a flat solid magenta background (#ff00ff), no text, no border, no shadow.',
    p.prompt.trim(),
  ].join(' ')

  const raw = await genImage(fullPrompt, '1:1', [])
  let img = removeBackground(raw)
  // Snap the AI's fake-pixel blocks to a true grid before downscaling.
  img = pixelize(img, detectPixelScale(img))
  img = resample(img, width, height)
  img = flattenOverWhite(img)

  const paletteSize = p.paletteSize === undefined ? 16 : Math.trunc(Number(p.paletteSize))
  if (paletteSize > 1) {
    const palette = buildSharedPalette([img], paletteSize)
    if (palette) applyPalette(img, palette)
  }

  return { base64: encodePNG(img).toString('base64'), mediaType: 'image/png', width, height }
}
