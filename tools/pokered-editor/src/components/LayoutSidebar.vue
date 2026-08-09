<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useLayoutStore } from '../stores/layoutStore'

const store = useLayoutStore()
const { layoutNames, activeName, dirty, loading } = storeToRefs(store)

defineProps<{
  filter?: string
}>()

const emit = defineEmits<{
  select: [name: string]
}>()

const filterText = defineModel<string>('filter', { default: '' })

onMounted(() => {
  if (layoutNames.value.length === 0) store.loadList()
})

const filteredLayouts = computed(() => {
  const q = (filterText.value ?? '').trim().toLowerCase()
  if (!q) return layoutNames.value
  return layoutNames.value.filter(n => n.toLowerCase().includes(q))
})

function handleSelect(name: string) {
  emit('select', name)
}
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-3">Layout Editor</h2>

    <input
      v-model="filterText"
      type="text"
      placeholder="Filter layout..."
      class="w-full p-1.5 mb-3 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-xs"
    />

    <div v-if="loading" class="text-[11px] text-text-muted mb-2">Loading...</div>

    <nav class="space-y-0.5">
      <button
        v-for="name in filteredLayouts"
        :key="name"
        class="layout-nav-btn"
        :class="{ active: activeName === name }"
        @click="handleSelect(name)"
      >
        <span class="layout-nav-label">{{ name }}</span>
        <span v-if="activeName === name && dirty" class="layout-dirty">●</span>
      </button>
    </nav>

    <div v-if="filteredLayouts.length === 0 && !loading" class="text-[11px] text-text-muted mt-2">
      No matching layouts.
    </div>
  </div>
</template>

<style scoped>
.layout-nav-btn {
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

.layout-nav-btn:hover {
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
}

.layout-nav-btn.active {
  background: rgba(78, 204, 163, 0.08);
  color: var(--color-accent);
}

.layout-nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.layout-dirty {
  color: var(--color-warning);
  font-size: 10px;
}
</style>
