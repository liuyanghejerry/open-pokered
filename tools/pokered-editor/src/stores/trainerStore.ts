import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { TrainerClassFile, TrainerParty, TrainerMon } from '../types/trainer'
import { dataFetch } from '../composables/dataAdapter'
import { injectTrainer } from '../composables/usePokeredRunner'

export const useTrainerStore = defineStore('trainer', () => {
  const classNames = ref<string[]>([])
  const activeClass = ref<string | null>(null)
  const activePartyIndex = ref<number>(0)
  const data = ref<TrainerClassFile | null>(null)
  const loading = ref(false)
  const dirty = ref(false)
  const error = ref<string | null>(null)

  async function loadClassList() {
    try {
      const res = await dataFetch('/api/trainers')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      classNames.value = await res.json()
    } catch (e) {
      error.value = `Failed to load trainer list: ${(e as Error).message}`
    }
  }

  async function loadClass(name: string) {
    if (dirty.value && !confirm('Discard unsaved trainer changes?')) return
    loading.value = true
    error.value = null
    try {
      const res = await dataFetch(`/api/trainers/${encodeURIComponent(name)}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      data.value = await res.json()
      activeClass.value = name
      activePartyIndex.value = 0
      dirty.value = false
    } catch (e) {
      error.value = `Failed to load ${name}: ${(e as Error).message}`
      data.value = null
    } finally {
      loading.value = false
    }
  }

  async function save() {
    if (!data.value || !activeClass.value) return
    try {
      const res = await dataFetch(`/api/trainers/${encodeURIComponent(activeClass.value)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data.value),
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      dirty.value = false
      // WYSIWYG: push the saved trainer parties into the running game.
      try {
        await injectTrainer(activeClass.value, JSON.stringify(data.value))
      } catch {
        /* preview injection is best-effort */
      }
    } catch (e) {
      error.value = `Failed to save: ${(e as Error).message}`
    }
  }

  function selectParty(index: number) {
    if (data.value && index >= 0 && index < data.value.parties.length) {
      activePartyIndex.value = index
    }
  }

  function activeParty(): TrainerParty | null {
    if (!data.value) return null
    return data.value.parties[activePartyIndex.value] ?? null
  }

  function updateMon(monIdx: number, patch: Partial<TrainerMon>) {
    const party = activeParty()
    if (!party) return
    const mon = party.pokemon[monIdx]
    if (!mon) return
    party.pokemon[monIdx] = { ...mon, ...patch }
    dirty.value = true
  }

  function addMon() {
    const party = activeParty()
    if (!party || party.pokemon.length >= 6) return
    party.pokemon.push({ level: 5, species: 'Bulbasaur' })
    dirty.value = true
  }

  function removeMon(monIdx: number) {
    const party = activeParty()
    if (!party) return
    party.pokemon.splice(monIdx, 1)
    dirty.value = true
  }

  function addParty() {
    if (!data.value) return
    data.value.parties.push({ pokemon: [{ level: 5, species: 'Bulbasaur' }] })
    activePartyIndex.value = data.value.parties.length - 1
    dirty.value = true
  }

  function removeParty(index: number) {
    if (!data.value || data.value.parties.length <= 1) return
    data.value.parties.splice(index, 1)
    if (activePartyIndex.value >= data.value.parties.length) {
      activePartyIndex.value = data.value.parties.length - 1
    }
    dirty.value = true
  }

  const partyCount = computed(() => data.value?.parties.length ?? 0)

  return {
    classNames,
    activeClass,
    activePartyIndex,
    data,
    loading,
    dirty,
    error,
    partyCount,
    loadClassList,
    loadClass,
    save,
    selectParty,
    updateMon,
    addMon,
    removeMon,
    addParty,
    removeParty,
  }
})
