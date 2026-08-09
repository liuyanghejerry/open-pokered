<script setup lang="ts">
import { ref, computed, watch, onMounted, nextTick } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'
import { TILE_SIZE } from '../types/constants'
import BlockEditor from './BlockEditor.vue'
import NewTilesetDialog from './NewTilesetDialog.vue'
import { useStaticMode } from '../composables/useStaticMode'

const store = useMapStore()
const { currentMap, selectedBlockId, currentTool } = storeToRefs(store)
const staticMode = useStaticMode()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const editorOpen = ref(false)
const newTilesetOpen = ref(false)
const PALETTE_COLS = 8
const BLOCK_PX = 32 // 16px source × 2 zoom

const blockset = computed(() => {
  const map = currentMap.value
  if (!map) return undefined
  return store.getBlockset(map.header.tileset)
})

const blockIds = computed(() => {
  const bs = blockset.value
  if (!bs) return []
  return Object.keys(bs.blocks)
    .map((k) => parseInt(k, 10))
    .filter((n) => !Number.isNaN(n))
    .sort((a, b) => a - b)
})

const tilesetImg = computed(() => {
  const map = currentMap.value
  if (!map) return undefined
  return store.tilesetImages[map.header.tileset]
})

function render() {
  const canvas = canvasRef.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const ids = blockIds.value
  const cols = PALETTE_COLS
  const rows = Math.max(1, Math.ceil(ids.length / cols))
  const w = cols * BLOCK_PX
  const h = rows * BLOCK_PX
  canvas.width = w
  canvas.height = h
  canvas.style.width = `${w}px`
  canvas.style.height = `${h}px`

  ctx.imageSmoothingEnabled = false
  ctx.fillStyle = '#000'
  ctx.fillRect(0, 0, w, h)

  const bs = blockset.value
  const img = tilesetImg.value
  if (!bs) return

  const blockNativePx = 16 // 4 tiles × TILE_SIZE
  const scale = BLOCK_PX / blockNativePx

  for (let i = 0; i < ids.length; i++) {
    const id = ids[i]
    const cx = (i % cols) * BLOCK_PX
    const cy = Math.floor(i / cols) * BLOCK_PX
    const tiles = bs.blocks[id]
    if (img && tiles) {
      const tilesPerRow = Math.floor(img.width / TILE_SIZE)
      for (let ty = 0; ty < 4; ty++) {
        for (let tx = 0; tx < 4; tx++) {
          const tileId = tiles[ty * 4 + tx]
          const srcX = (tileId % tilesPerRow) * TILE_SIZE
          const srcY = Math.floor(tileId / tilesPerRow) * TILE_SIZE
          ctx.drawImage(
            img,
            srcX, srcY, TILE_SIZE, TILE_SIZE,
            cx + tx * TILE_SIZE * scale,
            cy + ty * TILE_SIZE * scale,
            TILE_SIZE * scale,
            TILE_SIZE * scale,
          )
        }
      }
    } else {
      ctx.fillStyle = `hsl(${id * 7}, 50%, 40%)`
      ctx.fillRect(cx, cy, BLOCK_PX, BLOCK_PX)
    }
  }

  // Selection highlight
  const selIdx = ids.indexOf(selectedBlockId.value)
  if (selIdx >= 0) {
    const cx = (selIdx % cols) * BLOCK_PX
    const cy = Math.floor(selIdx / cols) * BLOCK_PX
    ctx.strokeStyle = '#fff'
    ctx.lineWidth = 2
    ctx.strokeRect(cx + 1, cy + 1, BLOCK_PX - 2, BLOCK_PX - 2)
    ctx.strokeStyle = 'rgba(78, 204, 163, 1)'
    ctx.lineWidth = 1
    ctx.strokeRect(cx + 0.5, cy + 0.5, BLOCK_PX - 1, BLOCK_PX - 1)
  }
}

function handleClick(e: MouseEvent) {
  const canvas = canvasRef.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const x = Math.floor((e.clientX - rect.left) / BLOCK_PX)
  const y = Math.floor((e.clientY - rect.top) / BLOCK_PX)
  const idx = y * PALETTE_COLS + x
  const ids = blockIds.value
  if (idx < 0 || idx >= ids.length) return
  store.setSelectedBlockId(ids[idx])
  render()
}

function handleDoubleClick(e: MouseEvent) {
  // Click selects, double-click opens the per-block editor.
  handleClick(e)
  if (currentMap.value) editorOpen.value = true
}

function activateTileTool() {
  store.setTool('edit-tiles')
}

function openBlockEditor() {
  if (currentMap.value) editorOpen.value = true
}

onMounted(() => {
  nextTick(() => render())
})

watch([currentMap, blockset, tilesetImg, selectedBlockId], () => {
  nextTick(() => render())
})
</script>

<template>
  <div class="bg-bg-inset p-2.5 rounded-md">
    <div class="flex items-center justify-between mb-2 gap-1.5 flex-wrap">
      <h3 class="text-accent text-[13px] font-bold">Block Palette</h3>
      <div class="flex gap-1">
        <button
          class="px-2 py-0.5 text-[10px] rounded border border-accent"
          :class="currentTool === 'edit-tiles' ? 'bg-accent text-bg-panel font-bold' : 'bg-transparent text-accent hover:bg-accent/10'"
          @click="activateTileTool"
        >
          {{ currentTool === 'edit-tiles' ? '✓ Painting' : 'Paint (T)' }}
        </button>
        <button
          class="px-2 py-0.5 text-[10px] rounded border border-accent text-accent bg-transparent hover:bg-accent/10"
          :disabled="!currentMap"
          @click="openBlockEditor"
        >Edit Block</button>
        <button
          class="px-2 py-0.5 text-[10px] rounded border border-accent text-accent bg-transparent hover:bg-accent/10 disabled:opacity-30 disabled:cursor-not-allowed"
          :title="staticMode ? 'New tilesets need the local backend (npm run dev)' : 'Create a new tileset by cloning an existing one'"
          :disabled="staticMode"
          @click="newTilesetOpen = true"
        >+ New Tileset</button>
      </div>
    </div>
    <div v-if="!blockset" class="text-[10px] text-text-muted">No blockset loaded.</div>
    <div v-else>
      <div class="overflow-auto max-h-[260px] border border-accent/30 rounded bg-black">
        <canvas
          ref="canvasRef"
          class="block cursor-pointer"
          @click="handleClick"
          @dblclick="handleDoubleClick"
        ></canvas>
      </div>
      <div class="mt-1.5 text-[10px] text-text-muted font-mono">
        Selected: 0x{{ selectedBlockId.toString(16).padStart(2, '0') }}
        ({{ selectedBlockId }}) — {{ blockIds.length }} blocks
      </div>
      <div class="mt-1 text-[10px] text-text-muted">
        Tip: press <b class="text-text">T</b> to paint blocks on the map.
        <b class="text-text">Double-click</b> a block (or <b>Edit Block</b>) to
        change its 4×4 tile arrangement & toggle tile passability.
      </div>
    </div>

    <BlockEditor
      v-if="currentMap"
      :open="editorOpen"
      :tileset="currentMap.header.tileset"
      :block-id="selectedBlockId"
      @close="editorOpen = false"
    />
    <NewTilesetDialog
      :open="newTilesetOpen"
      @close="newTilesetOpen = false"
    />
  </div>
</template>
