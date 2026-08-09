<script setup lang="ts">
// ───────────────────────────────────────────────────────────────────────────
// AI settings modal — text + image provider editors in one overlay, opened
// from the AssistantPanel header (pokered-editor has no settings activity).
// ───────────────────────────────────────────────────────────────────────────
import { ref } from 'vue'
import ProviderSettings from './ProviderSettings.vue'
import ImageProviderSettings from './ImageProviderSettings.vue'

const emit = defineEmits<{ close: [] }>()

const tab = ref<'text' | 'image'>('text')
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60" @click.self="emit('close')">
    <div class="w-[44rem] max-w-[95vw] max-h-[85vh] flex flex-col bg-gray-900 border border-gray-700 rounded-lg shadow-xl">
      <div class="flex items-center gap-2 px-4 py-2.5 border-b border-gray-700 shrink-0">
        <span class="text-sm font-bold text-blue-400">⚙ AI Settings</span>
        <div class="flex gap-1 ml-4">
          <button
            @click="tab = 'text'"
            :class="['px-2.5 py-1 text-[11px] rounded', tab === 'text' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-gray-200 bg-gray-800']"
          >Text providers</button>
          <button
            @click="tab = 'image'"
            :class="['px-2.5 py-1 text-[11px] rounded', tab === 'image' ? 'bg-purple-600 text-white' : 'text-gray-400 hover:text-gray-200 bg-gray-800']"
          >Image providers</button>
        </div>
        <button @click="emit('close')" class="ml-auto text-gray-500 hover:text-gray-300 text-sm">✕</button>
      </div>
      <div class="flex-1 overflow-y-auto min-h-0">
        <ProviderSettings v-if="tab === 'text'" />
        <ImageProviderSettings v-else />
      </div>
    </div>
  </div>
</template>
