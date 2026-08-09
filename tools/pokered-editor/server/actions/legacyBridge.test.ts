import { describe, expect, it } from 'vitest'
import { legacyEmit } from './legacyBridge'

function capture() {
  const out: Array<{ event: string; data: any }> = []
  return { send: (event: string, data: unknown) => out.push({ event, data }), out }
}

describe('legacyEmit — refine-character', () => {
  it('maps partial/done to the bare object and drops start/usage', () => {
    const { send, out } = capture()
    const emit = legacyEmit('refine-character', send)
    emit('start', { actionId: 'refine-character' })
    emit('partial', { object: { role: 'mentor' } })
    emit('usage', { inputTokens: 10 })
    emit('done', { result: { role: 'mentor', name: 'X' } })
    expect(out).toEqual([
      { event: 'partial', data: { role: 'mentor' } },
      { event: 'done', data: { role: 'mentor', name: 'X' } },
    ])
  })

  it('maps error to { message }', () => {
    const { send, out } = capture()
    legacyEmit('refine-character', send)('error', { message: 'bad' })
    expect(out).toEqual([{ event: 'error', data: { message: 'bad' } }])
  })
})

describe('legacyEmit — generate-scene', () => {
  it('maps text/reasoning/tool/done to the old vocab and drops start/usage', () => {
    const { send, out } = capture()
    const emit = legacyEmit('generate-scene', send)
    emit('start', {})
    emit('text', { delta: 'he' })
    emit('reasoning', { delta: 'th' })
    emit('tool-call', { name: 'read_file', path: 'a.scene' })
    emit('usage', { inputTokens: 1 })
    emit('done', { result: { content: 'X', scene: 'S', storyline: 'L', targetRel: 'r' } })
    expect(out).toEqual([
      { event: 'text', data: { text: 'he' } },
      { event: 'reasoning', data: { text: 'th' } },
      { event: 'tool', data: { name: 'read_file', path: 'a.scene' } },
      { event: 'done', data: { content: 'X', scene: 'S', storyline: 'L', targetRel: 'r' } },
    ])
  })
})

describe('legacyEmit — unknown action', () => {
  it('forwards standard events verbatim', () => {
    const { send, out } = capture()
    legacyEmit('chat', send)('text', { delta: 'hi' })
    expect(out).toEqual([{ event: 'text', data: { delta: 'hi' } }])
  })
})
