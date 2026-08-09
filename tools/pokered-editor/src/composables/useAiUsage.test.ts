import { describe, expect, it } from 'vitest'
import { useAiUsage } from './useAiUsage'

describe('useAiUsage', () => {
  it('accumulates input/output tokens and call count across usage shapes', () => {
    const u = useAiUsage()
    u.reset()
    u.record({ inputTokens: 100, outputTokens: 40 })
    u.record({ promptTokens: 50, completionTokens: 10 }) // alternate field names
    u.record(null)
    u.record({ inputTokens: 0, outputTokens: 0 }) // ignored (no tokens)
    expect(u.inputTokens.value).toBe(150)
    expect(u.outputTokens.value).toBe(50)
    expect(u.total.value).toBe(200)
    expect(u.calls.value).toBe(2)
    u.reset()
    expect(u.total.value).toBe(0)
  })

  it('formats a compact label', () => {
    const u = useAiUsage()
    u.reset()
    u.record({ inputTokens: 1500, outputTokens: 300 })
    expect(u.label.value).toBe('1.5k↑ 300↓')
  })
})
