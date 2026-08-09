<script setup lang="ts">
import { ref, computed } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'
import { MUSIC_LIST } from '../types/constants'
import type { ConnectionEntry } from '../types'

const store = useMapStore()
const { maps } = storeToRefs(store)
const currentMap = computed(() => store.currentMap)

const editingDirection = ref<'north' | 'south' | 'west' | 'east' | null>(null)
const tempTargetMap = ref('')
const tempOffset = ref(0)

function startEditDirection(dir: 'north' | 'south' | 'west' | 'east') {
  const existing = currentMap.value?.connections?.[dir]
  editingDirection.value = dir
  tempTargetMap.value = existing?.targetMap ?? ''
  tempOffset.value = existing?.offset ?? 0
}

function cancelEdit() {
  editingDirection.value = null
}

function saveConnection() {
  if (!editingDirection.value) return
  
  if (tempTargetMap.value) {
    const entry: ConnectionEntry = {
      targetMap: tempTargetMap.value,
      offset: tempOffset.value,
    }
    store.updateMapConnection(editingDirection.value, entry)
  } else {
    store.updateMapConnection(editingDirection.value, null)
  }
  
  editingDirection.value = null
}

function removeConnection(dir: 'north' | 'south' | 'west' | 'east') {
  store.updateMapConnection(dir, null)
}

function navigateToMap(mapName: string) {
  if (mapName) {
    store.navigateToMap(mapName)
  }
}

const mapNames = computed(() => maps.value.map(m => m.name))
</script>

<template>
  <div class="bg-bg-inset p-2.5 rounded-md font-mono text-[11px]">
    <h3 class="text-accent text-[13px] font-bold mb-2 font-sans">Map Header Editor</h3>
    
    <template v-if="currentMap">
      <!-- Music Selection -->
      <div class="mb-3">
        <label class="block text-xs mb-1">BGM Music:</label>
        <select
          class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
          :value="currentMap?.header.music"
          @change="(e) => store.updateMapMusic((e.target as HTMLSelectElement).value)"
        >
          <option v-for="music in MUSIC_LIST" :key="music" :value="music">
            {{ music }}
          </option>
        </select>
      </div>

      <!-- Connections Editor -->
      <div class="mb-2">
        <label class="block text-xs mb-1 font-bold">Connections:</label>
        
        <!-- North -->
        <div class="my-1.5 p-1.5 bg-bg rounded">
          <div class="flex items-center justify-between">
            <span class="text-xs font-bold">⬆ North</span>
            <div v-if="!editingDirection || editingDirection !== 'north'">
              <button
                v-if="currentMap.connections.north"
                class="px-1.5 py-0.5 text-[10px] bg-[#2c3e50] text-text rounded hover:bg-[#34495e]"
                @click="navigateToMap(currentMap.connections.north!.targetMap)"
              >
                Go
              </button>
              <button
                class="px-1.5 py-0.5 text-[10px] bg-[#3498db] text-white rounded hover:bg-[#2980b9] ml-1"
                @click="startEditDirection('north')"
              >
                Edit
              </button>
              <button
                v-if="currentMap.connections.north"
                class="px-1.5 py-0.5 text-[10px] bg-[#e74c3c] text-white rounded hover:bg-[#c0392b] ml-1"
                @click="removeConnection('north')"
              >
                ✕
              </button>
            </div>
          </div>
          <div v-if="currentMap.connections.north && (!editingDirection || editingDirection !== 'north')" class="mt-1 text-text-muted">
            → {{ currentMap.connections.north.targetMap }} (offset: {{ currentMap.connections.north.offset }})
          </div>
          <div v-if="editingDirection === 'north'" class="mt-2 space-y-1">
            <select
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model="tempTargetMap"
            >
              <option value="">-- None --</option>
              <option v-for="name in mapNames" :key="name" :value="name">{{ name }}</option>
            </select>
            <input
              type="number"
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model.number="tempOffset"
              placeholder="Offset"
              min="-20"
              max="20"
            />
            <div class="flex gap-1">
              <button class="px-2 py-0.5 text-[10px] bg-[#27ae60] text-white rounded hover:bg-[#229954]" @click="saveConnection">Save</button>
              <button class="px-2 py-0.5 text-[10px] bg-[#7f8c8d] text-white rounded hover:bg-[#95a5a6]" @click="cancelEdit">Cancel</button>
            </div>
          </div>
        </div>

        <!-- South -->
        <div class="my-1.5 p-1.5 bg-bg rounded">
          <div class="flex items-center justify-between">
            <span class="text-xs font-bold">⬇ South</span>
            <div v-if="!editingDirection || editingDirection !== 'south'">
              <button
                v-if="currentMap.connections.south"
                class="px-1.5 py-0.5 text-[10px] bg-[#2c3e50] text-text rounded hover:bg-[#34495e]"
                @click="navigateToMap(currentMap.connections.south!.targetMap)"
              >
                Go
              </button>
              <button
                class="px-1.5 py-0.5 text-[10px] bg-[#3498db] text-white rounded hover:bg-[#2980b9] ml-1"
                @click="startEditDirection('south')"
              >
                Edit
              </button>
              <button
                v-if="currentMap.connections.south"
                class="px-1.5 py-0.5 text-[10px] bg-[#e74c3c] text-white rounded hover:bg-[#c0392b] ml-1"
                @click="removeConnection('south')"
              >
                ✕
              </button>
            </div>
          </div>
          <div v-if="currentMap.connections.south && (!editingDirection || editingDirection !== 'south')" class="mt-1 text-text-muted">
            → {{ currentMap.connections.south.targetMap }} (offset: {{ currentMap.connections.south.offset }})
          </div>
          <div v-if="editingDirection === 'south'" class="mt-2 space-y-1">
            <select
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model="tempTargetMap"
            >
              <option value="">-- None --</option>
              <option v-for="name in mapNames" :key="name" :value="name">{{ name }}</option>
            </select>
            <input
              type="number"
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model.number="tempOffset"
              placeholder="Offset"
              min="-20"
              max="20"
            />
            <div class="flex gap-1">
              <button class="px-2 py-0.5 text-[10px] bg-[#27ae60] text-white rounded hover:bg-[#229954]" @click="saveConnection">Save</button>
              <button class="px-2 py-0.5 text-[10px] bg-[#7f8c8d] text-white rounded hover:bg-[#95a5a6]" @click="cancelEdit">Cancel</button>
            </div>
          </div>
        </div>

        <!-- West -->
        <div class="my-1.5 p-1.5 bg-bg rounded">
          <div class="flex items-center justify-between">
            <span class="text-xs font-bold">⬅ West</span>
            <div v-if="!editingDirection || editingDirection !== 'west'">
              <button
                v-if="currentMap.connections.west"
                class="px-1.5 py-0.5 text-[10px] bg-[#2c3e50] text-text rounded hover:bg-[#34495e]"
                @click="navigateToMap(currentMap.connections.west!.targetMap)"
              >
                Go
              </button>
              <button
                class="px-1.5 py-0.5 text-[10px] bg-[#3498db] text-white rounded hover:bg-[#2980b9] ml-1"
                @click="startEditDirection('west')"
              >
                Edit
              </button>
              <button
                v-if="currentMap.connections.west"
                class="px-1.5 py-0.5 text-[10px] bg-[#e74c3c] text-white rounded hover:bg-[#c0392b] ml-1"
                @click="removeConnection('west')"
              >
                ✕
              </button>
            </div>
          </div>
          <div v-if="currentMap.connections.west && (!editingDirection || editingDirection !== 'west')" class="mt-1 text-text-muted">
            → {{ currentMap.connections.west.targetMap }} (offset: {{ currentMap.connections.west.offset }})
          </div>
          <div v-if="editingDirection === 'west'" class="mt-2 space-y-1">
            <select
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model="tempTargetMap"
            >
              <option value="">-- None --</option>
              <option v-for="name in mapNames" :key="name" :value="name">{{ name }}</option>
            </select>
            <input
              type="number"
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model.number="tempOffset"
              placeholder="Offset"
              min="-20"
              max="20"
            />
            <div class="flex gap-1">
              <button class="px-2 py-0.5 text-[10px] bg-[#27ae60] text-white rounded hover:bg-[#229954]" @click="saveConnection">Save</button>
              <button class="px-2 py-0.5 text-[10px] bg-[#7f8c8d] text-white rounded hover:bg-[#95a5a6]" @click="cancelEdit">Cancel</button>
            </div>
          </div>
        </div>

        <!-- East -->
        <div class="my-1.5 p-1.5 bg-bg rounded">
          <div class="flex items-center justify-between">
            <span class="text-xs font-bold">⬅ East</span>
            <div v-if="!editingDirection || editingDirection !== 'east'">
              <button
                v-if="currentMap.connections.east"
                class="px-1.5 py-0.5 text-[10px] bg-[#2c3e50] text-text rounded hover:bg-[#34495e]"
                @click="navigateToMap(currentMap.connections.east!.targetMap)"
              >
                Go
              </button>
              <button
                class="px-1.5 py-0.5 text-[10px] bg-[#3498db] text-white rounded hover:bg-[#2980b9] ml-1"
                @click="startEditDirection('east')"
              >
                Edit
              </button>
              <button
                v-if="currentMap.connections.east"
                class="px-1.5 py-0.5 text-[10px] bg-[#e74c3c] text-white rounded hover:bg-[#c0392b] ml-1"
                @click="removeConnection('east')"
              >
                ✕
              </button>
            </div>
          </div>
          <div v-if="currentMap.connections.east && (!editingDirection || editingDirection !== 'east')" class="mt-1 text-text-muted">
            → {{ currentMap.connections.east.targetMap }} (offset: {{ currentMap.connections.east.offset }})
          </div>
          <div v-if="editingDirection === 'east'" class="mt-2 space-y-1">
            <select
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model="tempTargetMap"
            >
              <option value="">-- None --</option>
              <option v-for="name in mapNames" :key="name" :value="name">{{ name }}</option>
            </select>
            <input
              type="number"
              class="w-full p-1 rounded border border-accent bg-bg text-text text-[10px]"
              v-model.number="tempOffset"
              placeholder="Offset"
              min="-20"
              max="20"
            />
            <div class="flex gap-1">
              <button class="px-2 py-0.5 text-[10px] bg-[#27ae60] text-white rounded hover:bg-[#229954]" @click="saveConnection">Save</button>
              <button class="px-2 py-0.5 text-[10px] bg-[#7f8c8d] text-white rounded hover:bg-[#95a5a6]" @click="cancelEdit">Cancel</button>
            </div>
          </div>
        </div>
      </div>
    </template>
    <template v-else>
      <p>Loading map data...</p>
    </template>
  </div>
</template>