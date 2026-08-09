<script setup lang="ts">
import { ref, computed, type PropType } from 'vue'
import { COMMON_FLAGS } from '../types/save-data'

const props = defineProps({
  flags: {
    type: Object as PropType<Record<string, boolean>>,
    required: true,
  },
})

const emit = defineEmits<{
  'update:flags': [flags: Record<string, boolean>]
}>()

const searchQuery = ref('')
const newFlagName = ref('')

const filteredFlags = computed((): [string, boolean][] => {
  const entries = Object.entries(props.flags)
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return entries.sort(([a], [b]) => a.localeCompare(b))
  return entries
    .filter(([name]) => name.toLowerCase().includes(q))
    .sort(([a], [b]) => a.localeCompare(b))
})

function toggleFlag(name: string) {
  const newFlags = { ...props.flags, [name]: !props.flags[name] }
  emit('update:flags', newFlags)
}

function setFlag(name: string, value: boolean) {
  const newFlags = { ...props.flags, [name]: value }
  emit('update:flags', newFlags)
}

function removeFlag(name: string) {
  const newFlags = { ...props.flags }
  delete newFlags[name]
  emit('update:flags', newFlags)
}

function addFlag() {
  const name = newFlagName.value.trim()
  if (!name || name in props.flags) return
  const newFlags = { ...props.flags, [name]: false }
  emit('update:flags', newFlags)
  newFlagName.value = ''
}

function addCommonFlag(name: string) {
  if (name in props.flags) return
  const newFlags = { ...props.flags, [name]: false }
  emit('update:flags', newFlags)
}

const trueCount = computed(() =>
  Object.values(props.flags).filter(Boolean).length,
)

const commonFlagsNotInStore = computed(() =>
  COMMON_FLAGS.filter((f) => !(f in props.flags)),
)
</script>

<template>
  <div class="flag-editor">
    <div class="flex items-center justify-between mb-3">
      <h3 class="text-accent text-[13px] font-bold">
        Event Flags
        <span class="text-text-muted text-[10px] ml-2">
          ({{ trueCount }} / {{ Object.keys(props.flags).length }} active)
        </span>
      </h3>
    </div>

    <!-- Search and add -->
    <div class="mb-3 space-y-2">
      <input
        type="text"
        class="w-full p-1.5 rounded border border-accent bg-bg text-text text-[11px] font-mono"
        v-model="searchQuery"
        placeholder="Search flags..."
      />
      <div class="flex gap-2">
        <input
          type="text"
          class="flex-1 p-1.5 rounded border border-accent bg-bg text-text text-[11px] font-mono"
          v-model="newFlagName"
          placeholder="New flag name..."
          @keyup.enter="addFlag()"
        />
        <button
          class="px-2 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover shrink-0"
          @click="addFlag()"
        >
          Add
        </button>
      </div>
    </div>

    <!-- All flags set / clear -->
    <div class="flex gap-2 mb-2">
      <button
        class="px-2 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent"
        @click="Object.keys(props.flags).forEach(k => setFlag(k, true))"
      >
        Set All
      </button>
      <button
        class="px-2 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent"
        @click="Object.keys(props.flags).forEach(k => setFlag(k, false))"
      >
        Clear All
      </button>
    </div>

    <!-- Flag list -->
    <div class="max-h-[400px] overflow-y-auto space-y-0.5">
      <div
        v-for="[name, value] in filteredFlags"
        :key="name"
        class="flex items-center justify-between p-1.5 rounded hover:bg-bg-inset group"
      >
        <label class="flex items-center gap-2 cursor-pointer flex-1 min-w-0">
          <input
            type="checkbox"
            :checked="value"
            class="accent-accent shrink-0"
            @change="toggleFlag(name)"
          />
          <span class="text-[11px] font-mono text-text truncate">{{ name }}</span>
        </label>
        <span class="text-[10px] shrink-0 ml-2" :class="value ? 'text-accent' : 'text-text-muted'">
          {{ value ? 'ON' : 'OFF' }}
        </span>
        <button
          class="text-[10px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none ml-1 opacity-0 group-hover:opacity-100 transition-opacity"
          @click="removeFlag(name)"
        >
          ✕
        </button>
      </div>
    </div>

    <!-- Add common flags -->
    <div v-if="commonFlagsNotInStore.length > 0" class="mt-3 pt-3 border-t border-[rgba(255,255,255,0.06)]">
      <p class="text-[10px] text-text-muted mb-1">Add common flags:</p>
      <div class="flex flex-wrap gap-1">
        <button
          v-for="flag in commonFlagsNotInStore"
          :key="flag"
          class="px-1.5 py-0.5 rounded text-[10px] cursor-pointer bg-bg-inset text-text-muted border border-[rgba(255,255,255,0.06)] hover:text-accent hover:border-accent"
          @click="addCommonFlag(flag)"
        >
          + {{ flag }}
        </button>
      </div>
    </div>
  </div>
</template>
