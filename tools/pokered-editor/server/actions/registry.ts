// ───────────────────────────────────────────────────────────────────────────
// AI action registry + runner.
// ───────────────────────────────────────────────────────────────────────────
import type { AiAction, ActionContext } from './types'

const registry = new Map<string, AiAction>()

export function registerAction(action: AiAction): void {
  registry.set(action.id, action)
}

export function getAction(id: string): AiAction | undefined {
  return registry.get(id)
}

export function listActions(): Array<{ id: string; kind: string; title: string }> {
  return [...registry.values()].map(a => ({ id: a.id, kind: a.kind, title: a.title }))
}

/**
 * Run an action end-to-end against a streaming context: emit `start`, execute,
 * then emit `done` (with the result) or `error`. All emission is handled here so
 * callers (the /api/ai/run endpoint and the legacy shims) only wire transport —
 * they never emit start/done/error themselves and never need a try/catch.
 */
export async function runAction(action: AiAction, ctx: ActionContext): Promise<void> {
  ctx.emit('start', { actionId: action.id })
  try {
    const result = await action.run(ctx)
    ctx.emit('done', { result })
  } catch (e) {
    ctx.emit('error', { message: (e as Error).message })
  }
}
