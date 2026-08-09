<script setup lang="ts">
import { ref, reactive } from 'vue'
import PartyEditor from './PartyEditor.vue'
import FlagEditor from './FlagEditor.vue'
import SaveItemsEditor from './SaveItemsEditor.vue'
import {
  type SaveDataSnapshot,
  type PokemonEntry,
  type ItemEntry,
  type PlayerInfo,
  type Facing,
  createDefaultSaveData,
  BADGE_NAMES,
  FACING_DIRECTIONS,
  MAP_NAMES,
} from '../types/save-data'

type SaveTab = 'info' | 'party' | 'flags' | 'items'

const activeTab = ref<SaveTab>('info')
const saveData = reactive<SaveDataSnapshot>(createDefaultSaveData())

function updatePlayer<K extends keyof PlayerInfo>(key: K, value: PlayerInfo[K]) {
  saveData.player = { ...saveData.player, [key]: value }
}

function toggleBadge(index: number) {
  const badges = [...saveData.badges]
  badges[index] = !badges[index]
  saveData.badges = badges
}

function importJson() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = '.json'
  input.onchange = () => {
    const file = input.files?.[0]
    if (!file) return
    const reader = new FileReader()
    reader.onload = () => {
      try {
        const data = JSON.parse(reader.result as string)
        if (data) {
          saveData.player = data.player ?? saveData.player
          saveData.badges = data.badges ?? saveData.badges
          saveData.party = data.party ?? saveData.party
          saveData.items = data.items ?? saveData.items
          saveData.flags = data.flags ?? saveData.flags
        }
      } catch (e) {
        alert('Failed to parse JSON: ' + (e as Error).message)
      }
    }
    reader.readAsText(file)
  }
  input.click()
}

function exportJson() {
  const json = JSON.stringify(saveData, null, 2)
  const blob = new Blob([json], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'save-data.json'
  a.click()
  URL.revokeObjectURL(url)
}

function exportJsonPrint() {
  const json = JSON.stringify(saveData, null, 2)
  // Output to console for copy-paste
  console.log(json)
}

const tabs: { id: SaveTab; label: string; icon: string }[] = [
  { id: 'info', label: 'Info', icon: 'ℹ' },
  { id: 'party', label: 'Party', icon: '⚔' },
  { id: 'flags', label: 'Flags', icon: '🏳' },
  { id: 'items', label: 'Items', icon: '🎒' },
]
</script>

<template>
  <div class="save-editor flex flex-col h-full">
    <!-- Header -->
    <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
      <h2 class="text-accent text-sm font-bold">Save Editor</h2>
      <div class="flex gap-2">
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent"
          @click="importJson()"
        >
          📂 Import
        </button>
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover"
          @click="exportJson()"
        >
          💾 Export
        </button>
      </div>
    </div>

    <!-- Sub-tabs -->
    <div class="sub-tab-bar shrink-0">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="sub-tab-btn"
        :class="{ active: activeTab === tab.id }"
        @click="activeTab = tab.id"
      >
        <span class="tab-icon">{{ tab.icon }}</span>
        {{ tab.label }}
      </button>
    </div>

    <!-- Content area -->
    <div class="flex-1 overflow-y-auto p-3 min-h-0">
      <!-- Info Tab -->
      <div v-show="activeTab === 'info'" class="space-y-4">
        <h3 class="text-accent text-[13px] font-bold">Player Info</h3>
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Player Name</label>
            <input
              type="text"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs font-mono"
              :value="saveData.player.playerName"
              maxlength="11"
              @change="updatePlayer('playerName', ($event.target as HTMLInputElement).value)"
            />
          </div>
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Rival Name</label>
            <input
              type="text"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs font-mono"
              :value="saveData.player.rivalName"
              maxlength="11"
              @change="updatePlayer('rivalName', ($event.target as HTMLInputElement).value)"
            />
          </div>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Map</label>
            <select
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              :value="saveData.player.mapName"
              @change="updatePlayer('mapName', ($event.target as HTMLSelectElement).value)"
            >
              <option v-for="m in MAP_NAMES" :key="m" :value="m">{{ m }}</option>
            </select>
          </div>
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Facing</label>
            <select
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              :value="saveData.player.facing"
              @change="updatePlayer('facing', ($event.target as HTMLSelectElement).value as Facing)"
            >
              <option v-for="d in FACING_DIRECTIONS" :key="d" :value="d">{{ d }}</option>
            </select>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Position X</label>
            <input
              type="number"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              min="0"
              max="255"
              :value="saveData.player.positionX"
              @change="updatePlayer('positionX', parseInt(($event.target as HTMLInputElement).value, 10))"
            />
          </div>
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Position Y</label>
            <input
              type="number"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              min="0"
              max="255"
              :value="saveData.player.positionY"
              @change="updatePlayer('positionY', parseInt(($event.target as HTMLInputElement).value, 10))"
            />
          </div>
        </div>

        <div class="grid grid-cols-3 gap-3">
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Play Time (Hours)</label>
            <input
              type="number"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              min="0"
              max="255"
              :value="saveData.player.playTimeHours"
              @change="updatePlayer('playTimeHours', parseInt(($event.target as HTMLInputElement).value, 10))"
            />
          </div>
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Play Time (Minutes)</label>
            <input
              type="number"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              min="0"
              max="59"
              :value="saveData.player.playTimeMinutes"
              @change="updatePlayer('playTimeMinutes', parseInt(($event.target as HTMLInputElement).value, 10))"
            />
          </div>
          <div>
            <label class="block text-[10px] text-text-muted mb-0.5">Money (₽)</label>
            <input
              type="number"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              min="0"
              max="999999"
              :value="saveData.player.money"
              @change="updatePlayer('money', parseInt(($event.target as HTMLInputElement).value, 10))"
            />
          </div>
        </div>

        <!-- Badges -->
        <div>
          <h3 class="text-accent text-[13px] font-bold mb-2">Badges</h3>
          <div class="bg-bg-inset p-2 rounded">
            <div class="flex flex-wrap gap-2">
              <label
                v-for="(badge, idx) in BADGE_NAMES"
                :key="idx"
                class="flex items-center gap-1.5 cursor-pointer"
              >
                <input
                  type="checkbox"
                  :checked="saveData.badges[idx]"
                  class="accent-accent"
                  @change="toggleBadge(idx)"
                />
                <span
                  class="text-[11px]"
                  :class="saveData.badges[idx] ? 'text-accent' : 'text-text-muted'"
                >
                  🏅 {{ badge }}
                </span>
              </label>
            </div>
          </div>
        </div>

        <!-- Danger zone -->
        <div class="pt-3 border-t border-[rgba(255,255,255,0.06)]">
          <p class="text-[10px] text-text-muted mb-2">Debug: Print JSON to console for copy-paste</p>
          <button
            class="px-3 py-1.5 rounded text-[11px] font-bold cursor-pointer bg-danger text-white border-none hover:opacity-90"
            @click="exportJsonPrint()"
          >
            🖨 Print to Console
          </button>
        </div>
      </div>

      <!-- Party Tab -->
      <div v-show="activeTab === 'party'" class="h-full">
        <PartyEditor
          :party="saveData.party"
          @update:party="(p: PokemonEntry[]) => (saveData.party = p)"
        />
      </div>

      <!-- Flags Tab -->
      <div v-show="activeTab === 'flags'" class="h-full">
        <FlagEditor
          :flags="saveData.flags"
          @update:flags="(f: Record<string, boolean>) => (saveData.flags = f)"
        />
      </div>

      <!-- Items Tab -->
      <div v-show="activeTab === 'items'" class="h-full">
        <SaveItemsEditor
          :items="saveData.items"
          @update:items="(i: ItemEntry[]) => (saveData.items = i)"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.sub-tab-bar {
  display: flex;
  align-items: center;
  gap: 0;
  background: var(--color-bg);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  padding: 0 8px;
  flex-shrink: 0;
}

.sub-tab-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 6px 14px;
  font-size: 11px;
  font-weight: 600;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: color 0.15s, border-color 0.15s, background 0.15s;
}

.sub-tab-btn:hover {
  color: var(--color-text);
  background: rgba(255, 255, 255, 0.03);
}

.sub-tab-btn.active {
  color: var(--color-accent);
  border-bottom-color: var(--color-accent);
  background: rgba(78, 204, 163, 0.05);
}
</style>
