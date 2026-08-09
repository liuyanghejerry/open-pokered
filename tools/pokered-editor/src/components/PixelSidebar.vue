<script setup lang="ts">
import { ref, computed } from 'vue'
import { usePixelStore } from '../stores/pixelStore'
import { storeToRefs } from 'pinia'
import {
  getPokemonFrontAssets,
  getPokemonBackAssets,
  getTrainerAssets,
  getNpcAssets,
  getTilesetAssets,
  getUiAssets,
  getEffectsAssets,
  type AssetEntry,
  type AssetCategory,
} from '../types/pixel'
import TileGrid from './TileGrid.vue'
import { gfxUrl } from '../utils/assetUrl'

type TabId = 'pokemon' | 'tilesets' | 'trainers' | 'overworld' | 'ui' | 'effects'

const store = usePixelStore()
const { activeAsset, loading, frames, activeFrame } = storeToRefs(store)

const activeTab = ref<TabId>('pokemon')
const searchText = ref('')
const selectedTileset = ref<AssetEntry | null>(null)

const tabs: { id: TabId; label: string }[] = [
  { id: 'pokemon', label: 'Pokemon' },
  { id: 'tilesets', label: 'Tilesets' },
  { id: 'trainers', label: 'Trainers' },
  { id: 'overworld', label: 'Overworld' },
  { id: 'ui', label: 'UI' },
  { id: 'effects', label: 'Effects' },
]

const allAssets = computed<AssetEntry[]>(() => {
  switch (activeTab.value) {
    case 'pokemon':
      return [...getPokemonFrontAssets(), ...getPokemonBackAssets()]
    case 'tilesets':
      return getTilesetAssets()
    case 'trainers':
      return getTrainerAssets()
    case 'overworld':
      return getNpcAssets()
    case 'ui':
      return getUiAssets()
    case 'effects':
      return getEffectsAssets()
  }
})

const filteredAssets = computed<AssetEntry[]>(() => {
  const q = searchText.value.toLowerCase().trim()
  if (!q) return allAssets.value
  return allAssets.value.filter(
    (a) =>
      a.displayName.toLowerCase().includes(q) ||
      a.id.toLowerCase().includes(q),
  )
})

function selectAsset(entry: AssetEntry) {
  if (entry.category === 'tileset') {
    selectedTileset.value = entry
    return
  }
  selectedTileset.value = null
  store.loadAsset(entry)
}

function switchFrame(index: number) {
  store.switchFrame(index)
}

function switchTab(tab: TabId) {
  activeTab.value = tab
  searchText.value = ''
  selectedTileset.value = null
}

function getAssetUrl(entry: AssetEntry): string {
  const cat: AssetCategory = entry.category
  switch (cat) {
    case 'pokemon-front':
      return gfxUrl(`pokemon/front/${entry.filename}`)
    case 'pokemon-back':
      return gfxUrl(`pokemon/back/${entry.filename}`)
    case 'trainer':
      return gfxUrl(`trainers/${entry.filename}`)
    case 'npc':
      return gfxUrl(`sprites/${entry.filename}`)
    case 'tileset':
      return gfxUrl(`tilesets/${entry.filename}`)
    case 'ui':
    case 'effects':
      return gfxUrl(entry.filename)
  }
}

function subtitle(entry: AssetEntry): string {
  switch (entry.category) {
    case 'pokemon-front':
      return 'Front'
    case 'pokemon-back':
      return 'Back'
    case 'tileset':
      return entry.tileCount != null ? `${entry.tileCount} tiles` : ''
    case 'ui':
    case 'effects':
      return entry.filename.split('/')[0]
    default:
      return entry.category
  }
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- Category Tabs -->
    <div class="flex border-b border-[rgba(255,255,255,0.06)]">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="flex-1 py-2 text-[11px] font-bold cursor-pointer border-l-2 transition-colors"
        :class="
          activeTab === tab.id
            ? 'text-accent border-accent bg-accent/5'
            : 'text-text-muted border-transparent hover:text-text hover:bg-[rgba(255,255,255,0.04)]'
        "
        @click="switchTab(tab.id)"
      >
        {{ tab.label }}
      </button>
    </div>

    <!-- Search -->
    <div class="p-2">
      <input
        v-model="searchText"
        type="text"
        placeholder="Search assets..."
        class="w-full px-2 py-1 rounded border border-[rgba(255,255,255,0.1)] bg-bg-inset text-text text-[11px] font-mono placeholder:text-text-muted/50 focus:outline-none focus:border-accent"
      />
    </div>

    <!-- Asset List -->
    <div class="flex-1 overflow-y-auto">
      <div
        v-if="filteredAssets.length === 0"
        class="p-4 text-center text-[11px] text-text-muted"
      >
        No assets found
      </div>
      <button
        v-for="entry in filteredAssets"
        :key="entry.category + '/' + entry.id"
        class="w-full flex items-center gap-2 px-3 py-1.5 text-left cursor-pointer transition-colors hover:bg-[rgba(255,255,255,0.04)] border-l-2"
        :class="
          activeAsset?.id === entry.id &&
          activeAsset?.category === entry.category
            ? 'bg-accent/5 border-accent'
            : 'border-transparent'
        "
        @click="selectAsset(entry)"
        :disabled="loading"
      >
        <img
          :src="getAssetUrl(entry)"
          class="w-8 h-8 rounded flex-shrink-0 object-contain bg-bg"
          style="image-rendering: pixelated"
          loading="lazy"
          alt=""
        />
        <div class="min-w-0 flex-1">
          <div class="text-[11px] text-text truncate">
            {{ entry.displayName }}
          </div>
          <div class="text-[9px] text-text-muted truncate">
            {{ subtitle(entry) }}
          </div>
        </div>
        <div
          v-if="loading && activeAsset?.id === entry.id"
          class="text-[9px] text-accent animate-pulse"
        >
          Loading…
        </div>
      </button>
    </div>

    <!-- Frame Switcher (visible when Pokemon frames exist) -->
    <div v-if="activeTab === 'pokemon' && frames.length > 1" class="border-t border-[rgba(255,255,255,0.06)] p-2">
      <div class="text-[10px] text-text-muted mb-2 font-bold">Frames</div>
      <div class="flex gap-2">
        <button
          v-for="(frame, idx) in frames"
          :key="idx"
          class="flex flex-col items-center gap-1 px-2 py-1.5 rounded cursor-pointer transition-colors border"
          :class="activeFrame === idx ? 'border-accent bg-accent/10' : 'border-[rgba(255,255,255,0.1)] hover:border-[rgba(255,255,255,0.3)]'"
          @click="switchFrame(idx)"
        >
          <img
            :src="getAssetUrl(frame)"
            class="w-12 h-12 rounded object-contain bg-bg"
            style="image-rendering: pixelated"
            alt=""
          />
          <span class="text-[9px]" :class="activeFrame === idx ? 'text-accent' : 'text-text-muted'">
            {{ idx === 0 ? 'Front' : 'Back' }}
          </span>
      </button>
    </div>
    </div>

    <!-- Tileset Tile Grid -->
    <div v-if="selectedTileset">
      <div class="flex items-center gap-2 px-2 pt-2">
        <button
          class="text-[10px] text-accent hover:text-accent/80 cursor-pointer transition-colors"
          @click="selectedTileset = null"
        >
          &larr; Back to tilesets
        </button>
        <span class="text-[10px] text-text-muted">
          {{ selectedTileset.displayName }}
        </span>
      </div>
      <TileGrid :entry="selectedTileset" />
    </div>
  </div>
</template>
