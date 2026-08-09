import { describe, expect, it } from 'vitest'
import { getAction, listActions, registerAction, runAction } from './registry'
import type { ActionContext, AiAction, AiEventType } from './types'

function fakeCtx(input: Record<string, any> = {}) {
  const events: Array<{ type: AiEventType; payload: any }> = []
  const ctx = {
    actionId: 'test', input, profile: {} as any, apiKey: 'k', project: {} as any,
    emit: (type: AiEventType, payload?: unknown) => events.push({ type, payload }),
  } as ActionContext
  return { ctx, events }
}

describe('action registry', () => {
  it('registers, gets, and lists actions', () => {
    const a: AiAction = { id: 'a1', kind: 'object', title: 'A', run: async () => 42 }
    registerAction(a)
    expect(getAction('a1')).toBe(a)
    expect(listActions().some(x => x.id === 'a1' && x.kind === 'object' && x.title === 'A')).toBe(true)
    expect(getAction('does-not-exist')).toBeUndefined()
  })

  it('runAction emits start then done carrying the result', async () => {
    const a: AiAction = { id: 'ok', kind: 'object', title: '', run: async ctx => ({ v: ctx.input.x }) }
    const { ctx, events } = fakeCtx({ x: 7 })
    await runAction(a, ctx)
    expect(events.map(e => e.type)).toEqual(['start', 'done'])
    expect(events[0].payload).toEqual({ actionId: 'ok' })
    expect(events[1].payload).toEqual({ result: { v: 7 } })
  })

  it('runAction emits start then error on throw (never done)', async () => {
    const a: AiAction = { id: 'boom', kind: 'object', title: '', run: async () => { throw new Error('nope') } }
    const { ctx, events } = fakeCtx()
    await runAction(a, ctx)
    expect(events.map(e => e.type)).toEqual(['start', 'error'])
    expect(events[1].payload).toEqual({ message: 'nope' })
  })
})
