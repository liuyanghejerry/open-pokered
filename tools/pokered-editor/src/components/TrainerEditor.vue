<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useTrainerStore } from '../stores/trainerStore'
import { usePokemonStore } from '../stores/pokemonStore'
import { injectTrainer } from '../composables/usePokeredRunner'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import { SPECIES_LIST } from '../types/constants'

const store = useTrainerStore()
const playtestOverlay = usePlaytestOverlay()
const { data, activeClass, activePartyIndex, dirty, error, loading } = storeToRefs(store)

// Species options come from the live API list so species added through the
// editor show up here immediately; SPECIES_LIST is the loading fallback.
const pokemonStore = usePokemonStore()
const speciesOptions = computed(() =>
  pokemonStore.speciesNames.length > 0 ? pokemonStore.speciesNames : SPECIES_LIST,
)

onMounted(() => {
  if (pokemonStore.speciesNames.length === 0) pokemonStore.loadSpeciesList()
})

const activeParty = computed(() => {
  if (!data.value) return null
  return data.value.parties[activePartyIndex.value] ?? null
})

/**
 * Open the floating playtest in a trainer battle vs this class' current party
 * (party tab index). The (possibly unsaved) party data is injected into the
 * running game first, so edits show up without saving.
 */
async function testBattle() {
  if (!data.value || !activeClass.value) return
  try {
    await injectTrainer(activeClass.value, JSON.stringify(data.value))
  } catch {
    /* preview injection is best-effort */
  }
  playtestOverlay.launch({
    kind: 'trainerBattle',
    class: activeClass.value,
    partyIndex: activePartyIndex.value,
  })
}

function setLevel(idx: number, raw: string) {
  const v = parseInt(raw, 10)
  if (!Number.isFinite(v)) return
  store.updateMon(idx, { level: Math.max(1, Math.min(100, v)) })
}

function setSpecies(idx: number, raw: string) {
  store.updateMon(idx, { species: raw })
}
</script>

<template>
  <div class="flex-1 flex flex-col min-h-0">
    <!-- Header -->
    <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
      <div class="flex items-baseline gap-3">
        <h2 class="text-accent text-sm font-bold">Trainer Editor</h2>
        <span v-if="data" class="text-text text-[12px] font-mono">
          {{ data.class }}
          <span class="text-text-muted">({{ data.constName }})</span>
        </span>
      </div>
      <div class="flex gap-2">
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!data || !activeParty"
          title="Open the floating playtest in a trainer battle vs this party (no save impact)"
          @click="testBattle"
        >
          ⚔ 试玩对战
        </button>
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!dirty"
          @click="store.save()"
        >
          💾 Save
        </button>
      </div>
    </div>

    <!-- Body -->
    <div class="flex-1 overflow-y-auto p-3 min-h-0">
      <div v-if="error" class="mb-3 p-2 rounded bg-danger/10 border border-danger text-danger text-[11px]">
        {{ error }}
      </div>

      <div v-if="!activeClass" class="text-text-muted text-xs">
        Select a trainer class from the sidebar to edit its parties.
      </div>

      <div v-else-if="loading" class="text-text-muted text-xs">Loading...</div>

      <div v-else-if="data">
        <!-- Party tab strip -->
        <div class="flex flex-wrap items-center gap-1 mb-3">
          <button
            v-for="(_, i) in data.parties"
            :key="i"
            class="px-2 py-0.5 rounded text-[11px] cursor-pointer border"
            :class="i === activePartyIndex
              ? 'bg-accent text-bg border-accent'
              : 'bg-bg-inset text-text-muted border-[rgba(255,255,255,0.1)] hover:text-text'"
            @click="store.selectParty(i)"
          >
            #{{ i + 1 }}
          </button>
          <button
            class="px-2 py-0.5 rounded text-[11px] cursor-pointer bg-bg-inset text-text-muted border border-[rgba(255,255,255,0.1)] hover:text-accent"
            @click="store.addParty()"
          >
            + Party
          </button>
          <button
            v-if="data.parties.length > 1"
            class="px-2 py-0.5 rounded text-[11px] cursor-pointer bg-bg-inset text-text-muted border border-[rgba(255,255,255,0.1)] hover:text-danger ml-auto"
            @click="store.removeParty(activePartyIndex)"
          >
            ✕ Remove Party #{{ activePartyIndex + 1 }}
          </button>
        </div>

        <!-- Party rows -->
        <div v-if="activeParty">
          <div
            v-for="(mon, idx) in activeParty.pokemon"
            :key="idx"
            class="bg-bg p-2 rounded mb-2 border border-[rgba(255,255,255,0.06)]"
          >
            <div class="flex items-center justify-between mb-1">
              <span class="text-text text-[11px] font-bold">Slot #{{ idx + 1 }}</span>
              <button
                class="text-[10px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none"
                @click="store.removeMon(idx)"
              >
                ✕ Remove
              </button>
            </div>

            <div class="grid grid-cols-2 gap-2">
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Level</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="1"
                  max="100"
                  :value="mon.level"
                  @change="setLevel(idx, ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Species</label>
                <select
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  :value="mon.species"
                  @change="setSpecies(idx, ($event.target as HTMLSelectElement).value)"
                >
                  <option v-for="s in speciesOptions" :key="s" :value="s">{{ s }}</option>
                </select>
              </div>
            </div>
          </div>

          <button
            v-if="activeParty.pokemon.length < 6"
            class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-accent border border-accent hover:bg-accent hover:text-bg"
            @click="store.addMon()"
          >
            + Add Pokémon
          </button>
          <span v-else class="text-[10px] text-text-muted">Party is full (6).</span>
        </div>
      </div>
    </div>
  </div>
</template>
