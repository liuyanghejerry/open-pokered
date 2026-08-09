// ───────────────────────────────────────────────────────────────────────────
// Vendor-agnostic AI backend for the Story Designer.
//
// Runs inside the Vite dev-server middleware (Node). Supports any
// Anthropic-shape or OpenAI-shape vendor via the Vercel AI SDK, with a custom
// baseURL. API keys are supplied per-request (they live in the browser's
// localStorage) and are never persisted here.
//
// The heavy deps (ai, @ai-sdk/*, zod) are imported dynamically so the dev
// server starts fast and works even when AI is never used.
// ───────────────────────────────────────────────────────────────────────────

import { proxyFetchFn } from './proxy'

export interface ProviderProfile {
  id: string
  kind: 'anthropic' | 'openai'
  baseURL: string
  model: string
  /** Optional HTTP(S) proxy for reaching the provider, e.g. http://127.0.0.1:9085. */
  proxyUrl?: string
  /** Optional embedding model id (openai-compatible) — enables retrieval/RAG. */
  embeddingModel?: string
  /** @deprecated image generation uses ImageProviderProfile (separate config). */
  imageModel?: string
}

/**
 * Provider profile for IMAGE generation, kept separate from text providers.
 * `openai` = OpenAI-compatible images API; `gemini` = Google `generateContent`
 * (Nano Banana). `model` is the image model id.
 */
export interface ImageProviderProfile {
  id: string
  kind: 'openai' | 'gemini'
  baseURL: string
  model: string
  /** Optional HTTP(S) proxy for reaching the provider, e.g. http://127.0.0.1:9085. */
  proxyUrl?: string
}

export interface RefineParams {
  profile: ProviderProfile
  apiKey: string
  /** Existing character record (preserved id/name, enriched fields). */
  character: Record<string, unknown>
  /** Locale codes for localized text fields. */
  locales: string[]
  /** Assembled project context (DSL guide excerpts, example scenes, …). */
  context: string
  /** Streaming sink: emits ("partial" | "done" | "error", payload). */
  onEvent: (event: string, data: unknown) => void
}

/**
 * Smoke-test a provider profile + transient key with a tiny prompt. Returns the
 * model's reply on success, or a human-readable error. Never throws — the caller
 * surfaces `{ ok, text?, error? }` directly to the UI.
 */
export async function testProvider(
  profile: ProviderProfile,
  apiKey: string,
  prompt?: string,
): Promise<{ ok: boolean; text?: string; error?: string }> {
  try {
    const { generateText } = await import('ai')
    const model = await buildModel(profile, apiKey)
    const { text } = await generateText({
      model,
      prompt: prompt?.trim() || 'Reply with a single word: OK.',
    })
    return { ok: true, text: (text || '').trim() }
  } catch (e) {
    return { ok: false, error: (e as Error).message }
  }
}

/** Build a LanguageModel for the given profile + transient key. */
export async function buildModel(profile: ProviderProfile, apiKey: string): Promise<any> {
  const fetchFn = await proxyFetchFn(profile.proxyUrl)
  if (profile.kind === 'anthropic') {
    const { createAnthropic } = await import('@ai-sdk/anthropic')
    const provider = createAnthropic({ apiKey, baseURL: profile.baseURL || undefined, ...(fetchFn ? { fetch: fetchFn } : {}) })
    return provider(profile.model)
  }
  // openai-compatible covers OpenAI, DeepSeek, Moonshot, OpenRouter, Ollama, vLLM, …
  const { createOpenAICompatible } = await import('@ai-sdk/openai-compatible')
  const provider = createOpenAICompatible({
    name: profile.id || 'openai',
    apiKey,
    baseURL: profile.baseURL,
    ...(fetchFn ? { fetch: fetchFn } : {}),
  })
  return provider(profile.model)
}

/**
 * Refine a character profile (人设) into an enriched, game-ready version.
 * Streams partial objects via onEvent("partial", …) and resolves to the final
 * validated object. id/name are preserved by the caller on accept.
 */
export async function refineCharacter(p: RefineParams): Promise<Record<string, unknown>> {
  const { streamObject } = await import('ai')
  const { z } = await import('zod')

  const model = await buildModel(p.profile, p.apiKey)

  const schema = z.object({
    role: z.string().describe('Short role label, e.g. mentor, rival, antagonist'),
    appearance: z.string().describe('Physical description, usable as a pixel-art sprite brief'),
    personality: z.string(),
    backstory: z.string(),
    motivation: z.string(),
    speechStyle: z.string().describe('How they talk — tone and verbal tics'),
    relationships: z
      .array(z.object({ to: z.string(), kind: z.string() }))
      .describe('Relationships to other characters by id'),
    spriteSpec: z
      .object({
        palette: z.array(z.string()).describe('A few hex or named colours (GB-style, limited)'),
        poses: z.array(z.string()),
        size: z.string().describe('e.g. "16x16" overworld or "56x56" battle'),
        style: z.string(),
        notes: z.string(),
      })
      .describe('A brief an image model could render into a sprite'),
  })

  const system = [
    'You are a narrative designer fleshing out a character profile for a 2D JRPG.',
    'Stay consistent with fields already provided; enrich thin or empty ones. Do not contradict given facts.',
    'Be concrete and game-ready. appearance + spriteSpec must work as a brief for a small, limited-palette pixel-art sprite.',
    p.context ? `\nProject context:\n${p.context}` : '',
  ]
    .filter(Boolean)
    .join('\n')

  const prompt =
    `Existing character record (JSON), locales ${JSON.stringify(p.locales)}:\n` +
    `${JSON.stringify(p.character, null, 2)}\n\n` +
    `Return an enriched profile. Keep relationship targets referring to existing character ids when known.`

  const result = streamObject({ model, schema, system, prompt })

  for await (const partial of result.partialObjectStream) {
    p.onEvent('partial', partial)
  }
  const object = (await result.object) as Record<string, unknown>
  try { p.onEvent('usage', await result.usage) } catch { /* usage unavailable */ }
  return object
}
