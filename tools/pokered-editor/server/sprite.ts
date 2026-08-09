// ───────────────────────────────────────────────────────────────────────────
// Sprite generation — turns a character's appearance + spriteSpec into an image
// via an OpenAI-compatible image model (Vercel AI SDK generateImage). The result
// is a reference sprite PNG; pixel-art finishing is left to the artist / the
// editor's pixel tooling. Heavy deps are imported dynamically.
// ───────────────────────────────────────────────────────────────────────────
import type { ProviderProfile } from './ai'

export interface SpriteGenParams {
  profile: ProviderProfile
  apiKey: string
  prompt: string
  /** e.g. "1024x1024" */
  size?: string
}

export async function generateSprite(p: SpriteGenParams): Promise<{ base64: string; mediaType: string }> {
  if (p.profile.kind !== 'openai') {
    throw new Error('Sprite generation needs an OpenAI-compatible provider (anthropic has no image model).')
  }
  const imageModelId = p.profile.imageModel || p.profile.model
  if (!imageModelId) throw new Error('This provider has no image model configured.')

  const { generateImage } = await import('ai')
  const { createOpenAICompatible } = await import('@ai-sdk/openai-compatible')

  const provider = createOpenAICompatible({
    name: p.profile.id || 'openai',
    apiKey: p.apiKey,
    baseURL: p.profile.baseURL,
  })
  const model = provider.imageModel(imageModelId)

  const result = await generateImage({ model, prompt: p.prompt, size: (p.size || '1024x1024') as any })
  return { base64: result.image.base64, mediaType: result.image.mediaType || 'image/png' }
}
