// ───────────────────────────────────────────────────────────────────────────
// generate.ts — integration layer: drives the pipeline across multiple animation
// states (and 8-direction sets with mirroring), and adapts the Vercel AI SDK as
// the image source. The actual AI call is injectable so the orchestration is
// testable without a provider.
// ───────────────────────────────────────────────────────────────────────────
import type { ImageProviderProfile } from '../ai'
import { type Img, decodePNG, encodePNG, flipH } from './image'
import { geminiGenerateImage } from './gemini'
import { proxyFetchFn } from '../proxy'
import { type GenerateImageFn, type ProgressEvent, type StateResult, generateState } from './pipeline'
import { composeAtlas } from './atlas'
import type { Manifest, StateFrames, StateSpec } from './types'

/** A state to generate. `mirrorOf` derives it by horizontally mirroring another state's frames. */
export interface AnimatedState extends StateSpec {
  mirrorOf?: string
}

export interface AnimatedGenParams {
  profile: ImageProviderProfile
  apiKey: string
  character: string // id, for the manifest
  description: string
  styleKey: string
  styleCustom?: string
  states: AnimatedState[]
  cellSize: number
  margin?: number
  base?: Img | null
  feedback?: string
  signal?: AbortSignal
  onProgress?: (e: ProgressEvent & { stateIndex: number; totalStates: number }) => void
}

export interface AnimatedGenResult {
  states: StateResult[]
  sheet: Img
  manifest: Manifest
}

/** Build the AI image call from an image-provider profile (OpenAI-compatible or Gemini). */
export function makeGenImage(profile: ImageProviderProfile, apiKey: string): GenerateImageFn {
  return async (prompt, aspect, refs) => {
    if (profile.kind === 'gemini') {
      // Gemini generateContent is multimodal — pass the reference images so the
      // base character locks identity across the strip.
      return geminiGenerateImage({ baseURL: profile.baseURL, apiKey, model: profile.model, prompt, refs, proxyUrl: profile.proxyUrl })
    }
    // OpenAI-compatible images API: text→image (refs not supported here; identity
    // comes from the brief + shared-palette quantization + drift-detection retries,
    // and the base image still drives the server-side identity CHECK in pipeline.ts).
    const { generateImage } = await import('ai')
    const { createOpenAICompatible } = await import('@ai-sdk/openai-compatible')
    const proxyFetch = await proxyFetchFn(profile.proxyUrl)
    const provider = createOpenAICompatible({ name: profile.id || 'openai', apiKey, baseURL: profile.baseURL, ...(proxyFetch ? { fetch: proxyFetch } : {}) })
    const model = provider.imageModel(profile.model)
    // Pass aspectRatio (the right concept for variable-width strips); providers
    // that don't support it fall back to their default size and the pipeline
    // still segments by content. Cast: the SDK's param union varies by version.
    const result = await generateImage({ model, prompt, aspectRatio: aspect, n: 1 } as any)
    return decodePNG(Buffer.from(result.image.base64, 'base64'))
  }
}

/**
 * generateAnimatedSprite — run the pipeline for each state, derive mirror states
 * by horizontal flip, and compose a sprite sheet + manifest. `genImage` defaults
 * to the provider-backed call but can be injected for testing.
 */
export async function generateAnimatedSprite(
  p: AnimatedGenParams,
  genImage: GenerateImageFn = makeGenImage(p.profile, p.apiKey),
): Promise<AnimatedGenResult> {
  if (p.profile.kind !== 'openai' && p.profile.kind !== 'gemini') {
    throw new Error('Animated sprite generation needs an image provider (OpenAI-compatible or Gemini).')
  }
  if (!p.profile.model) throw new Error('This image provider has no model configured.')
  if (!p.states.length) throw new Error('At least one animation state is required.')

  const results: (StateResult | null)[] = new Array(p.states.length).fill(null)
  const byName = new Map<string, StateResult>()

  // Pass 1: AI-generated states (no mirrorOf).
  let aiIndex = 0
  const aiTotal = p.states.filter((s) => !s.mirrorOf).length
  for (let i = 0; i < p.states.length; i++) {
    const spec = p.states[i]
    if (spec.mirrorOf) continue
    const r = await generateState(
      {
        description: p.description, styleKey: p.styleKey, styleCustom: p.styleCustom,
        state: spec, base: p.base ?? null, cellSize: p.cellSize, margin: p.margin, feedback: p.feedback,
      },
      genImage,
      { signal: p.signal, onProgress: (e) => p.onProgress?.({ ...e, stateIndex: aiIndex, totalStates: aiTotal }) },
    )
    results[i] = r
    byName.set(spec.name, r)
    aiIndex++
  }

  // Pass 2: mirror states (derive by horizontal flip of the source's frames).
  for (let i = 0; i < p.states.length; i++) {
    const spec = p.states[i]
    if (!spec.mirrorOf) continue
    const src = byName.get(spec.mirrorOf)
    if (!src) throw new Error(`Mirror source state "${spec.mirrorOf}" was not generated for "${spec.name}".`)
    const frames = src.frames.map(flipH)
    const r: StateResult = {
      name: spec.name, frames, rawStrip: src.rawStrip ? flipH(src.rawStrip) : null,
      expected: spec.frames, found: frames.length, warnings: [], scores: src.scores,
    }
    results[i] = r
    byName.set(spec.name, r)
  }

  const finalResults = results.map((r, i) => r ?? emptyStateResult(p.states[i]))
  const stateFrames: StateFrames[] = p.states.map((spec, i) => ({ spec, frames: finalResults[i].frames }))
  const { sheet, manifest } = composeAtlas(p.character, stateFrames, p.cellSize, p.cellSize)
  return { states: finalResults, sheet, manifest }
}

function emptyStateResult(spec: StateSpec): StateResult {
  return { name: spec.name, frames: [], rawStrip: null, expected: spec.frames, found: 0, warnings: ['Not generated.'], scores: { identity: 0, motion: 0, contact: 0, overall: 0 } }
}

/**
 * testImageProvider — render one tiny image to verify a provider profile + key.
 * Never throws; returns a small PNG preview (base64) on success.
 */
export async function testImageProvider(
  profile: ImageProviderProfile,
  apiKey: string,
): Promise<{ ok: boolean; error?: string; base64?: string; width?: number; height?: number }> {
  try {
    if (profile.kind !== 'openai' && profile.kind !== 'gemini') return { ok: false, error: 'Unknown image provider kind.' }
    if (!profile.model) return { ok: false, error: 'No image model configured.' }
    const img = await makeGenImage(profile, apiKey)(
      'A single flat magenta square centered on a plain white background. Simple, flat, no text.',
      '1:1',
      [],
    )
    return { ok: true, base64: encodePNG(img).toString('base64'), width: img.width, height: img.height }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}
