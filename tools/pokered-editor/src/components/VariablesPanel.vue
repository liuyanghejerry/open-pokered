<script setup lang="ts">
import { computed } from 'vue'
import type { TemplateVariable } from '../composables/useVariableExtract'

const props = defineProps<{
  variables: TemplateVariable[]
  menuName: string
}>()

const emit = defineEmits<{
  (e: 'update', key: string, value: string): void
}>()

const searchQuery = defineModel<string>('search', { default: '' })

const filtered = computed(() => {
  if (!searchQuery.value) return props.variables
  const q = searchQuery.value.toLowerCase()
  return props.variables.filter(v => v.key.toLowerCase().includes(q))
})
</script>

<template>
  <div class="border-t border-[rgba(255,255,255,0.06)] bg-bg-inset/20">
    <div class="flex items-center gap-2 p-2 border-b border-[rgba(255,255,255,0.06)]">
      <span class="text-[10px] font-bold text-text-muted uppercase tracking-wider">🧩 Variables</span>
      <span class="text-[10px] text-text-muted">{{ variables.length }}</span>
      <input
        v-model="searchQuery"
        placeholder="Filter..."
        class="flex-1 px-2 py-0.5 text-[11px] rounded border border-[rgba(255,255,255,0.08)] bg-bg text-text placeholder-text-muted/50 outline-none"
      />
    </div>
    <div class="max-h-48 overflow-y-auto">
      <div v-for="v in filtered" :key="v.key"
        class="flex items-center gap-2 px-2 py-1 border-b border-[rgba(255,255,255,0.03)] hover:bg-bg-inset/40 text-[11px]">
        <span class="font-mono text-text-muted w-32 shrink-0 truncate" :title="v.key">{{ v.key }}</span>
        <span class="w-6 text-[9px] uppercase font-bold"
          :class="{'text-blue-400':v.type==='string','text-green-400':v.type==='number','text-yellow-400':v.type==='boolean','text-purple-400':v.type==='list'}">
          {{ v.type[0] }}
        </span>
        <input v-if="v.type==='string'||v.type==='number'" :type="v.type==='number'?'number':'text'"
          :value="v.currentValue ?? v.defaultValue"
          @input="emit('update', v.key, ($event.target as HTMLInputElement).value)"
          class="flex-1 px-1.5 py-0.5 rounded border border-[rgba(255,255,255,0.08)] bg-bg text-text text-[11px] outline-none"
          :placeholder="v.defaultValue" />
        <input v-else-if="v.type==='boolean'" type="checkbox"
          :checked="v.currentValue === 'true' || (v.currentValue === '' && v.defaultValue === 'true')"
          @change="emit('update', v.key, ($event.target as HTMLInputElement).checked ? 'true' : 'false')"
          class="accent-accent" />
        <textarea v-else-if="v.type==='list'" :value="v.currentValue ?? v.defaultValue"
          @input="emit('update', v.key, ($event.target as HTMLTextAreaElement).value)"
          class="flex-1 px-1.5 py-0.5 rounded border border-[rgba(255,255,255,0.08)] bg-bg text-text text-[10px] outline-none resize-none"
          rows="1" placeholder="line1\nline2\n..." />
      </div>
    </div>
  </div>
</template>
