<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { storeToRefs } from 'pinia'
import { usePixelStore } from '../stores/pixelStore'
import { DMG_COLORS } from '../types/pixel'

const store = usePixelStore()
const {
  activeColorIndex, colorMode,
  gbaSubPalettes, activeGbaSubPalette, fullColor,
} = storeToRefs(store)

const COLOR_NAMES = ['White', 'Light Gray', 'Dark Gray', 'Black']

const dmgCssColors: string[] = DMG_COLORS.map((c) => `#${c.toString(16).padStart(6, '0')}`)

function selectColor(index: number) {
  store.activeColorIndex = index
}

// ── HSL ↔ RGB utilities ─────────────────────────────────────────────────

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  s /= 100
  l /= 100
  const c = (1 - Math.abs(2 * l - 1)) * s
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1))
  const m = l - c / 2
  let r = 0, g = 0, b = 0
  if (h < 60) { r = c; g = x }
  else if (h < 120) { r = x; g = c }
  else if (h < 180) { g = c; b = x }
  else if (h < 240) { g = x; b = c }
  else if (h < 300) { r = x; b = c }
  else { r = c; b = x }
  return [
    Math.round((r + m) * 255),
    Math.round((g + m) * 255),
    Math.round((b + m) * 255),
  ]
}

function rgbToHsl(ir: number, ig: number, ib: number): [number, number, number] {
  const r = ir / 255, g = ig / 255, b = ib / 255
  const max = Math.max(r, g, b), min = Math.min(r, g, b)
  const l = (max + min) / 2
  let h = 0, s = 0
  if (max !== min) {
    const d = max - min
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
    switch (max) {
      case r: h = ((g - b) / d + (g < b ? 6 : 0)) / 6; break
      case g: h = ((b - r) / d + 2) / 6; break
      case b: h = ((r - g) / d + 4) / 6; break
    }
  }
  return [Math.round(h * 360), Math.round(s * 100), Math.round(l * 100)]
}

function rgbToCssHex(r: number, g: number, b: number): string {
  return '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')
}

function parseHex(hex: string): [number, number, number] | null {
  const m = hex.trim().match(/^#?([0-9a-fA-F]{6})$/)
  if (!m) return null
  const v = parseInt(m[1], 16)
  return [(v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff]
}

// ── FullColor picker state ──────────────────────────────────────────────

const [hue, sat, lit] = rgbToHsl(fullColor.value.r, fullColor.value.g, fullColor.value.b)
const pickerHue = ref(hue)
const pickerSat = ref(sat)
const pickerLit = ref(lit)
const pickerAlpha = ref(fullColor.value.a)
const hexInput = ref(rgbToCssHex(fullColor.value.r, fullColor.value.g, fullColor.value.b))
const hexValid = ref(true)

const previewColor = computed(() => {
  const [r, g, b] = hslToRgb(pickerHue.value, pickerSat.value, pickerLit.value)
  return `rgba(${r},${g},${b},${pickerAlpha.value / 255})`
})

const slBgStyle = computed(() => ({
  background: `linear-gradient(to top, #000, transparent),
    linear-gradient(to right, #fff, hsl(${pickerHue.value}, 100%, 50%))`,
}))

const slCursorStyle = computed(() => ({
  left: `${pickerSat.value}%`,
  top: `${100 - pickerLit.value}%`,
}))

function updateFromSl(clientX: number, clientY: number, el: HTMLElement) {
  const rect = el.getBoundingClientRect()
  const x = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
  const y = Math.max(0, Math.min(1, (clientY - rect.top) / rect.height))
  pickerSat.value = Math.round(x * 100)
  pickerLit.value = Math.round((1 - y) * 100)
  commitFullColor()
}

function updateFromHue(clientY: number, el: HTMLElement) {
  const rect = el.getBoundingClientRect()
  const y = Math.max(0, Math.min(1, (clientY - rect.top) / rect.height))
  pickerHue.value = Math.round(y * 360)
  commitFullColor()
}

function updateFromAlpha(clientX: number, el: HTMLElement) {
  const rect = el.getBoundingClientRect()
  const x = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
  pickerAlpha.value = Math.round(x * 255)
  commitFullColor()
}

function commitFullColor() {
  const [r, g, b] = hslToRgb(pickerHue.value, pickerSat.value, pickerLit.value)
  hexInput.value = rgbToCssHex(r, g, b)
  hexValid.value = true
  store.updateFullColor({ r, g, b, a: pickerAlpha.value })
}

function onHexInput() {
  const parsed = parseHex(hexInput.value)
  if (parsed) {
    hexValid.value = true
    const [r, g, b] = parsed
    const [h, s, l] = rgbToHsl(r, g, b)
    pickerHue.value = h
    pickerSat.value = s
    pickerLit.value = l
    store.updateFullColor({ r, g, b, a: pickerAlpha.value })
  } else {
    hexValid.value = false
  }
}

function onHexBlur() {
  if (!hexValid.value) {
    const [r, g, b] = hslToRgb(pickerHue.value, pickerSat.value, pickerLit.value)
    hexInput.value = rgbToCssHex(r, g, b)
    hexValid.value = true
  }
}

// Sync from store when fullColor changes externally (eyedropper)
watch(fullColor, (fc) => {
  const [h, s, l] = rgbToHsl(fc.r, fc.g, fc.b)
  pickerHue.value = h
  pickerSat.value = s
  pickerLit.value = l
  pickerAlpha.value = fc.a
  hexInput.value = rgbToCssHex(fc.r, fc.g, fc.b)
  hexValid.value = true
}, { deep: true })

// ── Pointer drag state for FullColor picker ─────────────────────────────

let dragTarget: 'sl' | 'hue' | 'alpha' | null = null

function onPointerDownSl(e: PointerEvent) {
  dragTarget = 'sl'
  const el = e.currentTarget as HTMLElement
  el.setPointerCapture(e.pointerId)
  updateFromSl(e.clientX, e.clientY, el)
}
function onPointerDownHue(e: PointerEvent) {
  dragTarget = 'hue'
  const el = e.currentTarget as HTMLElement
  el.setPointerCapture(e.pointerId)
  updateFromHue(e.clientY, el)
}
function onPointerDownAlpha(e: PointerEvent) {
  dragTarget = 'alpha'
  const el = e.currentTarget as HTMLElement
  el.setPointerCapture(e.pointerId)
  updateFromAlpha(e.clientX, el)
}
function onPointerMove(e: PointerEvent) {
  if (!dragTarget) return
  const el = e.currentTarget as HTMLElement
  if (dragTarget === 'sl') updateFromSl(e.clientX, e.clientY, el)
  else if (dragTarget === 'hue') updateFromHue(e.clientY, el)
  else if (dragTarget === 'alpha') updateFromAlpha(e.clientX, el)
}
function onPointerUp(e: PointerEvent) {
  dragTarget = null
  ;(e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId)
}

// ── Keyboard ────────────────────────────────────────────────────────────

function onKeyDown(e: KeyboardEvent) {
  if (colorMode.value !== 'dmg') return
  const key = parseInt(e.key)
  if (key >= 1 && key <= 4) {
    selectColor(key - 1)
  }
}

onMounted(() => window.addEventListener('keydown', onKeyDown))
onUnmounted(() => window.removeEventListener('keydown', onKeyDown))

// ── GBA sub-palette colors as CSS ───────────────────────────────────────

const gbaActivePalette = computed(() => {
  return gbaSubPalettes.value[activeGbaSubPalette.value]
    || gbaSubPalettes.value[0]
})

const gbaCssColors = computed(() => {
  return gbaActivePalette.value.colors.map(
    (c) => `#${c.toString(16).padStart(6, '0')}`,
  )
})

function selectGbaSubPalette(index: number) {
  store.activeGbaSubPalette = index
}
</script>

<template>
  <div class="flex items-center gap-1.5">
    <!-- DMG mode: 4 swatches -->
    <div
      v-if="colorMode === 'dmg'"
      class="flex items-center gap-1.5 px-2 py-1.5 bg-bg-inset rounded border border-[rgba(255,255,255,0.06)]"
    >
      <div
        v-for="(css, idx) in dmgCssColors"
        :key="idx"
        class="flex flex-col items-center gap-0.5"
      >
        <button
          :title="`${COLOR_NAMES[idx]} (${idx + 1})`"
          class="w-6 h-6 rounded border-2 cursor-pointer transition-all duration-75"
          :class="[
            activeColorIndex === idx
              ? 'border-accent scale-110 shadow-[0_0_6px_rgba(78,204,163,0.4)]'
              : 'border-[rgba(255,255,255,0.15)] hover:border-[rgba(255,255,255,0.4)]',
          ]"
          :style="{ backgroundColor: css }"
          @click="selectColor(idx)"
        />
        <span class="text-[9px] leading-none text-text-muted select-none">{{ idx + 1 }}</span>
      </div>
    </div>

    <!-- GBA mode: 2×8 grid + sub-palette selector -->
    <div
      v-else-if="colorMode === 'gba'"
      class="flex flex-col gap-1.5"
    >
      <!-- Sub-palette selector -->
      <div class="flex items-center gap-0.5">
        <span class="text-[9px] text-text-muted leading-none mr-1">Pal:</span>
        <button
          v-for="idx in 16"
          :key="idx"
          class="w-5 h-4 rounded text-[8px] font-bold cursor-pointer transition-colors leading-none flex items-center justify-center"
          :class="activeGbaSubPalette === idx - 1
            ? 'bg-accent text-bg'
            : 'text-text-muted hover:text-text hover:bg-[rgba(255,255,255,0.06)]'"
          @click="selectGbaSubPalette(idx - 1)"
        >{{ idx }}</button>
      </div>
      <!-- 2×8 color grid -->
      <div class="flex gap-1.5">
        <div class="grid grid-cols-8 gap-1">
          <button
            v-for="(css, idx) in gbaCssColors"
            :key="idx"
            :title="`Color ${idx + 1}`"
            class="w-5 h-5 rounded-sm border cursor-pointer transition-all duration-75"
            :class="[
              activeColorIndex === idx
                ? 'border-accent scale-110 ring-1 ring-accent/50'
                : 'border-[rgba(255,255,255,0.1)] hover:border-[rgba(255,255,255,0.4)]',
            ]"
            :style="{ backgroundColor: css }"
            @click="selectColor(idx)"
          />
        </div>
      </div>
    </div>

    <!-- FullColor mode: HSL picker -->
    <div
      v-else-if="colorMode === 'fullcolor'"
      class="flex flex-col gap-2 p-2 bg-bg-inset rounded border border-[rgba(255,255,255,0.06)]"
    >
      <div class="flex gap-2">
        <!-- SL square -->
        <div
          class="relative w-[100px] h-[100px] rounded cursor-crosshair select-none shrink-0"
          :style="slBgStyle"
          @pointerdown.prevent="onPointerDownSl"
          @pointermove.prevent="onPointerMove"
          @pointerup.prevent="onPointerUp"
        >
          <div
            class="absolute w-3 h-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_2px_rgba(0,0,0,0.6)] pointer-events-none"
            :style="slCursorStyle"
          />
        </div>

        <!-- Hue slider -->
        <div
          class="relative w-4 h-[100px] rounded cursor-pointer select-none shrink-0"
          style="background: linear-gradient(to bottom, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00)"
          @pointerdown.prevent="onPointerDownHue"
          @pointermove.prevent="onPointerMove"
          @pointerup.prevent="onPointerUp"
        >
          <div
            class="absolute left-0 right-0 h-1 -translate-y-1/2 bg-white rounded shadow-[0_0_2px_rgba(0,0,0,0.6)] pointer-events-none"
            :style="{ top: `${(pickerHue / 360) * 100}%` }"
          />
        </div>
      </div>

      <!-- Hex input + alpha + preview -->
      <div class="flex items-center gap-2">
        <input
          type="text"
          v-model="hexInput"
          class="w-[70px] h-5 px-1.5 rounded text-[10px] font-mono bg-bg border outline-none"
          :class="hexValid ? 'border-[rgba(255,255,255,0.1)] text-text' : 'border-danger text-danger'"
          @input="onHexInput"
          @blur="onHexBlur"
        />
        <!-- Alpha slider -->
        <div
          class="relative w-[60px] h-4 rounded cursor-pointer select-none shrink-0"
          style="background: linear-gradient(to right, transparent, currentColor)"
          @pointerdown.prevent="onPointerDownAlpha"
          @pointermove.prevent="onPointerMove"
          @pointerup.prevent="onPointerUp"
        >
          <div
            class="absolute inset-0 rounded"
            style="background-image: repeating-conic-gradient(#888 0% 25%, #ccc 0% 50%); background-size: 6px 6px;"
          />
          <div
            class="absolute inset-0 rounded"
            :style="{ background: `linear-gradient(to right, transparent, ${previewColor})` }"
          />
          <div
            class="absolute top-0 bottom-0 w-1.5 -translate-x-1/2 bg-white rounded shadow-[0_0_2px_rgba(0,0,0,0.6)] pointer-events-none"
            :style="{ left: `${(pickerAlpha / 255) * 100}%` }"
          />
        </div>
        <!-- Preview -->
        <div
          class="w-5 h-5 rounded border border-[rgba(255,255,255,0.15)] shrink-0"
          :style="{ backgroundColor: previewColor }"
        />
      </div>
    </div>
  </div>
</template>
