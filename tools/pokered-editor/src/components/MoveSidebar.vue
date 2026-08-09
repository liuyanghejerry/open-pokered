<script setup lang="ts">
import { onMounted, computed, ref, nextTick } from 'vue'
import { storeToRefs } from 'pinia'
import { useMoveStore } from '../stores/moveStore'

const store = useMoveStore()
const { moveNames, activeMove, dirty, loading, error } = storeToRefs(store)

defineProps<{
  filter?: string
}>()

const emit = defineEmits<{
  select: [name: string]
}>()

onMounted(() => {
  if (moveNames.value.length === 0) store.loadMoveList()
})

const filterText = defineModel<string>('filter', { default: '' })

// ── "New move" inline create row ──────────────────────────────────────────
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
  const ok = await store.createMove(name)
  creatingBusy.value = false
  if (ok) {
    creating.value = false
    newName.value = ''
    emit('select', name)
  }
}

const filteredMoves = computed(() => {
  const q = (filterText.value ?? '').trim().toLowerCase()
  if (!q) return moveNames.value
  return moveNames.value.filter(n => n.toLowerCase().includes(q))
})

function handleSelect(name: string) {
  emit('select', name)
}
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-3">Move Editor</h2>

    <button
      v-if="!creating"
      class="w-full mb-3 px-2 py-1.5 rounded text-[11px] font-bold cursor-pointer bg-accent/10 text-accent border border-accent/30 hover:bg-accent/20 transition-colors"
      @click="startCreate"
    >
      ＋ New Move
    </button>
    <div v-else class="mb-3">
      <div class="flex gap-1.5">
        <input
          ref="nameInput"
          v-model="newName"
          type="text"
          placeholder="PascalCase, e.g. Thunder2"
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
      placeholder="Filter moves..."
      class="w-full p-1.5 mb-3 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-xs"
    />

    <div v-if="loading" class="text-[11px] text-text-muted mb-2">Loading...</div>

    <div class="text-[10px] text-text-muted mb-1">
      {{ filteredMoves.length }} / {{ moveNames.length }}
    </div>

    <nav class="space-y-0.5">
      <button
        v-for="name in filteredMoves"
        :key="name"
        class="move-nav-btn"
        :class="{ active: activeMove === name }"
        @click="handleSelect(name)"
      >
        <span class="move-nav-label">{{ name }}</span>
        <span v-if="activeMove === name && dirty" class="move-dirty">●</span>
      </button>
    </nav>

    <div v-if="filteredMoves.length === 0 && !loading" class="text-[11px] text-text-muted mt-2">
      No matching moves.
    </div>
  </div>
</template>

<style scoped>
.move-nav-btn {
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

.move-nav-btn:hover {
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
}

.move-nav-btn.active {
  background: rgba(78, 204, 163, 0.08);
  color: var(--color-accent);
}

.move-nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.move-dirty {
  color: var(--color-warning);
  font-size: 10px;
}
</style>
