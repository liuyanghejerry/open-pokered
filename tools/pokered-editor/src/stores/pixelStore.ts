import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { AssetEntry, AssetCategory, DrawTool, TilesetTileMeta, TileColorMode, Rgba, GbaSubPalette, DmgPalette } from '../types/pixel'
import { DMG_COLORS, hexToRgba, createDefaultGbaPalettes } from '../types/pixel'
import { speciesToSpriteName } from '../types/pokemon'
import { gfxRel, assetUrl } from '../utils/assetUrl'
import { dataFetch } from '../composables/dataAdapter'

const MAX_UNDO_DEPTH = 50
const DEFAULT_COLOR_INDEX = 3
const WHITE_INDEX = 0
const FRAME_FRONT = 0
const FRAME_BACK = 1

function hexToRgb(hex: number): [number, number, number] {
  return [(hex >> 16) & 0xff, (hex >> 8) & 0xff, hex & 0xff]
}

function rgbToHex(r: number, g: number, b: number): number {
  return (r << 16) | (g << 8) | b
}

function cloneImageData(source: ImageData): ImageData {
  return new ImageData(
    new Uint8ClampedArray(source.data),
    source.width,
    source.height,
  )
}

export const usePixelStore = defineStore('pixel', () => {
  // ── Core state ──────────────────────────────────────────────────────
  const activeAsset = ref<AssetEntry | null>(null)
  const imageData = ref<ImageData | null>(null)
  const canvasWidth = ref(0)
  const canvasHeight = ref(0)
  const activeTool = ref<DrawTool>('pencil')
  const activeColorIndex = ref(DEFAULT_COLOR_INDEX)
  const zoom = ref(4)
  const showGrid = ref(true)
  const isDirty = ref(false)
  const activeFrame = ref(FRAME_FRONT)
  const isTilesetMode = ref(false)
  const tilesetMeta = ref<TilesetTileMeta | null>(null)
  const frames = ref<AssetEntry[]>([])
  const previousTool = ref<DrawTool>('pencil')
  const loading = ref(false)
  const error = ref<string | null>(null)
  const renderVersion = ref(0)

  // ── Color mode state ──────────────────────────────────────────────────
  const colorMode = ref<TileColorMode>('dmg')
  const dmgPalette = ref<DmgPalette>({ colors: DMG_COLORS })
  const gbaSubPalettes = ref<GbaSubPalette[]>(createDefaultGbaPalettes())
  const activeGbaSubPalette = ref(0)
  const fullColor = ref<Rgba>({ r: 255, g: 0, b: 0, a: 255 })

  function setColorMode(mode: TileColorMode) {
    colorMode.value = mode
    if (mode === 'dmg' && activeColorIndex.value > 3) {
      activeColorIndex.value = DEFAULT_COLOR_INDEX
    }
  }

  function getColorForIndex(index: number): [number, number, number, number] {
    switch (colorMode.value) {
      case 'dmg':
        return hexToRgba(DMG_COLORS[index] ?? DMG_COLORS[0])
      case 'gba': {
        const pal = gbaSubPalettes.value[activeGbaSubPalette.value]
        if (!pal) return [255, 255, 255, 255]
        return hexToRgba(pal.colors[index] ?? pal.colors[0])
      }
      case 'fullcolor':
        return [fullColor.value.r, fullColor.value.g, fullColor.value.b, fullColor.value.a]
    }
  }

  function updateFullColor(update: Partial<Rgba>) {
    fullColor.value = { ...fullColor.value, ...update }
  }

  function bumpRenderVersion() { renderVersion.value++ }

  // ── Undo / Redo ─────────────────────────────────────────────────────
  const undoStack = ref<{ imageData: ImageData }[]>([])
  const redoStack = ref<{ imageData: ImageData }[]>([])

  // Stroke tracking: push undo once per brush stroke, not per pixel.
  let strokeSnapshotPushed = false

  // ── Getters ─────────────────────────────────────────────────────────
  const canUndo = computed(() => undoStack.value.length > 1)
  const canRedo = computed(() => redoStack.value.length > 0)
  const activeFrames = computed(() => frames.value)

  // ── Internal helpers ────────────────────────────────────────────────

  function pushUndoSnapshot() {
    if (!imageData.value) return
    const snapshot = cloneImageData(imageData.value)
    undoStack.value.push({ imageData: snapshot })
    if (undoStack.value.length > MAX_UNDO_DEPTH) {
      undoStack.value.shift()
    }
    redoStack.value = []
  }

  function hexFromPixel(x: number, y: number): number {
    if (!imageData.value) return DMG_COLORS[0]
    const idx = (y * canvasWidth.value + x) * 4
    const d = imageData.value.data
    return (d[idx] << 16) | (d[idx + 1] << 8) | d[idx + 2]
  }

  function setPixel(x: number, y: number, hexColor: number) {
    if (!imageData.value) return
    const [r, g, b] = hexToRgb(hexColor)
    const offset = (y * canvasWidth.value + x) * 4
    imageData.value.data[offset] = r
    imageData.value.data[offset + 1] = g
    imageData.value.data[offset + 2] = b
    imageData.value.data[offset + 3] = 255
  }

  function inBounds(x: number, y: number): boolean {
    return x >= 0 && x < canvasWidth.value && y >= 0 && y < canvasHeight.value
  }

  // ── URL helper ──────────────────────────────────────────────────────

  function getAssetUrl(entry: AssetEntry): string {
    return assetUrl(getAssetRelUrl(entry))
  }

  /** Base-less /gfx URL — used with dataFetch so static mode can serve the
   *  edited delta instead of the baseline; getAssetUrl keeps the base prefix
   *  for <img> / background-image consumers. */
  function getAssetRelUrl(entry: AssetEntry): string {
    const cat: AssetCategory = entry.category
    switch (cat) {
      case 'pokemon-front':
        return gfxRel(`pokemon/front/${entry.filename}`)
      case 'pokemon-back':
        return gfxRel(`pokemon/back/${entry.filename}`)
      case 'trainer':
        return gfxRel(`trainers/${entry.filename}`)
      case 'npc':
        return gfxRel(`sprites/${entry.filename}`)
      case 'tileset':
        return gfxRel(`tilesets/${entry.filename}`)
      case 'ui':
      case 'effects':
        return gfxRel(entry.filename)
    }
  }

  // ── Actions ─────────────────────────────────────────────────────────

  function setTool(tool: DrawTool) {
    if (tool === 'eyedropper' && activeTool.value !== 'eyedropper') {
      previousTool.value = activeTool.value
    }
    activeTool.value = tool
  }

  async function loadAsset(entry: AssetEntry) {
    loading.value = true
    error.value = null
    try {
      const resp = await dataFetch(getAssetRelUrl(entry))
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const blob = await resp.blob()
      const img = await createImageBitmap(blob)

      const w = img.width
      const h = img.height
      const canvas = new OffscreenCanvas(w, h)
      const ctx = canvas.getContext('2d')!
      ctx.drawImage(img, 0, 0)
      const data = ctx.getImageData(0, 0, w, h)

      activeAsset.value = entry
      imageData.value = data
      canvasWidth.value = w
      canvasHeight.value = h
      isDirty.value = false
      isTilesetMode.value = false
      tilesetMeta.value = null

      // Discover frames for Pokemon assets (front ↔ back)
      if (entry.category === 'pokemon-front') {
        const stem = speciesToSpriteName(entry.id)
        frames.value = [
          entry,
          {
            category: 'pokemon-back',
            id: entry.id,
            filename: `${stem}b.png`,
            displayName: entry.displayName,
          },
        ]
        activeFrame.value = FRAME_FRONT
      } else if (entry.category === 'pokemon-back') {
        const stem = speciesToSpriteName(entry.id)
        frames.value = [
          {
            category: 'pokemon-front',
            id: entry.id,
            filename: `${stem}.png`,
            displayName: entry.displayName,
          },
          entry,
        ]
        activeFrame.value = FRAME_BACK
      } else {
        frames.value = [entry]
        activeFrame.value = FRAME_FRONT
      }

      undoStack.value = [{ imageData: cloneImageData(data) }]
      redoStack.value = []
      bumpRenderVersion()
    } catch (e) {
      error.value = `Failed to load asset: ${(e as Error).message}`
    } finally {
      loading.value = false
    }
  }

  function beginStroke() {
    strokeSnapshotPushed = false
  }

  function endStroke() {
    strokeSnapshotPushed = false
  }

  /**
   * Replace the canvas with an externally-produced image (e.g. AI generation).
   * Pushes an undo snapshot and marks dirty; saving stays on save()/PUT /gfx.
   */
  function loadGeneratedImage(data: ImageData) {
    if (!activeAsset.value) return
    pushUndoSnapshot()
    imageData.value = data
    canvasWidth.value = data.width
    canvasHeight.value = data.height
    isDirty.value = true
    bumpRenderVersion()
  }

  function drawPixel(x: number, y: number) {
    if (!inBounds(x, y) || !imageData.value) return
    if (!strokeSnapshotPushed) {
      pushUndoSnapshot()
      strokeSnapshotPushed = true
    }
    const [r, g, b, a] = getColorForIndex(activeColorIndex.value)
    const offset = (y * canvasWidth.value + x) * 4
    imageData.value.data[offset] = r
    imageData.value.data[offset + 1] = g
    imageData.value.data[offset + 2] = b
    imageData.value.data[offset + 3] = a
    isDirty.value = true
    bumpRenderVersion()
  }

  function erasePixel(x: number, y: number) {
    if (!inBounds(x, y) || !imageData.value) return
    if (!strokeSnapshotPushed) {
      pushUndoSnapshot()
      strokeSnapshotPushed = true
    }
    setPixel(x, y, DMG_COLORS[WHITE_INDEX])
    isDirty.value = true
    bumpRenderVersion()
  }

  function pickColor(x: number, y: number) {
    if (!inBounds(x, y) || !imageData.value) return
    const target = hexFromPixel(x, y)
    const [tr, tg, tb] = hexToRgb(target)

    switch (colorMode.value) {
      case 'dmg': {
        let bestIdx = 0
        let bestDist = Infinity
        for (let i = 0; i < DMG_COLORS.length; i++) {
          const [cr, cg, cb] = hexToRgb(DMG_COLORS[i])
          const dist = (tr - cr) ** 2 + (tg - cg) ** 2 + (tb - cb) ** 2
          if (dist < bestDist) {
            bestDist = dist
            bestIdx = i
          }
        }
        activeColorIndex.value = bestIdx
        break
      }
      case 'gba': {
        const pal = gbaSubPalettes.value[activeGbaSubPalette.value]
        if (!pal) break
        let bestIdx = 0
        let bestDist = Infinity
        for (let i = 0; i < pal.colors.length; i++) {
          const [cr, cg, cb] = hexToRgb(pal.colors[i])
          const dist = (tr - cr) ** 2 + (tg - cg) ** 2 + (tb - cb) ** 2
          if (dist < bestDist) {
            bestDist = dist
            bestIdx = i
          }
        }
        activeColorIndex.value = bestIdx
        break
      }
      case 'fullcolor':
        fullColor.value = { r: tr, g: tg, b: tb, a: 255 }
        break
    }
    activeTool.value = previousTool.value
  }

  function fillAt(startX: number, startY: number) {
    if (!inBounds(startX, startY) || !imageData.value) return
    const targetHex = hexFromPixel(startX, startY)
    const [fillR, fillG, fillB, fillA] = getColorForIndex(activeColorIndex.value)
    const fillColorHex = rgbToHex(fillR, fillG, fillB)
    if (targetHex === fillColorHex) return

    pushUndoSnapshot()

    const w = canvasWidth.value
    const h = canvasHeight.value
    const data = imageData.value.data

    // Iterative BFS flood fill, 4-directional (avoids stack overflow on large regions)
    const queue: [number, number][] = [[startX, startY]]
    const visited = new Uint8Array(w * h)
    visited[startY * w + startX] = 1

    let head = 0
    while (head < queue.length) {
      const [cx, cy] = queue[head++]
      const offset = (cy * w + cx) * 4
      data[offset] = fillR
      data[offset + 1] = fillG
      data[offset + 2] = fillB
      data[offset + 3] = fillA

      const neighbors: [number, number][] = [
        [cx, cy - 1],
        [cx, cy + 1],
        [cx - 1, cy],
        [cx + 1, cy],
      ]
      for (const [nx, ny] of neighbors) {
        if (nx < 0 || nx >= w || ny < 0 || ny >= h) continue
        const vidx = ny * w + nx
        if (visited[vidx]) continue
        const noffset = vidx * 4
        const ncolor =
          (data[noffset] << 16) | (data[noffset + 1] << 8) | data[noffset + 2]
        if (ncolor !== targetHex) continue
        visited[vidx] = 1
        queue.push([nx, ny])
      }
    }

    isDirty.value = true
    bumpRenderVersion()
  }

  function undo(): boolean {
    if (undoStack.value.length <= 1) return false
    const current = undoStack.value.pop()!
    redoStack.value.push({ imageData: cloneImageData(imageData.value!) })
    imageData.value = current.imageData
    canvasWidth.value = current.imageData.width
    canvasHeight.value = current.imageData.height
    isDirty.value = true
    bumpRenderVersion()
    return true
  }

  function redo(): boolean {
    if (redoStack.value.length === 0) return false
    const next = redoStack.value.pop()!
    undoStack.value.push({ imageData: cloneImageData(imageData.value!) })
    imageData.value = next.imageData
    canvasWidth.value = next.imageData.width
    canvasHeight.value = next.imageData.height
    isDirty.value = true
    bumpRenderVersion()
    return true
  }

  async function save() {
    if (!activeAsset.value || !imageData.value) return
    const canvas = new OffscreenCanvas(canvasWidth.value, canvasHeight.value)
    const ctx = canvas.getContext('2d')!
    ctx.putImageData(imageData.value, 0, 0)
    const blob = await canvas.convertToBlob({ type: 'image/png' })

    const url = getAssetRelUrl(activeAsset.value)
    const resp = await dataFetch(url, { method: 'PUT', body: blob })
    if (!resp.ok) {
      error.value = `Save failed: HTTP ${resp.status}`
      return
    }
    isDirty.value = false
    undoStack.value = [{ imageData: cloneImageData(imageData.value) }]
    redoStack.value = []
    bumpRenderVersion()
  }

  async function switchFrame(index: number) {
    const target = frames.value[index]
    if (!target) return
    await loadAsset(target)
    activeFrame.value = index
  }

  async function loadTilesetTile(tilesetName: string, tileIndex: number) {
    loading.value = true
    error.value = null
    try {
      const url = gfxRel(`tilesets/${tilesetName}.png`)
      const resp = await dataFetch(url)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const blob = await resp.blob()
      const img = await createImageBitmap(blob)

      const tilePixelWidth = img.width
      const tilesPerRow = tilePixelWidth / 8
      const tileX = (tileIndex % tilesPerRow) * 8
      const tileY = Math.floor(tileIndex / tilesPerRow) * 8

      const canvas = new OffscreenCanvas(tilePixelWidth, img.height)
      const ctx = canvas.getContext('2d')!
      ctx.drawImage(img, 0, 0)
      const fullImageData = ctx.getImageData(0, 0, tilePixelWidth, img.height)

      const tileData = new ImageData(8, 8)
      for (let ty = 0; ty < 8; ty++) {
        for (let tx = 0; tx < 8; tx++) {
          const srcIdx = ((tileY + ty) * tilePixelWidth + (tileX + tx)) * 4
          const dstIdx = (ty * 8 + tx) * 4
          tileData.data[dstIdx] = fullImageData.data[srcIdx]
          tileData.data[dstIdx + 1] = fullImageData.data[srcIdx + 1]
          tileData.data[dstIdx + 2] = fullImageData.data[srcIdx + 2]
          tileData.data[dstIdx + 3] = fullImageData.data[srcIdx + 3]
        }
      }

      imageData.value = tileData
      canvasWidth.value = 8
      canvasHeight.value = 8
      isTilesetMode.value = true
      tilesetMeta.value = { tilesetName, tileIndex, x: tileX, y: tileY }
      activeAsset.value = {
        category: 'tileset',
        id: tilesetName,
        filename: `${tilesetName}.png`,
        displayName: `${tilesetName} Tile #${tileIndex}`,
      }
      isDirty.value = false

      undoStack.value = [{ imageData: cloneImageData(tileData) }]
      redoStack.value = []
      bumpRenderVersion()
    } catch (e) {
      error.value = `Failed to load tileset tile: ${(e as Error).message}`
    } finally {
      loading.value = false
    }
  }

  async function saveTilesetTile() {
    if (!isTilesetMode.value || !tilesetMeta.value || !imageData.value) return
    const meta = tilesetMeta.value
    const url = gfxRel(`tilesets/${meta.tilesetName}.png`)

    loading.value = true
    error.value = null
    try {
      const resp = await dataFetch(url)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const blob = await resp.blob()
      const img = await createImageBitmap(blob)

      const tilePixelWidth = img.width
      const tilePixelHeight = img.height

      const canvas = new OffscreenCanvas(tilePixelWidth, tilePixelHeight)
      const ctx = canvas.getContext('2d')!
      ctx.drawImage(img, 0, 0)
      const fullImageData = ctx.getImageData(0, 0, tilePixelWidth, tilePixelHeight)

      for (let ty = 0; ty < 8; ty++) {
        for (let tx = 0; tx < 8; tx++) {
          const srcIdx = (ty * 8 + tx) * 4
          const dstIdx = ((meta.y + ty) * tilePixelWidth + (meta.x + tx)) * 4
          fullImageData.data[dstIdx] = imageData.value.data[srcIdx]
          fullImageData.data[dstIdx + 1] = imageData.value.data[srcIdx + 1]
          fullImageData.data[dstIdx + 2] = imageData.value.data[srcIdx + 2]
          fullImageData.data[dstIdx + 3] = imageData.value.data[srcIdx + 3]
        }
      }

      ctx.putImageData(fullImageData, 0, 0)
      const outBlob = await canvas.convertToBlob({ type: 'image/png' })
      const putResp = await dataFetch(url, { method: 'PUT', body: outBlob })
      if (!putResp.ok) throw new Error(`HTTP ${putResp.status}`)

      isDirty.value = false
      undoStack.value = [{ imageData: cloneImageData(imageData.value) }]
      redoStack.value = []
      bumpRenderVersion()
    } catch (e) {
      error.value = `Failed to save tileset tile: ${(e as Error).message}`
    } finally {
      loading.value = false
    }
  }

  // ── Format export ───────────────────────────────────────────────────

  /**
   * Encode a single 2bpp tile row into low/high bytes.
   * `colors` must be 8 values (0–3 per pixel).
   */
  function encode2bppRow(colors: number[]): [number, number] {
    let lo = 0
    let hi = 0
    for (let col = 0; col < 8; col++) {
      const bit = 7 - col
      const idx = colors[col] ?? 0
      if (idx & 1) lo |= 1 << bit
      if (idx & 2) hi |= 1 << bit
    }
    return [lo, hi]
  }

  /**
   * Map an RGB pixel (from ImageData) to the closest DMG colour index (0–3).
   */
  function pixelRgbToDmgIndex(r: number, g: number, b: number): number {
    let bestIdx = 0
    let bestDist = Infinity
    for (let i = 0; i < DMG_COLORS.length; i++) {
      const [cr, cg, cb] = hexToRgb(DMG_COLORS[i])
      const dist = (r - cr) ** 2 + (g - cg) ** 2 + (b - cb) ** 2
      if (dist < bestDist) {
        bestDist = dist
        bestIdx = i
      }
    }
    return bestIdx
  }

  /**
   * Map an RGB pixel to the closest index in the current GBA sub-palette (0–15).
   */
  function pixelRgbToGbaIndex(r: number, g: number, b: number): number {
    const pal = gbaSubPalettes.value[activeGbaSubPalette.value]
    if (!pal) return 0
    let bestIdx = 0
    let bestDist = Infinity
    for (let i = 0; i < pal.colors.length; i++) {
      const [cr, cg, cb] = hexToRgb(pal.colors[i])
      const dist = (r - cr) ** 2 + (g - cg) ** 2 + (b - cb) ** 2
      if (dist < bestDist) {
        bestDist = dist
        bestIdx = i
      }
    }
    return bestIdx
  }

  /**
   * Export the current image as a Game Boy 2bpp blob.
   *
   * Each 8×8 tile region is encoded as 16 bytes (2 bytes/row, low+high bitplanes).
   * Out-of-bounds areas (when width/height is not a multiple of 8) are padded
   * with colour index 0.
   */
  function exportAs2bpp(): Blob | null {
    if (!imageData.value) return null
    const w = imageData.value.width
    const h = imageData.value.height
    const data = imageData.value.data
    const tilesX = Math.ceil(w / 8)
    const tilesY = Math.ceil(h / 8)
    const bytes = new Uint8Array(tilesY * tilesX * 16)

    for (let ty = 0; ty < tilesY; ty++) {
      for (let tx = 0; tx < tilesX; tx++) {
        const tileBase = (ty * tilesX + tx) * 16
        const colors: number[] = new Array(8)
        for (let row = 0; row < 8; row++) {
          const py = ty * 8 + row
          for (let col = 0; col < 8; col++) {
            const px = tx * 8 + col
            if (px < w && py < h) {
              const offset = (py * w + px) * 4
              colors[col] = pixelRgbToDmgIndex(data[offset], data[offset + 1], data[offset + 2])
            } else {
              colors[col] = 0
            }
          }
          const [lo, hi] = encode2bppRow(colors)
          bytes[tileBase + row * 2] = lo
          bytes[tileBase + row * 2 + 1] = hi
        }
      }
    }

    return new Blob([bytes], { type: 'application/octet-stream' })
  }

  /**
   * Export the current image as a GBA 4bpp blob.
   *
   * Each 8×8 tile is encoded as 32 bytes: 2 bitplanes × 16 bytes each.
   * Plane 0 (first 16 bytes) contributes bits 0–1; plane 1 (next 16 bytes)
   * contributes bits 2–3, giving 16 colours (0–15) per tile.
   * Uses the current active GBA sub-palette for colour matching.
   */
  function exportAs4bpp(): Blob | null {
    if (!imageData.value) return null
    const pal = gbaSubPalettes.value[activeGbaSubPalette.value]
    if (!pal) return null
    const w = imageData.value.width
    const h = imageData.value.height
    const data = imageData.value.data
    const tilesX = Math.ceil(w / 8)
    const tilesY = Math.ceil(h / 8)
    const bytes = new Uint8Array(tilesY * tilesX * 32)

    for (let ty = 0; ty < tilesY; ty++) {
      for (let tx = 0; tx < tilesX; tx++) {
        const tileBase = (ty * tilesX + tx) * 32
        const colors: number[] = new Array(8)
        for (let row = 0; row < 8; row++) {
          const py = ty * 8 + row
          for (let col = 0; col < 8; col++) {
            const px = tx * 8 + col
            if (px < w && py < h) {
              const offset = (py * w + px) * 4
              colors[col] = pixelRgbToGbaIndex(data[offset], data[offset + 1], data[offset + 2])
            } else {
              colors[col] = 0
            }
          }
          // Plane 0: bits 0–1
          const p0 = colors.map((c) => c & 0b0011)
          const [p0_lo, p0_hi] = encode2bppRow(p0)
          bytes[tileBase + row * 2] = p0_lo
          bytes[tileBase + row * 2 + 1] = p0_hi
          // Plane 1: bits 2–3
          const p1 = colors.map((c) => (c >> 2) & 0b0011)
          const [p1_lo, p1_hi] = encode2bppRow(p1)
          bytes[tileBase + 16 + row * 2] = p1_lo
          bytes[tileBase + 16 + row * 2 + 1] = p1_hi
        }
      }
    }

    return new Blob([bytes], { type: 'application/octet-stream' })
  }

  /**
   * Export the active GBA sub-palette as a `.pal` text file.
   *
   * Format: 16 lines of `RR,GG,BB` uppercase hex values (no alpha).
   */
  function exportAsPal(): Blob | null {
    const pal = gbaSubPalettes.value[activeGbaSubPalette.value]
    if (!pal) return null
    const lines = pal.colors.map((c) => {
      const r = (c >> 16) & 0xff
      const g = (c >> 8) & 0xff
      const b = c & 0xff
      return `${r.toString(16).padStart(2, '0').toUpperCase()},${g.toString(16).padStart(2, '0').toUpperCase()},${b.toString(16).padStart(2, '0').toUpperCase()}`
    })
    return new Blob([lines.join('\n') + '\n'], { type: 'text/plain' })
  }

  return {
    activeAsset,
    imageData,
    canvasWidth,
    canvasHeight,
    activeTool,
    activeColorIndex,
    zoom,
    showGrid,
    undoStack,
    redoStack,
    isDirty,
    activeFrame,
    isTilesetMode,
    tilesetMeta,
    frames,
    previousTool,
    loading,
    error,
    canUndo,
    canRedo,
    dmgPalette,
    activeFrames,
    renderVersion,
    colorMode,
    gbaSubPalettes,
    activeGbaSubPalette,
    fullColor,
    setTool,
    getAssetUrl,
    loadAsset,
    loadGeneratedImage,
    beginStroke,
    endStroke,
    drawPixel,
    erasePixel,
    pickColor,
    fillAt,
    undo,
    redo,
    save,
    switchFrame,
    loadTilesetTile,
    saveTilesetTile,
    setColorMode,
    getColorForIndex,
    updateFullColor,
    exportAs2bpp,
    exportAs4bpp,
    exportAsPal,
  }
})
