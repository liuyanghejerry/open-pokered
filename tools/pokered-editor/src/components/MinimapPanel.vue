<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'
import { TOWN_MAP_COORDS, type TownMapCoord } from '../types/constants'
import NewMapDialog from './NewMapDialog.vue'
import { useStaticMode } from '../composables/useStaticMode'

const store = useMapStore()
const { currentMap, extraTownMapEntries } = storeToRefs(store)
const staticMode = useStaticMode()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const hoveredLocation = ref<TownMapCoord | null>(null)
const tooltipPos = ref({ x: 0, y: 0 })
const placementMode = ref(false)
const dialogOpen = ref(false)
const pendingX = ref<number | undefined>(undefined)
const pendingY = ref<number | undefined>(undefined)

const allCoords = computed<TownMapCoord[]>(() => {
  // Merge built-in town-map coords with user-added extras. When an extra
  // shares the same mapName as a built-in entry, the extra replaces it.
  const extraNames = new Set(extraTownMapEntries.value.map((e) => e.mapName))
  const baseFiltered = TOWN_MAP_COORDS.filter((c) => !extraNames.has(c.mapName))
  const extras: TownMapCoord[] = extraTownMapEntries.value.map((e, i) => ({
    mapId: 0x100 + i,
    mapName: e.mapName,
    x: e.x,
    y: e.y,
    displayName: e.displayName,
  }))
  return [...baseFiltered, ...extras]
})

const CELL_SIZE = 16
const MAP_WIDTH = 16
const MAP_HEIGHT = 16
const PADDING = 4

// Kanto region terrain map (16x16 grid)
// 0 = water, 1 = land, 2 = forest, 3 = mountains
const KANTO_TERRAIN: number[][] = [
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0],
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
  [1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0],
  [1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 1],
  [1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1],
  [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
  [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
  [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
  [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
  [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0],
  [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
  [0, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1],
  [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0],
  [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
  [0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
]

function getCurrentMapCoord(): TownMapCoord | undefined {
  if (!currentMap.value) return undefined
  return allCoords.value.find(c => c.mapName === currentMap.value?.name)
}

function getMapColor(coord: TownMapCoord, isCurrentMap: boolean): string {
  if (isCurrentMap) return '#e74c3c'
  // User-added maps in cyan
  if (extraTownMapEntries.value.some((e) => e.mapName === coord.mapName)) return '#1abc9c'
  if (coord.mapName.includes('City') || coord.mapName.includes('Town')) return '#f1c40f'
  if (coord.mapName.includes('Route')) return '#2ecc71'
  if (coord.mapName === 'IndigoPlateau') return '#9b59b6'
  return '#3498db'
}

function render() {
  const canvas = canvasRef.value
  if (!canvas) return

  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const width = MAP_WIDTH * CELL_SIZE + PADDING * 2
  const height = MAP_HEIGHT * CELL_SIZE + PADDING * 2
  canvas.width = width
  canvas.height = height

  // Background
  ctx.fillStyle = '#0a1628'
  ctx.fillRect(0, 0, width, height)

  // Draw terrain
  for (let y = 0; y < MAP_HEIGHT; y++) {
    for (let x = 0; x < MAP_WIDTH; x++) {
      const terrain = KANTO_TERRAIN[y]?.[x] ?? 0
      const px = PADDING + x * CELL_SIZE
      const py = PADDING + y * CELL_SIZE

      if (terrain === 0) {
        // Water
        ctx.fillStyle = '#1a3a5c'
      } else if (terrain === 1) {
        // Land
        ctx.fillStyle = '#2d5a3d'
      } else if (terrain === 2) {
        // Forest/mountain
        ctx.fillStyle = '#1e4a2e'
      } else {
        ctx.fillStyle = '#3a6a4d'
      }
      ctx.fillRect(px, py, CELL_SIZE, CELL_SIZE)
    }
  }

  // Draw terrain border lines
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)'
  ctx.lineWidth = 0.5
  for (let y = 0; y < MAP_HEIGHT; y++) {
    for (let x = 0; x < MAP_WIDTH; x++) {
      const terrain = KANTO_TERRAIN[y]?.[x] ?? 0
      const px = PADDING + x * CELL_SIZE
      const py = PADDING + y * CELL_SIZE

      // Check neighbors and draw borders
      if (x > 0 && KANTO_TERRAIN[y]?.[x - 1] !== terrain) {
        ctx.beginPath()
        ctx.moveTo(px, py)
        ctx.lineTo(px, py + CELL_SIZE)
        ctx.stroke()
      }
      if (y > 0 && KANTO_TERRAIN[y - 1]?.[x] !== terrain) {
        ctx.beginPath()
        ctx.moveTo(px, py)
        ctx.lineTo(px + CELL_SIZE, py)
        ctx.stroke()
      }
    }
  }

  const currentCoord = getCurrentMapCoord()

  // Draw location markers
  allCoords.value.forEach(coord => {
    if (coord.mapName === 'UnusedMap0B') return

    const isCurrent = currentCoord?.mapName === coord.mapName
    const px = PADDING + coord.x * CELL_SIZE + CELL_SIZE / 2
    const py = PADDING + coord.y * CELL_SIZE + CELL_SIZE / 2
    const radius = isCurrent ? 7 : 5

    ctx.beginPath()
    ctx.arc(px, py, radius, 0, Math.PI * 2)
    ctx.fillStyle = getMapColor(coord, isCurrent)
    ctx.fill()

    if (isCurrent) {
      ctx.strokeStyle = '#fff'
      ctx.lineWidth = 2
      ctx.stroke()
    }
  })

  // Draw pulse ring around current location
  if (currentCoord && currentCoord.mapName !== 'UnusedMap0B') {
    const px = PADDING + currentCoord.x * CELL_SIZE + CELL_SIZE / 2
    const py = PADDING + currentCoord.y * CELL_SIZE + CELL_SIZE / 2
    ctx.beginPath()
    ctx.arc(px, py, 10, 0, Math.PI * 2)
    ctx.strokeStyle = 'rgba(231, 76, 60, 0.5)'
    ctx.lineWidth = 2
    ctx.stroke()
  }
}

function handleMouseMove(e: MouseEvent) {
  const canvas = canvasRef.value
  if (!canvas) return

  const rect = canvas.getBoundingClientRect()
  const x = e.clientX - rect.left - PADDING
  const y = e.clientY - rect.top - PADDING

  const gridX = Math.floor(x / CELL_SIZE)
  const gridY = Math.floor(y / CELL_SIZE)

  const coord = allCoords.value.find(c => c.x === gridX && c.y === gridY && c.mapName !== 'UnusedMap0B')
  hoveredLocation.value = coord || null
  tooltipPos.value = { x: e.clientX, y: e.clientY }
}

function handleMouseLeave() {
  hoveredLocation.value = null
}

function handleClick(e: MouseEvent) {
  if (placementMode.value) {
    const canvas = canvasRef.value
    if (!canvas) return
    const rect = canvas.getBoundingClientRect()
    const x = e.clientX - rect.left - PADDING
    const y = e.clientY - rect.top - PADDING
    const gridX = Math.max(0, Math.min(15, Math.floor(x / CELL_SIZE)))
    const gridY = Math.max(0, Math.min(15, Math.floor(y / CELL_SIZE)))
    pendingX.value = gridX
    pendingY.value = gridY
    placementMode.value = false
    dialogOpen.value = true
    return
  }

  if (!hoveredLocation.value) return

  const mapIndex = store.maps.findIndex(m => m.name === hoveredLocation.value?.mapName)
  if (mapIndex >= 0) {
    store.selectMap(mapIndex)
  }
}

function startAddMap() {
  pendingX.value = undefined
  pendingY.value = undefined
  placementMode.value = true
}

function openDialogDirect() {
  pendingX.value = undefined
  pendingY.value = undefined
  placementMode.value = false
  dialogOpen.value = true
}

function onDialogClose() {
  dialogOpen.value = false
  placementMode.value = false
}

onMounted(() => {
  render()
})

onUnmounted(() => {
})

watch([currentMap, extraTownMapEntries], () => {
  render()
}, { deep: true })
</script>

<template>
  <div class="bg-bg-inset p-2.5 rounded-md">
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-accent text-[13px] font-bold font-sans">World Map</h3>
      <div class="flex gap-1">
        <button
          v-if="!placementMode"
          class="px-2 py-0.5 text-[10px] rounded border border-accent text-accent hover:bg-accent/10 disabled:opacity-30 disabled:cursor-not-allowed"
          title="Click an empty cell on the world map to place a new map"
          :disabled="staticMode"
          @click="startAddMap"
        >
          + Place
        </button>
        <button
          v-else
          class="px-2 py-0.5 text-[10px] rounded border border-warning text-warning"
          @click="placementMode = false"
        >
          Cancel
        </button>
        <button
          class="px-2 py-0.5 text-[10px] rounded border border-accent text-accent hover:bg-accent/10 disabled:opacity-30 disabled:cursor-not-allowed"
          :title="staticMode ? 'New maps need the local backend (npm run dev)' : 'Create a new map without placing it on the world map'"
          :disabled="staticMode"
          @click="openDialogDirect"
        >
          + New Map
        </button>
      </div>
    </div>
    <div v-if="placementMode" class="mb-1.5 text-[10px] text-warning font-bold">
      Click an empty cell on the world map to place the new map…
    </div>
    <div class="relative">
      <canvas
        ref="canvasRef"
        class="block border border-accent/30 rounded"
        :class="placementMode ? 'cursor-crosshair' : 'cursor-pointer'"
        @mousemove="handleMouseMove"
        @mouseleave="handleMouseLeave"
        @click="handleClick"
      ></canvas>
      
      <Teleport to="body">
        <div
          v-if="hoveredLocation"
          class="fixed bg-bg-panel/95 px-2 py-1 rounded border border-accent text-[11px] font-mono pointer-events-none z-50"
          :style="{ left: tooltipPos.x + 10 + 'px', top: tooltipPos.y + 10 + 'px' }"
        >
          <b>{{ hoveredLocation.displayName }}</b>
        </div>
      </Teleport>
    </div>
    
    <div class="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[10px]">
      <div class="flex items-center gap-1">
        <div class="w-2.5 h-2.5 rounded-full bg-[#f1c40f]"></div>
        <span>City</span>
      </div>
      <div class="flex items-center gap-1">
        <div class="w-2.5 h-2.5 rounded-full bg-[#2ecc71]"></div>
        <span>Route</span>
      </div>
      <div class="flex items-center gap-1">
        <div class="w-2.5 h-2.5 rounded-full bg-[#1abc9c]"></div>
        <span>Custom</span>
      </div>
      <div class="flex items-center gap-1">
        <div class="w-2.5 h-2.5 rounded-full bg-[#e74c3c] border border-white"></div>
        <span>Current</span>
      </div>
    </div>

    <NewMapDialog
      :open="dialogOpen"
      :initial-x="pendingX"
      :initial-y="pendingY"
      @close="onDialogClose"
    />
  </div>
</template>