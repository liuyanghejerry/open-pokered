<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useAiProviders } from '../../composables/useAiProviders'
import { useAssistantChat, messageText, messageTools } from '../../composables/useAssistantChat'
import { useAiUsage } from '../../composables/useAiUsage'
import { getStoredKey, setStoredKey } from '../../composables/useAiStream'
import { renderMarkdown } from '../../composables/useMarkdown'
import { quickPromptsFor, type QuickPrompt } from '../../composables/useQuickPrompts'
import AiKeyPrompt from './AiKeyPrompt.vue'
import AiSettingsModal from './AiSettingsModal.vue'
import ProposalCard from './ProposalCard.vue'
import { buildArtifacts, summarize, type Artifact } from './artifacts'
import { CONTENT_KINDS, isMetaKind } from './autoApply'
import type { ProviderProfile } from '../../types/ai'

interface MentionItem { kind: string; id: string; label: string; table?: string }

// activity = the editor's current activity id (map/script/pokemon/…), used for
// the context-aware quick prompts and sent along as uiContext.
const props = defineProps<{ activity: string }>()
const emit = defineEmits<{ close: []; jump: [activity: string] }>()

const route = useRoute()
const { providers, loadProviders } = useAiProviders()
const assistant = useAssistantChat()
const { messages, proposals, busy, error, phase, activeTool, stopped, plan, autoApplyKinds, threads, activeThreadId } = assistant
const usage = useAiUsage()

// Human-readable label for the current working state (header + activity row).
const statusLabel = computed(() => {
  if (phase.value === 'error') return 'Error'
  if (activeTool.value) return `Running ${activeTool.value}…`
  if (phase.value === 'writing') return 'Writing…'
  return 'Thinking…'
})

// ── chat threads (multi-session) ────────────────────────────────────────────
// Thread index + active id are shared singletons. All mutations are locked
// while busy: a running stream is bound to the live chat instance and must not
// switch threads mid-flight.
const threadsOpen = ref(false)

function toggleThreads() { if (!busy.value) threadsOpen.value = !threadsOpen.value }
function onNewThread() { assistant.newThread(); threadsOpen.value = false }
function onSwitchThread(id: string) { assistant.switchThread(id); threadsOpen.value = false }
function onDeleteThread(id: string) { assistant.deleteThread(id) }

/** Compact relative timestamp for a thread row ("just now" / "5m ago" / …). */
function relTime(ts: number): string {
  const mins = Math.floor((Date.now() - ts) / 60000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.floor(hours / 24)}d ago`
}

// ── session artifacts ("produced this session") ─────────────────────────────
// Derived from the shared proposal tray: everything currently applied. Clearing
// the chat empties the tray, so this list clears with it.
const artifacts = computed(() => buildArtifacts(proposals.value))
const artifactStats = computed(() => summarize(artifacts.value))
const artifactsOpen = ref(true) // collapsible, expanded by default
const autoApplyOpen = ref(false)

// Header aggregate: N files · +X/−Y lines · Z tokens (session token meter).
const summaryLine = computed(() =>
  `${artifactStats.value.files} files · +${artifactStats.value.add}/−${artifactStats.value.del} lines · ${fmtTokens(usage.total.value)} tokens`,
)

/** Compact "1.2k" token count (mirrors the formatting inside useAiUsage). */
function fmtTokens(n: number): string {
  return n >= 1000 ? (n / 1000).toFixed(n >= 10000 ? 0 : 1) + 'k' : String(n)
}

// Artifact activity TYPE (from artifacts.ts) → pokered-editor activity id.
// Kinds without a natural home here render non-clickable.
const ACTIVITY_JUMP: Record<string, string> = {
  script: 'script',
  map: 'map',
  ui: 'layout',
}

/** Resolve an artifact's activity TYPE to a pokered activity id. */
function activityIdFor(a: Artifact): string | null {
  return a.activityType ? (ACTIVITY_JUMP[a.activityType] ?? null) : null
}

/** Row click: ask the parent to switch to the activity owning the artifact. */
function jumpToArtifact(a: Artifact) {
  const id = activityIdFor(a)
  if (id) emit('jump', id)
}

const providerId = ref('')
const draft = ref('')
const showKeyPrompt = ref(false)
const settingsOpen = ref(false)
const pendingText = ref('')
const scrollEl = ref<HTMLElement | null>(null)
const taRef = ref<HTMLTextAreaElement | null>(null)

// ── resizable width ─────────────────────────────────────────────────────────
const MIN_W = 320, MAX_W = 760
const width = ref(clampW(Number(localStorage.getItem('jrpg-assistant-width')) || 384))
function clampW(w: number) { return Math.max(MIN_W, Math.min(MAX_W, w)) }
let dragStartX = 0, dragStartW = 0
function onDragMove(e: MouseEvent) { width.value = clampW(dragStartW + (dragStartX - e.clientX)) }
function onDragEnd() {
  window.removeEventListener('mousemove', onDragMove)
  window.removeEventListener('mouseup', onDragEnd)
  document.body.style.userSelect = ''
  localStorage.setItem('jrpg-assistant-width', String(width.value))
}
function startResize(e: MouseEvent) {
  dragStartX = e.clientX; dragStartW = width.value
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onDragMove)
  window.addEventListener('mouseup', onDragEnd)
}
onBeforeUnmount(onDragEnd)

// ── @mention autocomplete ───────────────────────────────────────────────────
const mentionItems = ref<MentionItem[]>([])
const mentionOpen = ref(false)
const mentionQuery = ref('')
const mentionActive = ref(0)
let mentionStart = -1
let mentionsLoaded = false

async function ensureMentions() {
  if (mentionsLoaded) return
  mentionsLoaded = true
  try { const r = await fetch('/api/ai/mentions'); mentionItems.value = r.ok ? await r.json() : [] }
  catch { mentionItems.value = [] }
}

const mentionMatches = computed(() => {
  if (!mentionOpen.value) return []
  const q = mentionQuery.value.toLowerCase()
  return mentionItems.value
    .filter(it => !q || it.id.toLowerCase().includes(q) || String(it.label).toLowerCase().includes(q))
    .slice(0, 8)
})

function onInput() {
  const ta = taRef.value
  if (!ta) return
  const before = draft.value.slice(0, ta.selectionStart ?? draft.value.length)
  const m = before.match(/@([^\s@]*)$/)
  if (m) { mentionStart = (ta.selectionStart ?? 0) - m[0].length; mentionQuery.value = m[1]; mentionActive.value = 0; mentionOpen.value = true; ensureMentions() }
  else mentionOpen.value = false
}

function applyMention(it: MentionItem) {
  const ta = taRef.value
  const pos = ta?.selectionStart ?? draft.value.length
  const token = '@' + it.id + ' '
  draft.value = draft.value.slice(0, mentionStart) + token + draft.value.slice(pos)
  mentionOpen.value = false
  nextTick(() => { if (ta) { const c = mentionStart + token.length; ta.selectionStart = ta.selectionEnd = c; ta.focus() } })
}

function onKeydown(e: KeyboardEvent) {
  const matches = mentionMatches.value
  if (mentionOpen.value && matches.length) {
    if (e.key === 'ArrowDown') { e.preventDefault(); mentionActive.value = (mentionActive.value + 1) % matches.length; return }
    if (e.key === 'ArrowUp') { e.preventDefault(); mentionActive.value = (mentionActive.value - 1 + matches.length) % matches.length; return }
    if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); applyMention(matches[mentionActive.value]); return }
    if (e.key === 'Escape') { e.preventDefault(); mentionOpen.value = false; return }
  }
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit() }
}

// ── quick-prompt chips ──────────────────────────────────────────────────────
// Context-aware canned instructions above the input; a click sends one like a
// typed message. Selection comes from the current activity id.
const quickPrompts = computed(() => quickPromptsFor({ activity: props.activity || null }))

// ── providers + send ────────────────────────────────────────────────────────
onMounted(async () => {
  await loadProviders()
  providerId.value = providerId.value || providers.value[0]?.id || ''
})

function pickProvider(): ProviderProfile | undefined {
  return providers.value.find(p => p.id === providerId.value) || providers.value[0]
}

function submit() {
  sendText(draft.value.trim())
}

/** One-click chip: send the canned instruction like typed text. */
function runQuickPrompt(p: QuickPrompt) {
  sendText(p.prompt)
}

/** Shared send path (textarea + chips): resolve provider → API key → fire. */
function sendText(text: string) {
  if (!text || busy.value) return
  const provider = pickProvider()
  if (!provider) return
  const key = getStoredKey(provider.id)
  if (!key) { pendingText.value = text; showKeyPrompt.value = true; return }
  fire(text, provider, key)
}

function onKeySubmit(key: string, remember: boolean) {
  showKeyPrompt.value = false
  const provider = pickProvider()
  if (!provider) return
  if (remember) setStoredKey(provider.id, key)
  fire(pendingText.value, provider, key)
}

async function fire(text: string, provider: ProviderProfile, key: string) {
  draft.value = ''
  mentionOpen.value = false
  // Tell the agent what the user is looking at.
  const uiContext = { activity: props.activity || undefined, route: route.fullPath }
  await assistant.send(text, provider, key, uiContext)
}

watch(
  () => [messages.value.length, messages.value.length ? messageText(messages.value[messages.value.length - 1]) : '', proposals.value.length, plan.value.length, phase.value, activeTool.value],
  async () => { await nextTick(); if (scrollEl.value) scrollEl.value.scrollTop = scrollEl.value.scrollHeight },
)
</script>

<template>
  <aside class="relative bg-gray-800 border-l border-gray-700 flex flex-col shrink-0" :style="{ width: width + 'px' }">
    <!-- drag-to-resize handle on the left edge -->
    <div class="absolute left-0 top-0 h-full w-1.5 -ml-0.5 cursor-col-resize hover:bg-blue-500/50 z-20" @mousedown.prevent="startResize" />

    <div class="flex items-center gap-2 px-3 py-2 border-b border-gray-700 shrink-0">
      <span class="text-sm font-bold text-blue-400">✨ AI Assistant</span>
      <!-- live working-state indicator: a pulsing dot + label, hidden when idle -->
      <span v-if="phase !== 'idle'" :title="statusLabel"
        class="flex items-center gap-1 max-w-[9rem] text-[10px]"
        :class="phase === 'error' ? 'text-red-400' : 'text-blue-300'">
        <span class="status-dot" :class="phase === 'error' ? 'is-error' : 'is-busy'" />
        <span class="truncate">{{ statusLabel }}</span>
      </span>
      <button @click="toggleThreads" :disabled="busy"
        :title="busy ? 'Finish or stop the current run first' : 'Chats'"
        class="text-gray-500 hover:text-gray-300 text-xs disabled:opacity-40 disabled:hover:text-gray-500">🗂</button>
      <button @click="onNewThread" :disabled="busy"
        :title="busy ? 'Finish or stop the current run first' : 'New chat'"
        class="text-gray-500 hover:text-gray-300 text-xs disabled:opacity-40 disabled:hover:text-gray-500">＋</button>
      <select v-if="providers.length" v-model="providerId"
        class="ml-auto bg-gray-700 text-gray-200 text-[11px] rounded px-1.5 py-0.5 border border-gray-600 max-w-[8rem]">
        <option v-for="p in providers" :key="p.id" :value="p.id">{{ p.id }}</option>
      </select>
      <span v-if="usage.total.value > 0" :title="`Session tokens (input↑ / output↓) over ${usage.calls.value} calls`"
        class="text-[10px] text-gray-500 tabular-nums">{{ usage.label.value }}</span>
      <button @click="settingsOpen = true" title="AI provider settings" class="text-gray-500 hover:text-gray-300 text-xs">⚙</button>
      <button @click="assistant.clear()" title="Clear chat" class="text-gray-500 hover:text-gray-300 text-xs">⟲</button>
      <button @click="emit('close')" class="text-gray-500 hover:text-gray-300 text-sm">✕</button>
    </div>

    <!-- chat threads dropdown: one row per session, most recently active first -->
    <div v-if="threadsOpen"
      class="absolute left-2 right-2 top-9 z-30 max-h-72 overflow-y-auto bg-gray-900 border border-gray-700 rounded shadow-xl">
      <div v-for="th in threads" :key="th.id" @click="onSwitchThread(th.id)"
        :title="busy ? 'Finish or stop the current run first' : undefined"
        :class="['flex items-center gap-2 px-2.5 py-1.5 text-xs',
          th.id === activeThreadId ? 'bg-blue-600/20 text-gray-100' : 'text-gray-300 hover:bg-gray-800',
          busy ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer']">
        <span class="flex-1 min-w-0 truncate">{{ th.title || 'New chat' }}</span>
        <span class="shrink-0 text-[10px] text-gray-500 tabular-nums">{{ relTime(th.updatedAt) }}</span>
        <button @click.stop="onDeleteThread(th.id)" :disabled="busy"
          :title="busy ? 'Finish or stop the current run first' : 'Delete this chat'"
          class="shrink-0 text-gray-600 hover:text-red-400 disabled:opacity-40 disabled:hover:text-gray-600">✕</button>
      </div>
    </div>

    <div ref="scrollEl" class="flex-1 overflow-y-auto px-3 py-3 space-y-3">
      <p v-if="!messages.length" class="text-xs text-gray-500 leading-relaxed">Ask me to inspect or edit your game. I propose changes for you to review — I never apply them directly.</p>

      <template v-for="(m, i) in messages" :key="m.id || i">
        <div v-if="m.role === 'user'" class="flex justify-end">
          <div class="max-w-[85%] bg-blue-600/90 text-white text-xs rounded-lg rounded-br-sm px-2.5 py-1.5 whitespace-pre-wrap break-words">{{ messageText(m) }}</div>
        </div>
        <div v-else class="space-y-1.5">
          <div v-if="messageTools(m).length" class="text-[10px] text-gray-500">
            <span class="opacity-70">Inspecting:</span> {{ messageTools(m).join(' · ') }}
          </div>
          <div v-if="messageText(m)" class="md text-xs text-gray-200 leading-relaxed break-words" v-html="renderMarkdown(messageText(m))" />
        </div>
      </template>

      <!-- the agent's working checklist (update_plan) -->
      <div v-if="plan.length" class="rounded border border-gray-700/70 bg-gray-800/40 px-2.5 py-2 space-y-1">
        <div class="text-[10px] font-semibold text-gray-400 flex items-center gap-1">📋 Plan</div>
        <div v-for="(s, i) in plan" :key="i" class="flex items-start gap-1.5 text-[11px]">
          <span class="mt-[1px] shrink-0"
            :class="s.status === 'done' ? 'text-emerald-400' : s.status === 'active' ? 'text-blue-400' : 'text-gray-600'">{{ s.status === 'done' ? '✓' : s.status === 'active' ? '▸' : '○' }}</span>
          <span :class="s.status === 'done' ? 'text-gray-500 line-through' : s.status === 'active' ? 'text-gray-100' : 'text-gray-400'">{{ s.title }}</span>
        </div>
      </div>

      <div v-if="proposals.length" class="pt-1 space-y-2">
        <div class="flex items-center gap-2">
          <span class="text-[11px] font-semibold text-gray-400">Proposed changes ({{ proposals.length }})</span>
          <!-- meta operations (project config/scaffold/map-create) are excluded: they always need a manual apply -->
          <button v-if="proposals.some(p => p.status === 'pending' && !isMetaKind(p.target?.kind))" @click="assistant.applyAll()"
            class="ml-auto text-[10px] px-2 py-0.5 rounded bg-emerald-700 text-white hover:bg-emerald-600">Apply all</button>
        </div>
        <ProposalCard v-for="p in proposals" :key="p.uid" :proposal="p"
          @apply="assistant.applyProposal(p)" @apply-subset="(acc) => assistant.applySubset(p, new Set(acc))"
          @force-apply="assistant.forceApply(p)"
          @discard="assistant.discard(p)" @revert="assistant.revertProposal(p)" />
      </div>

      <!-- session artifacts: proposals currently applied; a row jumps to the owning activity -->
      <div v-if="artifacts.length" class="pt-1">
        <button @click="artifactsOpen = !artifactsOpen" class="flex w-full items-center gap-1.5 text-left">
          <span class="inline-block w-2.5 shrink-0 text-[10px] text-gray-500">{{ artifactsOpen ? '▾' : '▸' }}</span>
          <span class="text-[11px] font-semibold text-gray-400">Produced this session</span>
          <span class="ml-auto text-[10px] text-gray-500 tabular-nums">{{ summaryLine }}</span>
        </button>
        <div v-if="artifactsOpen" class="mt-1 space-y-0.5">
          <button v-for="a in artifacts" :key="a.uid" @click="jumpToArtifact(a)" :disabled="!activityIdFor(a)"
            :title="activityIdFor(a) ? a.path : undefined"
            class="flex w-full items-center gap-1.5 px-1.5 py-1 rounded text-left hover:bg-gray-700/40 disabled:hover:bg-transparent disabled:cursor-default">
            <span class="shrink-0 text-[11px]">{{ a.icon }}</span>
            <span class="min-w-0 flex-1 truncate text-[11px]" :class="activityIdFor(a) ? 'text-gray-300' : 'text-gray-500'">{{ a.path }}</span>
            <span class="shrink-0 text-[10px] tabular-nums"><span class="text-emerald-500">+{{ a.add }}</span><span v-if="a.del" class="text-red-500 ml-1">−{{ a.del }}</span></span>
          </button>
        </div>
      </div>

      <!-- persistent working-state footer: always visible while busy (even mid-stream),
           so the user can always tell the assistant is still working vs. finished -->
      <div v-if="busy" class="flex items-center gap-2 text-[11px] text-blue-300">
        <span class="typing-dots"><i /><i /><i /></span>
        <span>{{ statusLabel }}</span>
      </div>
      <div v-else-if="stopped" class="flex items-center gap-1.5 text-[11px] text-amber-400/90">
        <span class="text-[10px]">⏹</span>Stopped
      </div>
    </div>

    <div v-if="error" class="px-3 py-1 text-[11px] text-red-400 border-t border-gray-700 shrink-0">{{ error }}</div>

    <div class="relative border-t border-gray-700 p-2 shrink-0">
      <!-- @mention autocomplete -->
      <div v-if="mentionOpen && mentionMatches.length"
        class="absolute bottom-full left-2 right-2 mb-1 max-h-52 overflow-y-auto bg-gray-900 border border-gray-700 rounded shadow-xl z-30">
        <button v-for="(it, idx) in mentionMatches" :key="it.kind + it.id"
          @mousedown.prevent="applyMention(it)" @mouseenter="mentionActive = idx"
          :class="['w-full flex items-center gap-2 px-2 py-1 text-left text-xs', idx === mentionActive ? 'bg-blue-600/30' : 'hover:bg-gray-800']">
          <span class="text-[9px] uppercase text-gray-500 w-10 shrink-0">{{ it.kind }}</span>
          <span class="text-gray-200 truncate">{{ it.label }}</span>
          <span v-if="String(it.label) !== it.id" class="text-gray-500 truncate text-[10px]">{{ it.id }}</span>
        </button>
      </div>

      <p v-if="!providers.length" class="text-[11px] text-amber-400 px-1 pb-1">
        No AI provider configured yet —
        <button @click="settingsOpen = true" class="underline hover:text-amber-300">open ⚙ AI Settings</button>
        to add one.
      </p>
      <!-- per-kind auto-apply: content kinds get a switch; meta operations never auto-apply -->
      <div class="px-1 pb-1">
        <button @click="autoApplyOpen = !autoApplyOpen"
          title="Apply each change the moment the assistant proposes it, without reviewing. The drift guard still blocks conflicting writes."
          class="flex items-center gap-1 text-[10px] text-gray-500 hover:text-gray-300 select-none">
          <span class="inline-block w-2.5">{{ autoApplyOpen ? '▾' : '▸' }}</span>Auto-apply proposals
        </button>
        <div v-if="autoApplyOpen" class="mt-1 pl-3.5 flex flex-wrap items-center gap-x-3 gap-y-1">
          <!-- labels reuse the raw proposal-kind text shown on the review-card badges -->
          <label v-for="k in CONTENT_KINDS" :key="k" class="flex items-center gap-1 text-[10px] text-gray-400 cursor-pointer select-none">
            <input type="checkbox" v-model="autoApplyKinds[k]" class="accent-emerald-500" />{{ k }}
          </label>
          <span class="text-[10px] text-gray-600">Project-level changes (config / scaffold / new map) always need manual review</span>
        </div>
      </div>
      <!-- context-aware quick prompts: one click sends a canned instruction -->
      <div v-if="quickPrompts.length" class="flex flex-wrap gap-1 px-1 pb-1">
        <button v-for="p in quickPrompts" :key="p.id" @click="runQuickPrompt(p)" :disabled="busy || !providers.length"
          class="text-[10px] px-2 py-0.5 rounded-full border border-gray-700 bg-gray-800 hover:border-gray-500 text-gray-300 disabled:opacity-40 disabled:hover:border-gray-700">
          {{ p.icon }} {{ p.label }}
        </button>
      </div>
      <div class="flex items-end gap-1.5">
        <textarea ref="taRef" v-model="draft" rows="2" placeholder="Ask about the project, or describe an edit… use @name to reference an item"
          @input="onInput" @keydown="onKeydown"
          class="flex-1 resize-none bg-gray-900 border border-gray-700 rounded px-2 py-1.5 text-xs text-gray-100 focus:border-blue-500 focus:outline-none"></textarea>
        <button v-if="!busy" @click="submit" :disabled="!draft.trim() || !providers.length"
          class="px-3 py-1.5 text-xs rounded bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-40">Send</button>
        <button v-else @click="assistant.stop()" :title="statusLabel"
          class="flex items-center gap-1 px-3 py-1.5 text-xs rounded bg-gray-600 text-white hover:bg-gray-500">
          <span class="inline-block w-2 h-2 rounded-[1px] bg-white/90" />Stop</button>
      </div>
    </div>

    <AiKeyPrompt v-if="showKeyPrompt" :provider-id="providerId" @submit="onKeySubmit" @cancel="showKeyPrompt = false" />
    <AiSettingsModal v-if="settingsOpen" @close="settingsOpen = false" />
  </aside>
</template>

<style scoped>
/* ── working-state indicators ─────────────────────────────────────────────── */
/* header status dot: pulses while busy, solid red on error */
.status-dot { width: 6px; height: 6px; border-radius: 9999px; flex: none; }
.status-dot.is-busy { background: #60a5fa; animation: statusPulse 1s ease-in-out infinite; }
.status-dot.is-error { background: #f87171; }
@keyframes statusPulse { 0%, 100% { opacity: 0.35; transform: scale(0.85); } 50% { opacity: 1; transform: scale(1.15); } }

/* bottom activity row: three bouncing dots */
.typing-dots { display: inline-flex; align-items: center; gap: 3px; }
.typing-dots i { width: 5px; height: 5px; border-radius: 9999px; background: currentColor; display: inline-block; animation: typingBounce 1.2s ease-in-out infinite; }
.typing-dots i:nth-child(2) { animation-delay: 0.15s; }
.typing-dots i:nth-child(3) { animation-delay: 0.3s; }
@keyframes typingBounce { 0%, 80%, 100% { opacity: 0.3; transform: translateY(0); } 40% { opacity: 1; transform: translateY(-3px); } }

/* Markdown rendered into v-html — :deep so scoped styles reach the injected nodes. */
.md :deep(p) { margin: 0 0 0.45rem; }
.md :deep(p:last-child) { margin-bottom: 0; }
.md :deep(h1), .md :deep(h2), .md :deep(h3), .md :deep(h4) { font-weight: 600; color: #f3f4f6; margin: 0.5rem 0 0.3rem; line-height: 1.25; }
.md :deep(h1) { font-size: 0.95rem; }
.md :deep(h2) { font-size: 0.9rem; }
.md :deep(h3), .md :deep(h4) { font-size: 0.82rem; }
.md :deep(ul) { list-style: disc; padding-left: 1.1rem; margin: 0.3rem 0; }
.md :deep(ol) { list-style: decimal; padding-left: 1.25rem; margin: 0.3rem 0; }
.md :deep(li) { margin: 0.1rem 0; }
.md :deep(code) { background: #0f172a; padding: 0.05rem 0.25rem; border-radius: 3px; font-size: 0.92em; }
.md :deep(pre) { background: #0f172a; padding: 0.5rem; border-radius: 4px; overflow-x: auto; margin: 0.4rem 0; }
.md :deep(pre code) { background: transparent; padding: 0; }
.md :deep(a) { color: #60a5fa; text-decoration: underline; }
.md :deep(strong) { font-weight: 600; color: #e5e7eb; }
.md :deep(em) { font-style: italic; }
.md :deep(blockquote) { border-left: 2px solid #475569; padding-left: 0.5rem; color: #94a3b8; margin: 0.3rem 0; }
.md :deep(hr) { border: none; border-top: 1px solid #374151; margin: 0.5rem 0; }
</style>
