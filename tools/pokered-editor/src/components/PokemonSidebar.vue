<script setup lang="ts">
import { onMounted, computed, ref, nextTick } from 'vue'
import { storeToRefs } from 'pinia'
import { usePokemonStore } from '../stores/pokemonStore'
import { SPECIES_LIST } from '../types/constants'

const store = usePokemonStore()
const { speciesNames, activeSpecies, dirty, loading, error } = storeToRefs(store)

defineProps<{
  filter?: string
}>()

const emit = defineEmits<{
  select: [name: string]
}>()

onMounted(() => {
  if (speciesNames.value.length === 0) store.loadSpeciesList()
})

const filterText = defineModel<string>('filter', { default: '' })

// ── "New species" inline create row ────────────────────────────────────────
const creating = ref(false)
const creatingBusy = ref(false)
const newName = ref('')
const nameInput = ref<HTMLInputElement | null>(null)

function startCreate() {
  creating.value = true
  newName.value = ''
  error.value = null
  nextTick(() => nameInput.value?.focus())
}

function cancelCreate() {
  creating.value = false
  newName.value = ''
}

async function confirmCreate() {
  const name = newName.value.trim()
  if (!name || creatingBusy.value) return
  creatingBusy.value = true
  const ok = await store.createSpecies(name)
  creatingBusy.value = false
  if (ok) {
    creating.value = false
    newName.value = ''
    emit('select', name)
  }
}

function dexNumber(species: string): string {
  const idx = SPECIES_LIST.indexOf(species)
  return idx > 0 ? String(idx).padStart(3, '0') : '???'
}

const filteredSpecies = computed(() => {
  const q = (filterText.value ?? '').trim().toLowerCase()
  if (!q) return speciesNames.value
  return speciesNames.value.filter(n => {
    return n.toLowerCase().includes(q) || dexNumber(n).includes(q)
  })
})

function handleSelect(name: string) {
  emit('select', name)
}
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-3">Pokemon Editor</h2>

    <button
      v-if="!creating"
      class="w-full mb-3 px-2 py-1.5 rounded text-[11px] font-bold cursor-pointer bg-accent/10 text-accent border border-accent/30 hover:bg-accent/20 transition-colors"
      @click="startCreate"
    >
      ＋ New Pokemon
    </button>
    <div v-else class="mb-3">
      <div class="flex gap-1.5">
        <input
          ref="nameInput"
          v-model="newName"
          type="text"
          placeholder="PascalCase, e.g. Pikachu2"
          class="flex-1 min-w-0 px-2 py-1 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-[11px]"
          @keyup.enter="confirmCreate"
          @keyup.esc="cancelCreate"
        />
        <button
          class="px-2 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="creatingBusy || !newName.trim()"
          @click="confirmCreate"
        >
          Create
        </button>
        <button
          class="px-2 py-1 rounded text-[11px] cursor-pointer bg-transparent text-text-muted border border-[rgba(255,255,255,0.1)] hover:text-text"
          @click="cancelCreate"
        >
          ✕
        </button>
      </div>
      <div v-if="error" class="mt-1.5 text-[10px] text-danger">{{ error }}</div>
    </div>

    <input
      v-model="filterText"
      type="text"
      placeholder="Filter by name or #dex…"
      class="w-full p-1.5 mb-3 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-xs"
    />

    <div v-if="loading" class="text-[11px] text-text-muted mb-2">Loading...</div>

    <div class="text-[10px] text-text-muted mb-1">
      {{ filteredSpecies.length }} / {{ speciesNames.length }} · National Dex order
    </div>

    <nav class="space-y-0.5">
      <button
        v-for="name in filteredSpecies"
        :key="name"
        class="pokemon-nav-btn"
        :class="{ active: activeSpecies === name }"
        @click="handleSelect(name)"
      >
        <span class="pokemon-nav-label">
          <span class="pokemon-nav-dex">#{{ dexNumber(name) }}</span>
          {{ name }}
        </span>
        <span v-if="activeSpecies === name && dirty" class="pokemon-dirty">●</span>
      </button>
    </nav>

    <div v-if="filteredSpecies.length === 0 && !loading" class="text-[11px] text-text-muted mt-2">
      No matching species.
    </div>
  </div>
</template>

<style scoped>
.pokemon-nav-btn {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 500;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  border-radius: 4px;
  text-align: left;
  transition: background 0.15s, color 0.15s;
}

.pokemon-nav-btn:hover {
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
}

.pokemon-nav-btn.active {
  background: rgba(78, 204, 163, 0.08);
  color: var(--color-accent);
}

.pokemon-nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pokemon-nav-dex {
  font-family: monospace;
  font-size: 10px;
  color: var(--color-text-muted);
  margin-right: 4px;
}

.pokemon-nav-btn.active .pokemon-nav-dex {
  color: var(--color-accent);
  opacity: 0.7;
}

.pokemon-dirty {
  color: var(--color-warning);
  font-size: 10px;
}
</style>
