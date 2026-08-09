// Quick-setup vendor presets: shape integrity and protocol-kind sanity.
import { describe, it, expect } from 'vitest'
import { PROVIDER_PRESETS, DEFAULT_PRESET_ID, presetById } from './providerPresets'

describe('PROVIDER_PRESETS', () => {
  it('every preset has a non-empty id/label and a legal protocol kind', () => {
    for (const p of PROVIDER_PRESETS) {
      expect(p.id.trim()).not.toBe('')
      expect(p.label.trim()).not.toBe('')
      expect(['openai', 'anthropic']).toContain(p.kind)
    }
  })

  it('has unique ids and covers the expected vendors, moonshot first', () => {
    const ids = PROVIDER_PRESETS.map(p => p.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(ids).toEqual(['moonshot', 'openai', 'anthropic', 'custom'])
    expect(DEFAULT_PRESET_ID).toBe('moonshot')
  })

  it('non-custom presets point at https console pages for API keys', () => {
    for (const p of PROVIDER_PRESETS.filter(p => p.id !== 'custom')) {
      expect(p.keyUrl).toMatch(/^https:\/\//)
      expect(p.modelExample.trim()).not.toBe('')
    }
  })

  it('openai-kind presets carry an https baseURL; anthropic leaves it empty (SDK default)', () => {
    for (const p of PROVIDER_PRESETS) {
      if (p.kind === 'anthropic') expect(p.baseURL).toBe('')
      else if (p.id !== 'custom') expect(p.baseURL).toMatch(/^https:\/\//)
    }
  })

  it('presetById resolves known ids and falls back to the first preset', () => {
    expect(presetById('anthropic').kind).toBe('anthropic')
    expect(presetById('nope')).toBe(PROVIDER_PRESETS[0])
  })
})
