<script setup lang="ts">
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'
import MapInfoPanel from './MapInfoPanel.vue'
import MapHeaderEditor from './MapHeaderEditor.vue'
import WildEncountersEditor from './WildEncountersEditor.vue'
import MinimapPanel from './MinimapPanel.vue'
import BlockPalette from './BlockPalette.vue'
import SearchableSelect from './SearchableSelect.vue'

const store = useMapStore()
const {
  filteredMaps,
  currentMapIndex,
  searchQuery,
  displayOptions,
  hasUnsavedChanges,
  canGoBack,
  loading,
  currentPassableTiles,
  scriptEditorOpen,
} = storeToRefs(store)
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-4">Map Editor</h2>

    <div v-if="loading" class="text-text-muted text-xs mb-3">Loading...</div>

    <button
      v-if="canGoBack"
      class="w-full mb-3 px-3 py-1.5 bg-[#e67e22] text-white border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#d35400]"
      @click="store.goBack()"
    >
      ← Back
    </button>

    <!-- World Map (always visible) -->
    <div class="mb-3">
      <div class="text-xs mb-1 font-bold">World Map</div>
      <MinimapPanel />
    </div>

    <!-- Map selector -->
    <label class="block text-xs mb-1">Map:</label>
    <SearchableSelect
      :options="
        filteredMaps.map(({ map, index }) => ({ name: map.name, value: index }))
      "
      :model-value="currentMapIndex"
      @update:model-value="store.selectMap($event)"
    />

    <!-- MapInfo (always visible) -->
    <div class="border-t border-[rgba(255,255,255,0.06)] my-3"></div>
    <div class="mb-3">
      <MapInfoPanel />
    </div>
    <div class="border-b border-[rgba(255,255,255,0.06)] mb-3"></div>

    <!-- Search -->
    <label class="block text-xs mb-1 mt-3">Search:</label>
    <input
      v-model="searchQuery"
      type="text"
      placeholder="Filter maps..."
      class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
    />

    <!-- Display Options accordion -->
    <details class="mt-3 group" open>
      <summary
        class="text-accent text-[13px] font-bold cursor-pointer select-none hover:text-accent-hover"
      >
        Display Options
      </summary>
      <div class="mt-2 space-y-1">
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showTiles" type="checkbox" class="w-auto" /> Show Tiles
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showCollision" type="checkbox" class="w-auto" /> Show Collision
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showWarps" type="checkbox" class="w-auto" /> Show Warps
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showSigns" type="checkbox" class="w-auto" /> Show Signs
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showNpcs" type="checkbox" class="w-auto" /> Show NPCs
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showCoordEvents" type="checkbox" class="w-auto" /> Show Coord Events
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showConnections" type="checkbox" class="w-auto" /> Show Connections
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-xs">
          <input v-model="displayOptions.showGrid" type="checkbox" class="w-auto" /> Show Grid
        </label>
      </div>
    </details>

    <!-- Action buttons -->
    <div class="flex gap-1.5 flex-wrap mt-3">
      <button
        class="px-3 py-1.5 bg-[#27ae60] text-white border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#229954]"
        :disabled="!hasUnsavedChanges"
        :class="!hasUnsavedChanges ? 'opacity-50 cursor-not-allowed' : ''"
        @click="store.saveCurrentMap()"
      >
        Save
      </button>
      <button
        class="px-3 py-1.5 border-none rounded cursor-pointer text-[11px] font-bold"
        :class="
          scriptEditorOpen
            ? 'bg-accent text-bg-panel hover:opacity-85'
            : 'bg-[#2c3e50] text-text hover:bg-[#34495e]'
        "
        @click="scriptEditorOpen ? store.closeScriptEditor() : store.openScriptEditor()"
      >
        {{ scriptEditorOpen ? '🗺 Map' : '{ } Script' }}
      </button>
      <button
        class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#444]"
        @click="store.prevMap()"
      >
        ◀
      </button>
      <button
        class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#444]"
        @click="store.nextMap()"
      >
        ▶
      </button>
    </div>

    <div v-if="hasUnsavedChanges" class="mt-2 text-warning text-[11px] font-bold">
      *Unsaved Changes*
    </div>

    <!-- Accordion sections -->
    <details class="mt-4 group" open>
      <summary
        class="text-accent text-[13px] font-bold cursor-pointer select-none hover:text-accent-hover"
      >
        Block Palette
      </summary>
      <div class="mt-2">
        <BlockPalette />
      </div>
    </details>

    <details class="mt-4 group">
      <summary
        class="text-accent text-[13px] font-bold cursor-pointer select-none hover:text-accent-hover"
      >
        Passable Tiles
      </summary>
      <div class="mt-2 bg-bg-inset p-2.5 rounded-md">
        <div
          v-if="currentPassableTiles.length > 0"
          class="max-h-[150px] overflow-y-auto font-mono text-[10px] space-y-0.5"
        >
          <div
            v-for="tileId in currentPassableTiles"
            :key="tileId"
            class="flex items-center gap-1.5 p-0.5 hover:bg-bg"
          >
            <span class="w-[30px]">0x{{ tileId.toString(16).padStart(2, '0') }}</span>
            <span class="text-accent">Passable</span>
          </div>
        </div>
        <p v-else class="text-[10px] text-text-muted">No tileset loaded</p>
      </div>
    </details>

    <details class="mt-4 group">
      <summary
        class="text-accent text-[13px] font-bold cursor-pointer select-none hover:text-accent-hover"
      >
        Map Header
      </summary>
      <div class="mt-2">
        <MapHeaderEditor />
      </div>
    </details>

    <details class="mt-4 group">
      <summary
        class="text-accent text-[13px] font-bold cursor-pointer select-none hover:text-accent-hover"
      >
        Wild Encounters
      </summary>
      <div class="mt-2">
        <WildEncountersEditor />
      </div>
    </details>
  </div>
</template>
