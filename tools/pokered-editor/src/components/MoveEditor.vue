<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useMoveStore } from '../stores/moveStore'
import { injectMove } from '../composables/usePokeredRunner'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import { POKEMON_TYPES, MOVE_EFFECTS } from '../types/constants'

const store = useMoveStore()
const playtestOverlay = usePlaytestOverlay()
const { data, activeMove, dirty, error, loading } = storeToRefs(store)

/**
 * Open the floating playtest in a battle that uses this move — a Lv25 tester
 * knows the move against a wild Lv25 Pidgey. The (possibly unsaved) move data
 * is injected first, so power/accuracy/PP/effect/type edits show up live.
 */
async function testMove() {
  if (!data.value || !activeMove.value) return
  try {
    await injectMove(activeMove.value, JSON.stringify(data.value))
  } catch {
    /* preview injection is best-effort */
  }
  playtestOverlay.launch({ kind: 'moveTest', move: activeMove.value })
}

function setU8(field: 'power' | 'accuracy' | 'pp', raw: string) {
  const v = parseInt(raw, 10)
  if (!Number.isFinite(v)) return
  let max = 255
  if (field === 'accuracy') max = 100
  if (field === 'pp') max = 40
  const min = field === 'pp' ? 1 : 0
  store.updateField(field, Math.max(min, Math.min(max, v)))
}
</script>

<template>
  <div class="flex-1 flex flex-col min-h-0">
    <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
      <div class="flex items-baseline gap-3">
        <h2 class="text-accent text-sm font-bold">Move Editor</h2>
        <span v-if="data" class="text-text text-[12px] font-mono">{{ data.id }}</span>
      </div>
      <div class="flex gap-2">
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!data"
          title="Open the floating playtest in a battle that uses this move (no save impact)"
          @click="testMove"
        >
          ⚔ 试玩招式
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

      <div v-if="!activeMove" class="text-text-muted text-xs">
        Select a move from the sidebar to edit.
      </div>

      <div v-else-if="loading" class="text-text-muted text-xs">Loading...</div>

      <div v-else-if="data" class="space-y-4 max-w-2xl">
        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <h3 class="text-accent text-[12px] font-bold mb-2">Battle Stats</h3>
          <div class="grid grid-cols-3 gap-2">
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Power (0-255)</label>
              <input
                type="number"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                min="0"
                max="255"
                :value="data.power"
                @change="setU8('power', ($event.target as HTMLInputElement).value)"
              />
            </div>
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Accuracy (0-100)</label>
              <input
                type="number"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                min="0"
                max="100"
                :value="data.accuracy"
                @change="setU8('accuracy', ($event.target as HTMLInputElement).value)"
              />
            </div>
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">PP (1-40)</label>
              <input
                type="number"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                min="1"
                max="40"
                :value="data.pp"
                @change="setU8('pp', ($event.target as HTMLInputElement).value)"
              />
            </div>
          </div>
        </section>

        <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
          <h3 class="text-accent text-[12px] font-bold mb-2">Type & Effect</h3>
          <div class="grid grid-cols-2 gap-2">
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Move Type</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="data.type"
                @change="store.updateField('type', ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="t in POKEMON_TYPES" :key="t" :value="t">{{ t }}</option>
              </select>
            </div>
            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Effect</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="data.effect"
                @change="store.updateField('effect', ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="e in MOVE_EFFECTS" :key="e" :value="e">{{ e }}</option>
              </select>
            </div>
          </div>
          <p class="text-text-muted text-[10px] mt-2">
            <span class="font-bold">Note:</span> Effect determines the move's behavior beyond raw damage
            (status, multi-hit, side-effects, etc.). See <code class="text-[10px]">crates/pokered-data/src/moves.rs</code>
            for the full <code class="text-[10px]">MoveEffect</code> enum.
          </p>
        </section>
      </div>
    </div>
  </div>
</template>
