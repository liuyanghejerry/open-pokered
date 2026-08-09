<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useItemStore } from '../stores/itemStore'
import type { ItemEntry } from '../types/save-data'

/**
 * Save Editor's item list — the real item bag editor. (The previous wiring
 * mounted the item *data* editor here, whose `items` prop silently vanished.)
 * Item names are free text with a datalist of the live /api/items names; the
 * runner-side conversion (save_editor.rs) parses PascalCase / SCREAMING_SNAKE
 * / display forms tolerantly.
 */

const props = defineProps<{ items: ItemEntry[] }>()
const emit = defineEmits<{ 'update:items': [value: ItemEntry[]] }>()

const itemStore = useItemStore()
onMounted(() => {
  if (itemStore.items.length === 0) itemStore.loadItemList()
})

const suggestions = computed(() => itemStore.items)

function update(idx: number, patch: Partial<ItemEntry>) {
  emit(
    'update:items',
    props.items.map((it, i) => (i === idx ? { ...it, ...patch } : it)),
  )
}

function remove(idx: number) {
  emit('update:items', props.items.filter((_, i) => i !== idx))
}

function add() {
  emit('update:items', [
    ...props.items,
    { name: suggestions.value[0] ?? 'Potion', quantity: 1 },
  ])
}

function setName(idx: number, raw: string) {
  update(idx, { name: raw })
}

function setQuantity(idx: number, raw: string) {
  const v = parseInt(raw, 10)
  if (!Number.isFinite(v)) return
  update(idx, { quantity: Math.max(1, Math.min(99, v)) })
}
</script>

<template>
  <div class="h-full flex flex-col min-h-0">
    <div class="flex items-center justify-between mb-2 shrink-0">
      <span class="text-text-muted text-[11px]">
        存档道具（名称支持 PokeBall / POKE_BALL / POKE BALL 等写法）
      </span>
      <button
        class="px-2 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-accent border border-accent hover:bg-accent hover:text-bg"
        @click="add"
      >
        + Add Item
      </button>
    </div>

    <div class="flex-1 overflow-y-auto space-y-1.5 min-h-0">
      <div
        v-for="(it, idx) in items"
        :key="idx"
        class="flex items-center gap-2"
      >
        <input
          list="save-item-names"
          class="flex-1 min-w-0 p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono"
          :value="it.name"
          placeholder="e.g. PokeBall"
          @change="setName(idx, ($event.target as HTMLInputElement).value)"
        />
        <label class="text-[10px] text-text-muted shrink-0">×</label>
        <input
          type="number"
          min="1"
          max="99"
          class="w-16 p-1 rounded border border-accent bg-bg text-text text-[11px]"
          :value="it.quantity"
          @change="setQuantity(idx, ($event.target as HTMLInputElement).value)"
        />
        <button
          class="text-[11px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none shrink-0"
          title="Remove this item"
          @click="remove(idx)"
        >
          ✕
        </button>
      </div>

      <p v-if="items.length === 0" class="text-[10px] text-text-muted">
        空背包 — 点击 "+ Add Item" 添加道具。
      </p>

      <datalist id="save-item-names">
        <option v-for="n in suggestions" :key="n" :value="n" />
      </datalist>
    </div>
  </div>
</template>
