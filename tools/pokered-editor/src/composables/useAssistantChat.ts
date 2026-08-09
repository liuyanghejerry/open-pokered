// ───────────────────────────────────────────────────────────────────────────
// useAssistantChat — the AI Assistant panel on the Vercel AI SDK UI stream.
//
// Wraps @ai-sdk/vue `useChat` (transport → POST /api/ai/chat). Assistant text +
// tool-call activity live in useChat's `messages` (UIMessage.parts). Review
// proposals arrive as TRANSIENT `data-proposal` parts via `onData` and go into a
// shared useProposals tray (apply/revert via /api/ai/apply-change).
//
// Multi-session: each conversation lives in a per-thread localStorage snapshot
// (see useChatThreads); tray + plan are module singletons so they survive the
// panel being toggled (the panel is kept mounted with v-show).
// ───────────────────────────────────────────────────────────────────────────
import { computed, ref, watch } from 'vue'
import { useChat } from '@ai-sdk/vue'
import { DefaultChatTransport } from 'ai'
import { useProposals } from './useProposals'
import { useChatThreads } from './useChatThreads'
import { useAiUsage } from './useAiUsage'
import { staticMode } from './useStaticMode'
import { buildBrowserModel, buildStaticSystem, buildStaticTools, toolPartsFromSteps, autoApplyKindFor, type StaticAiEmit } from './assistantStatic'
import {
  defaultAutoApplySettings, isMetaKind, shouldAutoApply, type AutoApplySettings,
} from '../components/assistant/autoApply'
import type { ProviderProfile } from '../types/ai'

export type { DiffOp, AssistantProposal } from './useProposals'

/** Coarse working state the panel renders a status indicator from. */
export type AssistantPhase = 'idle' | 'thinking' | 'tool' | 'writing' | 'error'

export interface PlanStep { title: string; status: 'pending' | 'active' | 'done' | string }

/** What the user is looking at in the editor, sent along with each message. */
export interface UiContext { activity?: string; route?: string }

// Legacy all-or-nothing boolean key (pre per-kind switches) — migrated on load.
const LEGACY_AUTOAPPLY_KEY = 'jrpg-assistant-autoapply'
const AUTOAPPLY_KINDS_KEY = 'jrpg-assistant-autoapply-kinds'
// Multi-session threads: index + per-thread snapshots in localStorage (see
// useChatThreads; the legacy single-conversation keys migrate on first load).
// Module singleton so panel instances share the threads.
const threadStore = useChatThreads()
// The tray is a module singleton so staged proposals survive the panel being
// toggled (the panel is kept mounted with v-show). Not self-persisted anymore:
// durability is per-thread through threadStore snapshots.
const tray = useProposals()
// The agent's current working checklist (update_plan). Module-level so it
// survives panel toggles; persisted as part of the per-thread snapshot.
const plan = ref<PlanStep[]>([])
// Hydrate the singletons from the active thread (page load / reload). The chat
// messages hydrate per panel instance in useAssistantChat() below.
const bootSnapshot = threadStore.activeSnapshot()
tray.replace(bootSnapshot.tray)
plan.value = bootSnapshot.plan
// Per-kind opt-in: apply a proposal the moment it arrives, per content kind.
// Meta kinds (project-config / project-scaffold / map-create) are NEVER
// auto-applied — shouldAutoApply hard-blocks them, since they reshape the
// project itself and must always pass human review. The drift guard still
// fires on auto-applied writes, so a conflict is never silently clobbered.
// Module-level so both the dev and static chat implementations share one
// switch (their watchers just persist it).
const autoApplyKinds = ref<AutoApplySettings>(loadAutoApplyKinds())

export function useAssistantChat() {
  // Static hosting (GitHub Pages): no /api backend — the assistant runs
  // browser-direct against an OpenAI-compatible provider with pokered-shaped
  // dataFetch-backed tools (see useStaticAssistantChat).
  if (staticMode.value) return useStaticAssistantChat()

  // True only while the user deliberately interrupted the last run (chat.stop()),
  // so the panel can show a "Stopped" badge distinct from a natural finish.
  const stopped = ref(false)

  const chat = useChat({
    transport: new DefaultChatTransport({ api: '/api/ai/chat' }),
    messages: threadStore.activeSnapshot().messages, // hydrate the active thread across reloads
    onData: (part: any) => {
      if (part?.type === 'data-proposal' && part.data) {
        tray.add(part.data)
        const p = tray.proposals.value[tray.proposals.value.length - 1]
        // shouldAutoApply hard-blocks meta kinds — they never auto-apply.
        if (p && shouldAutoApply(p.target?.kind, autoApplyKinds.value)) void tray.applyProposal(p)
      }
      else if (part?.type === 'data-plan' && Array.isArray(part.data?.steps)) plan.value = part.data.steps
    },
    onFinish: ({ message }: any) => {
      const u = message?.metadata?.usage
      if (u) useAiUsage().record(u)
      persistCurrent() // durable snapshot once a turn settles
    },
  })

  const busy = computed(() => chat.status.value === 'submitted' || chat.status.value === 'streaming')
  const error = computed(() => chat.error.value?.message || '')

  const lastMessage = computed(() => chat.messages.value[chat.messages.value.length - 1])

  /** Name of the tool currently executing (input received, output pending), or ''. */
  const activeTool = computed(() => (busy.value ? runningToolOf(lastMessage.value) : '') || '')

  /**
   * The assistant's working state, derived from the SDK status + the live parts:
   *   thinking — request sent / model reasoning, no visible output yet
   *   tool     — a read/propose tool is running (activeTool names it)
   *   writing  — response text is streaming in
   */
  const phase = computed<AssistantPhase>(() => {
    const s = chat.status.value
    if (s === 'error') return 'error'
    if (s === 'submitted') return 'thinking'
    if (s === 'streaming') {
      if (activeTool.value) return 'tool'
      const m = lastMessage.value
      return m && m.role === 'assistant' && messageText(m) ? 'writing' : 'thinking'
    }
    return 'idle'
  })

  watch(autoApplyKinds, v => saveJson(AUTOAPPLY_KINDS_KEY, v), { deep: true })

  /** Everything the current thread owns, written to its snapshot. */
  function persistCurrent(): void {
    threadStore.saveSnapshot(threadStore.activeThreadId.value, {
      messages: chat.messages.value, tray: tray.proposals.value, plan: plan.value,
    })
  }

  /** Load the active thread's snapshot into the live chat, tray and plan. */
  function loadActive(): void {
    const snap = threadStore.activeSnapshot()
    chat.messages.value = snap.messages
    tray.replace(snap.tray)
    plan.value = snap.plan
    stopped.value = false
    chat.clearError()
  }

  // Tray mutations (apply/revert/discard) happen between turns — persist them
  // through the same per-thread snapshot as the message saves.
  watch(tray.proposals, () => persistCurrent(), { deep: true })

  /** Start a fresh empty thread. Locked while busy: a running stream is bound
   *  to this chat instance, so switching mid-stream would cross the wires. */
  function newThread(): void {
    if (busy.value) return
    persistCurrent()
    threadStore.createThread()
    loadActive()
  }

  /** Switch to another thread: snapshot the current one, load the target. */
  function switchThread(id: string): void {
    if (busy.value || id === threadStore.activeThreadId.value) return
    persistCurrent()
    threadStore.setActive(id)
    loadActive()
  }

  /** Delete a thread. Deleting the open one falls back to the most recent
   *  remaining thread (or a fresh empty one) — a thread always exists. */
  function deleteThread(id: string): void {
    if (busy.value) return
    const wasActive = id === threadStore.activeThreadId.value
    threadStore.deleteThread(id)
    if (wasActive) loadActive()
  }

  async function send(text: string, provider: ProviderProfile, key: string, uiContext?: UiContext): Promise<void> {
    if (!text.trim() || busy.value) return
    stopped.value = false
    plan.value = [] // a new task supersedes the previous turn's checklist
    await chat.sendMessage({ text }, { body: { profile: provider, apiKey: key, ...(uiContext ? { uiContext } : {}) } })
  }

  function stop(): void { stopped.value = true; chat.stop(); persistCurrent() }

  /** Clear the current thread's content (the thread itself is kept). */
  function clear(): void {
    stopped.value = false
    chat.messages.value = []
    tray.clear()
    plan.value = []
    chat.clearError()
    persistCurrent()
  }

  return {
    messages: chat.messages, status: chat.status, busy, error,
    phase, activeTool, stopped, plan, autoApplyKinds,
    proposals: tray.proposals, send, stop, clear,
    threads: threadStore.threads, activeThreadId: threadStore.activeThreadId,
    newThread, switchThread, deleteThread,
    applyProposal: tray.applyProposal, forceApply: tray.forceApply, applySubset: tray.applySubset,
    // Meta operations stay manual-only even under "Apply all" (same guard as
    // the per-proposal auto-apply path).
    applyAll: () => tray.applyAll(p => !isMetaKind(p.target?.kind)),
    revertProposal: tray.revertProposal, discard: tray.discard,
  }
}

// ── Static-hosting chat (browser-direct, no /api backend) ────────────────────
// Same Panel-facing interface as the dev implementation, but `send` runs
// `streamText` in the browser against the OpenAI-compatible provider and the
// tools read/write through `dataFetch` (IndexedDB deltas over baselines).
// Proposals ride straight into the shared review tray via `emit('proposal')`.

type ChatStatus = 'ready' | 'submitted' | 'streaming' | 'error'

function uid(): string {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `m${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

function useStaticAssistantChat() {
  const messages = ref<any[]>(threadStore.activeSnapshot().messages)
  const status = ref<ChatStatus>('ready')
  const error = ref('')
  const stopped = ref(false)
  let abortRef: AbortController | null = null

  const busy = computed(() => status.value === 'submitted' || status.value === 'streaming')

  const lastMessage = computed(() => messages.value[messages.value.length - 1])
  const activeTool = computed(() => (busy.value ? runningToolOf(lastMessage.value) : '') || '')

  const phase = computed<AssistantPhase>(() => {
    const s = status.value
    if (s === 'error') return 'error'
    if (s === 'submitted') return 'thinking'
    if (s === 'streaming') {
      if (activeTool.value) return 'tool'
      const m = lastMessage.value
      return m && m.role === 'assistant' && messageText(m) ? 'writing' : 'thinking'
    }
    return 'idle'
  })

  watch(autoApplyKinds, v => saveJson(AUTOAPPLY_KINDS_KEY, v), { deep: true })

  function persistCurrent(): void {
    threadStore.saveSnapshot(threadStore.activeThreadId.value, {
      messages: messages.value, tray: tray.proposals.value, plan: plan.value,
    })
  }

  function loadActive(): void {
    const snap = threadStore.activeSnapshot()
    messages.value = snap.messages
    tray.replace(snap.tray)
    plan.value = snap.plan
    stopped.value = false
    error.value = ''
    status.value = 'ready'
  }

  watch(tray.proposals, () => persistCurrent(), { deep: true })

  function newThread(): void {
    if (busy.value) return
    persistCurrent()
    threadStore.createThread()
    loadActive()
  }

  function switchThread(id: string): void {
    if (busy.value || id === threadStore.activeThreadId.value) return
    persistCurrent()
    threadStore.setActive(id)
    loadActive()
  }

  function deleteThread(id: string): void {
    if (busy.value) return
    const wasActive = id === threadStore.activeThreadId.value
    threadStore.deleteThread(id)
    if (wasActive) loadActive()
  }

  function clear(): void {
    stopped.value = false
    messages.value = []
    tray.clear()
    plan.value = []
    error.value = ''
    status.value = 'ready'
    persistCurrent()
  }

  function stop(): void {
    stopped.value = true
    abortRef?.abort()
    persistCurrent()
  }

  async function send(text: string, provider: ProviderProfile, key: string, uiContext?: UiContext): Promise<void> {
    if (!text.trim() || busy.value) return
    stopped.value = false
    plan.value = []
    error.value = ''
    status.value = 'submitted'

    // User message goes into the model conversation; the assistant placeholder
    // is appended AFTER conversion so convertToModelMessages never sees it.
    const userMsg = { id: uid(), role: 'user', parts: [{ type: 'text', text }] }
    messages.value.push(userMsg)
    const asstMsg = { id: uid(), role: 'assistant', parts: [] as any[] }
    messages.value.push(asstMsg)

    const ac = new AbortController()
    abortRef = ac
    try {
      const emit: StaticAiEmit = (type, payload) => {
        if (type === 'proposal') {
          tray.add(payload)
          const p = tray.proposals.value[tray.proposals.value.length - 1]
          // Same auto-apply gate as the dev path (see the onData handler):
          // static kinds map onto the shared story/data/scene/gui/map switches
          // (pokemon/move/trainer/item → data, layout → gui); meta kinds are
          // never emitted here and shouldAutoApply hard-blocks them anyway.
          if (p && shouldAutoApply(autoApplyKindFor(p.target), autoApplyKinds.value)) void tray.applyProposal(p)
        } else if (type === 'plan') plan.value = (payload as { steps: PlanStep[] }).steps
      }
      const { streamText, convertToModelMessages, stepCountIs } = await import('ai')
      const model = await buildBrowserModel(provider, key)
      const modelMessages = await convertToModelMessages(messages.value.filter(m => m !== asstMsg))
      // The SDK's ToolSet type is opaque across module boundaries — the tool
      // objects are constructed by buildStaticTools with the same `tool()`
      // helper, so the cast is shape-safe.
      const tools = (await buildStaticTools(emit)) as any
      const result = streamText({
        model,
        system: buildStaticSystem(uiContext),
        messages: modelMessages,
        tools,
        stopWhen: [stepCountIs(16)],
        abortSignal: ac.signal,
      })

      // Stream assistant text into the placeholder's text part.
      let acc = ''
      for await (const delta of result.textStream) {
        status.value = 'streaming'
        acc += delta
        const tp = asstMsg.parts.find((p: any) => p.type === 'text')
        if (tp) tp.text = acc
        else asstMsg.parts.push({ type: 'text', text: acc })
      }

      // Record the tool calls AND their results as `tool-<name>` parts — the
      // Panel's "Inspecting:" list AND the next turn's model-message
      // round-trip (v6 needs input + non-undefined output + providerExecuted).
      const steps = await result.steps
      asstMsg.parts.push(...toolPartsFromSteps(steps))

      try { useAiUsage().record(await result.usage) } catch { /* usage unavailable */ }
      status.value = 'ready'
    } catch (e) {
      if (stopped.value) {
        status.value = 'ready'
      } else {
        error.value = (e as Error).message
        status.value = 'error'
      }
    } finally {
      abortRef = null
      persistCurrent()
    }
  }

  return {
    messages, status, busy, error,
    phase, activeTool, stopped, plan, autoApplyKinds,
    proposals: tray.proposals, send, stop, clear,
    threads: threadStore.threads, activeThreadId: threadStore.activeThreadId,
    newThread, switchThread, deleteThread,
    applyProposal: tray.applyProposal, forceApply: tray.forceApply, applySubset: tray.applySubset,
    applyAll: () => tray.applyAll(p => !isMetaKind(p.target?.kind)),
    revertProposal: tray.revertProposal, discard: tray.discard,
  }
}

// ── UIMessage.parts helpers (text + tool activity) ───────────────────────────

export function messageText(m: any): string {
  return (m?.parts ?? []).filter((p: any) => p?.type === 'text').map((p: any) => p.text).join('')
}

export function messageTools(m: any): string[] {
  const names: string[] = []
  for (const p of (m?.parts ?? [])) {
    const n = toolPartName(p)
    if (n) names.push(n)
  }
  return [...new Set(names)]
}

// ── auto-apply persistence (browser only; no-op under node/test) ─────────────
/**
 * Load the per-kind auto-apply switches, migrating the legacy all-or-nothing
 * boolean: it used to mean "apply everything", so `1` maps to all CONTENT
 * kinds on. Meta kinds stay manual-only either way (they reshape the project,
 * so auto-applying them was never acceptable).
 */
function loadAutoApplyKinds(): AutoApplySettings {
  if (typeof localStorage === 'undefined') return defaultAutoApplySettings()
  try {
    const s = localStorage.getItem(AUTOAPPLY_KINDS_KEY)
    if (s) return { ...defaultAutoApplySettings(), ...JSON.parse(s) }
    if (localStorage.getItem(LEGACY_AUTOAPPLY_KEY) === '1') {
      const migrated = defaultAutoApplySettings(true)
      localStorage.setItem(AUTOAPPLY_KINDS_KEY, JSON.stringify(migrated))
      localStorage.removeItem(LEGACY_AUTOAPPLY_KEY)
      return migrated
    }
  } catch { /* corrupted value → fall through to the defaults */ }
  return defaultAutoApplySettings()
}
function saveJson(key: string, v: unknown): void {
  if (typeof localStorage === 'undefined') return
  try { localStorage.setItem(key, JSON.stringify(v)) } catch { /* best effort */ }
}

/** Tool name of a UIMessage part, whether a static `tool-<name>` or `dynamic-tool`. */
function toolPartName(p: any): string | null {
  if (typeof p?.type === 'string' && p.type.startsWith('tool-')) return p.type.slice(5)
  if (p?.type === 'dynamic-tool' && p.toolName) return String(p.toolName)
  return null
}

/**
 * The tool currently executing in a message: a tool part whose args have been
 * received but whose output hasn't arrived yet (`input-streaming` /
 * `input-available`). '' once it resolves to `output-available` / `output-error`.
 */
function runningToolOf(m: any): string {
  for (const p of (m?.parts ?? [])) {
    const name = toolPartName(p)
    if (name && (p?.state === 'input-streaming' || p?.state === 'input-available')) return name
  }
  return ''
}
