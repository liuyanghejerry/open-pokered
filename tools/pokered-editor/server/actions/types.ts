// ───────────────────────────────────────────────────────────────────────────
// AI action framework — shared types.
//
// Every AI feature (refine-character, generate-scene, the chat assistant, …) is
// an `AiAction` in a registry, run behind ONE streaming endpoint (/api/ai/run)
// that emits a STANDARD event vocabulary. This replaces the per-feature
// middlewares that each re-implemented body parsing, key validation and the SSE
// write loop. See tools/dotzuki-editor/docs/AI_AGENT_FRAMEWORK.md.
// ───────────────────────────────────────────────────────────────────────────
import type { ProjectContext } from '../context/projectContext'
import type { ProviderProfile } from '../ai'

/** Standardized streaming event vocabulary shared by every AI action. */
export type AiEventType =
  | 'start'        // { actionId } — run began
  | 'text'         // { delta }    — assistant prose
  | 'reasoning'    // { delta }    — model reasoning
  | 'partial'      // { object }   — streamed structured output
  | 'tool-call'    // { name, args?, path? }
  | 'tool-result'  // { name, ok, summary? }
  | 'proposal'     // { id, target, diff, rationale } — a reviewable edit (M2)
  | 'plan'         // { steps: {title, status}[] } — the agent's working plan/todo
  | 'progress'     // { label, pct? }
  | 'usage'        // { inputTokens?, outputTokens?, totalTokens? }
  | 'done'         // { result }
  | 'error'        // { message, where? }

export type AiEmit = (type: AiEventType, payload?: unknown) => void

/** Everything an action needs to run + stream. */
export interface ActionContext {
  actionId: string
  /** Action-specific request params from the client. */
  input: Record<string, any>
  profile: ProviderProfile
  apiKey: string
  project: ProjectContext
  /** Emit a standardized streaming event. */
  emit: AiEmit
  /** Aborted when the client disconnects. */
  signal?: AbortSignal
}

export interface AiAction {
  id: string
  /** UI/metadata hint for how the result streams. */
  kind: 'object' | 'agent' | 'chat'
  title: string
  /** Run the action, streaming via ctx.emit; resolve to the final result payload. */
  run(ctx: ActionContext): Promise<unknown>
}
