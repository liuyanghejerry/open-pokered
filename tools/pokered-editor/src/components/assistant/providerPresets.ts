// ───────────────────────────────────────────────────────────────────────────
// Preset AI vendors for the welcome quick-setup form: picking a vendor
// pre-fills id/kind/baseURL so a first-time user only pastes an API key and
// types a model name. `modelExample` is shown as an input placeholder only —
// never pre-filled. No API keys live here.
// ───────────────────────────────────────────────────────────────────────────
import type { ProviderKind } from '../../types/ai'

export interface ProviderPreset {
  /** Also used as the default profile id (except `custom`, which stays blank). */
  id: string
  label: string
  kind: ProviderKind
  /** Pre-filled base URL; empty = editable from scratch (SDK default / custom). */
  baseURL: string
  /** Placeholder example for the model input — a hint, not a default value. */
  modelExample: string
  /** Console page where the user creates an API key (opened in a new tab). */
  keyUrl: string
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: 'moonshot',
    label: 'Moonshot (Kimi)',
    kind: 'openai',
    baseURL: 'https://api.moonshot.cn/v1',
    modelExample: 'kimi-k2-0711-preview',
    keyUrl: 'https://platform.moonshot.cn/console/api-keys',
  },
  {
    id: 'openai',
    label: 'OpenAI',
    kind: 'openai',
    baseURL: 'https://api.openai.com/v1',
    modelExample: 'gpt-4o',
    keyUrl: 'https://platform.openai.com/api-keys',
  },
  {
    // Anthropic speaks its native protocol; an empty baseURL falls back to the
    // SDK default endpoint (see buildModel in server/ai.ts).
    id: 'anthropic',
    label: 'Anthropic',
    kind: 'anthropic',
    baseURL: '',
    modelExample: 'claude-sonnet-4-5',
    keyUrl: 'https://console.anthropic.com/settings/keys',
  },
  {
    id: 'custom',
    label: 'Custom (OpenAI-compatible)',
    kind: 'openai',
    baseURL: '',
    modelExample: '',
    keyUrl: '',
  },
]

/** The vendor selected when the form first opens. */
export const DEFAULT_PRESET_ID = 'moonshot'

export function presetById(id: string): ProviderPreset {
  return PROVIDER_PRESETS.find(p => p.id === id) ?? PROVIDER_PRESETS[0]
}
