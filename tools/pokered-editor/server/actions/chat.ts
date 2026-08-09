// ───────────────────────────────────────────────────────────────────────────
// streamChat — the chat surface on the Vercel AI SDK UI message stream.
//
// Unlike /api/ai/run (our bespoke SSE vocab), this speaks the AI SDK *UI message
// stream* so the client can use @ai-sdk/vue `useChat` directly: assistant text
// and tool-call parts flow through `result.toUIMessageStream()`, and our review
// proposals ride as TRANSIENT custom `data-proposal` parts (captured client-side
// via useChat's onData, where the review tray owns their apply lifecycle).
//
// Reuses the same READ/PROPOSE tool surface + ProjectContext system prompt as the
// assistant action; only the transport/runner differs.
// ───────────────────────────────────────────────────────────────────────────
import type { ServerResponse } from 'http'
import { buildModel, type ProviderProfile } from '../ai'
import type { ProjectContext } from '../context/projectContext'
import type { ActionContext } from './types'
import { ChangeSet } from './changeSet'
import { buildReadTools, buildProposeTools, buildScaffoldTools, buildPlanTools, buildMemoryTools } from './tools'
import { readMemories } from './memory'
import { buildAssistantSystem, buildScaffoldSystem, type UiContext } from './assistantSystem'

export interface StreamChatOptions {
  res: ServerResponse
  /** null = creation mode (no project open): scaffold-drafting tools only. */
  project: ProjectContext | null
  profile: ProviderProfile
  apiKey: string
  /** UIMessage[] sent by useChat. */
  uiMessages: any[]
  /** What the user is currently viewing (activity + route), when a project is open. */
  uiContext?: UiContext
  signal?: AbortSignal
}

export async function streamChat(opts: StreamChatOptions): Promise<void> {
  const { createUIMessageStream, pipeUIMessageStreamToResponse, streamText, stepCountIs, convertToModelMessages } = await import('ai')
  const userText = lastUserText(opts.uiMessages)
  // Assistant memory (global + project) is folded into the system prompt; the
  // agent appends to it via the remember_fact tool.
  const memories = readMemories(opts.project)
  let system = opts.project
    ? buildAssistantSystem(opts.project, userText, [], opts.uiContext, memories)
    : buildScaffoldSystem(opts.uiContext, memories)

  // Optional embeddings RAG: when the provider has an embeddingModel, augment the
  // system with the top-K most relevant project chunks. Off by default (no model).
  if (opts.project && opts.profile.embeddingModel) {
    try {
      const { retrieve } = await import('../retrieval')
      const hits = await retrieve(opts.project, opts.profile, opts.apiKey, userText)
      if (hits.length) system += '\n\nRetrieved project context:\n' + hits.map(h => `# ${h.id}\n${h.text}`).join('\n\n')
    } catch { /* retrieval is best-effort; fall back to the structured context */ }
  }

  const stream = createUIMessageStream({
    execute: async ({ writer }) => {
      // PROPOSE tools emit('proposal', …) and update_plan emits('plan', …) → transient
      // data parts the client collects (the review tray / the plan checklist).
      const emit: ActionContext['emit'] = (type, payload) => {
        if (type === 'proposal') writer.write({ type: 'data-proposal', data: payload as any, transient: true })
        else if (type === 'plan') writer.write({ type: 'data-plan', data: payload as any, transient: true })
      }
      const ctx = {
        actionId: 'assistant', input: {}, profile: opts.profile, apiKey: opts.apiKey,
        project: opts.project, emit, signal: opts.signal,
      } as ActionContext

      // Creation mode (no project): only draft_project_scaffold + update_plan —
      // every tool that reads or proposes against a ProjectContext is excluded.
      // remember_fact is registered in both modes (it is the agent's own memory,
      // not project content; with no project it lands in the global file).
      const cs = new ChangeSet()
      const tools = opts.project
        ? { ...(await buildReadTools(ctx)), ...(await buildProposeTools(ctx, cs)), ...(await buildPlanTools(ctx)), ...(await buildMemoryTools(ctx)) }
        : { ...(await buildScaffoldTools(ctx, cs)), ...(await buildPlanTools(ctx)), ...(await buildMemoryTools(ctx)) }
      const model = await buildModel(opts.profile, opts.apiKey)
      const modelMessages = await convertToModelMessages(opts.uiMessages)

      // Anthropic prompt-caching of the large, stable system block (no-op for
      // openai-compatible providers, which use the plain `system` field).
      const cached = opts.profile.kind === 'anthropic'
      const result = streamText({
        model,
        tools,
        // Higher than a plain Q&A loop: the agent now iterates draft → check_scene
        // → fix → re-check before proposing, which costs a few extra tool steps.
        stopWhen: [stepCountIs(16)],
        abortSignal: opts.signal,
        ...(cached
          ? { messages: [{ role: 'system', content: system, providerOptions: { anthropic: { cacheControl: { type: 'ephemeral' } } } } as any, ...modelMessages] }
          : { system, messages: modelMessages }),
      })

      // Surface token usage to the client (useChat onFinish → the cost meter).
      writer.merge(result.toUIMessageStream({
        messageMetadata: ({ part }: any) => part?.type === 'finish' ? { usage: part.totalUsage } : undefined,
      }))
    },
    onError: (err) => (err instanceof Error ? err.message : String(err)),
  })

  pipeUIMessageStreamToResponse({ response: opts.res, stream })
}

/** Latest user message's text (for @mention resolution in the system prompt). */
function lastUserText(uiMessages: any[]): string {
  for (let i = uiMessages.length - 1; i >= 0; i--) {
    const m = uiMessages[i]
    if (m?.role === 'user' && Array.isArray(m.parts)) {
      return m.parts.filter((p: any) => p?.type === 'text').map((p: any) => p.text).join(' ')
    }
  }
  return ''
}
