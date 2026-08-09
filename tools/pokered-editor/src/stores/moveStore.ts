import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { MoveFile } from '../types/pokemon'
import { dataFetch } from '../composables/dataAdapter'
import { injectMove } from '../composables/usePokeredRunner'

export const useMoveStore = defineStore('move', () => {
  const moveNames = ref<string[]>([])
  const activeMove = ref<string | null>(null)
  const data = ref<MoveFile | null>(null)
  const loading = ref(false)
  const dirty = ref(false)
  const error = ref<string | null>(null)

  async function loadMoveList() {
    try {
      const res = await dataFetch('/api/moves')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      moveNames.value = await res.json()
    } catch (e) {
      error.value = `Failed to load move list: ${(e as Error).message}`
    }
  }

  async function loadMove(name: string) {
    if (dirty.value && !confirm('Discard unsaved move changes?')) return
    loading.value = true
    error.value = null
    try {
      const res = await dataFetch(`/api/moves/${encodeURIComponent(name)}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      data.value = await res.json()
      activeMove.value = name
      dirty.value = false
    } catch (e) {
      error.value = `Failed to load ${name}: ${(e as Error).message}`
      data.value = null
    } finally {
      loading.value = false
    }
  }

  /**
   * Create a new move: the server writes a template JSON to
   * `moves/<name>.json` (it becomes a `MoveId` enum variant on the next
   * `cargo build`). Returns false and sets `error` on failure.
   */
  async function createMove(name: string): Promise<boolean> {
    try {
      const res = await dataFetch('/api/moves', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      })
      if (!res.ok) {
        const msg = await res.json().catch(() => null)
        error.value = `Failed to create ${name}: ${msg?.error ?? `HTTP ${res.status}`}`
        return false
      }
      dirty.value = false // the confirm in loadMove would block the fresh load otherwise
      await loadMoveList()
      await loadMove(name)
      return true
    } catch (e) {
      error.value = `Failed to create ${name}: ${(e as Error).message}`
      return false
    }
  }

  async function save() {
    if (!data.value || !activeMove.value) return
    try {
      const res = await dataFetch(`/api/moves/${encodeURIComponent(activeMove.value)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data.value),
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      dirty.value = false
      // WYSIWYG: push the saved move into the running game.
      try {
        await injectMove(activeMove.value, JSON.stringify(data.value))
      } catch {
        /* preview injection is best-effort */
      }
    } catch (e) {
      error.value = `Failed to save: ${(e as Error).message}`
    }
  }

  function updateField<K extends keyof MoveFile>(field: K, value: MoveFile[K]) {
    if (!data.value) return
    data.value[field] = value
    dirty.value = true
  }

  return {
    moveNames,
    activeMove,
    data,
    loading,
    dirty,
    error,
    loadMoveList,
    loadMove,
    createMove,
    save,
    updateField,
  }
})
