<script setup lang="ts">
import { type PropType } from 'vue'
import type { PokemonEntry } from '../types/save-data'
import { COMMON_SPECIES } from '../types/save-data'

const props = defineProps({
  party: {
    type: Array as PropType<PokemonEntry[]>,
    required: true,
  },
})

const emit = defineEmits<{
  'update:party': [party: PokemonEntry[]]
}>()

function updateParty(newParty: PokemonEntry[]) {
  emit('update:party', newParty)
}

function addPokemon() {
  const newParty = [...props.party, {
    species: 'Bulbasaur',
    level: 5,
    currentHp: 20,
    maxHp: 20,
    moves: ['TACKLE', 'GROWL', '', ''],
    nickname: '',
  }]
  updateParty(newParty)
}

function removePokemon(index: number) {
  const newParty = props.party.filter((_, i) => i !== index)
  updateParty(newParty)
}

function updateSpecies(index: number, species: string) {
  const newParty = props.party.map((p, i) =>
    i === index ? { ...p, species } : p,
  )
  updateParty(newParty)
}

function updateLevel(index: number, value: string) {
  const level = parseInt(value, 10)
  if (Number.isFinite(level)) {
    const newParty = props.party.map((p, i) =>
      i === index ? { ...p, level: Math.max(1, Math.min(100, level)) } : p,
    )
    updateParty(newParty)
  }
}

function updateHp(index: number, field: 'currentHp' | 'maxHp', value: string) {
  const hp = parseInt(value, 10)
  if (Number.isFinite(hp)) {
    const newParty = props.party.map((p, i) =>
      i === index ? { ...p, [field]: Math.max(0, Math.min(999, hp)) } : p,
    )
    updateParty(newParty)
  }
}

function updateNickname(index: number, nickname: string) {
  const newParty = props.party.map((p, i) =>
    i === index ? { ...p, nickname } : p,
  )
  updateParty(newParty)
}

function updateMove(index: number, moveSlot: number, move: string) {
  const newParty = props.party.map((p, i) => {
    if (i !== index) return p
    const moves = [...p.moves]
    while (moves.length < 4) moves.push('')
    moves[moveSlot] = move
    return { ...p, moves }
  })
  updateParty(newParty)
}
</script>

<template>
  <div class="party-editor">
    <div class="flex items-center justify-between mb-3">
      <h3 class="text-accent text-[13px] font-bold">Party ({{ props.party.length }}/6)</h3>
      <button
        v-if="props.party.length < 6"
        class="px-2 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover"
        @click="addPokemon()"
      >
        + Add Pokémon
      </button>
    </div>

    <div v-if="props.party.length === 0" class="text-text-muted text-xs py-4 text-center">
      No Pokémon in party. Click "Add Pokémon" to add one.
    </div>

    <div v-for="(pokemon, idx) in props.party" :key="idx" class="bg-bg p-3 rounded mb-2 border border-[rgba(255,255,255,0.06)]">
      <div class="flex items-center justify-between mb-2">
        <span class="text-text text-xs font-bold">#{{ idx + 1 }}</span>
        <button
          class="text-[10px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none"
          @click="removePokemon(idx)"
        >
          ✕ Remove
        </button>
      </div>

      <div class="grid grid-cols-2 gap-2 mb-2">
        <div>
          <label class="block text-[10px] text-text-muted mb-0.5">Species</label>
          <select
            class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
            :value="pokemon.species"
            @change="updateSpecies(idx, ($event.target as HTMLSelectElement).value)"
          >
            <option v-for="s in COMMON_SPECIES" :key="s" :value="s">{{ s }}</option>
          </select>
        </div>
        <div>
          <label class="block text-[10px] text-text-muted mb-0.5">Nickname</label>
          <input
            type="text"
            class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono"
            :value="pokemon.nickname"
            placeholder="(none)"
            maxlength="10"
            @change="updateNickname(idx, ($event.target as HTMLInputElement).value)"
          />
        </div>
      </div>

      <div class="grid grid-cols-3 gap-2 mb-2">
        <div>
          <label class="block text-[10px] text-text-muted mb-0.5">Level</label>
          <input
            type="number"
            class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
            min="1"
            max="100"
            :value="pokemon.level"
            @change="updateLevel(idx, ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div>
          <label class="block text-[10px] text-text-muted mb-0.5">HP (Current)</label>
          <input
            type="number"
            class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
            min="0"
            max="999"
            :value="pokemon.currentHp"
            @change="updateHp(idx, 'currentHp', ($event.target as HTMLInputElement).value)"
          />
        </div>
        <div>
          <label class="block text-[10px] text-text-muted mb-0.5">HP (Max)</label>
          <input
            type="number"
            class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
            min="1"
            max="999"
            :value="pokemon.maxHp"
            @change="updateHp(idx, 'maxHp', ($event.target as HTMLInputElement).value)"
          />
        </div>
      </div>

      <div class="grid grid-cols-2 gap-2">
        <div v-for="(_, moveIdx) in 4" :key="moveIdx">
          <label class="block text-[10px] text-text-muted mb-0.5">Move {{ moveIdx + 1 }}</label>
          <input
            type="text"
            class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono"
            :value="pokemon.moves[moveIdx] || ''"
            :placeholder="'move ' + (moveIdx + 1)"
            @change="updateMove(idx, moveIdx, ($event.target as HTMLInputElement).value)"
          />
        </div>
      </div>
    </div>
  </div>
</template>
