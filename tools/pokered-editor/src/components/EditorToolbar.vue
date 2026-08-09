<script setup lang="ts">
import { ref } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import { storeToRefs } from 'pinia'

const store = useMapStore()
const playtestOverlay = usePlaytestOverlay()
const { currentTool, zoom, currentMap } = storeToRefs(store)
const importWarnings = ref<string[]>([])

const tools = [
  { id: 'view' as const, label: 'View' },
  { id: 'edit' as const, label: 'Edit Collision' },
  { id: 'edit-tiles' as const, label: 'Edit Tiles' },
]

async function handleImportTmx() {
  importWarnings.value = []
  try {
    const warnings = await store.importTmxFromFile()
    importWarnings.value = warnings
  } catch (e) {
    importWarnings.value = [(e as Error).message]
  }
}

function handleExportTmx() {
  store.exportCurrentMapToTmx()
}

/** Open the floating playtest pre-targeted at the map being edited. */
function testCurrentMap() {
  const name = currentMap.value?.name
  if (!name) return
  playtestOverlay.launch({ kind: 'map', map: name })
}
</script>

<template>
  <div class="flex flex-col gap-2 mb-2.5">
    <div class="flex items-center gap-2.5">
      <button
        v-for="tool in tools"
        :key="tool.id"
        class="px-4 py-2 rounded text-xs cursor-pointer border-2 transition-colors"
        :class="
          currentTool === tool.id
            ? 'border-accent bg-[#2a4a3e] text-text'
            : 'border-transparent bg-[#333] text-text hover:bg-[#444]'
        "
        @click="store.setTool(tool.id)"
      >
        {{ tool.label }}
      </button>

      <button
        class="px-4 py-2 rounded text-xs cursor-pointer border-2 transition-colors border-accent bg-[#2a4a3e] text-text"
        title="Open the floating playtest and warp to the current map (Test mode, no save impact)"
        @click="testCurrentMap"
      >
        ▶ Test this map
      </button>

      <div class="flex items-center gap-1.5 ml-4">
        <button
          class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-xs hover:bg-[#444]"
          @click="store.zoomOut()"
        >
          -
        </button>
        <span class="text-xs min-w-[50px] text-center">{{ zoom }}x</span>
        <button
          class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-xs hover:bg-[#444]"
          @click="store.zoomIn()"
        >
          +
        </button>
      </div>

      <div class="flex items-center gap-1.5 ml-auto">
        <button
          class="px-3 py-1.5 bg-[#2c3e50] text-text border border-[rgba(255,255,255,0.1)] rounded cursor-pointer text-[11px] font-bold hover:bg-[#34495e] hover:border-accent transition-colors"
          title="Import a Tiled .tmx file (JSON format)"
          @click="handleImportTmx"
        >
          📥 Import TMX
        </button>
        <button
          class="px-3 py-1.5 bg-[#2c3e50] text-text border border-[rgba(255,255,255,0.1)] rounded cursor-pointer text-[11px] font-bold hover:bg-[#34495e] hover:border-accent transition-colors"
          title="Export current map as Tiled .tmx (JSON format)"
          @click="handleExportTmx"
        >
          📤 Export TMX
        </button>
      </div>
    </div>

    <!-- Import warnings -->
    <div
      v-if="importWarnings.length > 0"
      class="bg-[#2c2c15] border border-[#e6b422] rounded p-2 text-[11px]"
    >
      <div class="flex items-center justify-between mb-1">
        <span class="text-warning font-bold">⚠ Import Warnings</span>
        <button
          class="text-text-muted hover:text-text text-[10px] cursor-pointer bg-transparent border-none"
          @click="importWarnings = []"
        >
          ✕
        </button>
      </div>
      <ul class="list-disc list-inside text-text-muted space-y-0.5">
        <li v-for="(w, i) in importWarnings" :key="i">{{ w }}</li>
      </ul>
    </div>
  </div>
</template>
