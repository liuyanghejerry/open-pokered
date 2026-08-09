<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'
import { TILE_SIZE } from '../types/constants'

const props = defineProps<{ open: boolean; tileset: string; blockId: number }>()
const emit = defineEmits<{ close: [] }>()

const store = useMapStore()
const { blocksets, tilesetImages, passableTiles } = storeToRefs(store)

const TILE_PX = 32 // 8px source × 4 zoom — easy to click
const PALETTE_COLS = 16

const selectedTileId = ref<number>(0)
const blockCanvas = ref<HTMLCanvasElement | null>(null)
const tileCanvas = ref<HTMLCanvasElement | null>(null)

const blockTiles = computed<number[] | undefined>(() => {
  const bs = blocksets.value[props.tileset]
  return bs?.[props.blockId]
})

const tilesetImg = computed(() => tilesetImages.value[props.tileset])
const passableForTileset = computed(() => passableTiles.value[props.tileset] ?? [])
const passableSet = computed(() => new Set(passableForTileset.value))

const totalTiles = computed(() => {
  const img = tilesetImg.value
  if (!img) return 0
  const cols = Math.floor(img.width / TILE_SIZE)
  const rows = Math.floor(img.height / TILE_SIZE)
  return cols * rows
})

function drawBlock() {
  const canvas = blockCanvas.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  const tiles = blockTiles.value
  const img = tilesetImg.value
  const w = TILE_PX * 4
  const h = TILE_PX * 4
  canvas.width = w
  canvas.height = h
  canvas.style.width = `${w}px`
  canvas.style.height = `${h}px`
  ctx.imageSmoothingEnabled = false
  ctx.fillStyle = '#000'
  ctx.fillRect(0, 0, w, h)
  if (!tiles || !img) return
  const cols = Math.floor(img.width / TILE_SIZE)
  for (let i = 0; i < 16; i++) {
    const tx = i % 4
    const ty = Math.floor(i / 4)
    const tileId = tiles[i]
    const sx = (tileId % cols) * TILE_SIZE
    const sy = Math.floor(tileId / cols) * TILE_SIZE
    ctx.drawImage(img, sx, sy, TILE_SIZE, TILE_SIZE,
      tx * TILE_PX, ty * TILE_PX, TILE_PX, TILE_PX)
    // small label
    ctx.fillStyle = 'rgba(0,0,0,0.55)'
    ctx.fillRect(tx * TILE_PX, ty * TILE_PX + TILE_PX - 11, 22, 11)
    ctx.fillStyle = '#fff'
    ctx.font = '9px monospace'
    ctx.fillText(`0x${tileId.toString(16).padStart(2, '0')}`, tx * TILE_PX + 1, ty * TILE_PX + TILE_PX - 2)
  }
  // grid
  ctx.strokeStyle = 'rgba(78,204,163,0.6)'
  ctx.lineWidth = 1
  for (let i = 0; i <= 4; i++) {
    ctx.beginPath()
    ctx.moveTo(i * TILE_PX + 0.5, 0)
    ctx.lineTo(i * TILE_PX + 0.5, h)
    ctx.stroke()
    ctx.beginPath()
    ctx.moveTo(0, i * TILE_PX + 0.5)
    ctx.lineTo(w, i * TILE_PX + 0.5)
    ctx.stroke()
  }
}

function drawPalette() {
  const canvas = tileCanvas.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  const img = tilesetImg.value
  const total = totalTiles.value
  if (!img || total === 0) return
  const cols = PALETTE_COLS
  const rows = Math.ceil(total / cols)
  const tilePx = 24
  const w = cols * tilePx
  const h = rows * tilePx
  canvas.width = w
  canvas.height = h
  canvas.style.width = `${w}px`
  canvas.style.height = `${h}px`
  ctx.imageSmoothingEnabled = false
  ctx.fillStyle = '#000'
  ctx.fillRect(0, 0, w, h)
  const srcCols = Math.floor(img.width / TILE_SIZE)
  for (let i = 0; i < total; i++) {
    const cx = (i % cols) * tilePx
    const cy = Math.floor(i / cols) * tilePx
    const sx = (i % srcCols) * TILE_SIZE
    const sy = Math.floor(i / srcCols) * TILE_SIZE
    ctx.drawImage(img, sx, sy, TILE_SIZE, TILE_SIZE, cx, cy, tilePx, tilePx)
    // passable overlay
    if (passableSet.value.has(i)) {
      ctx.fillStyle = 'rgba(78,204,163,0.35)'
      ctx.fillRect(cx, cy, tilePx, tilePx)
    } else {
      ctx.fillStyle = 'rgba(231,76,60,0.18)'
      ctx.fillRect(cx, cy, tilePx, tilePx)
    }
  }
  // selection
  const sel = selectedTileId.value
  if (sel < total) {
    const cx = (sel % cols) * tilePx
    const cy = Math.floor(sel / cols) * tilePx
    ctx.strokeStyle = '#fff'
    ctx.lineWidth = 2
    ctx.strokeRect(cx + 1, cy + 1, tilePx - 2, tilePx - 2)
  }
}

function onBlockClick(e: MouseEvent) {
  const canvas = blockCanvas.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const tx = Math.floor((e.clientX - rect.left) / TILE_PX)
  const ty = Math.floor((e.clientY - rect.top) / TILE_PX)
  if (tx < 0 || tx >= 4 || ty < 0 || ty >= 4) return
  store.setBlocksetTile(props.tileset, props.blockId, ty * 4 + tx, selectedTileId.value)
  drawBlock()
}

function onPaletteClick(e: MouseEvent) {
  const canvas = tileCanvas.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const tilePx = 24
  const tx = Math.floor((e.clientX - rect.left) / tilePx)
  const ty = Math.floor((e.clientY - rect.top) / tilePx)
  const idx = ty * PALETTE_COLS + tx
  if (idx < 0 || idx >= totalTiles.value) return
  selectedTileId.value = idx
  drawPalette()
}

function onPaletteRightClick(e: MouseEvent) {
  e.preventDefault()
  const canvas = tileCanvas.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const tilePx = 24
  const tx = Math.floor((e.clientX - rect.left) / tilePx)
  const ty = Math.floor((e.clientY - rect.top) / tilePx)
  const idx = ty * PALETTE_COLS + tx
  if (idx < 0 || idx >= totalTiles.value) return
  store.togglePassableTile(props.tileset, idx)
  drawPalette()
}

watch(() => [props.open, props.tileset, props.blockId], () => {
  if (props.open) nextTick(() => { drawBlock(); drawPalette() })
})
watch([blockTiles, tilesetImg, selectedTileId, passableSet], () => {
  if (props.open) nextTick(() => { drawBlock(); drawPalette() })
})

onMounted(() => {
  if (props.open) nextTick(() => { drawBlock(); drawPalette() })
})
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
    @click.self="emit('close')"
  >
    <div class="bg-bg-panel border border-accent rounded-lg p-4 max-w-[860px] w-[95%] max-h-[92vh] overflow-auto">
      <div class="flex items-start justify-between mb-3">
        <div>
          <h2 class="text-accent font-bold text-base">
            Edit Block 0x{{ blockId.toString(16).padStart(2, '0') }} — {{ tileset }}
          </h2>
          <p class="text-[11px] text-text-muted mt-0.5">
            Click a tile in the palette to select it, then click a cell in the
            4×4 block to paint. <b>Right-click</b> a tile in the palette to
            toggle its passable / blocked state for this tileset. Use
            <b>Save</b> in the sidebar to persist <code class="text-accent">.bst</code>
            and collision changes.
          </p>
        </div>
        <button
          class="px-2 py-0.5 text-xs bg-bg-inset rounded hover:bg-[#444] text-text"
          @click="emit('close')"
        >Close ✕</button>
      </div>

      <div class="flex gap-4 flex-wrap">
        <div>
          <h3 class="text-accent text-[12px] font-bold mb-1">Block (4×4 tiles)</h3>
          <canvas
            ref="blockCanvas"
            class="block bg-black cursor-cell border border-accent/40"
            @click="onBlockClick"
          ></canvas>
          <div class="mt-1 text-[10px] text-text-muted font-mono">
            Selected tile to paint:
            <span class="text-accent">0x{{ selectedTileId.toString(16).padStart(2, '0') }}</span>
            ({{ selectedTileId }})
          </div>
        </div>

        <div class="flex-1 min-w-[420px]">
          <h3 class="text-accent text-[12px] font-bold mb-1">
            Tile Palette ({{ totalTiles }} tiles, green=passable, red=blocked)
          </h3>
          <div class="overflow-auto max-h-[480px] border border-accent/30 rounded bg-black">
            <canvas
              ref="tileCanvas"
              class="block cursor-pointer"
              @click="onPaletteClick"
              @contextmenu="onPaletteRightClick"
            ></canvas>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
