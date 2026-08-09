// ── AI provider profiles ───────────────────────────────────────────────────

/** Which wire protocol a vendor speaks. */
export type ProviderKind = 'anthropic' | 'openai'

/**
 * A named LLM provider profile for TEXT generation. NOTE: never carries an
 * API key — keys live in the browser (localStorage) and are sent per-request.
 * This is config only.
 */
export interface ProviderProfile {
  id: string
  kind: ProviderKind
  baseURL: string
  model: string
  /** Optional HTTP(S) proxy for reaching the provider, e.g. http://127.0.0.1:9085. */
  proxyUrl?: string
  /** Optional embedding model id (openai-compatible) — enables retrieval/RAG. */
  embeddingModel?: string
  /** @deprecated image generation now uses a separate ImageProviderProfile. */
  imageModel?: string
}

/** Which wire protocol an IMAGE vendor speaks. */
export type ImageProviderKind = 'openai' | 'gemini'

/**
 * A named provider profile for IMAGE generation, kept separate from the text
 * providers. `openai` = OpenAI-compatible images API; `gemini` = Google Gemini
 * `generateContent` (supports reference images). Config only; the API key
 * lives in the browser.
 */
export interface ImageProviderProfile {
  id: string
  kind: ImageProviderKind
  baseURL: string
  /** The image model id, e.g. `gpt-image-1` or `gemini-2.5-flash-image`. */
  model: string
  /** Optional HTTP(S) proxy for reaching the provider, e.g. http://127.0.0.1:9085. */
  proxyUrl?: string
}
