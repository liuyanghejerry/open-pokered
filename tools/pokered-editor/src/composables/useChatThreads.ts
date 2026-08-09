// ───────────────────────────────────────────────────────────────────────────
// useChatThreads — multi-session threads for the AI Assistant panel.
//
// Each thread owns an independent conversation + proposals tray + plan, kept
// as a localStorage snapshot per thread:
//   jrpg-assistant-threads        index [{id,title,createdAt,updatedAt}] (new→old)
//   jrpg-assistant-active-thread  the open thread's id
//   jrpg-assistant-thread-<id>    snapshot {messages, tray, plan?}
//
// First load migrates the legacy single-conversation keys
// (jrpg-assistant-messages / jrpg-assistant-tray) into one thread and removes
// them. Factory with injectable storage so unit tests run under plain node
// (no localStorage); the assistant holds one module singleton shared by the
// welcome and dock panel instances. Chat-binding concerns (busy lock, live
// message state) stay in useAssistantChat — this store is pure thread state.
// ───────────────────────────────────────────────────────────────────────────
import { ref } from 'vue'
import type { AssistantProposal } from './useProposals'

export interface ThreadMeta { id: string; title: string; createdAt: number; updatedAt: number }

/** One thread's durable state. `plan` is optional (pre-plan snapshots). */
export interface ThreadSnapshot {
  messages: any[]
  tray: AssistantProposal[]
  plan?: { title: string; status: string }[]
}

/** Minimal Storage subset — injectable so tests don't need a DOM. */
export interface StorageLike {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

export const THREADS_KEY = 'jrpg-assistant-threads'
export const ACTIVE_KEY = 'jrpg-assistant-active-thread'
export const LEGACY_MESSAGES_KEY = 'jrpg-assistant-messages'
export const LEGACY_TRAY_KEY = 'jrpg-assistant-tray'

/** Derived titles are truncated to this many characters. */
export const TITLE_MAX = 30

export function threadKey(id: string): string { return `jrpg-assistant-thread-${id}` }

/**
 * Display title for a thread: its first user message, whitespace-collapsed and
 * truncated. '' when there is no user message yet — the UI falls back to a
 * localized "New chat" label.
 */
export function deriveTitle(messages: any[]): string {
  const first = (messages ?? []).find(m => m?.role === 'user')
  const text = textOf(first).trim().replace(/\s+/g, ' ')
  if (!text) return ''
  return text.length > TITLE_MAX ? text.slice(0, TITLE_MAX) + '…' : text
}

function textOf(m: any): string {
  if (!m) return ''
  if (typeof m.content === 'string') return m.content
  return (m.parts ?? []).filter((p: any) => p?.type === 'text').map((p: any) => p.text ?? '').join('')
}

function genId(): string {
  return `t${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`
}

/** In-memory fallback where localStorage is missing (node/tests): the store
 *  stays functional, nothing persists. */
function memoryStorage(): StorageLike {
  const m = new Map<string, string>()
  return {
    getItem: k => (m.has(k) ? m.get(k)! : null),
    setItem: (k, v) => { m.set(k, v) },
    removeItem: k => { m.delete(k) },
  }
}

function defaultStorage(): StorageLike {
  return typeof localStorage === 'undefined' ? memoryStorage() : localStorage
}

export function useChatThreads(storage: StorageLike = defaultStorage()) {
  const threads = ref<ThreadMeta[]>([]) // index, kept sorted most-recent-first
  const activeThreadId = ref('')

  init()

  /** Load the index, else migrate the legacy keys, else bootstrap one thread. */
  function init(): void {
    const raw = readJson(THREADS_KEY)
    const index = Array.isArray(raw)
      ? raw.filter(isMeta).map(m => ({ ...m, title: typeof m.title === 'string' ? m.title : '' }))
      : []
    if (index.length) threads.value = sortByRecent(index)
    else if (!migrateLegacy()) createThread()
    const stored = safeGet(ACTIVE_KEY)
    setActive(threads.value.some(t => t.id === stored) ? stored as string : threads.value[0].id)
  }

  /** Fold the pre-threads single conversation into a first thread. Returns
   *  false when no legacy keys exist (nothing to migrate). */
  function migrateLegacy(): boolean {
    const rawMessages = safeGet(LEGACY_MESSAGES_KEY)
    const rawTray = safeGet(LEGACY_TRAY_KEY)
    if (rawMessages == null && rawTray == null) return false
    const messages = parseArray(rawMessages)
    const tray = parseArray(rawTray)
    const id = genId()
    const ts = Date.now()
    threads.value = [{ id, title: deriveTitle(messages), createdAt: ts, updatedAt: ts }]
    writeJson(threadKey(id), { messages, tray, plan: [] })
    persistIndex()
    try {
      storage.removeItem(LEGACY_MESSAGES_KEY)
      storage.removeItem(LEGACY_TRAY_KEY)
    } catch { /* best effort */ }
    return true
  }

  /** Append a fresh empty thread and make it active. Returns its id. */
  function createThread(): string {
    const id = genId()
    const ts = Date.now()
    threads.value = sortByRecent([...threads.value, { id, title: '', createdAt: ts, updatedAt: ts }])
    writeJson(threadKey(id), { messages: [], tray: [], plan: [] })
    persistIndex()
    setActive(id)
    return id
  }

  /** Make an existing thread active (unknown ids are ignored). */
  function setActive(id: string): void {
    if (!threads.value.some(t => t.id === id)) return
    activeThreadId.value = id
    try { storage.setItem(ACTIVE_KEY, id) } catch { /* quota / disabled — best effort */ }
  }

  /**
   * Delete a thread and its snapshot. Deleting the active thread falls back to
   * the most recent remaining one; deleting the very last thread bootstraps a
   * fresh empty one, so a thread always exists.
   */
  function deleteThread(id: string): void {
    try { storage.removeItem(threadKey(id)) } catch { /* best effort */ }
    threads.value = threads.value.filter(t => t.id !== id)
    if (!threads.value.length) { createThread(); return }
    persistIndex()
    if (activeThreadId.value === id) setActive(threads.value[0].id)
  }

  /**
   * Persist a thread's content. Bumps updatedAt (so the index re-sorts) and
   * derives the title from the first user message — only while the title is
   * unset, so a named thread is never auto-renamed.
   */
  function saveSnapshot(id: string, snap: ThreadSnapshot): void {
    const meta = threads.value.find(t => t.id === id)
    if (!meta) return
    writeJson(threadKey(id), {
      messages: snap.messages ?? [], tray: snap.tray ?? [], plan: snap.plan ?? [],
    })
    if (!meta.title) meta.title = deriveTitle(snap.messages ?? [])
    meta.updatedAt = Date.now()
    threads.value = sortByRecent(threads.value)
    persistIndex()
  }

  /** Read a thread's snapshot, normalized to empty arrays on any corruption. */
  function loadSnapshot(id: string): Required<ThreadSnapshot> {
    const v = readJson(threadKey(id))
    return {
      messages: Array.isArray(v?.messages) ? v.messages : [],
      tray: Array.isArray(v?.tray) ? v.tray : [],
      plan: Array.isArray(v?.plan) ? v.plan : [],
    }
  }

  function activeSnapshot(): Required<ThreadSnapshot> {
    return loadSnapshot(activeThreadId.value)
  }

  // ── storage helpers (JSON + best-effort writes) ───────────────────────────
  function safeGet(key: string): string | null {
    try { return storage.getItem(key) } catch { return null }
  }
  function readJson(key: string): any {
    const s = safeGet(key)
    if (s == null) return undefined
    try { return JSON.parse(s) } catch { return undefined }
  }
  function parseArray(s: string | null): any[] {
    if (s == null) return []
    try { const v = JSON.parse(s); return Array.isArray(v) ? v : [] } catch { return [] }
  }
  function writeJson(key: string, v: unknown): void {
    try { storage.setItem(key, JSON.stringify(v)) } catch { /* quota / disabled — best effort */ }
  }
  function persistIndex(): void { writeJson(THREADS_KEY, threads.value) }

  return {
    threads, activeThreadId,
    createThread, setActive, deleteThread,
    saveSnapshot, loadSnapshot, activeSnapshot,
  }
}

function isMeta(v: any): v is ThreadMeta {
  return !!v && typeof v.id === 'string' && typeof v.createdAt === 'number' && typeof v.updatedAt === 'number'
}

/** Index order: most recently updated first (createdAt breaks ties). */
function sortByRecent(list: ThreadMeta[]): ThreadMeta[] {
  return [...list].sort((a, b) => b.updatedAt - a.updatedAt || b.createdAt - a.createdAt)
}
