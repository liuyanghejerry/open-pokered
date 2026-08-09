<script setup lang="ts">
import { onMounted, watch, ref, nextTick } from 'vue'
import { storeToRefs } from 'pinia'
import { useItemStore } from '../stores/itemStore'

const store = useItemStore()
const { items, filteredItems, categories, selectedCategory, searchQuery, activeItemId, dirty, loading, error } = storeToRefs(store)

defineProps<{
  filter?: string
}>()

const emit = defineEmits<{
  select: [name: string]
}>()

const filterText = defineModel<string>('filter', { default: '' })

const localSelected = ref<string>('')

// ── "New item" inline create row ──────────────────────────────────────────
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
  const ok = await store.createItem(name)
  creatingBusy.value = false
  if (ok) {
    creating.value = false
    newName.value = ''
    emit('select', name)
  }
}

onMounted(async () => {
  await store.loadItemList()
  await store.loadCategories()
  if (items.value.length > 0) {
    selectItem(items.value[0])
  }
})

watch(filterText, (val) => {
  searchQuery.value = val
})

watch(activeItemId, (id) => {
  if (id) localSelected.value = id
})

function selectItem(id: string) {
  localSelected.value = id
  store.setCategory(null)
  store.loadItem(id)
  emit('select', id)
}

function selectCategory(catId: string | null) {
  store.setCategory(catId)
}
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-3">Item Editor</h2>

    <button
      v-if="!creating"
      class="w-full mb-3 px-2 py-1.5 rounded text-[11px] font-bold cursor-pointer bg-accent/10 text-accent border border-accent/30 hover:bg-accent/20 transition-colors"
      @click="startCreate"
    >
      ＋ New Item
    </button>
    <div v-else class="mb-3">
      <div class="flex gap-1.5">
        <input
          ref="nameInput"
          v-model="newName"
          type="text"
          placeholder="PascalCase, e.g. SuperPotion2"
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
      placeholder="Search items..."
      class="w-full p-1.5 mb-3 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-xs"
    />

    <div v-if="loading" class="text-[11px] text-text-muted mb-2">Loading...</div>

    <div class="flex flex-wrap gap-1 mb-3">
      <button
        class="px-2 py-1 rounded text-[10px] font-medium cursor-pointer border-none transition-all duration-150"
        :class="!selectedCategory
          ? 'bg-accent/20 text-accent'
          : 'bg-bg-inset text-text-muted hover:text-text hover:bg-bg-inset/80'"
        @click="selectCategory(null)"
      >
        All
      </button>
      <button
        v-for="cat in categories"
        :key="cat.id"
        class="px-2 py-1 rounded text-[10px] font-medium cursor-pointer border-none transition-all duration-150"
        :class="selectedCategory === cat.id
          ? 'bg-accent/20 text-accent'
          : 'bg-bg-inset text-text-muted hover:text-text hover:bg-bg-inset/80'"
        :style="{ borderLeft: `3px solid ${cat.color}` }"
        @click="selectCategory(cat.id)"
      >
        {{ cat.label }}
      </button>
    </div>

    <div class="text-[10px] text-text-muted mb-1">
      {{ filteredItems.length }} / {{ items.length }}
    </div>

    <nav class="space-y-0.5">
      <button
        v-for="name in filteredItems"
        :key="name"
        class="item-nav-btn"
        :class="{ active: activeItemId === name }"
        @click="selectItem(name)"
      >
        <span class="item-nav-label">{{ name }}</span>
        <span v-if="activeItemId === name && dirty" class="item-dirty">●</span>
      </button>
    </nav>

    <div v-if="filteredItems.length === 0 && !loading" class="text-[11px] text-text-muted mt-2">
      No matching items.
    </div>
  </div>
</template>

<style scoped>
.item-nav-btn {
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

.item-nav-btn:hover {
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
}

.item-nav-btn.active {
  background: rgba(78, 204, 163, 0.08);
  color: var(--color-accent);
}

.item-nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-dirty {
  color: var(--color-warning);
  font-size: 10px;
}
</style>
