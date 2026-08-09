// ───────────────────────────────────────────────────────────────────────────
// gemini.ts — Google Gemini image generation via `generateContent` (Nano Banana:
// gemini-2.5-flash-image / gemini-3-pro-image / gemini-3.1-flash-image).
//
// Unlike the OpenAI-compatible images API (text→image only), this multimodal
// endpoint accepts INPUT IMAGES, so the base character + front-strip references
// can be attached to lock identity across the strip. Mirrors the project's wuxia
// gemini_gen.py (v1beta generateContent, `x-goog-api-key`, responseModalities
// IMAGE), with optional proxy support (see proxy.ts).
// ───────────────────────────────────────────────────────────────────────────
import { type Img, decodePNG, encodePNG } from './image'
import { proxiedFetch, resolveProxy } from '../proxy'

export interface GeminiImageParams {
  baseURL?: string
  apiKey: string
  model: string
  prompt: string
  /** Reference images attached as inline parts (base character, motion strip, …). */
  refs?: Img[]
  /** Optional HTTP(S) proxy (e.g. http://127.0.0.1:9085); falls back to env vars. */
  proxyUrl?: string
}

const DEFAULT_BASE = 'https://generativelanguage.googleapis.com'

/** geminiGenerateImage — generate one image (PNG) from a prompt + optional refs. */
export async function geminiGenerateImage(p: GeminiImageParams): Promise<Img> {
  // Tolerate a base that accidentally includes a trailing path (/v1beta…): strip
  // it back to the host so we don't build /v1beta/v1beta/….
  let base = (p.baseURL?.trim() || DEFAULT_BASE).replace(/\/+$/, '')
  base = base.replace(/\/v1beta.*$/i, '')
  const url = `${base}/v1beta/models/${encodeURIComponent(p.model)}:generateContent`

  const parts: any[] = [{ text: p.prompt }]
  for (const r of p.refs ?? []) {
    parts.push({ inlineData: { mimeType: 'image/png', data: encodePNG(r).toString('base64') } })
  }
  const body = {
    contents: [{ role: 'user', parts }],
    generationConfig: { responseModalities: ['TEXT', 'IMAGE'] },
  }

  let resp: Response
  try {
    resp = await proxiedFetch(p.proxyUrl, url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'x-goog-api-key': p.apiKey },
      body: JSON.stringify(body),
    })
  } catch (e) {
    // Connection-level failure (DNS/TLS/blocked/proxy) — make it actionable.
    const cause = (e as any)?.cause
    const detail = cause?.code || cause?.message || (e as Error).message
    const proxy = resolveProxy(p.proxyUrl)
    const where = proxy ? `via proxy ${proxy}` : 'with no proxy configured'
    throw new Error(`Could not reach Gemini (${where}): ${detail}. If your network blocks Google, set a proxy on the image provider.`)
  }
  if (!resp.ok) {
    const txt = await resp.text().catch(() => '')
    throw new Error(`Gemini ${resp.status}: ${txt.slice(0, 300)}`)
  }
  const data: any = await resp.json()
  const cand = data?.candidates?.[0]
  const outParts: any[] = cand?.content?.parts ?? []
  for (const part of outParts) {
    const inline = part.inlineData ?? part.inline_data
    if (inline?.data) return decodePNG(Buffer.from(inline.data, 'base64'))
  }
  // No image part — surface the model's text / block reason.
  const text = outParts.map((x) => x.text).filter(Boolean).join(' ')
  const reason = cand?.finishReason ?? data?.promptFeedback?.blockReason
  throw new Error(`Gemini returned no image${reason ? ` (${reason})` : ''}${text ? `: ${text.slice(0, 200)}` : ''}`)
}
