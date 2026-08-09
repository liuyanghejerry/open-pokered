<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'

const props = defineProps<{
  options: { name: string; value: number }[]
  modelValue: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: number]
}>()

const searchText = ref('')
const isOpen = ref(false)
const highlightIndex = ref(-1)
const inputRef = ref<HTMLInputElement | null>(null)
const listRef = ref<HTMLDivElement | null>(null)

const filtered = computed(() => {
  const q = searchText.value.toLowerCase().trim()
  if (!q) return props.options
  return props.options.filter((opt) => opt.name.toLowerCase().includes(q))
})

const selectedOption = computed(() =>
  props.options.find((opt) => opt.value === props.modelValue),
)

function selectValue(value: number) {
  emit('update:modelValue', value)
  searchText.value = ''
  isOpen.value = false
  highlightIndex.value = -1
}

function onInputFocus() {
  isOpen.value = true
  highlightIndex.value = -1
}

function onInputInput() {
  isOpen.value = true
  highlightIndex.value = filtered.value.length > 0 ? 0 : -1
}

function onBlur() {
  setTimeout(() => {
    // Don't close if blur target is inside the dropdown
    if (
      document.activeElement !== inputRef.value &&
      listRef.value &&
      !listRef.value.contains(document.activeElement)
    ) {
      isOpen.value = false
      highlightIndex.value = -1
      // Reset search text to display selected name
      searchText.value = ''
    }
  }, 150)
}

function onInput(e: Event) {
  searchText.value = (e.target as HTMLInputElement).value
  onInputInput()
}

function onKeydown(e: KeyboardEvent) {
  if (!isOpen.value) {
    if (e.key === 'ArrowDown') {
      isOpen.value = true
      highlightIndex.value = 0
      e.preventDefault()
    }
    return
  }

  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      highlightIndex.value = Math.min(
        highlightIndex.value + 1,
        filtered.value.length - 1,
      )
      scrollToHighlighted()
      break
    case 'ArrowUp':
      e.preventDefault()
      highlightIndex.value = Math.max(highlightIndex.value - 1, 0)
      scrollToHighlighted()
      break
    case 'Enter':
      e.preventDefault()
      if (highlightIndex.value >= 0 && filtered.value[highlightIndex.value]) {
        selectValue(filtered.value[highlightIndex.value].value)
      }
      break
    case 'Escape':
      e.preventDefault()
      isOpen.value = false
      highlightIndex.value = -1
      searchText.value = ''
      break
  }
}

function scrollToHighlighted() {
  nextTick(() => {
    if (!listRef.value) return
    const items = listRef.value.children
    if (highlightIndex.value >= 0 && items[highlightIndex.value]) {
      items[highlightIndex.value].scrollIntoView({ block: 'nearest' })
    }
  })
}

// Sync selected text on external change
watch(
  () => props.modelValue,
  () => {
    searchText.value = ''
    isOpen.value = false
    highlightIndex.value = -1
  },
)

function highlightMatch(text: string, query: string): string {
  if (!query) return text
  const idx = text.toLowerCase().indexOf(query.toLowerCase())
  if (idx === -1) return text
  return (
    text.substring(0, idx) +
    '<mark class="bg-accent/30 text-accent">' +
    text.substring(idx, idx + query.length) +
    '</mark>' +
    text.substring(idx + query.length)
  )
}
</script>

<template>
  <div class="relative" @touchstart.passive>
    <input
      ref="inputRef"
      type="text"
      class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
      :value="
        isOpen || searchText
          ? searchText
          : (selectedOption?.name ?? '')
      "
      :placeholder="selectedOption?.name ?? 'Select map...'"
      @focus="onInputFocus"
      @input="onInput"
      @blur="onBlur"
      @keydown="onKeydown"
    />

    <div
      v-show="isOpen && filtered.length > 0"
      ref="listRef"
      class="absolute left-0 right-0 top-full mt-1 bg-bg border border-accent rounded shadow-lg z-20 max-h-[200px] overflow-y-auto"
    >
      <div
        v-for="(opt, idx) in filtered"
        :key="opt.value"
        class="px-2 py-1.5 text-xs cursor-pointer flex items-center gap-1.5 hover:bg-bg-inset"
        :class="{
          'bg-accent/20': idx === highlightIndex,
        }"
        @mousedown.prevent="selectValue(opt.value)"
        @mouseenter="highlightIndex = idx"
      >
        <span class="w-4 shrink-0 text-accent text-[10px]"
          >{{ opt.value === modelValue ? '✓' : '' }}
        </span>
        <span
          class="truncate"
          v-html="highlightMatch(opt.name, searchText.trim())"
        ></span>
      </div>
    </div>

    <div
      v-show="isOpen && searchText.trim() && filtered.length === 0"
      class="absolute left-0 right-0 top-full mt-1 bg-bg border border-accent rounded shadow-lg z-20 p-2 text-xs text-text-muted"
    >
      No maps found
    </div>
  </div>
</template>
