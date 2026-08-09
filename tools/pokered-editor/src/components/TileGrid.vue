<script setup lang="ts">
import { computed } from 'vue'
import { usePixelStore } from '../stores/pixelStore'
import { storeToRefs } from 'pinia'
import type { AssetEntry } from '../types/pixel'
import { gfxUrl } from '../utils/assetUrl'

const props = defineProps<{
  entry: AssetEntry
}>()

const store = usePixelStore()
const { tilesetMeta, isTilesetMode } = storeToRefs(store)

const tilesetUrl = computed(() => gfxUrl(`tilesets/${props.entry.filename}`))
const cols = computed(() => (props.entry.tilePixelWidth || 128) / 8)
const rows = computed(() => (props.entry.tilePixelHeight || 48) / 8)
const tileCount = computed(() => cols.value * rows.value)
const tileIndices = computed(() => Array.from({ length: tileCount.value }, (_, i) => i))

function selectTile(tileIndex: number) {
  store.loadTilesetTile(props.entry.id, tileIndex)
}

function isActive(index: number): boolean {
  return isTilesetMode.value
    && tilesetMeta.value?.tileIndex === index
    && tilesetMeta.value?.tilesetName === props.entry.id
}

const ZOOM = 4

function tileStyle(index: number) {
  const c = cols.value
  const tileX = (index % c) * 8
  const tileY = Math.floor(index / c) * 8
  const tw = props.entry.tilePixelWidth || 128
  const th = props.entry.tilePixelHeight || 48
  return {
    backgroundImage: `url(${tilesetUrl.value})`,
    backgroundPosition: `-${tileX * ZOOM}px -${tileY * ZOOM}px`,
    backgroundSize: `${tw * ZOOM}px ${th * ZOOM}px`,
    backgroundRepeat: 'no-repeat',
    width: '32px',
    height: '32px',
    imageRendering: 'pixelated' as const,
  }
}
</script>

<template>
  <div class="p-2">
    <div class="text-[10px] text-text-muted mb-2">
      {{ tileCount }} tiles &mdash; click to edit
    </div>
    <div class="grid grid-cols-6 gap-0.5 max-h-[300px] overflow-y-auto">
      <button
        v-for="idx in tileIndices"
        :key="idx"
        class="rounded cursor-pointer border-2 flex items-center justify-center transition-all"
        :class="
          isActive(idx)
            ? 'border-accent bg-accent/10'
            : 'border-[rgba(255,255,255,0.1)] bg-bg-inset hover:border-[rgba(255,255,255,0.3)]'
        "
        :style="tileStyle(idx)"
        :title="'Tile #' + idx"
        @click="selectTile(idx)"
      />
    </div>
  </div>
</template>
