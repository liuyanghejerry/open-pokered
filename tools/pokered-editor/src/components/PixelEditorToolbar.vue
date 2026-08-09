<script setup lang="ts">
import { usePixelStore } from '../stores/pixelStore'
import { storeToRefs } from 'pinia'
import { ref, onMounted, onUnmounted } from 'vue'
import PaletteSelector from './PaletteSelector.vue'
import SpriteAiGenerateDialog from './SpriteAiGenerateDialog.vue'
import { useStaticMode } from '../composables/useStaticMode'

const store = usePixelStore()
const { activeTool, zoom, canUndo, canRedo, isDirty, isTilesetMode, tilesetMeta, activeFrame, activeAsset, loading, colorMode } = storeToRefs(store)
const staticMode = useStaticMode()

const showExportMenu = ref(false)
const showAiDialog = ref(false)

function handleUndo() {
  store.endStroke()
  store.undo()
}

function handleRedo() {
  store.endStroke()
  store.redo()
}

function handleSave() {
  store.endStroke()
  store.save()
}

function selectTool(tool: 'pencil' | 'erase' | 'eyedropper' | 'fill') {
  store.setTool(tool)
}

function zoomIn() {
  if (store.zoom < 32) store.zoom++
}

function zoomOut() {
  if (store.zoom > 1) store.zoom--
}

function toggleGrid() {
  store.showGrid = !store.showGrid
}

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

function handleExport2bpp() {
  showExportMenu.value = false
  const blob = store.exportAs2bpp()
  if (blob) downloadBlob(blob, `${activeAsset.value?.id ?? 'sprite'}.2bpp`)
}

function handleExport4bpp() {
  showExportMenu.value = false
  const blob = store.exportAs4bpp()
  if (blob) downloadBlob(blob, `${activeAsset.value?.id ?? 'sprite'}.4bpp`)
}

function handleExportPal() {
  showExportMenu.value = false
  const blob = store.exportAsPal()
  if (blob) downloadBlob(blob, `${activeAsset.value?.id ?? 'palette'}.pal`)
}

function toggleExportMenu() {
  showExportMenu.value = !showExportMenu.value
}

function closeExportMenu() {
  showExportMenu.value = false
}

function onKeyDown(e: KeyboardEvent) {
  const isCtrlOrCmd = e.ctrlKey || e.metaKey

  if (isCtrlOrCmd && e.key === 'z' && !e.shiftKey) {
    e.preventDefault()
    handleUndo()
    return
  }
  if (isCtrlOrCmd && e.key === 'z' && e.shiftKey) {
    e.preventDefault()
    handleRedo()
    return
  }
  if (isCtrlOrCmd && e.key === 's') {
    e.preventDefault()
    handleSave()
    return
  }
  if (!isCtrlOrCmd && !e.altKey) {
    switch (e.key.toLowerCase()) {
      case 'b': case 'p': selectTool('pencil'); e.preventDefault(); break
      case 'e': selectTool('erase'); e.preventDefault(); break
      case 'i': case 'o': selectTool('eyedropper'); e.preventDefault(); break
      case 'g': case 'f': selectTool('fill'); e.preventDefault(); break
    }
  }
}

onMounted(() => window.addEventListener('keydown', onKeyDown))
onUnmounted(() => window.removeEventListener('keydown', onKeyDown))
</script>

<template>
  <div class="flex items-center gap-2 px-3 py-2 bg-bg-panel border-b border-[rgba(255,255,255,0.06)] shrink-0 flex-wrap">
    <!-- Tool buttons -->
    <div class="flex items-center gap-0.5 bg-bg-inset rounded p-0.5">
      <button
        v-for="tool in ([
          { id: 'pencil', icon: '✏️', label: 'Pencil (B)' },
          { id: 'erase', icon: '🧹', label: 'Eraser (E)' },
          { id: 'eyedropper', icon: '💉', label: 'Eyedropper (I)' },
          { id: 'fill', icon: '🪣', label: 'Fill (G)' },
        ] as const)"
        :key="tool.id"
        :title="tool.label"
        class="w-8 h-8 flex items-center justify-center rounded text-sm cursor-pointer transition-colors"
        :class="activeTool === tool.id ? 'bg-accent text-bg' : 'text-text-muted hover:text-text hover:bg-[rgba(255,255,255,0.06)]'"
        @click="selectTool(tool.id)"
      >{{ tool.icon }}</button>
    </div>

    <div class="w-px h-6 bg-[rgba(255,255,255,0.08)]" />

    <PaletteSelector />

    <!-- Color mode switcher -->
    <div class="flex items-center gap-0.5 bg-bg-inset rounded p-0.5">
      <button
        v-for="mode in ([
          { id: 'dmg' as const, label: 'DMG' },
          { id: 'gba' as const, label: 'GBA' },
          { id: 'fullcolor' as const, label: 'RGB' },
        ])"
        :key="mode.id"
        :title="mode.id === 'dmg' ? 'Game Boy (4 grayscale)' : mode.id === 'gba' ? 'GBA (16 colors × 16 palettes)' : 'Full Color (any color)'"
        class="px-2 py-1 rounded text-[10px] font-bold cursor-pointer transition-colors"
        :class="colorMode === mode.id ? 'bg-accent text-bg' : 'text-text-muted hover:text-text hover:bg-[rgba(255,255,255,0.06)]'"
        @click="store.setColorMode(mode.id)"
      >{{ mode.label }}</button>
    </div>

    <div class="w-px h-6 bg-[rgba(255,255,255,0.08)]" />

    <!-- Zoom controls -->
    <div class="flex items-center gap-1">
      <button
        class="w-7 h-7 flex items-center justify-center rounded text-xs cursor-pointer text-text-muted hover:text-text hover:bg-[rgba(255,255,255,0.06)] disabled:opacity-30 disabled:cursor-not-allowed"
        :disabled="zoom <= 1"
        @click="zoomOut()"
      >−</button>
      <span class="text-xs text-text-muted min-w-[28px] text-center tabular-nums">{{ zoom }}x</span>
      <button
        class="w-7 h-7 flex items-center justify-center rounded text-xs cursor-pointer text-text-muted hover:text-text hover:bg-[rgba(255,255,255,0.06)] disabled:opacity-30 disabled:cursor-not-allowed"
        :disabled="zoom >= 32"
        @click="zoomIn()"
      >+</button>
    </div>

    <!-- Grid toggle -->
    <button
      class="px-2 py-1 rounded text-[10px] font-bold cursor-pointer border transition-colors"
      :class="store.showGrid ? 'border-accent text-accent bg-accent/10' : 'border-[rgba(255,255,255,0.1)] text-text-muted hover:text-text'"
      @click="toggleGrid()"
    >Grid</button>

    <div class="w-px h-6 bg-[rgba(255,255,255,0.08)]" />

    <!-- AI sprite generation (loads result into the current canvas) — needs
         the local backend, so it's disabled on static hosting -->
    <button
      class="px-2 py-1 rounded text-[10px] font-bold cursor-pointer border transition-colors disabled:opacity-30 disabled:cursor-not-allowed border-[rgba(255,255,255,0.1)] text-text-muted hover:text-text hover:border-accent"
      :disabled="!activeAsset || isTilesetMode || loading || staticMode"
      :title="staticMode ? 'AI sprite generation needs the local backend (npm run dev)' : 'Generate a sprite with AI into the current canvas'"
      @click="showAiDialog = true"
    >✨ AI</button>

    <div class="flex-1" />

    <!-- Mode indicator -->
    <span v-if="isTilesetMode && tilesetMeta" class="text-[10px] text-accent font-mono">
      Tile #{{ tilesetMeta.tileIndex }}
    </span>
    <span v-else-if="activeAsset?.category?.startsWith('pokemon')" class="text-[10px] text-accent font-mono">
      {{ activeFrame === 0 ? 'Front' : 'Back' }}
    </span>

    <!-- Undo/Redo -->
    <div class="flex items-center gap-0.5">
      <button
        class="w-7 h-7 flex items-center justify-center rounded text-xs cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed text-text-muted hover:text-text hover:bg-[rgba(255,255,255,0.06)]"
        :disabled="!canUndo"
        title="Undo (Ctrl+Z)"
        @click="handleUndo()"
      >↩</button>
      <button
        class="w-7 h-7 flex items-center justify-center rounded text-xs cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed text-text-muted hover:text-text hover:bg-[rgba(255,255,255,0.06)]"
        :disabled="!canRedo"
        title="Redo (Ctrl+Shift+Z)"
        @click="handleRedo()"
      >↪</button>
    </div>

    <!-- Save / Export -->
    <div class="relative inline-flex">
      <button
        class="px-3 py-1.5 rounded-l text-[11px] font-bold cursor-pointer transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        :class="isDirty ? 'bg-accent text-bg hover:bg-accent-hover' : 'bg-bg-inset text-text-muted border border-[rgba(255,255,255,0.1)]'"
        :disabled="!isDirty || loading"
        @click="handleSave()"
      >{{ loading ? 'Saving…' : '💾 Save' }}</button>
      <button
        class="px-1.5 py-1.5 rounded-r text-[11px] font-bold cursor-pointer transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        :class="isDirty ? 'bg-accent text-bg hover:bg-accent-hover border-l border-accent/30' : 'bg-bg-inset text-text-muted border border-l-0 border-[rgba(255,255,255,0.1)]'"
        :disabled="loading"
        @click.stop="toggleExportMenu"
      >▼</button>
      <!-- Export dropdown -->
      <div
        v-if="showExportMenu"
        class="absolute top-full right-0 mt-1 w-52 bg-bg-panel border border-[rgba(255,255,255,0.1)] rounded shadow-xl z-50 py-1"
      >
        <!-- DMG mode -->
        <template v-if="colorMode === 'dmg'">
          <button
            class="w-full px-3 py-2 text-left text-[11px] text-text hover:bg-[rgba(255,255,255,0.06)] flex items-center gap-2"
            @click="closeExportMenu(); handleSave()"
          >💾 Save as PNG</button>
          <button
            class="w-full px-3 py-2 text-left text-[11px] text-text hover:bg-[rgba(255,255,255,0.06)] flex items-center gap-2"
            @click="handleExport2bpp"
          >📦 Export as .2bpp</button>
        </template>
        <!-- GBA mode -->
        <template v-else-if="colorMode === 'gba'">
          <button
            class="w-full px-3 py-2 text-left text-[11px] text-text hover:bg-[rgba(255,255,255,0.06)] flex items-center gap-2"
            @click="closeExportMenu(); handleSave()"
          >💾 Save as PNG</button>
          <button
            class="w-full px-3 py-2 text-left text-[11px] text-text hover:bg-[rgba(255,255,255,0.06)] flex items-center gap-2"
            @click="handleExport4bpp"
          >📦 Export as .4bpp</button>
          <button
            class="w-full px-3 py-2 text-left text-[11px] text-text hover:bg-[rgba(255,255,255,0.06)] flex items-center gap-2"
            @click="handleExportPal"
          >🎨 Export palette .pal</button>
        </template>
        <!-- FullColor mode -->
        <template v-else>
          <button
            class="w-full px-3 py-2 text-left text-[11px] text-text hover:bg-[rgba(255,255,255,0.06)] flex items-center gap-2"
            @click="closeExportMenu(); handleSave()"
          >💾 Save as PNG</button>
        </template>
      </div>
    </div>

    <!-- AI sprite generation dialog -->
    <SpriteAiGenerateDialog :open="showAiDialog" @close="showAiDialog = false" />
  </div>
</template>
