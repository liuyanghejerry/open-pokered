// Per-kind auto-apply gates: content kinds follow the switches, meta kinds are hard-blocked.
import { describe, it, expect } from 'vitest'
import {
  CONTENT_KINDS, META_KINDS, defaultAutoApplySettings, isMetaKind, shouldAutoApply,
} from './autoApply'

describe('auto-apply kind gates', () => {
  it('covers the five content kinds and the three meta kinds', () => {
    expect([...CONTENT_KINDS]).toEqual(['story', 'data', 'scene', 'gui', 'map'])
    expect([...META_KINDS]).toEqual(['project-config', 'project-scaffold', 'map-create'])
  })

  it('defaults to everything off, or all content kinds on', () => {
    expect(defaultAutoApplySettings()).toEqual({ story: false, data: false, scene: false, gui: false, map: false })
    expect(defaultAutoApplySettings(true)).toEqual({ story: true, data: true, scene: true, gui: true, map: true })
  })

  it('auto-applies a content kind only when its switch is on', () => {
    const s = { ...defaultAutoApplySettings(), scene: true }
    expect(shouldAutoApply('scene', s)).toBe(true)
    expect(shouldAutoApply('story', s)).toBe(false)
    // a missing key / unknown kind is off, not on
    expect(shouldAutoApply('gui', {})).toBe(false)
    expect(shouldAutoApply('nonsense', s)).toBe(false)
  })

  it('NEVER auto-applies meta kinds, whatever the switches say', () => {
    const allOn = { story: true, data: true, scene: true, gui: true, map: true, 'project-config': true, 'project-scaffold': true, 'map-create': true }
    for (const kind of META_KINDS) {
      expect(isMetaKind(kind)).toBe(true)
      expect(shouldAutoApply(kind, allOn)).toBe(false)
    }
    expect(isMetaKind('scene')).toBe(false)
    expect(isMetaKind(undefined)).toBe(false)
  })
})
