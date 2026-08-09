// ───────────────────────────────────────────────────────────────────────────
// Legacy SSE bridge.
//
// The existing clients (CharacterEditor.vue, SceneGenerator.vue) POST to
// /api/ai/refine-character and /api/ai/generate-scene and parse a per-feature
// event vocabulary. Those endpoints now run through the action registry, which
// emits the STANDARD vocab; this adapter translates the standard stream back to
// what each old client expects, so nothing breaks while we migrate. New surfaces
// should use /api/ai/run + the standard vocab directly.
// ───────────────────────────────────────────────────────────────────────────
import type { AiEmit } from './types'

type RawSend = (event: string, data: unknown) => void

export function legacyEmit(actionId: string, send: RawSend): AiEmit {
  if (actionId === 'refine-character') {
    // CharacterEditor.vue treats EVERY non-`error` event as the proposal object,
    // so we must emit ONLY partial/done (carrying the object) and error —
    // dropping start/usage/reasoning/etc. which would otherwise clobber it.
    return (type, payload: any) => {
      if (type === 'partial') send('partial', payload?.object ?? payload)
      else if (type === 'done') send('done', payload?.result ?? payload)
      else if (type === 'error') send('error', { message: payload?.message ?? 'AI error' })
    }
  }
  if (actionId === 'generate-scene') {
    // SceneGenerator.vue handles text/reasoning/tool/done/error and ignores the rest.
    return (type, payload: any) => {
      if (type === 'text') send('text', { text: payload?.delta ?? '' })
      else if (type === 'reasoning') send('reasoning', { text: payload?.delta ?? '' })
      else if (type === 'tool-call') send('tool', { name: payload?.name, path: payload?.path })
      else if (type === 'done') send('done', payload?.result ?? payload)
      else if (type === 'error') send('error', { message: payload?.message ?? 'AI error' })
    }
  }
  // Unknown action: pass standard events through verbatim.
  return (type, payload) => send(type, payload)
}
