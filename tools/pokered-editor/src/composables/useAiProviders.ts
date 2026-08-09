// ───────────────────────────────────────────────────────────────────────────
// AI provider profiles — module-level singleton shared by the Settings tab's
// provider editor and the Story activity's AI generators (character refine /
// scene generation).
//
// Dev/Electron: profiles persist to `.jrpg-editor.providers.json` via the
// `/api/ai/providers` endpoint. Static hosting (GitHub Pages): there is no
// /api backend, so profiles persist to localStorage instead, seeded with a
// DeepSeek profile (OpenAI-compatible, browser-direct) so the AI Assistant
// works out of the box. API keys always live in localStorage (see
// useAiStream), never on disk here.
// ───────────────────────────────────────────────────────────────────────────
import { ref } from 'vue'
import type { ProviderProfile } from '../types/ai'

const providers = ref<ProviderProfile[]>([])
let loadedOnce = false

/** Static-mode persistence key (only used when there is no /api backend). */
const STATIC_PROVIDERS_KEY = 'jrpg-ai-providers'

/** Default profile for static hosting: DeepSeek's OpenAI-compatible endpoint
 *  (browser-direct; `deepseek-chat` is their general Flash-class model). */
function defaultStaticProviders(): ProviderProfile[] {
  return [
    {
      id: 'deepseek',
      kind: 'openai',
      baseURL: 'https://api.deepseek.com/v1',
      model: 'deepseek-chat',
    },
  ]
}

function loadStaticProviders(): ProviderProfile[] {
  if (typeof localStorage === 'undefined') return defaultStaticProviders()
  try {
    const raw = localStorage.getItem(STATIC_PROVIDERS_KEY)
    if (raw) {
      const list = JSON.parse(raw)
      if (Array.isArray(list) && list.length > 0) return list
    }
  } catch {
    /* corrupted value → default */
  }
  return defaultStaticProviders()
}

function saveStaticProviders(next: ProviderProfile[]): void {
  try {
    localStorage.setItem(STATIC_PROVIDERS_KEY, JSON.stringify(next))
  } catch {
    /* storage full/unavailable — profiles just won't survive a reload */
  }
}

export function useAiProviders() {
  /** Fetch profiles once (idempotent); pass force to refetch. In static mode
   *  this reads localStorage (seeded with the DeepSeek default). */
  async function loadProviders(force = false): Promise<void> {
    if (loadedOnce && !force) return
    try {
      const resp = await fetch('/api/ai/providers')
      providers.value = resp.ok ? await resp.json() : loadStaticProviders()
    } catch {
      providers.value = loadStaticProviders()
    }
    loadedOnce = true
  }

  async function saveProviders(next: ProviderProfile[]): Promise<void> {
    const resp = await fetch('/api/ai/providers', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(next),
    })
    if (!resp.ok) {
      // No /api backend (static hosting): fall back to localStorage.
      saveStaticProviders(next)
      providers.value = next
      loadedOnce = true
      return
    }
    providers.value = next
    loadedOnce = true
  }

  return { providers, loadProviders, saveProviders }
}
