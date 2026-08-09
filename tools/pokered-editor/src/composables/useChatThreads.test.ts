// Multi-session chat threads: index/snapshot storage, legacy migration,
// title derivation, updatedAt ordering. Storage is injected (node has no
// localStorage) — a Map-backed mock mirrors the browser behavior.
import { describe, it, expect, afterEach, vi } from 'vitest'
import {
  useChatThreads, deriveTitle, threadKey,
  THREADS_KEY, ACTIVE_KEY, LEGACY_MESSAGES_KEY, LEGACY_TRAY_KEY,
  type StorageLike,
} from './useChatThreads'

function mockStorage() {
  const data = new Map<string, string>()
  const storage: StorageLike = {
    getItem: k => (data.has(k) ? data.get(k)! : null),
    setItem: (k, v) => { data.set(k, String(v)) },
    removeItem: k => { data.delete(k) },
  }
  return { storage, data }
}

const userMsg = (text: string) => ({ id: `u:${text}`, role: 'user', parts: [{ type: 'text', text }] })
const assistantMsg = (text: string) => ({ id: `a:${text}`, role: 'assistant', parts: [{ type: 'text', text }] })
const EMPTY = { messages: [], tray: [], plan: [] }

afterEach(() => { vi.useRealTimers() })

describe('useChatThreads bootstrap', () => {
  it('creates a single empty thread on first run', () => {
    const { storage, data } = mockStorage()
    const store = useChatThreads(storage)
    expect(store.threads.value).toHaveLength(1)
    expect(store.threads.value[0].title).toBe('')
    expect(store.activeThreadId.value).toBe(store.threads.value[0].id)
    expect(store.activeSnapshot()).toEqual(EMPTY)
    // persisted: index + active id + an empty snapshot key
    expect(JSON.parse(data.get(THREADS_KEY)!)).toHaveLength(1)
    expect(data.get(ACTIVE_KEY)).toBe(store.threads.value[0].id)
    expect(data.has(threadKey(store.threads.value[0].id))).toBe(true)
  })

  it('survives a reload (second instance over the same storage)', () => {
    const { storage } = mockStorage()
    const s1 = useChatThreads(storage)
    const id = s1.activeThreadId.value
    s1.saveSnapshot(id, { messages: [userMsg('persisted')], tray: [{ uid: 'p1' } as any] })
    const s2 = useChatThreads(storage)
    expect(s2.activeThreadId.value).toBe(id)
    expect(s2.threads.value[0].title).toBe('persisted')
    expect(s2.activeSnapshot().messages).toEqual([userMsg('persisted')])
    expect(s2.activeSnapshot().tray).toEqual([{ uid: 'p1' }])
  })

  it('bootstraps fresh when the stored index is corrupt', () => {
    const { storage, data } = mockStorage()
    data.set(THREADS_KEY, '{not json')
    const store = useChatThreads(storage)
    expect(store.threads.value).toHaveLength(1)
    expect(store.activeSnapshot()).toEqual(EMPTY)
  })

  it('falls back to the first thread when the stored active id is gone', () => {
    const { storage, data } = mockStorage()
    const s1 = useChatThreads(storage)
    const id = s1.activeThreadId.value
    data.set(ACTIVE_KEY, 'ghost')
    const s2 = useChatThreads(storage)
    expect(s2.activeThreadId.value).toBe(id)
  })
})

describe('useChatThreads legacy migration', () => {
  it('folds the legacy messages + tray keys into a first thread', () => {
    const { storage, data } = mockStorage()
    const messages = [userMsg('Build me a castle map'), assistantMsg('Sure')]
    const tray = [{ uid: 'p1', status: 'pending' }]
    data.set(LEGACY_MESSAGES_KEY, JSON.stringify(messages))
    data.set(LEGACY_TRAY_KEY, JSON.stringify(tray))
    const store = useChatThreads(storage)
    expect(store.threads.value).toHaveLength(1)
    expect(store.threads.value[0].title).toBe('Build me a castle map')
    expect(store.activeSnapshot().messages).toEqual(messages)
    expect(store.activeSnapshot().tray).toEqual(tray)
    expect(data.get(ACTIVE_KEY)).toBe(store.threads.value[0].id)
    // legacy keys are removed once migrated
    expect(data.has(LEGACY_MESSAGES_KEY)).toBe(false)
    expect(data.has(LEGACY_TRAY_KEY)).toBe(false)
  })

  it('migrates a tray-only legacy state with an empty title (UI falls back)', () => {
    const { storage, data } = mockStorage()
    data.set(LEGACY_TRAY_KEY, JSON.stringify([{ uid: 'p2' }]))
    const store = useChatThreads(storage)
    expect(store.threads.value).toHaveLength(1)
    expect(store.threads.value[0].title).toBe('')
    expect(store.activeSnapshot().tray).toEqual([{ uid: 'p2' }])
    expect(data.has(LEGACY_TRAY_KEY)).toBe(false)
  })

  it('does not migrate when a threads index already exists', () => {
    const { storage, data } = mockStorage()
    const s1 = useChatThreads(storage)
    const id = s1.activeThreadId.value
    // legacy keys reappearing later (e.g. written by an older build) stay put
    data.set(LEGACY_MESSAGES_KEY, JSON.stringify([userMsg('old')]))
    const s2 = useChatThreads(storage)
    expect(s2.threads.value).toHaveLength(1)
    expect(s2.threads.value[0].id).toBe(id)
    expect(s2.activeSnapshot().messages).toEqual([])
    expect(data.has(LEGACY_MESSAGES_KEY)).toBe(true)
  })
})

describe('deriveTitle', () => {
  it('uses the first user message, collapsing whitespace', () => {
    expect(deriveTitle([assistantMsg('hi'), userMsg('  hello\n  world  ')])).toBe('hello world')
  })
  it('truncates long titles with an ellipsis', () => {
    expect(deriveTitle([userMsg('x'.repeat(40))])).toBe('x'.repeat(30) + '…')
  })
  it('returns an empty title without a user message', () => {
    expect(deriveTitle([assistantMsg('hi')])).toBe('')
    expect(deriveTitle([])).toBe('')
  })
})

describe('useChatThreads titles + ordering', () => {
  it('derives the title once on save and never auto-renames', () => {
    const { storage } = mockStorage()
    const store = useChatThreads(storage)
    const id = store.activeThreadId.value
    store.saveSnapshot(id, { messages: [userMsg('first title')], tray: [] })
    expect(store.threads.value[0].title).toBe('first title')
    store.saveSnapshot(id, { messages: [userMsg('a different first message')], tray: [] })
    expect(store.threads.value[0].title).toBe('first title')
  })

  it('sorts threads by most recently updated', () => {
    vi.useFakeTimers()
    const { storage } = mockStorage()
    vi.setSystemTime(1000)
    const store = useChatThreads(storage)
    const a = store.activeThreadId.value
    vi.setSystemTime(2000)
    const b = store.createThread()
    expect(store.threads.value.map(t => t.id)).toEqual([b, a])
    // activity on the older thread moves it back to the top
    vi.setSystemTime(3000)
    store.saveSnapshot(a, { messages: [userMsg('revive a')], tray: [] })
    expect(store.threads.value.map(t => t.id)).toEqual([a, b])
    expect(store.activeThreadId.value).toBe(b) // saving does not switch threads
  })

  it('generates unique thread ids', () => {
    const { storage } = mockStorage()
    const store = useChatThreads(storage)
    const ids = new Set([store.activeThreadId.value, store.createThread(), store.createThread()])
    expect(ids.size).toBe(3)
  })
})

describe('useChatThreads switch + delete', () => {
  it('keeps each thread’s snapshot isolated across switches', () => {
    const { storage, data } = mockStorage()
    const store = useChatThreads(storage)
    const a = store.activeThreadId.value
    store.saveSnapshot(a, {
      messages: [userMsg('chat A')], tray: [{ uid: 'p1' } as any], plan: [{ title: 'step', status: 'done' }],
    })
    const b = store.createThread() // becomes active, starts empty
    expect(store.activeThreadId.value).toBe(b)
    expect(store.activeSnapshot()).toEqual(EMPTY)
    store.saveSnapshot(b, { messages: [userMsg('chat B')], tray: [] })
    store.setActive(a)
    expect(store.activeSnapshot().messages).toEqual([userMsg('chat A')])
    expect(store.activeSnapshot().tray).toEqual([{ uid: 'p1' }])
    expect(store.activeSnapshot().plan).toEqual([{ title: 'step', status: 'done' }])
    store.setActive(b)
    expect(store.activeSnapshot().messages).toEqual([userMsg('chat B')])
    // per-thread snapshot keys exist side by side
    expect(data.has(threadKey(a))).toBe(true)
    expect(data.has(threadKey(b))).toBe(true)
  })

  it('ignores setActive with an unknown id', () => {
    const { storage } = mockStorage()
    const store = useChatThreads(storage)
    const id = store.activeThreadId.value
    store.setActive('nope')
    expect(store.activeThreadId.value).toBe(id)
  })

  it('deletes a non-active thread without touching the active one', () => {
    const { storage, data } = mockStorage()
    const store = useChatThreads(storage)
    const a = store.activeThreadId.value
    store.saveSnapshot(a, { messages: [userMsg('keep me')], tray: [] })
    const b = store.createThread()
    store.setActive(a)
    store.deleteThread(b)
    expect(store.threads.value.map(t => t.id)).toEqual([a])
    expect(store.activeThreadId.value).toBe(a)
    expect(store.activeSnapshot().messages).toEqual([userMsg('keep me')])
    expect(data.has(threadKey(b))).toBe(false)
  })

  it('deleting the active thread falls back to the most recent remaining one', () => {
    vi.useFakeTimers()
    const { storage } = mockStorage()
    vi.setSystemTime(1000)
    const store = useChatThreads(storage)
    const a = store.activeThreadId.value
    vi.setSystemTime(2000)
    const b = store.createThread()
    vi.setSystemTime(3000)
    const c = store.createThread() // index order: c, b, a
    store.setActive(b)
    store.deleteThread(b)
    expect(store.threads.value.map(t => t.id)).toEqual([c, a])
    expect(store.activeThreadId.value).toBe(c)
  })

  it('deleting the last thread bootstraps a fresh empty one', () => {
    const { storage, data } = mockStorage()
    const store = useChatThreads(storage)
    const only = store.activeThreadId.value
    store.saveSnapshot(only, { messages: [userMsg('bye')], tray: [] })
    store.deleteThread(only)
    expect(store.threads.value).toHaveLength(1)
    const fresh = store.threads.value[0]
    expect(fresh.id).not.toBe(only)
    expect(fresh.title).toBe('')
    expect(store.activeThreadId.value).toBe(fresh.id)
    expect(store.activeSnapshot()).toEqual(EMPTY)
    expect(data.has(threadKey(only))).toBe(false)
  })
})
