<script setup lang="ts">
import type { ScriptFunction } from '../composables/useCodeMirror'
import type { DslBlock } from '../composables/useCodeMirror'
import ScriptFunctionList from './ScriptFunctionList.vue'

defineProps<{
  functions: ScriptFunction[]
  activeFunction?: string | null
  dslBlocks?: DslBlock[]
  activeDslBlock?: DslBlock | null
  isDslMode?: boolean
  mapName: string
}>()

const emit = defineEmits<{
  select: [func: ScriptFunction]
  selectDsl: [block: DslBlock]
}>()
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-4">Script Editor</h2>

    <div class="mb-3 bg-bg-inset p-2 rounded">
      <div class="text-[10px] text-text-muted uppercase tracking-wide mb-0.5">Script File</div>
      <div class="text-xs font-mono text-text truncate">{{ mapName }}/{{ isDslMode ? 'script.scene' : 'script.js' }}</div>
    </div>

    <div v-if="isDslMode && dslBlocks" class="script-fn-list">
      <div class="text-[10px] text-text-muted uppercase tracking-wide mb-1 px-1">
        DSL Blocks ({{ dslBlocks.length }})
      </div>
      <div v-if="dslBlocks.length === 0" class="text-[10px] text-text-muted px-1 italic">
        No blocks found
      </div>
      <div
        v-for="block in dslBlocks"
        :key="`${block.type}-${block.line}`"
        class="fn-item"
        :class="{ active: activeDslBlock?.line === block.line }"
        @click="emit('selectDsl', block)"
      >
        <span class="fn-name">@{{ block.type }}{{ block.name ? `("${block.name}")` : '' }}</span>
        <span class="fn-line">:{{ block.line }}</span>
      </div>
    </div>

    <ScriptFunctionList
      v-else
      :functions="functions"
      :active-function="activeFunction"
      @select="emit('select', $event)"
    />
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

.fn-line {
  font-size: 10px;
  color: var(--color-text-muted);
  margin-left: 6px;
  flex-shrink: 0;
}
</style>
