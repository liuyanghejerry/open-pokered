<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { storeToRefs } from 'pinia'
import { useLayoutStore } from '../stores/layoutStore'
import { useWasmPreview } from '../composables/useWasmPreview'
import { useCodeMirror } from '../composables/useCodeMirror'
import DiffViewer from './DiffViewer.vue'
import VariablesPanel from './VariablesPanel.vue'
import GuiDslHelp from './GuiDslHelp.vue'
import { extractVariables, DEFAULT_MOCK_VALUES, type TemplateVariable } from '../composables/useVariableExtract'
import type { TileRect, ScreenLayout } from '../types/ui-layout'

// Mock state counts per menu (from pokered-ui-preview/src/lib.rs)
const MOCK_STATE_COUNTS: Record<string, number> = {
  bag: 4,
  battle_bag: 3,
  battle_main: 2,
  battle_move: 3,
  battle_party: 3,
  battle_text: 3,
  dialog: 3,
  main: 2,
  mart: 7,
  naming: 3,
  oak_speech: 3,
  options: 2,
  party: 3,
  pokedex: 2,
  save: 3,
  start: 3,
  stats: 2,
  yes_no: 3,
}

const store = useLayoutStore()
const { activeName, rawJson, rawGui, parsedJson, dirty, error, loading, parseError, canUndo, canRedo, saveError, savedSnapshot } = storeToRefs(store)

const showDiff = ref(false)

const wasm = useWasmPreview()

const editorContainer = ref<HTMLElement | null>(null)
const cm = useCodeMirror(editorContainer, (content: string) => {
  if (store.mode === 'gui') {
    store.setRawGui(content)
  } else {
    store.setRawJson(content)
  }
})

const previewAvailable = computed(() => store.parsedJson !== null)

const canvasRef = ref<HTMLCanvasElement | null>(null)

const selectedRectId = ref<string | null>(null)
const activeMockStateId = ref(0)
const activeLang = ref(0) // 0=En, 1=Zh
const variableOverrides = ref<Record<string, string>>({})
const extractedVariables = ref<TemplateVariable[]>([])

const mockStateCount = computed(() => {
  if (!activeName.value) return 1
  return MOCK_STATE_COUNTS[activeName.value] ?? 1
})

let renderTimer: ReturnType<typeof setTimeout> | null = null

function scheduleRender() {
  if (renderTimer) clearTimeout(renderTimer)
  renderTimer = setTimeout(() => renderPreview(), 150)
}

function onVariableUpdate(key: string, value: string) {
  variableOverrides.value[key] = value
  scheduleRender()
}

async function renderPreview() {
  if (!canvasRef.value) return
  const menuName = store.activeName
  if (!menuName) return
  const canvas = canvasRef.value
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  try {
    const bytes = await wasm.render(menuName, rawJson.value, activeMockStateId.value, activeLang.value, variableOverrides.value)
    if (bytes.length === 0) {
      ctx.clearRect(0, 0, 160, 144)
      return
    }
    const imgData = new ImageData(new Uint8ClampedArray(bytes), 160, 144)
    ctx.putImageData(imgData, 0, 0)
  } catch {
    // render errors handled by store.parseError
  }
}

watch([parsedJson, activeName, activeMockStateId, activeLang], () => {
  if (activeName.value) scheduleRender()
})

watch(activeName, async (name) => {
  selectedRectId.value = null
  activeMockStateId.value = 0
  variableOverrides.value = {}
  if (name) {
    await nextTick()
    const content = store.mode === 'gui' ? store.rawGui : store.rawJson
    cm.create(content, store.mode === 'gui' ? 'gui' : 'js')
  }
})

watch([rawJson, rawGui], () => {
  const content = store.mode === 'gui' ? store.rawGui : store.rawJson
  cm.setContent(content)
  if (store.activeName) scheduleRender()
})

watch([parsedJson, activeName], () => {
  const json = parsedJson.value
  extractedVariables.value = extractVariables(json)
  // Populate the panel's display values from DEFAULT_MOCK_VALUES, but do NOT
  // force them into the render. Unedited variables fall through to the Rust
  // mock data, so the Mock state selector stays effective. Only variables the
  // user actually edits live in variableOverrides and override the mock.
  const defaultVals = DEFAULT_MOCK_VALUES[activeName.value ?? ''] ?? {}
  const overrides = variableOverrides.value
  for (const v of extractedVariables.value) {
    v.defaultValue = defaultVals[v.key] ?? ''
    v.currentValue = overrides[v.key] ?? v.defaultValue
  }
  if (activeName.value) scheduleRender()
})

onUnmounted(() => {
  if (renderTimer) clearTimeout(renderTimer)
  cm.destroy()
  window.removeEventListener('keydown', handleKeydown)
  // Defensive: drop any in-flight drag listeners
  window.removeEventListener('pointermove', handlePointerMove)
  window.removeEventListener('pointerup', handlePointerUp)
  window.removeEventListener('pointercancel', handlePointerUp)
})

// ── Canvas overlay scaling ──────────────────────────────────────────────

const canvasScale = ref(1)

function updateScale() {
  if (!canvasRef.value) return
  const rect = canvasRef.value.getBoundingClientRect()
  canvasScale.value = rect.width / 160
}

const resizeObs = ref<ResizeObserver | null>(null)

watch(canvasRef, (el) => {
  if (resizeObs.value) {
    resizeObs.value.disconnect()
    resizeObs.value = null
  }
  if (el) {
    resizeObs.value = new ResizeObserver(() => updateScale())
    resizeObs.value.observe(el)
    updateScale()
  }
})

onUnmounted(() => {
  if (resizeObs.value) resizeObs.value.disconnect()
})

// ── Drag state ──────────────────────────────────────────────────────────

type DragState = {
  rectId: string
  mode: 'move' | 'resize'
  startMouseX: number
  startMouseY: number
  startRect: TileRect
  path: (string | number)[]
}

const dragState = ref<DragState | null>(null)
let rafId: number | null = null

function clampRect(rect: TileRect): TileRect {
  const tw = Math.max(1, Math.min(20, Math.round(rect.tw)))
  const th = Math.max(1, Math.min(18, Math.round(rect.th)))
  const tx = Math.max(0, Math.min(20 - tw, Math.round(rect.tx)))
  const ty = Math.max(0, Math.min(18 - th, Math.round(rect.ty)))
  return { tx, ty, tw, th }
}

// ── LiveRect overlay (flattened rects for v2 elements) ───────────────
//
// Each LiveRect wraps one schema_version-2 layout element and carries an
// index path into the parsed JSON (e.g. ["elements", i]) so drag/resize
// mutations can be applied via store.updateRectAtPath (with undo history).

interface LiveRect {
  /** Unique identifier ("el-{i}"). */
  id: string
  /** Numeric rect for the overlay; {template} coords coerce to 0. */
  tileRect: TileRect
  /** Display height in tiles. */
  displayTh: number
  /** Path into the parsed JSON to reach the element object. */
  path: (string | number)[]
  /** Element type ("border", "text", "tile", "list", …). */
  type: string
}

/** Coerce an ElementRect coord (number or {template} string) to a number for the overlay. */
function numCoord(v: unknown): number {
  const n = typeof v === 'number' ? v : Number(v)
  return Number.isFinite(n) ? n : 0
}

function extractRects(layout: ScreenLayout | null): LiveRect[] {
  if (!layout || !Array.isArray(layout.elements)) return []
  return layout.elements.map((el, i) => {
    const rect = (el.rect ?? {}) as Record<string, unknown>
    const tileRect: TileRect = {
      tx: numCoord(rect.tx),
      ty: numCoord(rect.ty),
      tw: numCoord(rect.tw),
      th: numCoord(rect.th),
    }
    return {
      id: `el-${i}`,
      tileRect,
      displayTh: tileRect.th,
      path: ['elements', i],
      type: el.type ?? 'element',
    }
  })
}

/** Flattened overlay rects for the current layout's top-level elements. */
const liveRects = computed<LiveRect[]>(() => extractRects(parsedJson.value))

// Attach move/up listeners on window during a drag. Reasons:
//  1. setPointerCapture on the SVG <rect> would steal events away from the
//     canvas's local handlers, leaving the drag stuck if the cursor leaves
//     the rect or the rect is re-rendered (Vue v-for re-key).
//  2. window listeners survive any DOM churn and guarantee mouseup is seen
//     even if the cursor leaves the canvas/window.
function handleRectPointerDown(e: PointerEvent, lr: LiveRect, mode: 'move' | 'resize') {
  e.preventDefault()
  e.stopPropagation()
  selectedRectId.value = lr.id
  // In GUI (.gui DSL) mode the compiled JSON is a derived artifact, not the
  // source of truth. Dragging would mutate rawJson (lost on save/recompile)
  // and push a JSON snapshot onto the shared undo stack, so a later undo would
  // surface JSON in the DSL editor. Allow selection only — edits go via the DSL.
  if (store.mode === 'gui') return
  dragState.value = {
    rectId: lr.id,
    mode,
    startMouseX: e.clientX,
    startMouseY: e.clientY,
    startRect: { ...lr.tileRect },
    path: lr.path,
  }
  window.addEventListener('pointermove', handlePointerMove)
  window.addEventListener('pointerup', handlePointerUp)
  window.addEventListener('pointercancel', handlePointerUp)
}

function handlePointerMove(e: PointerEvent) {
  if (!dragState.value) return
  const ds = dragState.value
  const scale = canvasScale.value
  if (scale === 0) return

  const dx = (e.clientX - ds.startMouseX) / scale
  const dy = (e.clientY - ds.startMouseY) / scale
  const dtTilesX = dx / 8
  const dtTilesY = dy / 8

  if (rafId !== null) return
  rafId = requestAnimationFrame(() => {
    rafId = null
    const d = dragState.value
    if (!d) return
    if (d.mode === 'move') {
      const newRect = clampRect({
        tx: d.startRect.tx + dtTilesX,
        ty: d.startRect.ty + dtTilesY,
        tw: d.startRect.tw,
        th: d.startRect.th,
      })
      applyElementMove(d.path, newRect)
    } else {
      const newRect = clampRect({
        tx: d.startRect.tx,
        ty: d.startRect.ty,
        tw: d.startRect.tw + dtTilesX,
        th: d.startRect.th + dtTilesY,
      })
      applyElementResize(d.path, newRect)
    }
  })
}

function handlePointerUp(_e: PointerEvent) {
  if (dragState.value) {
    dragState.value = null
    if (rafId !== null) {
      cancelAnimationFrame(rafId)
      rafId = null
    }
  }
  window.removeEventListener('pointermove', handlePointerMove)
  window.removeEventListener('pointerup', handlePointerUp)
  window.removeEventListener('pointercancel', handlePointerUp)
}

function handleCanvasClick(e: MouseEvent) {
  if (dragState.value) return
  const scale = canvasScale.value
  if (scale === 0) return
  const rect = (e.target as HTMLCanvasElement).getBoundingClientRect()
  const px = (e.clientX - rect.left) / scale
  const py = (e.clientY - rect.top) / scale

  // Hit-test against liveRects (back to front for correct z-order)
  const rects = liveRects.value
  for (let i = rects.length - 1; i >= 0; i--) {
    const r = rects[i].tileRect
    const x = r.tx * 8
    const y = r.ty * 8
    const w = r.tw * 8
    const h = rects[i].displayTh * 8
    if (px >= x && px < x + w && py >= y && py < y + h) {
      selectedRectId.value = rects[i].id
      return
    }
  }

  selectedRectId.value = null
}

// ── applyElementMove / applyElementResize ──────────────────────────────
// Apply a drag result to the element at `path` via the store, which parses
// rawJson, patches the rect, and re-commits through setRawJson (preserving
// dirty/parse/undo state). The path originates from LiveRect.path, e.g.
// ["elements", i] for a top-level element.

function applyElementMove(path: (string | number)[], newRect: TileRect) {
  store.updateRectAtPath(path, { tx: newRect.tx, ty: newRect.ty })
}

function applyElementResize(path: (string | number)[], newRect: TileRect) {
  store.updateRectAtPath(path, { tw: newRect.tw, th: newRect.th })
}

// ── Undo/Redo keyboard shortcuts ───────────────────────────────────────
// Trade-off: Cmd+Z inside CodeMirror → CM's own undo (via historyKeymap).
// Outside CM (e.g., after dragging on canvas) → store undo.
// This is intentional: CM keeps its own undo stack for text edits,
// while the store undo covers structural changes (drag, inspector edits).
//
// The store undo/redo restore the mode-appropriate source (rawGui in GUI
// mode, rawJson in JSON mode); the `watch([rawJson, rawGui])` above syncs
// that back into CodeMirror (and re-renders the preview). We must NOT force
// rawJson here, or undo in GUI mode would replace the DSL with its compiled
// JSON.

function handleKeydown(e: KeyboardEvent) {
  const isMod = e.metaKey || e.ctrlKey
  if (!isMod) return

  // Cmd+S / Ctrl+S: ALWAYS save (even when focus is in CodeMirror/inputs)
  if (e.key === 's') {
    e.preventDefault()
    if (store.dirty && !store.parseError) {
      store.save()
    }
    return
  }

  // Cmd+Z / Cmd+Shift+Z: skip if focus inside CodeMirror/inputs (CM handles its own undo)
  if (e.target instanceof Element && (e.target.closest('.cm-editor') || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement)) return

  if (e.key === 'z' && !e.shiftKey) {
    e.preventDefault()
    store.undo()
  } else if ((e.key === 'z' && e.shiftKey) || e.key === 'y') {
    e.preventDefault()
    store.redo()
  }
}

function handleUndo() {
  store.undo()
}

function handleRedo() {
  store.redo()
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
})
</script>

<template>
  <div class="flex-1 flex flex-col min-h-0">
    <!-- Header -->
    <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
      <div class="flex items-center gap-3">
        <h2 class="text-accent text-sm font-bold">Layout Editor</h2>
        <span v-if="activeName" class="text-text text-[12px] font-mono">
          {{ activeName }}
        </span>
        <!-- "● Modified" indicator: shows unsaved-vs-disk state (Option A from spec) -->
        <span v-if="dirty" class="text-warning text-[10px] font-bold">● Modified</span>
        <select
          v-if="activeName && mockStateCount > 1"
          :value="activeMockStateId"
          class="p-1.5 rounded border border-accent bg-bg text-text text-xs"
          @change="activeMockStateId = Number(($event.target as HTMLSelectElement).value)"
        >
          <option v-for="i in mockStateCount" :key="i - 1" :value="i - 1">Mock state {{ i - 1 }}</option>
        </select>
        <select
          v-if="activeName"
          :value="activeLang"
          class="p-1.5 rounded border border-accent bg-bg text-text text-xs"
          @change="activeLang = Number(($event.target as HTMLSelectElement).value)"
        >
          <option :value="0">EN</option>
          <option :value="1">ZH</option>
        </select>
      </div>
      <div class="flex gap-2 items-center">
        <span v-if="parseError" class="text-danger text-[10px] mr-1">{{ parseError }}</span>
        <button
          class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#444] disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!canUndo"
          @click="handleUndo"
        >
          ↶ Undo
        </button>
        <button
          class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#444] disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!canRedo"
          @click="handleRedo"
        >
          ↷ Redo
        </button>
        <button
          class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#444] disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!activeName || !dirty"
          :title="activeName && dirty ? 'Show diff vs saved version' : 'No unsaved changes'"
          @click="showDiff = true"
        >
          ⇆ Diff
        </button>
        <div class="flex flex-col items-end">
          <button
            class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="!dirty || !!parseError"
            @click="store.save()"
          >
            💾 Save
          </button>
          <div v-if="saveError" class="text-red-500 text-[10px] mt-1">{{ saveError }}</div>
        </div>
      </div>
    </div>

    <!-- Body -->
    <div class="flex-1 flex flex-col min-h-0">
      <div v-if="error" class="m-3 p-2 rounded bg-danger/10 border border-danger text-danger text-[11px] shrink-0">
        {{ error }}
      </div>

      <div v-if="!activeName" class="text-text-muted text-xs p-3">
        Select a layout from the sidebar to edit.
      </div>

      <div v-else-if="loading" class="text-text-muted text-xs p-3">Loading...</div>

      <template v-else>
        <!-- Preview pinned at top, never scrolls with content -->
        <div v-if="!previewAvailable" class="shrink-0 p-3 pb-2 border-b border-[rgba(255,255,255,0.06)] bg-bg-inset/30">
          <div class="flex items-center justify-center h-[432px] text-text-muted text-xs">
            Preview unavailable — legacy v1 layout (no .gui source) or DSL compile error
          </div>
        </div>
        <div v-else class="shrink-0 p-3 pb-2 border-b border-[rgba(255,255,255,0.06)] bg-bg-inset/30">
          <div class="flex items-center justify-between mb-1">
            <label class="text-[10px] text-text-muted">Preview</label>
            <span class="text-[10px] text-text-muted">{{ activeName }} · state {{ activeMockStateId }}</span>
          </div>
          <div
            class="relative mx-auto rounded border border-[rgba(255,255,255,0.1)] bg-bg-inset overflow-hidden"
            style="width: 480px; height: 432px;"
          >
            <canvas
              ref="canvasRef"
              width="160"
              height="144"
              class="border border-[rgba(255,255,255,0.06)] block"
              style="image-rendering: pixelated; width: 100%; height: 100%;"
              @click="handleCanvasClick"
            />
            <svg
              v-if="liveRects.length > 0"
              class="absolute inset-0 pointer-events-none w-full h-full"
              :viewBox="'0 0 160 144'"
              preserveAspectRatio="none"
            >
              <!-- LiveRect overlay: v2 elements or v1 boxes -->
              <template v-for="lr in liveRects" :key="lr.id">
                <rect
                  :x="lr.tileRect.tx * 8"
                  :y="lr.tileRect.ty * 8"
                  :width="lr.tileRect.tw * 8"
                  :height="lr.displayTh * 8"
                  :fill="selectedRectId === lr.id ? 'rgba(78,204,163,0.08)' : 'transparent'"
                  :stroke="selectedRectId === lr.id ? '#4ecca3' : 'rgba(255,255,255,0.2)'"
                  stroke-width="1"
                  class="pointer-events-auto"
                  :class="store.mode === 'gui' ? 'cursor-pointer' : 'cursor-move'"
                  style="vector-effect: non-scaling-stroke;"
                  @pointerdown="handleRectPointerDown($event, lr, 'move')"
                />
                <rect
                  v-if="selectedRectId === lr.id && store.mode !== 'gui'"
                  :x="lr.tileRect.tx * 8 + lr.tileRect.tw * 8 - 4"
                  :y="lr.tileRect.ty * 8 + lr.displayTh * 8 - 4"
                  width="8"
                  height="8"
                  fill="#4ecca3"
                  stroke="#1a1a2e"
                  stroke-width="1"
                  class="pointer-events-auto cursor-nwse-resize"
                  @pointerdown="handleRectPointerDown($event, lr, 'resize')"
                />
              </template>
            </svg>
          </div>
        </div>

        <!-- Side-by-side: layout source editor + Variables Panel -->
        <div class="flex-1 flex min-h-0">
          <div class="flex-1 flex flex-col min-w-0 p-3 pr-1.5">
            <label class="block text-[10px] text-text-muted mb-1">{{ store.mode === 'gui' ? 'GUI Layout (.gui)' : 'JSON Layout' }}</label>
            <div ref="editorContainer" class="flex-1 rounded border border-[rgba(255,255,255,0.1)] overflow-hidden bg-bg" />
            <GuiDslHelp v-if="store.mode === 'gui'" />
          </div>
          <div class="w-96 shrink-0 flex flex-col p-3 pl-1.5 overflow-y-auto">
            <VariablesPanel
              :variables="extractedVariables"
              :menu-name="activeName ?? ''"
              @update="onVariableUpdate"
            />
          </div>
        </div>
      </template>
    </div>

    <DiffViewer
      v-if="showDiff && activeName"
      :original="savedSnapshot ?? ''"
      :modified="rawJson"
      :original-label="`Saved: ${activeName}.json`"
      :modified-label="`Current (unsaved edits)`"
      @close="showDiff = false"
    />
  </div>
</template>
