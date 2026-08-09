import { describe, it, expect } from 'vitest'
import { usePlaytestOverlay } from './usePlaytestOverlay'

// Module-level singleton state — exercising it mutates the singleton, so keep
// all assertions inside this one file (state is shared across imports).
const overlay = usePlaytestOverlay()

describe('usePlaytestOverlay', () => {
  it('launch opens the overlay in test mode and queues a quick entry', () => {
    overlay.launch({ kind: 'battle', species: 'Pikachu', level: 5 })
    expect(overlay.open.value).toBe(true)
    expect(overlay.mode.value).toBe('test')
    expect(overlay.target.value).toEqual({ kind: 'battle', species: 'Pikachu', level: 5 })
  })

  it('launch bumps the stamp so identical requests re-trigger', () => {
    const first = overlay.targetStamp.value
    overlay.launch({ kind: 'battle', species: 'Pikachu', level: 5 })
    expect(overlay.targetStamp.value).toBeGreaterThan(first)
    expect(overlay.target.value).toEqual({ kind: 'battle', species: 'Pikachu', level: 5 })
  })

  it('playSave launches into Play mode (persistent session)', () => {
    overlay.launch({ kind: 'playSave', save: '{}' })
    expect(overlay.mode.value).toBe('play')
    expect(overlay.target.value).toEqual({ kind: 'playSave', save: '{}' })
  })

  it('closeOverlay drops the pending target (no re-trigger on reopen)', () => {
    overlay.closeOverlay()
    expect(overlay.open.value).toBe(false)
    expect(overlay.target.value).toBeNull()
  })

  it('toggleOverlay flips open state and keeps the last mode', () => {
    overlay.mode.value = 'play'
    overlay.toggleOverlay()
    expect(overlay.open.value).toBe(true)
    expect(overlay.mode.value).toBe('play')
    overlay.toggleOverlay()
    expect(overlay.open.value).toBe(false)
  })
})
