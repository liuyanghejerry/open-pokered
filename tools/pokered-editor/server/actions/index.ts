// ───────────────────────────────────────────────────────────────────────────
// AI action framework — public entry point. Register built-in actions once and
// re-export the registry/runner/bridge for the dev-server endpoints.
// ───────────────────────────────────────────────────────────────────────────
import { registerAction } from './registry'
// pokered-editor: refineCharacter (Story-Designer-only) is not ported, so it is
// neither imported nor registered here.
import { generateSceneAction } from './builtin/generateScene'
import { generateGuiAction } from './builtin/generateGui'
import { generateDataSetAction, batchEditDataAction } from './builtin/generateData'
import { generateSceneSnippetAction } from './builtin/generateSceneSnippet'

let registered = false

/** Idempotently register the built-in actions. Call once at server startup. */
export function registerBuiltinActions(): void {
  if (registered) return
  registered = true
  registerAction(generateSceneAction)
  registerAction(generateGuiAction)
  registerAction(generateDataSetAction)
  registerAction(batchEditDataAction)
  registerAction(generateSceneSnippetAction)
}

export * from './types'
export { getAction, listActions, registerAction, runAction } from './registry'
export { legacyEmit } from './legacyBridge'
export { ChangeSet, lineDiff } from './changeSet'
export type { Proposal, ChangeTarget, DiffOp } from './changeSet'
export { applyChange } from './apply'
export type { ApplyChangeRequest, ApplyChangeResult } from './apply'
export { streamChat } from './chat'
