import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Evolution, LevelUpMove, PokedexInfo, PokemonFile } from '../types/pokemon'
import { SPECIES_LIST } from '../types/constants'
import { dataFetch } from '../composables/dataAdapter'
import { injectBaseStats } from '../composables/usePokeredRunner'

export const usePokemonStore = defineStore('pokemon', () => {
  const speciesNames = ref<string[]>([])
  const activeSpecies = ref<string | null>(null)
  const data = ref<PokemonFile | null>(null)
  const loading = ref(false)
  const dirty = ref(false)
  const error = ref<string | null>(null)

  async function loadSpeciesList() {
    try {
      const res = await dataFetch('/api/pokemon')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const fromApi: string[] = await res.json()
      const apiSet = new Set(fromApi)
      const dexOrdered = SPECIES_LIST.slice(1).filter(s => apiSet.has(s))
      const extras = fromApi
        .filter(s => !SPECIES_LIST.includes(s))
        .sort((a, b) => a.localeCompare(b))
      speciesNames.value = [...dexOrdered, ...extras]
    } catch (e) {
      error.value = `Failed to load pokemon list: ${(e as Error).message}`
    }
  }

  async function loadSpecies(name: string) {
    if (dirty.value && !confirm('Discard unsaved pokemon changes?')) return
    loading.value = true
    error.value = null
    try {
      const res = await dataFetch(`/api/pokemon/${encodeURIComponent(name)}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      data.value = await res.json()
      activeSpecies.value = name
      dirty.value = false
    } catch (e) {
      error.value = `Failed to load ${name}: ${(e as Error).message}`
      data.value = null
    } finally {
      loading.value = false
    }
  }

  /**
   * Create a new species: the server writes a template JSON to
   * `pokemon/<name>.json` (it becomes a `Species` enum variant on the next
   * `cargo build`). Returns false and sets `error` on failure.
   */
  async function createSpecies(name: string): Promise<boolean> {
    try {
      const res = await dataFetch('/api/pokemon', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      })
      if (!res.ok) {
        const msg = await res.json().catch(() => null)
        error.value = `Failed to create ${name}: ${msg?.error ?? `HTTP ${res.status}`}`
        return false
      }
      dirty.value = false // the confirm in loadSpecies would block the fresh load otherwise
      await loadSpeciesList()
      await loadSpecies(name)
      return true
    } catch (e) {
      error.value = `Failed to create ${name}: ${(e as Error).message}`
      return false
    }
  }

  async function save() {
    if (!data.value || !activeSpecies.value) return
    try {
      const res = await dataFetch(`/api/pokemon/${encodeURIComponent(activeSpecies.value)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data.value),
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      dirty.value = false
      // WYSIWYG: push the saved base stats into the running game.
      try {
        await injectBaseStats(activeSpecies.value, JSON.stringify(data.value))
      } catch {
        /* preview injection is best-effort */
      }
    } catch (e) {
      error.value = `Failed to save: ${(e as Error).message}`
    }
  }

  function updateField<K extends keyof PokemonFile>(field: K, value: PokemonFile[K]) {
    if (!data.value) return
    data.value[field] = value
    dirty.value = true
  }

  function updateBaseStat(stat: keyof PokemonFile['baseStats'], value: number) {
    if (!data.value) return
    const clamped = Math.max(1, Math.min(255, Math.floor(value)))
    data.value.baseStats = { ...data.value.baseStats, [stat]: clamped }
    dirty.value = true
  }

  function updateInitialMove(idx: number, moveName: string) {
    if (!data.value || idx < 0 || idx > 3) return
    const moves = [...data.value.initialMoves] as [string, string, string, string]
    moves[idx] = moveName
    data.value.initialMoves = moves
    dirty.value = true
  }

  function updateTmHmFlag(byteIdx: number, bitIdx: number, set: boolean) {
    if (!data.value || byteIdx < 0 || byteIdx > 6 || bitIdx < 0 || bitIdx > 7) return
    const flags = [...data.value.tmHmFlags]
    const mask = 1 << bitIdx
    if (set) {
      flags[byteIdx] = (flags[byteIdx] | mask) & 0xff
    } else {
      flags[byteIdx] = flags[byteIdx] & ~mask & 0xff
    }
    data.value.tmHmFlags = flags
    dirty.value = true
  }

  function addEvolution() {
    if (!data.value) return
    data.value.evolutions = [
      ...data.value.evolutions,
      { method: 'level', species: 'Bulbasaur', level: 16 },
    ]
    dirty.value = true
  }

  function updateEvolution(idx: number, patch: Partial<Evolution>) {
    if (!data.value) return
    const evs = [...data.value.evolutions]
    if (!evs[idx]) return
    evs[idx] = { ...evs[idx], ...patch }
    data.value.evolutions = evs
    dirty.value = true
  }

  function changeEvolutionMethod(idx: number, method: Evolution['method']) {
    if (!data.value) return
    const evs = [...data.value.evolutions]
    if (!evs[idx]) return
    const target = evs[idx].species
    if (method === 'level') {
      evs[idx] = { method: 'level', species: target, level: 16 }
    } else if (method === 'item') {
      evs[idx] = { method: 'item', species: target, item: 'FireStone', minLevel: 1 }
    } else {
      evs[idx] = { method: 'trade', species: target, minLevel: 1 }
    }
    data.value.evolutions = evs
    dirty.value = true
  }

  function removeEvolution(idx: number) {
    if (!data.value) return
    data.value.evolutions = data.value.evolutions.filter((_, i) => i !== idx)
    dirty.value = true
  }

  function addLearnsetEntry() {
    if (!data.value) return
    data.value.learnset = [...data.value.learnset, { level: 1, moveId: 'Tackle' }]
    dirty.value = true
  }

  function updateLearnsetEntry(idx: number, patch: Partial<LevelUpMove>) {
    if (!data.value) return
    const list = [...data.value.learnset]
    if (!list[idx]) return
    list[idx] = { ...list[idx], ...patch }
    data.value.learnset = list
    dirty.value = true
  }

  function removeLearnsetEntry(idx: number) {
    if (!data.value) return
    data.value.learnset = data.value.learnset.filter((_, i) => i !== idx)
    dirty.value = true
  }

  function sortLearnsetByLevel() {
    if (!data.value) return
    data.value.learnset = [...data.value.learnset].sort((a, b) => a.level - b.level)
    dirty.value = true
  }

  function updatePokedex<K extends keyof PokedexInfo>(key: K, value: PokedexInfo[K]) {
    if (!data.value) return
    data.value.pokedex = { ...data.value.pokedex, [key]: value }
    dirty.value = true
  }

  function updateFlavorPage(idx: number, text: string) {
    if (!data.value) return
    const pages = [...data.value.pokedex.flavorTextPages]
    if (idx < 0 || idx >= pages.length) return
    pages[idx] = text
    data.value.pokedex = { ...data.value.pokedex, flavorTextPages: pages }
    dirty.value = true
  }

  function addFlavorPage() {
    if (!data.value) return
    if (data.value.pokedex.flavorTextPages.length >= 4) return
    data.value.pokedex = {
      ...data.value.pokedex,
      flavorTextPages: [...data.value.pokedex.flavorTextPages, ''],
    }
    dirty.value = true
  }

  function removeFlavorPage(idx: number) {
    if (!data.value) return
    data.value.pokedex = {
      ...data.value.pokedex,
      flavorTextPages: data.value.pokedex.flavorTextPages.filter((_, i) => i !== idx),
    }
    dirty.value = true
  }

  const baseStatTotal = computed(() => {
    if (!data.value) return 0
    const s = data.value.baseStats
    return s.hp + s.attack + s.defense + s.speed + s.special
  })

  return {
    speciesNames,
    activeSpecies,
    data,
    loading,
    dirty,
    error,
    baseStatTotal,
    loadSpeciesList,
    loadSpecies,
    createSpecies,
    save,
    updateField,
    updateBaseStat,
    updateInitialMove,
    updateTmHmFlag,
    addEvolution,
    updateEvolution,
    changeEvolutionMethod,
    removeEvolution,
    addLearnsetEntry,
    updateLearnsetEntry,
    removeLearnsetEntry,
    sortLearnsetByLevel,
    updatePokedex,
    updateFlavorPage,
    addFlavorPage,
    removeFlavorPage,
  }
})
