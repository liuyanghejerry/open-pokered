<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { usePixelStore } from '../stores/pixelStore'
import { storeToRefs } from 'pinia'
import { usePixelCanvas } from '../composables/usePixelCanvas'
import { usePixelTools } from '../composables/usePixelTools'
import PixelEditorToolbar from './PixelEditorToolbar.vue'

const store = usePixelStore()
const { loading, error, activeAsset } = storeToRefs(store)

const canvasRef = ref<HTMLCanvasElement | null>(null)
const { render, screenToPixel } = usePixelCanvas(canvasRef)
const { onPointerDown, onPointerMove, onPointerUp } = usePixelTools(canvasRef, screenToPixel)

// Re-render when asset loads/changes
watch(() => store.imageData, () => {
  // Canvas composable already watches imageData changes via its internal watch
}, { deep: true })

// Initial render when canvas mounts
watch(canvasRef, (canvas) => {
  if (canvas) {
    render()
  }
})

// ── Toast ────────────────────────────────────────────────────────────
const toast = ref<{ type: 'success' | 'error'; message: string } | null>(null)
let toastTimer: ReturnType<typeof setTimeout> | null = null

function showToast(type: 'success' | 'error', message: string) {
  if (toastTimer) clearTimeout(toastTimer)
  toast.value = { type, message }
  toastTimer = setTimeout(() => {
    toast.value = null
  }, 2000)
}

// When isDirty transitions from true → false, save completed
watch(() => store.isDirty, (dirty, wasDirty) => {
  if (wasDirty && !dirty) {
    showToast('success', 'Saved ✓')
  }
})

// Watch for save errors
watch(() => store.error, (err) => {
  if (err) {
    showToast('error', `Save failed: ${err}`)
  }
})

// ── Shortcut Help Modal ──────────────────────────────────────────────
const showShortcuts = ref(false)

function onKeyDown(e: KeyboardEvent) {
  if (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey) {
    showShortcuts.value = !showShortcuts.value
    e.preventDefault()
  }
  if (e.key === 'Escape' && showShortcuts.value) {
    showShortcuts.value = false
    e.preventDefault()
  }
}

// ── Navigation Guard ─────────────────────────────────────────────────
function onBeforeUnload(e: BeforeUnloadEvent) {
  if (store.isDirty) {
    e.preventDefault()
    e.returnValue = ''
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('beforeunload', onBeforeUnload)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('beforeunload', onBeforeUnload)
  if (toastTimer) clearTimeout(toastTimer)
})
</script>

<template>
  <div class="flex-1 flex flex-col min-h-0 overflow-hidden">
    <!-- Toolbar -->
    <PixelEditorToolbar />

    <!-- Canvas / Loading / Error area -->
    <div class="flex-1 flex items-center justify-center overflow-auto bg-[#0f0f23] relative">
      <!-- Loading state -->
      <div
        v-if="loading"
        class="absolute inset-0 flex items-center justify-center bg-bg/80 z-10"
      >
        <div class="text-accent text-sm animate-pulse">Loading...</div>
      </div>

      <!-- Error state -->
      <div
        v-if="error"
        class="absolute inset-0 flex items-center justify-center bg-bg/80 z-10"
      >
        <div class="text-danger text-sm">{{ error }}</div>
      </div>

      <!-- Empty state -->
      <div
        v-if="!activeAsset && !loading"
        class="text-text-muted text-sm text-center px-4"
      >
        Select an asset from the sidebar to begin editing
      </div>

      <!-- Canvas -->
      <canvas
        v-show="activeAsset && !loading"
        ref="canvasRef"
        class="cursor-crosshair"
        :style="{ imageRendering: 'pixelated' }"
        @pointerdown="onPointerDown"
        @pointermove="onPointerMove"
        @pointerup="onPointerUp"
      />
    </div>

    <!-- Toast notification -->
    <Transition name="toast">
      <div
        v-if="toast"
        class="fixed bottom-4 right-4 z-50 px-3 py-2 rounded shadow-lg text-sm font-bold"
        :class="toast.type === 'success' ? 'bg-accent text-bg' : 'bg-danger text-white'"
      >
        {{ toast.message }}
      </div>
    </Transition>

    <!-- Shortcut Help Modal -->
    <Teleport to="body">
      <div
        v-if="showShortcuts"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        @click.self="showShortcuts = false"
      >
        <div class="bg-bg-panel rounded-lg border border-[rgba(255,255,255,0.1)] shadow-2xl w-[320px] max-h-[80vh] overflow-y-auto">
          <div class="flex items-center justify-between px-4 py-3 border-b border-[rgba(255,255,255,0.06)]">
            <h2 class="text-accent text-sm font-bold">Keyboard Shortcuts</h2>
            <button
              class="text-text-muted hover:text-text text-lg cursor-pointer"
              @click="showShortcuts = false"
            >✕</button>
          </div>
          <div class="p-4 space-y-3 text-xs">
            <div>
              <div class="text-accent font-bold mb-1">Tools</div>
              <div class="grid grid-cols-2 gap-1 text-text">
                <span>Pencil</span><span class="text-text-muted text-right font-mono">B / P</span>
                <span>Eraser</span><span class="text-text-muted text-right font-mono">E</span>
                <span>Eyedropper</span><span class="text-text-muted text-right font-mono">I / O</span>
                <span>Fill</span><span class="text-text-muted text-right font-mono">G / F</span>
              </div>
            </div>
            <div>
              <div class="text-accent font-bold mb-1">Colors</div>
              <div class="grid grid-cols-2 gap-1 text-text">
                <span>White</span><span class="text-text-muted text-right font-mono">1</span>
                <span>Light Gray</span><span class="text-text-muted text-right font-mono">2</span>
                <span>Dark Gray</span><span class="text-text-muted text-right font-mono">3</span>
                <span>Black</span><span class="text-text-muted text-right font-mono">4</span>
              </div>
            </div>
            <div>
              <div class="text-accent font-bold mb-1">Edit</div>
              <div class="grid grid-cols-2 gap-1 text-text">
                <span>Undo</span><span class="text-text-muted text-right font-mono">Ctrl+Z</span>
                <span>Redo</span><span class="text-text-muted text-right font-mono">Ctrl+Shift+Z</span>
                <span>Save</span><span class="text-text-muted text-right font-mono">Ctrl+S</span>
              </div>
            </div>
            <div>
              <div class="text-accent font-bold mb-1">View</div>
              <div class="grid grid-cols-2 gap-1 text-text">
                <span>Zoom In</span><span class="text-text-muted text-right font-mono">+ / =</span>
                <span>Zoom Out</span><span class="text-text-muted text-right font-mono">-</span>
                <span>Toggle Grid</span><span class="text-text-muted text-right font-mono">G</span>
                <span>Help</span><span class="text-text-muted text-right font-mono">?</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
canvas {
  display: block;
  image-rendering: pixelated;
}

.toast-enter-active { transition: all 0.2s ease-out; }
.toast-leave-active { transition: all 0.3s ease-in; }
.toast-enter-from { opacity: 0; transform: translateY(10px); }
.toast-leave-to { opacity: 0; transform: translateY(10px); }
</style>
