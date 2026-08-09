// ───────────────────────────────────────────────────────────────────────────
// Image-generation provider profiles — kept SEPARATE from the text providers
// (useAiProviders). Persist to `.jrpg-editor.image-providers.json` via
// `/api/ai/image-providers`; API keys live in localStorage (see useAiStream),
// never on disk here. kind: 'openai' (OpenAI-compatible images) | 'gemini'.
// ───────────────────────────────────────────────────────────────────────────
import { ref } from 'vue'
import type { ImageProviderProfile } from '../types/ai'

const imageProviders = ref<ImageProviderProfile[]>([])
let loadedOnce = false

export function useAiImageProviders() {
  async function loadImageProviders(force = false): Promise<void> {
    if (loadedOnce && !force) return
    try {
      const resp = await fetch('/api/ai/image-providers')
      imageProviders.value = resp.ok ? await resp.json() : []
    } catch {
      imageProviders.value = []
    }
    loadedOnce = true
  }

  async function saveImageProviders(next: ImageProviderProfile[]): Promise<void> {
    const resp = await fetch('/api/ai/image-providers', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(next),
    })
    if (!resp.ok) throw new Error(await resp.json().then((j) => j.error).catch(() => resp.statusText))
    imageProviders.value = next
    loadedOnce = true
  }

  return { imageProviders, loadImageProviders, saveImageProviders }
}
