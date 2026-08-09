<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { usePokemonStore } from '../stores/pokemonStore'
import { setWildOverride } from '../composables/usePokeredRunner'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import { SPECIES_LIST } from '../types/constants'
import type { WildEncounterTableJson } from '../types'

type WildVersion = 'red' | 'blue'
type WildTerrain = 'grass' | 'water'

const store = useMapStore()
const playtestOverlay = usePlaytestOverlay()
const currentMap = computed(() => store.currentMap)
const wild = computed(() => currentMap.value?.wild ?? null)

// Species options come from the live API list so species added through the
// editor show up here immediately; SPECIES_LIST is the loading fallback.
const pokemonStore = usePokemonStore()
const speciesOptions = computed(() =>
  pokemonStore.speciesNames.length > 0 ? pokemonStore.speciesNames : SPECIES_LIST,
)

onMounted(() => {
  if (pokemonStore.speciesNames.length === 0) pokemonStore.loadSpeciesList()
})

/**
 * Open the floating playtest in a battle vs this wild slot's species/level.
 * The current (possibly unsaved) wild tables are pushed into the runner first
 * so the encounter config shows up without saving.
 */
async function testSlot(version: WildVersion, terrain: WildTerrain, idx: number) {
  const map = currentMap.value
  const mon = tableFor(version, terrain)?.mons[idx]
  if (!map || !mon) return
  if (wild.value) {
    try {
      await setWildOverride(map.name, JSON.stringify(wild.value))
    } catch {
      /* preview injection is best-effort */
    }
  }
  playtestOverlay.launch({ kind: 'battle', species: mon.species, level: mon.level })
}

// Slot-percentage weights per slot index (0..9), as used by Gen 1 wild encounter tables.
// Sums to 100. Lower indices are more common.
const SLOT_PERCENTS = [20, 20, 10, 10, 10, 10, 5, 5, 5, 5]

function slotPercent(index: number): string {
  if (index < SLOT_PERCENTS.length) return `${SLOT_PERCENTS[index]}%`
  return '—'
}

function tableFor(version: WildVersion, terrain: WildTerrain): WildEncounterTableJson | null {
  const w = wild.value
  if (!w) return null
  const v = w[version]
  if (!v) return null
  return v[terrain] ?? null
}

function onRateInput(version: WildVersion, terrain: WildTerrain, e: Event) {
  const v = parseInt((e.target as HTMLInputElement).value, 10)
  if (Number.isFinite(v)) store.updateWildEncounterRate(version, terrain, v)
}

function onLevelInput(version: WildVersion, terrain: WildTerrain, idx: number, e: Event) {
  const v = parseInt((e.target as HTMLInputElement).value, 10)
  if (Number.isFinite(v)) store.updateWildMon(version, terrain, idx, { level: v })
}

function onSpeciesChange(version: WildVersion, terrain: WildTerrain, idx: number, e: Event) {
  const v = (e.target as HTMLSelectElement).value
  store.updateWildMon(version, terrain, idx, { species: v })
}

function addMon(version: WildVersion, terrain: WildTerrain) {
  store.addWildMon(version, terrain)
}

function removeMon(version: WildVersion, terrain: WildTerrain, idx: number) {
  store.removeWildMon(version, terrain, idx)
}

function fillTo10(version: WildVersion, terrain: WildTerrain) {
  store.fillWildMonsTo(version, terrain, 10)
}

function copyAcross(srcVersion: WildVersion, dstVersion: WildVersion) {
  store.copyWildTables(srcVersion, dstVersion)
  store.updateStatus(`Copied wild data ${srcVersion} → ${dstVersion}`)
}

function enableWild() {
  store.ensureWildData()
}

function disableWild() {
  if (confirm('Remove all wild encounter data for this map?')) {
    store.clearWildData()
  }
}
</script>

<template>
  <div class="bg-bg-inset p-2.5 rounded-md font-mono text-[11px]">
    <h3 class="text-accent text-[13px] font-bold mb-2 font-sans">Wild Encounters</h3>

    <template v-if="!currentMap">
      <p class="text-text-muted">No map loaded.</p>
    </template>

    <template v-else-if="!wild">
      <p class="text-text-muted my-1">This map has no wild encounter data.</p>
      <button
        class="px-2 py-1 rounded bg-accent text-bg text-xs hover:opacity-80"
        @click="enableWild"
      >+ Enable wild encounters</button>
    </template>

    <template v-else>
      <div class="flex justify-end mb-2 gap-1">
        <button
          class="px-2 py-0.5 rounded bg-bg border border-accent text-[10px] hover:bg-accent hover:text-bg"
          title="Copy Red tables to Blue"
          @click="copyAcross('red', 'blue')"
        >Red → Blue</button>
        <button
          class="px-2 py-0.5 rounded bg-bg border border-accent text-[10px] hover:bg-accent hover:text-bg"
          title="Copy Blue tables to Red"
          @click="copyAcross('blue', 'red')"
        >Blue → Red</button>
        <button
          class="px-2 py-0.5 rounded bg-bg border border-red-500 text-red-400 text-[10px] hover:bg-red-500 hover:text-bg"
          title="Remove all wild encounter data"
          @click="disableWild"
        >Disable</button>
      </div>

      <div v-for="version in (['red', 'blue'] as WildVersion[])" :key="version" class="mb-3">
        <p class="font-bold mb-1" :class="version === 'red' ? 'text-red-400' : 'text-blue-400'">
          {{ version === 'red' ? 'Red Version' : 'Blue Version' }}
        </p>

        <div
          v-for="terrain in (['grass', 'water'] as WildTerrain[])"
          :key="terrain"
          class="mb-2 p-1.5 bg-bg rounded"
        >
          <div class="flex items-center justify-between mb-1">
            <span class="font-bold capitalize">{{ terrain }}</span>
            <div class="flex items-center gap-1">
              <label class="text-text-muted">Rate:</label>
              <input
                type="number"
                min="0"
                max="255"
                class="w-14 px-1 py-0.5 rounded bg-bg-inset border border-accent text-xs"
                :value="tableFor(version, terrain)?.encounterRate ?? 0"
                @input="(e) => onRateInput(version, terrain, e)"
              />
              <span class="text-[10px] text-text-muted">/255</span>
            </div>
          </div>

          <template v-if="tableFor(version, terrain)">
            <div
              v-for="(mon, i) in tableFor(version, terrain)!.mons"
              :key="`${version}-${terrain}-${i}`"
              class="flex items-center gap-1 my-0.5"
            >
              <span class="w-6 text-[10px] text-text-muted text-right">#{{ i + 1 }}</span>
              <span class="w-8 text-[10px] text-text-muted">{{ slotPercent(i) }}</span>
              <label class="text-[10px]">Lv</label>
              <input
                type="number"
                min="1"
                max="100"
                class="w-12 px-1 py-0.5 rounded bg-bg-inset border border-accent text-xs"
                :value="mon.level"
                @input="(e) => onLevelInput(version, terrain, i, e)"
              />
              <select
                class="flex-1 min-w-0 px-1 py-0.5 rounded bg-bg-inset border border-accent text-xs"
                :value="mon.species"
                @change="(e) => onSpeciesChange(version, terrain, i, e)"
              >
                <option v-for="sp in speciesOptions" :key="sp" :value="sp">{{ sp }}</option>
              </select>
              <button
                class="px-1.5 py-0.5 rounded text-accent hover:bg-accent hover:text-bg text-[10px]"
                title="Fight this wild encounter in the playtest"
                @click="testSlot(version, terrain, i)"
              >▶</button>
              <button
                class="px-1.5 py-0.5 rounded text-red-400 hover:bg-red-500 hover:text-bg text-[10px]"
                title="Remove this slot"
                @click="removeMon(version, terrain, i)"
              >✕</button>
            </div>

            <div class="flex gap-1 mt-1">
              <button
                class="px-2 py-0.5 rounded bg-bg-inset border border-accent text-[10px] hover:bg-accent hover:text-bg"
                @click="addMon(version, terrain)"
              >+ Add slot</button>
              <button
                v-if="tableFor(version, terrain)!.mons.length < 10"
                class="px-2 py-0.5 rounded bg-bg-inset border border-accent text-[10px] hover:bg-accent hover:text-bg"
                title="Pad list to 10 slots (the original ROM format)"
                @click="fillTo10(version, terrain)"
              >Fill to 10</button>
            </div>

            <p
              v-if="tableFor(version, terrain)!.encounterRate > 0 && tableFor(version, terrain)!.mons.length === 0"
              class="text-[10px] text-yellow-400 mt-1"
            >
              ⚠ Encounter rate &gt; 0 but no mons defined.
            </p>
            <p
              v-else-if="tableFor(version, terrain)!.encounterRate > 0 && tableFor(version, terrain)!.mons.length !== 10"
              class="text-[10px] text-yellow-400 mt-1"
            >
              ⚠ {{ tableFor(version, terrain)!.mons.length }} slots — original ROM format expects 10.
            </p>
          </template>
        </div>
      </div>

      <p class="text-[10px] text-text-muted">
        Encounter rate is 0–255; slot probabilities are 20/20/10/10/10/10/5/5/5/5%.
        Save with the sidebar Save button.
      </p>
    </template>
  </div>
</template>
