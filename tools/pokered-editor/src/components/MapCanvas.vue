<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { useMapRenderer } from '../composables/useMapRenderer'
import { storeToRefs } from 'pinia'

const store = useMapStore()
const { statusMessage, hasUnsavedChanges, currentTool } = storeToRefs(store)
const canvasRef = ref<HTMLCanvasElement | null>(null)

const {
  render,
  tooltip,
  tooltipPosition,
  hoveringClickable,
  handleCanvasClick,
  handleCanvasMouseMove,
  handleCanvasMouseLeave,
  handleCanvasMouseDown,
  handleCanvasMouseUp,
} = useMapRenderer(canvasRef)

function handleKeydown(e: KeyboardEvent) {
  if ((e.target as HTMLElement).tagName === 'INPUT') return
  switch (e.key) {
    case 'ArrowLeft':
      store.prevMap()
      break
    case 'ArrowRight':
      store.nextMap()
      break
    case 'v':
      store.setTool('view')
      break
    case 'e':
      store.setTool('edit')
      break
    case 't':
      store.setTool('edit-tiles')
      break
    case 'Escape':
      store.selectEntity(null)
      break
  }
}

function toHex(n: number, pad = 2): string {
  return '0x' + n.toString(16).padStart(pad, '0')
}

function canvasCursorClass(): string {
  if (currentTool.value === 'edit') return 'cursor-crosshair'
  if (currentTool.value === 'edit-tiles') return 'cursor-cell'
  if (hoveringClickable.value) return 'cursor-pointer'
  return 'cursor-default'
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
  render()
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <div class="flex-1 relative">
    <div class="relative">
      <canvas
        ref="canvasRef"
        class="border-2 border-accent rounded"
        :class="canvasCursorClass()"
        @click="handleCanvasClick"
        @mousedown="handleCanvasMouseDown"
        @mouseup="handleCanvasMouseUp"
        @mousemove="handleCanvasMouseMove"
        @mouseleave="handleCanvasMouseLeave"
      ></canvas>
    </div>

    <div class="mt-2.5 text-[11px] text-text-muted">
      <span v-if="hasUnsavedChanges" class="text-warning font-bold mr-1">*Unsaved*</span>
      {{ statusMessage }}
    </div>

    <Teleport to="body">
      <div
        v-if="tooltip"
        class="fixed bg-bg-panel/95 px-2 py-1.5 rounded border border-accent text-[11px] font-mono pointer-events-none z-50"
        :style="{ left: tooltipPosition.x + 'px', top: tooltipPosition.y + 'px' }"
      >
        Tile: ({{ tooltip.tileX }}, {{ tooltip.tileY }})<br />
        Block: ({{ tooltip.blockX }}, {{ tooltip.blockY }}) #{{ tooltip.blockIndex }}<br />
        BlockID: {{ toHex(tooltip.blockId) }}<br />
        TileID: {{ toHex(tooltip.tileId) }}<br />
        <b :class="tooltip.passable ? 'text-accent' : 'text-danger'">
          {{ tooltip.passable ? 'PASSABLE' : 'BLOCKED' }}
        </b>
        <template v-if="tooltip.warp">
          <br />
          <b class="text-[#3498db]">WARP</b>
          <template v-if="tooltip.warp.destMap"> → {{ tooltip.warp.destMap }}</template>
        </template>
        <template v-if="tooltip.sign">
          <br /><b class="text-[#f1c40f]">SIGN</b> text#{{ tooltip.sign.textId }}
        </template>
        <template v-if="tooltip.npc">
          <br />
          <b :class="tooltip.npc.isTrainer ? 'text-danger' : tooltip.npc.itemId != null ? 'text-accent' : 'text-[#9b59b6]'">
            {{ tooltip.npc.isTrainer ? 'TRAINER' : tooltip.npc.itemId != null ? 'ITEM' : 'NPC' }}
          </b>
          {{ tooltip.npc.spriteName }}
          <template v-if="tooltip.npc.isTrainer"> ({{ tooltip.npc.trainerClass }})</template>
          <template v-if="tooltip.npc.itemId != null"> item={{ toHex(tooltip.npc.itemId) }}</template>
        </template>
        <template v-if="tooltip.coordEvent">
          <br />
          <b class="text-[#e67e22]">COORD</b> {{ tooltip.coordEvent.trigger }}
        </template>
      </div>
    </Teleport>
  </div>
</template>
