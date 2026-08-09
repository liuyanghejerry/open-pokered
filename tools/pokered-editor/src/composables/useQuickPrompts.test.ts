import { describe, expect, it } from 'vitest'
import { quickPromptsFor, MAX_QUICK_PROMPTS, type QuickPromptContext } from './useQuickPrompts'

const ctx = (over: Partial<QuickPromptContext> = {}): QuickPromptContext => ({
  activity: null,
  ...over,
})
const ids = (c: QuickPromptContext) => quickPromptsFor(c).map(p => p.id)

describe('quickPromptsFor', () => {
  it('map activity → new-map', () => {
    expect(ids(ctx({ activity: 'map' }))).toEqual(['new-map'])
  })

  it('script activity → write-scene', () => {
    expect(ids(ctx({ activity: 'script' }))).toEqual(['write-scene'])
  })

  it('pokemon activity → tune-pokemon', () => {
    expect(ids(ctx({ activity: 'pokemon' }))).toEqual(['tune-pokemon'])
  })

  it('move activity → design-move', () => {
    expect(ids(ctx({ activity: 'move' }))).toEqual(['design-move'])
  })

  it('trainer activity → trainer-team', () => {
    expect(ids(ctx({ activity: 'trainer' }))).toEqual(['trainer-team'])
  })

  it('falls back to whats-next for other activities or no activity', () => {
    expect(ids(ctx({ activity: 'save' }))).toEqual(['whats-next'])
    expect(ids(ctx({ activity: 'layout' }))).toEqual(['whats-next'])
    expect(ids(ctx({ activity: 'pixel' }))).toEqual(['whats-next'])
    expect(ids(ctx())).toEqual(['whats-next'])
  })

  it('never exceeds the chip cap and never repeats an id', () => {
    const matrix: QuickPromptContext[] = [
      ctx({ activity: 'map' }),
      ctx({ activity: 'script' }),
      ctx({ activity: 'pokemon' }),
      ctx({ activity: 'move' }),
      ctx({ activity: 'trainer' }),
      ctx({ activity: 'save' }),
      ctx(),
    ]
    for (const c of matrix) {
      const prompts = quickPromptsFor(c)
      expect(prompts.length).toBeLessThanOrEqual(MAX_QUICK_PROMPTS)
      expect(new Set(prompts.map(p => p.id)).size).toBe(prompts.length)
    }
  })

  it('every chip carries a non-empty label and prompt', () => {
    const matrix: QuickPromptContext[] = [
      ctx({ activity: 'map' }),
      ctx({ activity: 'script' }),
      ctx({ activity: 'pokemon' }),
      ctx({ activity: 'move' }),
      ctx({ activity: 'trainer' }),
      ctx(),
    ]
    for (const c of matrix) {
      for (const p of quickPromptsFor(c)) {
        expect(p.label.trim()).not.toBe('')
        expect(p.prompt.trim()).not.toBe('')
        expect(p.icon.trim()).not.toBe('')
      }
    }
  })
})
