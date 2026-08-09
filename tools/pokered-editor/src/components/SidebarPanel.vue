<script setup lang="ts">
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'
import EntityDetailPanel from './EntityDetailPanel.vue'
import MapInfoPanel from './MapInfoPanel.vue'
import MapHeaderEditor from './MapHeaderEditor.vue'
import WildEncountersEditor from './WildEncountersEditor.vue'
import MinimapPanel from './MinimapPanel.vue'
import BlockPalette from './BlockPalette.vue'

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

function handleMapChange(e: Event) {
  const select = e.target as HTMLSelectElement
  store.selectMap(parseInt(select.value))
}
</script>

<template>
  <aside
    class="w-80 shrink-0 bg-bg-panel p-4 rounded-lg overflow-y-auto"
    style="max-height: calc(100vh - 40px)"
  >
    <h2 class="text-accent text-base font-bold mb-4">Map Editor</h2>

    <div v-if="loading" class="text-text-muted text-xs mb-3">Loading...</div>

    <button
      v-if="canGoBack"
      class="w-full mb-3 px-3 py-1.5 bg-[#e67e22] text-white border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#d35400]"
      @click="store.goBack()"
    >
      ← Back
    </button>

    <label class="block text-xs mb-1 mt-3">Map:</label>
    <select
      class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
      :value="currentMapIndex"
      @change="handleMapChange"
    >
      <option
        v-for="{ map, index } in filteredMaps"
        :key="index"
        :value="index"
      >
        {{ map.name }}
      </option>
    </select>

    <label class="block text-xs mb-1 mt-3">Search:</label>
    <input
      v-model="searchQuery"
      type="text"
      placeholder="Filter maps..."
      class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
    />

    <div class="mt-3 space-y-1">
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showTiles" type="checkbox" class="w-auto" />
        Show Tiles
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showCollision" type="checkbox" class="w-auto" />
        Show Collision
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showWarps" type="checkbox" class="w-auto" />
        Show Warps
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showSigns" type="checkbox" class="w-auto" />
        Show Signs
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showNpcs" type="checkbox" class="w-auto" />
        Show NPCs
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showCoordEvents" type="checkbox" class="w-auto" />
        Show Coord Events
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showConnections" type="checkbox" class="w-auto" />
        Show Connections
      </label>
      <label class="flex items-center gap-1.5 cursor-pointer text-xs">
        <input v-model="displayOptions.showGrid" type="checkbox" class="w-auto" />
        Show Grid
      </label>
    </div>

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
        :class="scriptEditorOpen ? 'bg-accent text-bg-panel hover:opacity-85' : 'bg-[#2c3e50] text-text hover:bg-[#34495e]'"
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

    <div class="mt-4">
      <MinimapPanel />
    </div>

    <div class="mt-4">
      <BlockPalette />
    </div>

    <div class="mt-4 bg-bg-inset p-2.5 rounded-md">
      <h3 class="text-accent text-[13px] font-bold mb-2">Passable Tiles</h3>
      <div
        v-if="currentPassableTiles.length > 0"
        class="max-h-[150px] overflow-y-auto font-mono text-[10px] space-y-0.5"
      >
        <div
          v-for="tileId in currentPassableTiles"
          :key="tileId"
          class="flex items-center gap-1.5 p-0.5 hover:bg-bg"
        >
          <span class="w-[30px]">{{ '0x' + tileId.toString(16).padStart(2, '0') }}</span>
          <span class="text-accent">Passable</span>
        </div>
      </div>
      <p v-else class="text-[10px] text-text-muted">No tileset loaded</p>
    </div>

    <div class="mt-4">
      <EntityDetailPanel />
    </div>

    <div class="mt-4">
      <MapHeaderEditor />
    </div>

    <div class="mt-4">
      <WildEncountersEditor />
    </div>

    <div class="mt-4">
      <MapInfoPanel />
    </div>

    <div class="mt-4 bg-bg-inset p-2.5 rounded-md">
      <h3 class="text-accent text-[13px] font-bold mb-2">Legend</h3>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(78, 204, 163, 0.5)"></div>
        Passable
      </div>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(231, 76, 60, 0.5)"></div>
        Blocked
      </div>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(52, 152, 219, 0.8)"></div>
        Warp
      </div>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(241, 196, 15, 0.8)"></div>
        Sign
      </div>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(231, 76, 60, 0.8)"></div>
        NPC (Trainer)
      </div>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(46, 204, 113, 0.8)"></div>
        NPC (Item)
      </div>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(230, 126, 34, 0.8)"></div>
        Coord Event
      </div>
      <div class="flex items-center gap-2 text-[11px] my-1">
        <div class="w-3.5 h-3.5 rounded-sm" style="background: rgba(155, 89, 182, 0.8)"></div>
        NPC (Regular)
      </div>
    </div>
  </aside>
</template>
