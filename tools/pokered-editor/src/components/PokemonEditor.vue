<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import { usePokemonStore } from '../stores/pokemonStore'
import { useMoveStore } from '../stores/moveStore'
import { injectBaseStats } from '../composables/usePokeredRunner'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import {
  SPECIES_LIST,
  POKEMON_TYPES,
  GROWTH_RATES,
  MOVE_LIST,
  EVOLUTION_ITEMS,
} from '../types/constants'
import { speciesToSpriteName } from '../types/pokemon'
import { gfxUrl } from '../utils/assetUrl'

const store = usePokemonStore()
const moveStore = useMoveStore()
const playtestOverlay = usePlaytestOverlay()
const { data, activeSpecies, dirty, error, loading, baseStatTotal } = storeToRefs(store)

// Species/move dropdown options come from the live API lists so records added
// through the editor show up immediately; the static constants are the
// fallback while the lists are still loading (or in static mode).
const moveOptions = computed(() => {
  const names = moveStore.moveNames.length > 0 ? moveStore.moveNames : MOVE_LIST
  return ['None', ...names]
})
const learnsetMoveOptions = computed(() =>
  moveStore.moveNames.length > 0 ? moveStore.moveNames : MOVE_LIST,
)
const speciesOptions = computed(() =>
  store.speciesNames.length > 0 ? store.speciesNames : SPECIES_LIST,
)

onMounted(() => {
  if (moveStore.moveNames.length === 0) moveStore.loadMoveList()
})

const spriteStem = computed(() =>
  data.value ? speciesToSpriteName(data.value.species) : ''
)
const frontSpriteUrl = computed(() =>
  spriteStem.value ? gfxUrl(`pokemon/front/${spriteStem.value}.png`) : ''
)
const backSpriteUrl = computed(() =>
  spriteStem.value ? gfxUrl(`pokemon/back/${spriteStem.value}b.png`) : ''
)

function onSpriteError(e: Event) {
  ;(e.target as HTMLImageElement).style.visibility = 'hidden'
}

/**
 * Push the current (possibly unsaved) base stats into the running game so a
 * brand-new stat edit shows up in battle / Pokédex without saving first.
 * Best-effort: the runner may be unavailable (WASM not built) — the playtest
 * itself then explains how to build it.
 */
async function injectCurrentBaseStats() {
  if (!data.value || !activeSpecies.value) return
  try {
    await injectBaseStats(activeSpecies.value, JSON.stringify(data.value))
  } catch {
    /* preview injection is best-effort */
  }
}

/** Open the floating playtest in a wild battle vs this species (Lv5). */
async function testBattle() {
  if (!data.value || !activeSpecies.value) return
  await injectCurrentBaseStats()
  playtestOverlay.launch({ kind: 'battle', species: activeSpecies.value, level: 5 })
}

/** Open the floating playtest on this species' Pokédex entry. */
async function testPokedex() {
  if (!data.value || !activeSpecies.value) return
  await injectCurrentBaseStats()
  playtestOverlay.launch({ kind: 'pokedex', species: activeSpecies.value })
}

/** Open the floating playtest playing this species' evolution animation. */
function testEvolution(toSpecies: string) {
  if (!data.value || !activeSpecies.value) return
  playtestOverlay.launch({ kind: 'evolution', from: activeSpecies.value, to: toSpecies })
}

function setHeightFeet(raw: string) {
  const v = parseInt(raw, 10)
  if (Number.isFinite(v)) store.updatePokedex('heightFeet', Math.max(0, Math.min(99, v)))
}
function setHeightInches(raw: string) {
  const v = parseInt(raw, 10)
  if (Number.isFinite(v)) store.updatePokedex('heightInches', Math.max(0, Math.min(11, v)))
}
function setWeightLbs(raw: string) {
  const v = parseFloat(raw)
  if (!Number.isFinite(v)) return
  store.updatePokedex('weightDecipounds', Math.max(0, Math.min(999.9, v)) * 10 | 0)
}
function weightLbsDisplay(decipounds: number): string {
  return (decipounds / 10).toFixed(1)
}

const SPECIES_NO_NONE = computed(() => speciesOptions.value.filter(s => s !== 'None'))
const INITIAL_MOVE_OPTIONS = moveOptions

function setStat(stat: 'hp' | 'attack' | 'defense' | 'speed' | 'special', raw: string) {
  const v = parseInt(raw, 10)
  if (Number.isFinite(v)) store.updateBaseStat(stat, v)
}

function setU8(field: 'catchRate' | 'baseExp', raw: string) {
  const v = parseInt(raw, 10)
  if (!Number.isFinite(v)) return
  const clamped = Math.max(0, Math.min(255, v))
  store.updateField(field, clamped)
}

function setTmHmByte(idx: number, raw: string) {
  if (!data.value) return
  const v = parseInt(raw, 10)
  if (!Number.isFinite(v)) return
  const clamped = Math.max(0, Math.min(255, v))
  const flags = [...data.value.tmHmFlags]
  flags[idx] = clamped
  store.updateField('tmHmFlags', flags)
}

function bitIsSet(byte: number, bitIdx: number): boolean {
  return (byte & (1 << bitIdx)) !== 0
}

function tmHmLabel(byteIdx: number, bitIdx: number): string {
  const n = byteIdx * 8 + bitIdx + 1
  if (n <= 50) return `TM${String(n).padStart(2, '0')}`
  if (n <= 55) return `HM${String(n - 50).padStart(2, '0')}`
  return ''
}

const totalKnownTms = computed(() => {
  if (!data.value) return 0
  let count = 0
  for (let byteIdx = 0; byteIdx < 7; byteIdx++) {
    for (let bitIdx = 0; bitIdx < 8; bitIdx++) {
      const n = byteIdx * 8 + bitIdx + 1
      if (n > 55) break
      if (bitIsSet(data.value.tmHmFlags[byteIdx], bitIdx)) count++
    }
  }
  return count
})
</script>

<template>
  <div class="flex-1 flex flex-col min-h-0">
    <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
      <div class="flex items-baseline gap-3">
        <h2 class="text-accent text-sm font-bold">Pokemon Editor</h2>
        <span v-if="data" class="text-text text-[12px] font-mono">{{ data.species }}</span>
        <span v-if="data" class="text-text-muted text-[11px]">
          BST {{ baseStatTotal }}
        </span>
      </div>
      <div class="flex gap-2">
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!data"
          title="Open the floating playtest in a wild battle vs this species (no save impact)"
          @click="testBattle"
        >
          ⚔ 试玩战斗
        </button>
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!data"
          title="Open the floating playtest on this species' Pokédex entry (no save impact)"
          @click="testPokedex"
        >
          📖 试玩图鉴
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

    <div class="flex-1 overflow-y-auto p-3 min-h-0">
      <div v-if="error" class="mb-3 p-2 rounded bg-danger/10 border border-danger text-danger text-[11px]">
        {{ error }}
      </div>

      <div v-if="!activeSpecies" class="text-text-muted text-xs">
        Select a Pokemon species from the sidebar to edit.
      </div>

      <div v-else-if="loading" class="text-text-muted text-xs">Loading...</div>

      <div v-else-if="data" class="space-y-4">
        <!-- Sprite Preview + Pokedex Summary -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)] flex gap-4">
          <div class="flex flex-col items-center gap-1 shrink-0">
            <div class="bg-bg-inset rounded p-1 flex items-center justify-center" style="width: 80px; height: 80px;">
              <img
                :src="frontSpriteUrl"
                :alt="`${data.species} front sprite`"
                class="pixelated"
                style="image-rendering: pixelated; max-width: 72px; max-height: 72px;"
                @error="onSpriteError"
              />
            </div>
            <span class="text-[10px] text-text-muted">Front</span>
          </div>
          <div class="flex flex-col items-center gap-1 shrink-0">
            <div class="bg-bg-inset rounded p-1 flex items-center justify-center" style="width: 80px; height: 80px;">
              <img
                :src="backSpriteUrl"
                :alt="`${data.species} back sprite`"
                class="pixelated"
                style="image-rendering: pixelated; max-width: 56px; max-height: 56px;"
                @error="onSpriteError"
              />
            </div>
            <span class="text-[10px] text-text-muted">Back</span>
          </div>
          <div class="flex-1 grid grid-cols-2 gap-x-3 gap-y-1 text-[11px] self-center">
            <div class="text-text-muted">Pokédex Category</div>
            <div class="text-text font-mono">{{ data.pokedex.category || '—' }}</div>
            <div class="text-text-muted">Height</div>
            <div class="text-text font-mono">{{ data.pokedex.heightFeet }}'{{ data.pokedex.heightInches }}"</div>
            <div class="text-text-muted">Weight</div>
            <div class="text-text font-mono">{{ weightLbsDisplay(data.pokedex.weightDecipounds) }} lbs</div>
            <div class="text-text-muted">Pages</div>
            <div class="text-text font-mono">{{ data.pokedex.flavorTextPages.length }}</div>
          </div>
        </section>

        <!-- Pokedex Editing -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <h3 class="text-accent text-[12px] font-bold mb-2">Pokédex Entry</h3>
          <div class="grid grid-cols-2 gap-2 mb-3">
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Category (≤ 11 chars, A-Z + space)</label>
              <input
                type="text"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono uppercase"
                maxlength="11"
                :value="data.pokedex.category"
                @input="store.updatePokedex('category', ($event.target as HTMLInputElement).value.toUpperCase())"
              />
            </div>
            <div class="grid grid-cols-3 gap-1">
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">ft</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="0"
                  max="99"
                  :value="data.pokedex.heightFeet"
                  @change="setHeightFeet(($event.target as HTMLInputElement).value)"
                />
              </div>
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">in</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="0"
                  max="11"
                  :value="data.pokedex.heightInches"
                  @change="setHeightInches(($event.target as HTMLInputElement).value)"
                />
              </div>
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">lbs</label>
                <input
                  type="number"
                  step="0.1"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="0"
                  max="999.9"
                  :value="weightLbsDisplay(data.pokedex.weightDecipounds)"
                  @change="setWeightLbs(($event.target as HTMLInputElement).value)"
                />
              </div>
            </div>
          </div>

          <div class="flex items-baseline justify-between mb-1">
            <span class="text-text-muted text-[10px]">Flavor Text Pages</span>
            <button
              v-if="data.pokedex.flavorTextPages.length < 4"
              class="px-2 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-accent border border-accent hover:bg-accent hover:text-bg"
              @click="store.addFlavorPage()"
            >
              + Page
            </button>
          </div>

          <div
            v-for="(page, idx) in data.pokedex.flavorTextPages"
            :key="idx"
            class="mb-2 bg-bg-inset rounded p-2"
          >
            <div class="flex items-center justify-between mb-1">
              <span class="text-text text-[11px] font-bold">Page #{{ idx + 1 }}</span>
              <button
                class="text-[10px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none"
                @click="store.removeFlavorPage(idx)"
              >
                ✕ Remove
              </button>
            </div>
            <textarea
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-[11px] font-mono leading-snug"
              rows="3"
              :value="page"
              @input="store.updateFlavorPage(idx, ($event.target as HTMLTextAreaElement).value)"
            />
          </div>
          <p class="text-text-muted text-[10px] mt-1">
            Each line in a page becomes one in-game line. Use <code>#MON</code> for the
            <code>POKéMON</code> token. Pages render on separate screens.
          </p>
        </section>

        <!-- Base Stats -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <h3 class="text-accent text-[12px] font-bold mb-2">Base Stats</h3>
          <div class="grid grid-cols-5 gap-2">
            <div v-for="stat in ['hp', 'attack', 'defense', 'speed', 'special'] as const" :key="stat">
              <label class="block text-[10px] text-text-muted mb-0.5 uppercase">{{ stat }}</label>
              <input
                type="number"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                min="1"
                max="255"
                :value="data.baseStats[stat]"
                @change="setStat(stat, ($event.target as HTMLInputElement).value)"
              />
            </div>
          </div>
          <div class="text-[10px] text-text-muted mt-2">Total: {{ baseStatTotal }}</div>
        </section>

        <!-- Types & Growth -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <h3 class="text-accent text-[12px] font-bold mb-2">Types & Growth</h3>
          <div class="grid grid-cols-2 gap-2">
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Type 1</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="data.type1"
                @change="store.updateField('type1', ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="t in POKEMON_TYPES" :key="t" :value="t">{{ t }}</option>
              </select>
            </div>
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Type 2 (= Type 1 if single-typed)</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="data.type2"
                @change="store.updateField('type2', ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="t in POKEMON_TYPES" :key="t" :value="t">{{ t }}</option>
              </select>
            </div>
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Growth Rate</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="data.growthRate"
                @change="store.updateField('growthRate', ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="g in GROWTH_RATES" :key="g" :value="g">{{ g }}</option>
              </select>
            </div>
            <div class="grid grid-cols-2 gap-2">
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Catch Rate (0-255)</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="0"
                  max="255"
                  :value="data.catchRate"
                  @change="setU8('catchRate', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Base Exp (0-255)</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="0"
                  max="255"
                  :value="data.baseExp"
                  @change="setU8('baseExp', ($event.target as HTMLInputElement).value)"
                />
              </div>
            </div>
          </div>
        </section>

        <!-- Initial Moves -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <h3 class="text-accent text-[12px] font-bold mb-2">Initial Moves (4 slots)</h3>
          <div class="grid grid-cols-2 gap-2">
            <div v-for="(mv, idx) in data.initialMoves" :key="idx">
              <label class="block text-[10px] text-text-muted mb-0.5">Slot #{{ idx + 1 }}</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="mv"
                @change="store.updateInitialMove(idx, ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="m in INITIAL_MOVE_OPTIONS" :key="m" :value="m">{{ m }}</option>
              </select>
            </div>
          </div>
        </section>

        <!-- TM / HM Flags -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <div class="flex items-baseline justify-between mb-2">
            <h3 class="text-accent text-[12px] font-bold">TM / HM Compatibility</h3>
            <span class="text-text-muted text-[10px]">{{ totalKnownTms }} learnable</span>
          </div>
          <div class="grid grid-cols-7 gap-2 mb-2">
            <div v-for="(byte, idx) in data.tmHmFlags" :key="idx">
              <label class="block text-[10px] text-text-muted mb-0.5">Byte {{ idx }} (hex)</label>
              <input
                type="number"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono"
                min="0"
                max="255"
                :value="byte"
                @change="setTmHmByte(idx, ($event.target as HTMLInputElement).value)"
              />
            </div>
          </div>
          <div class="grid grid-cols-8 gap-1 text-[10px]">
            <template v-for="byteIdx in 7" :key="byteIdx - 1">
              <label
                v-for="bitIdx in 8"
                :key="`${byteIdx}-${bitIdx}`"
                class="flex items-center gap-1 cursor-pointer hover:text-accent"
                :class="tmHmLabel(byteIdx - 1, bitIdx - 1) === '' ? 'opacity-30 pointer-events-none' : ''"
              >
                <input
                  type="checkbox"
                  class="cursor-pointer"
                  :checked="bitIsSet(data.tmHmFlags[byteIdx - 1], bitIdx - 1)"
                  :disabled="tmHmLabel(byteIdx - 1, bitIdx - 1) === ''"
                  @change="store.updateTmHmFlag(byteIdx - 1, bitIdx - 1, ($event.target as HTMLInputElement).checked)"
                />
                <span class="font-mono">{{ tmHmLabel(byteIdx - 1, bitIdx - 1) || '–' }}</span>
              </label>
            </template>
          </div>
        </section>

        <!-- Evolutions -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <div class="flex items-baseline justify-between mb-2">
            <h3 class="text-accent text-[12px] font-bold">Evolutions ({{ data.evolutions.length }})</h3>
            <button
              class="px-2 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-accent border border-accent hover:bg-accent hover:text-bg"
              @click="store.addEvolution()"
            >
              + Add
            </button>
          </div>

          <div v-if="data.evolutions.length === 0" class="text-text-muted text-[10px]">
            Does not evolve.
          </div>

          <div
            v-for="(ev, idx) in data.evolutions"
            :key="idx"
            class="bg-bg-inset p-2 rounded mb-2"
          >
            <div class="flex items-center justify-between mb-1">
              <span class="text-text text-[11px] font-bold">#{{ idx + 1 }}</span>
              <div class="flex items-center gap-1.5">
                <button
                  class="px-1.5 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-accent border border-accent/50 hover:bg-accent hover:text-bg"
                  title="Play this evolution animation in the floating playtest"
                  @click="testEvolution(ev.species)"
                >
                  ✨ 试玩进化
                </button>
                <button
                  class="text-[10px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none"
                  @click="store.removeEvolution(idx)"
                >
                  ✕ Remove
                </button>
              </div>
            </div>
            <div class="grid grid-cols-3 gap-2">
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Method</label>
                <select
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  :value="ev.method"
                  @change="store.changeEvolutionMethod(idx, ($event.target as HTMLSelectElement).value as 'level' | 'item' | 'trade')"
                >
                  <option value="level">Level</option>
                  <option value="item">Item</option>
                  <option value="trade">Trade</option>
                </select>
              </div>
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Evolves Into</label>
                <select
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  :value="ev.species"
                  @change="store.updateEvolution(idx, { species: ($event.target as HTMLSelectElement).value })"
                >
                  <option v-for="s in SPECIES_NO_NONE" :key="s" :value="s">{{ s }}</option>
                </select>
              </div>
              <div v-if="ev.method === 'level'">
                <label class="block text-[10px] text-text-muted mb-0.5">Level</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="1"
                  max="100"
                  :value="ev.level ?? 1"
                  @change="store.updateEvolution(idx, { level: Math.max(1, Math.min(100, parseInt(($event.target as HTMLInputElement).value, 10) || 1)) })"
                />
              </div>
              <div v-else-if="ev.method === 'item'" class="col-span-1">
                <label class="block text-[10px] text-text-muted mb-0.5">Item</label>
                <select
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  :value="ev.item ?? 'FireStone'"
                  @change="store.updateEvolution(idx, { item: ($event.target as HTMLSelectElement).value })"
                >
                  <option v-for="i in EVOLUTION_ITEMS" :key="i" :value="i">{{ i }}</option>
                </select>
              </div>
              <div v-else>
                <label class="block text-[10px] text-text-muted mb-0.5">Min Level</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="1"
                  max="100"
                  :value="ev.minLevel ?? 1"
                  @change="store.updateEvolution(idx, { minLevel: Math.max(1, Math.min(100, parseInt(($event.target as HTMLInputElement).value, 10) || 1)) })"
                />
              </div>
            </div>
          </div>
        </section>

        <!-- Learnset -->
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <div class="flex items-baseline justify-between mb-2">
            <h3 class="text-accent text-[12px] font-bold">Level-Up Learnset ({{ data.learnset.length }})</h3>
            <div class="flex gap-2">
              <button
                class="px-2 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-text-muted border border-[rgba(255,255,255,0.1)] hover:text-accent"
                @click="store.sortLearnsetByLevel()"
              >
                Sort by Level
              </button>
              <button
                class="px-2 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-accent border border-accent hover:bg-accent hover:text-bg"
                @click="store.addLearnsetEntry()"
              >
                + Add
              </button>
            </div>
          </div>

          <div v-if="data.learnset.length === 0" class="text-text-muted text-[10px]">
            Learns no moves by level-up.
          </div>

          <div class="space-y-1">
            <div
              v-for="(entry, idx) in data.learnset"
              :key="idx"
              class="flex items-center gap-2 bg-bg-inset p-1.5 rounded group"
            >
              <span class="text-text-muted text-[10px] w-6 shrink-0">#{{ idx + 1 }}</span>
              <div class="shrink-0">
                <label class="text-[10px] text-text-muted">Lv.</label>
                <input
                  type="number"
                  class="w-14 p-1 ml-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="1"
                  max="100"
                  :value="entry.level"
                  @change="store.updateLearnsetEntry(idx, { level: Math.max(1, Math.min(100, parseInt(($event.target as HTMLInputElement).value, 10) || 1)) })"
                />
              </div>
              <select
                class="flex-1 p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="entry.moveId"
                @change="store.updateLearnsetEntry(idx, { moveId: ($event.target as HTMLSelectElement).value })"
              >
                <option v-for="m in learnsetMoveOptions" :key="m" :value="m">{{ m }}</option>
              </select>
              <button
                class="text-[11px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none opacity-0 group-hover:opacity-100 transition-opacity"
                @click="store.removeLearnsetEntry(idx)"
              >
                ✕
              </button>
            </div>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>
