<script setup lang="ts">
import type { ScriptFunction } from '../composables/useCodeMirror'

defineProps<{
  functions: ScriptFunction[]
  activeFunction?: string | null
}>()

const emit = defineEmits<{
  select: [func: ScriptFunction]
}>()
</script>

<template>
  <div class="script-fn-list">
    <div class="text-[10px] text-text-muted uppercase tracking-wide mb-1 px-1">
      Functions ({{ functions.length }})
    </div>
    <div v-if="functions.length === 0" class="text-[10px] text-text-muted px-1 italic">
      No functions found
    </div>
    <div
      v-for="fn in functions"
      :key="fn.name"
      class="fn-item"
      :class="{ active: activeFunction === fn.name }"
      @click="emit('select', fn)"
    >
      <span class="fn-name">{{ fn.name }}</span>
      <span class="fn-meta">
        <span v-if="fn.exported" class="fn-badge">exp</span>
        <span class="fn-line">:{{ fn.line }}</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.script-fn-list {
  overflow-y: auto;
  padding: 4px 0;
}

.fn-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 3px 8px;
  cursor: pointer;
  font-size: 11px;
  font-family: monospace;
  border-radius: 3px;
  transition: background 0.15s;
}

.fn-item:hover {
  background: rgba(78, 204, 163, 0.1);
}

.fn-item.active {
  background: rgba(78, 204, 163, 0.2);
  color: var(--color-accent);
}

.fn-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.fn-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: 6px;
  flex-shrink: 0;
}

.fn-badge {
  font-size: 9px;
  padding: 0 3px;
  border-radius: 2px;
  background: rgba(78, 204, 163, 0.3);
  color: var(--color-accent);
}

.fn-line {
  font-size: 10px;
  color: var(--color-text-muted);
}
</style>
