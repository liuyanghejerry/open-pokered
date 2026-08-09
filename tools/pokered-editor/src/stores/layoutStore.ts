import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ScreenLayout, LayoutElement, TileRect } from '../types/ui-layout'
import { useWasmPreview } from '../composables/useWasmPreview'
import { dataFetch } from '../composables/dataAdapter'

const MAX_UNDO_DEPTH = 100

export const useLayoutStore = defineStore('layout', () => {
  const layoutNames = ref<string[]>([])
  const activeName = ref<string | null>(null)
  const mode = ref<'json' | 'gui'>('json')
  const rawJson = ref<string>('')
  const rawGui = ref<string>('')
  const parsedJson = ref<ScreenLayout | null>(null)
  const dirty = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const parseError = ref<string | null>(null)
  const saveError = ref<string | null>(null)

  // Reuse the WASM preview composable singleton for .gui compilation
  const wasm = useWasmPreview()

  // ── Undo/Redo state ─────────────────────────────────────────────────
  const undoStack = ref<string[]>([])
  const redoStack = ref<string[]>([])
  const savedSnapshot = ref<string | null>(null)

  // Debounce mechanism: capture pre-edit snapshot, push on next idle (500ms)
  let pendingHistoryPush: string | null = null
  let idleTimer: ReturnType<typeof setTimeout> | null = null

  function pushHistory(snapshot: string) {
    // De-dup: don't push if identical to current top
    if (undoStack.value.length > 0 && snapshot === undoStack.value[undoStack.value.length - 1]) return
    undoStack.value.push(snapshot)
    if (undoStack.value.length > MAX_UNDO_DEPTH) {
      undoStack.value.shift()
    }
    // New edit branch: clear redo
    redoStack.value = []
  }

  function scheduleHistoryPush(snapshot: string) {
    if (pendingHistoryPush === null) pendingHistoryPush = snapshot
    if (idleTimer) clearTimeout(idleTimer)
    idleTimer = setTimeout(() => {
      if (pendingHistoryPush !== null) {
        pushHistory(pendingHistoryPush)
        pendingHistoryPush = null
      }
      idleTimer = null
    }, 500)
  }

  function flushPendingHistory() {
    if (idleTimer) { clearTimeout(idleTimer); idleTimer = null }
    if (pendingHistoryPush !== null) {
      pushHistory(pendingHistoryPush)
      pendingHistoryPush = null
    }
  }

  const canUndo = computed(() => undoStack.value.length > 0)
  const canRedo = computed(() => redoStack.value.length > 0)

  async function undo(): Promise<boolean> {
    flushPendingHistory()
    if (undoStack.value.length === 0) return false
    const snapshot = undoStack.value.pop()!
    if (mode.value === 'gui') {
      redoStack.value.push(rawGui.value)
      rawGui.value = snapshot
      await applyGuiCompile(snapshot)
    } else {
      redoStack.value.push(rawJson.value)
      rawJson.value = snapshot
      try {
        parsedJson.value = JSON.parse(snapshot)
        parseError.value = null
      } catch (e) {
        parsedJson.value = null
        parseError.value = `Invalid JSON: ${(e as Error).message}`
      }
    }
    const current = mode.value === 'gui' ? rawGui.value : rawJson.value
    dirty.value = current !== savedSnapshot.value
    return true
  }

  async function redo(): Promise<boolean> {
    flushPendingHistory()
    if (redoStack.value.length === 0) return false
    const snapshot = redoStack.value.pop()!
    if (mode.value === 'gui') {
      undoStack.value.push(rawGui.value)
      rawGui.value = snapshot
      await applyGuiCompile(snapshot)
    } else {
      undoStack.value.push(rawJson.value)
      rawJson.value = snapshot
      try {
        parsedJson.value = JSON.parse(snapshot)
        parseError.value = null
      } catch (e) {
        parsedJson.value = null
        parseError.value = `Invalid JSON: ${(e as Error).message}`
      }
    }
    const current = mode.value === 'gui' ? rawGui.value : rawJson.value
    dirty.value = current !== savedSnapshot.value
    return true
  }

  /** Compile a .gui source string via WASM and apply the result.
   *  Silently keeps the previous compiled state on failure so the
   *  preview doesn't disappear during mid-edit or navigation. */
  async function applyGuiCompile(source: string) {
    const result = await wasm.compileScreen(source)
    if (result.ok) {
      rawJson.value = result.output
      parsedJson.value = JSON.parse(result.output) as ScreenLayout
      parseError.value = null
    }
    // on failure: keep previous rawJson/parsedJson
  }

  async function loadList() {
    try {
      const res = await dataFetch('/api/ui-layouts')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      layoutNames.value = await res.json()
    } catch (e) {
      error.value = `Failed to load layout list: ${(e as Error).message}`
    }
  }

  async function loadLayout(name: string) {
    if (dirty.value && !confirm('Discard unsaved layout changes?')) return
    flushPendingHistory()
    loading.value = true
    error.value = null
    parseError.value = null
    try {
      const res = await dataFetch(`/api/ui-layouts/${encodeURIComponent(name)}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const contentType = res.headers.get('content-type') || ''

      if (contentType.includes('text/plain')) {
        // .gui file — load source for editing, and compile to JSON for preview
        mode.value = 'gui'
        const guiSource = await res.text()
        rawGui.value = guiSource
        const result = await wasm.compileScreen(guiSource)
        if (result.ok) {
          rawJson.value = result.output
          parsedJson.value = JSON.parse(result.output) as ScreenLayout
          parseError.value = null
        } else {
          // Compilation failed — keep rawJson empty so the WASM preview
          // falls back to static layout (still functional), but surface
          // the parse error so the user knows the DSL has issues.
          rawJson.value = ''
          parsedJson.value = null
          parseError.value = `GUI compile error: ${result.error}`
        }
      } else {
        // .json file
        mode.value = 'json'
        const json = await res.json()
        const text = JSON.stringify(json, null, 2)
        rawJson.value = text
        parsedJson.value = json
        rawGui.value = ''
      }
      activeName.value = name
      dirty.value = false
      parseError.value = null
      undoStack.value = []
      redoStack.value = []
      savedSnapshot.value = mode.value === 'gui' ? rawGui.value : rawJson.value
    } catch (e) {
      error.value = `Failed to load ${name}: ${(e as Error).message}`
      rawJson.value = ''
      rawGui.value = ''
      parsedJson.value = null
      activeName.value = null
    } finally {
      loading.value = false
    }
  }

  function setRawJson(text: string) {
    // No-op if content unchanged (e.g. CodeMirror echoing setContent)
    if (text === rawJson.value) return
    scheduleHistoryPush(rawJson.value)
    rawJson.value = text
    try {
      parsedJson.value = JSON.parse(text)
      parseError.value = null
    } catch (e) {
      parsedJson.value = null
      parseError.value = `Invalid JSON: ${(e as Error).message}`
    }
    // dirty = differs from saved snapshot (handles round-trip through CodeMirror)
    dirty.value = text !== savedSnapshot.value
  }

  async function setRawGui(text: string) {
    if (text === rawGui.value) return
    scheduleHistoryPush(rawGui.value)
    rawGui.value = text
    dirty.value = text !== savedSnapshot.value
    // Recompile .gui source via WASM for the preview renderer and overlay
    const result = await wasm.compileScreen(text)
    // Guard against stale results: if rawGui changed since we started
    // (faster typing), discard this result.
    if (rawGui.value !== text) return
    if (result.ok) {
      rawJson.value = result.output
      parsedJson.value = JSON.parse(result.output) as ScreenLayout
      parseError.value = null
    }
    // on failure: keep previous rawJson/parsedJson so the preview
    // doesn't disappear on intermediate edits
  }

  async function save() {
    if (!activeName.value) return
    if (mode.value === 'json' && parseError.value) return
    flushPendingHistory()
    saveError.value = null
    try {
      if (mode.value === 'gui') {
        const res = await dataFetch(`/api/ui-layouts/${encodeURIComponent(activeName.value)}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'text/plain' },
          body: rawGui.value,
        })
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        savedSnapshot.value = rawGui.value
      } else {
        const body = JSON.stringify(parsedJson.value, null, 2) + '\n'
        const res = await dataFetch(`/api/ui-layouts/${encodeURIComponent(activeName.value)}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body,
        })
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        savedSnapshot.value = rawJson.value
      }
      dirty.value = false
    } catch (e) {
      saveError.value = `Save failed: ${(e as Error).message}`
    }
  }

  // ── v2 element accessors ────────────────────────────────────────────

  /** Top-level elements of the active layout (schema_version 2). */
  const activeElements = computed<LayoutElement[] | null>(() => {
    return parsedJson.value?.elements ?? null
  })

  /**
   * Patch the `rect` of the element reached by `path` (e.g. `['elements', 2]`
   * or `['elements', 2, 'children', 0]` for a nested group child) and commit
   * the change through {@link setRawJson} so dirty/parse/undo state stays in
   * sync. No-op if the path or its `rect` cannot be resolved.
   */
  function updateRectAtPath(path: (string | number)[], patch: Partial<TileRect>) {
    const text = rawJson.value
    if (!text) return
    let obj: unknown
    try {
      obj = JSON.parse(text)
    } catch {
      return // parse errors surfaced via setRawJson on the next edit
    }
    let current: unknown = obj
    for (const key of path) {
      if (current == null || typeof current !== 'object') return
      current = (current as Record<string, unknown>)[key]
    }
    if (current == null || typeof current !== 'object') return
    const rect = (current as Record<string, unknown>).rect as Record<string, unknown> | undefined
    if (!rect) return
    if (patch.tx !== undefined) rect.tx = patch.tx
    if (patch.ty !== undefined) rect.ty = patch.ty
    if (patch.tw !== undefined) rect.tw = patch.tw
    if (patch.th !== undefined) rect.th = patch.th
    setRawJson(JSON.stringify(obj, null, 2))
  }

  /** Patch the rect of the top-level element at `index`. */
  function updateElementRect(index: number, patch: Partial<TileRect>) {
    updateRectAtPath(['elements', index], patch)
  }

  return {
    layoutNames,
    activeName,
    mode,
    rawJson,
    rawGui,
    parsedJson,
    dirty,
    loading,
    error,
    parseError,
    saveError,
    loadList,
    loadLayout,
    setRawJson,
    setRawGui,
    save,
    activeElements,
    updateRectAtPath,
    updateElementRect,
    undoStack,
    redoStack,
    savedSnapshot,
    canUndo,
    canRedo,
    undo,
    redo,
  }
})
