<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useTrainerStore } from '../stores/trainerStore'

const store = useTrainerStore()
const { classNames, activeClass, dirty, loading } = storeToRefs(store)

defineProps<{
  filter?: string
}>()

const emit = defineEmits<{
  select: [name: string]
}>()

onMounted(() => {
  if (classNames.value.length === 0) store.loadClassList()
})

const filterText = defineModel<string>('filter', { default: '' })

const filteredClasses = computed(() => {
  const q = (filterText.value ?? '').trim().toLowerCase()
  if (!q) return classNames.value
  return classNames.value.filter(n => n.toLowerCase().includes(q))
})

function handleSelect(name: string) {
  emit('select', name)
}
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-3">Trainer Editor</h2>

    <input
      v-model="filterText"
      type="text"
      placeholder="Filter trainer class..."
      class="w-full p-1.5 mb-3 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-xs"
    />

    <div v-if="loading" class="text-[11px] text-text-muted mb-2">Loading...</div>

    <nav class="space-y-0.5">
      <button
        v-for="name in filteredClasses"
        :key="name"
        class="trainer-nav-btn"
        :class="{ active: activeClass === name }"
        @click="handleSelect(name)"
      >
        <span class="trainer-nav-label">{{ name }}</span>
        <span v-if="activeClass === name && dirty" class="trainer-dirty">●</span>
      </button>
    </nav>

    <div v-if="filteredClasses.length === 0 && !loading" class="text-[11px] text-text-muted mt-2">
      No matching classes.
    </div>
  </div>
</template>

<style scoped>
.trainer-nav-btn {
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

.trainer-nav-btn:hover {
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
}

.trainer-nav-btn.active {
  background: rgba(78, 204, 163, 0.08);
  color: var(--color-accent);
}

.trainer-nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trainer-dirty {
  color: var(--color-warning);
  font-size: 10px;
}
</style>
