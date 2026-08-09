<script setup lang="ts">
import { computed } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { usePixelStore } from '../stores/pixelStore'
import { storeToRefs } from 'pinia'

type Activity = 'map' | 'script' | 'save' | 'trainer' | 'pokemon' | 'move' | 'layout' | 'pixel' | 'playtest'

const props = defineProps<{
  activeActivity: Activity
  lineInfo?: string
  sectionName?: string
}>()

const store = useMapStore()
const pixelStore = usePixelStore()
const { currentMap, hasUnsavedChanges, zoom } = storeToRefs(store)
const { zoom: pixelZoom } = storeToRefs(pixelStore)

const mapLabel = computed(() => currentMap.value?.name ?? 'No map')

const rightText = computed(() => {
  if (props.activeActivity === 'map') {
    return `zoom: ${zoom.value}x`
  }
  if (props.activeActivity === 'pixel') {
    return `zoom: ${pixelZoom.value}x`
  }
  if (props.activeActivity === 'script' && props.lineInfo) {
    return props.lineInfo
  }
  if (props.sectionName) {
    return props.sectionName
  }
  return ''
})
</script>

<template>
  <div class="status-bar">
    <div class="status-left">
      <span class="status-indicator" :class="hasUnsavedChanges ? 'dirty' : 'saved'" />
      <span class="status-text">{{ mapLabel }}</span>
      <span v-if="hasUnsavedChanges" class="status-dirty-badge">Unsaved</span>
    </div>
    <div class="status-right">
      <span class="status-text">{{ rightText }}</span>
    </div>
  </div>
</template>

<style scoped>
.status-bar {
  height: 24px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  background: var(--color-bg-inset);
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
  line-height: 1;
}

.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.status-indicator {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-indicator.saved {
  background: var(--color-accent);
}

.status-indicator.dirty {
  background: var(--color-warning);
}

.status-text {
  color: var(--color-text-muted);
}

.status-dirty-badge {
  font-size: 9px;
  padding: 0 4px;
  border-radius: 2px;
  background: rgba(241, 196, 15, 0.15);
  color: var(--color-warning);
}
</style>
