import { ref, watch, type Ref } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { TILE_SIZE, BLOCK_TILES } from '../types/constants'
import type { TileInfo, CoordEvent } from '../types'
import { renderTilesAndCollision, renderGrid } from './renderTiles'
import { renderWarps, renderSigns, renderNpcs, renderCoordEvents, renderSelectionHighlight, renderConnections } from './renderOverlays'
import { getTileInfoAt, hasClickableEntity } from './hitTest'

export function useMapRenderer(canvasRef: Ref<HTMLCanvasElement | null>) {
  const store = useMapStore()
  const tooltip = ref<TileInfo | null>(null)
  const tooltipPosition = ref({ x: 0, y: 0 })
  const hoveringClickable = ref(false)
  let animationFrameId: number | null = null

  function getCoordEvents(): CoordEvent[] {
    const config = store.currentScriptConfig
    if (!config?.coordEvents) return []
    return config.coordEvents.map(ce => ({ x: ce.position[0], y: ce.position[1], trigger: ce.trigger }))
  }

  function render() {
    const canvas = canvasRef.value
    const map = store.currentMap
    if (!canvas || !map) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const pw = map.header.width * BLOCK_TILES * TILE_SIZE
    const ph = map.header.height * BLOCK_TILES * TILE_SIZE
    canvas.width = pw * store.zoom
    canvas.height = ph * store.zoom
    canvas.style.width = `${pw * store.zoom}px`
    canvas.style.height = `${ph * store.zoom}px`

    ctx.imageSmoothingEnabled = false
    ctx.scale(store.zoom, store.zoom)
    ctx.fillStyle = '#000'
    ctx.fillRect(0, 0, pw, ph)

    const { showTiles, showCollision, showGrid, showWarps, showSigns, showNpcs, showCoordEvents, showConnections } = store.displayOptions
    const blockset = store.getBlockset(map.header.tileset)
    const tilesetImg = store.tilesetImages[map.header.tileset]
    const blocks = store.currentBlocks
    const passable = store.currentPassableTiles

    renderTilesAndCollision(ctx, map.header.width, map.header.height, blocks, passable, blockset, tilesetImg, showTiles, showCollision)

    if (showGrid) {
      renderGrid(ctx, pw, ph)
    }

    if (showWarps && map.warps) {
      renderWarps(ctx, map.warps)
    }

    if (showSigns && map.signs) {
      renderSigns(ctx, map.signs)
    }

    if (showNpcs && map.npcs) {
      renderNpcs(ctx, map.npcs)
    }

    if (showCoordEvents) {
      const coordEvents = getCoordEvents()
      if (coordEvents.length > 0) {
        renderCoordEvents(ctx, coordEvents)
      }
    }

    if (showConnections && map.connections) {
      renderConnections(ctx, map.connections, map.header.width, map.header.height)
    }

    if (store.selectedEntity) {
      renderSelectionHighlight(ctx, store.selectedEntity)
    }
  }

  function startSelectionAnimation() {
    if (animationFrameId != null) return
    function loop() {
      render()
      if (store.selectedEntity) {
        animationFrameId = requestAnimationFrame(loop)
      } else {
        animationFrameId = null
      }
    }
    loop()
  }

  function stopSelectionAnimation() {
    if (animationFrameId != null) {
      cancelAnimationFrame(animationFrameId)
      animationFrameId = null
    }
  }

  function paintAtClient(clientX: number, clientY: number) {
    const canvas = canvasRef.value
    const map = store.currentMap
    if (!canvas || !map) return
    const rect = canvas.getBoundingClientRect()
    const TS = TILE_SIZE * store.zoom
    const x = Math.floor((clientX - rect.left) / TS)
    const y = Math.floor((clientY - rect.top) / TS)
    const blockX = Math.floor(x / BLOCK_TILES)
    const blockY = Math.floor(y / BLOCK_TILES)
    if (blockX < 0 || blockX >= map.header.width || blockY < 0 || blockY >= map.header.height) return
    const blockIndex = blockY * map.header.width + blockX
    store.setBlockAt(blockIndex, store.selectedBlockId)
    render()
  }

  let painting = false

  function handleCanvasClick(e: MouseEvent) {
    const canvas = canvasRef.value
    const map = store.currentMap
    if (!canvas || !map) return

    if (store.currentTool === 'edit-tiles') {
      paintAtClient(e.clientX, e.clientY)
      return
    }

    const blocks = store.currentBlocks
    const passable = store.currentPassableTiles
    const coordEvents = getCoordEvents()
    const info = getTileInfoAt(canvas, map, store.getBlockset(map.header.tileset), blocks, passable, coordEvents, store.zoom, e.clientX, e.clientY)
    if (!info) return

    if (info.warp) {
      const warpIndex = map.warps?.indexOf(info.warp) ?? -1
      if (info.warp.destMap) {
        store.navigateToMap(info.warp.destMap)
      } else {
        store.selectEntity({ type: 'warp', data: info.warp, index: warpIndex })
      }
      return
    }

    if (info.sign) {
      const signIndex = map.signs?.indexOf(info.sign) ?? -1
      store.selectEntity({ type: 'sign', data: info.sign, index: signIndex })
      return
    }

    if (info.npc) {
      const npcIndex = map.npcs?.indexOf(info.npc) ?? -1
      store.selectEntity({ type: 'npc', data: info.npc, index: npcIndex })
      return
    }

    if (info.coordEvent) {
      const ceIndex = coordEvents.indexOf(info.coordEvent)
      store.selectEntity({ type: 'coordEvent', data: info.coordEvent, index: ceIndex })
      return
    }

    store.selectEntity(null)
  }

  function handleCanvasMouseDown(e: MouseEvent) {
    if (store.currentTool !== 'edit-tiles') return
    if (e.button !== 0) return
    painting = true
    paintAtClient(e.clientX, e.clientY)
  }

  function handleCanvasMouseUp() {
    painting = false
  }

  function handleCanvasMouseMove(e: MouseEvent) {
    const canvas = canvasRef.value
    const map = store.currentMap
    if (!canvas || !map) return

    if (store.currentTool === 'edit-tiles' && painting) {
      paintAtClient(e.clientX, e.clientY)
    }

    const blocks = store.currentBlocks
    const passable = store.currentPassableTiles
    const coordEvents = getCoordEvents()
    const info = getTileInfoAt(canvas, map, store.getBlockset(map.header.tileset), blocks, passable, coordEvents, store.zoom, e.clientX, e.clientY)
    tooltip.value = info
    tooltipPosition.value = { x: e.clientX + 15, y: e.clientY + 15 }

    if (store.currentTool === 'view' && info) {
      hoveringClickable.value = hasClickableEntity(info)
    } else {
      hoveringClickable.value = false
    }
  }

  function handleCanvasMouseLeave() {
    tooltip.value = null
    hoveringClickable.value = false
    painting = false
  }

  watch(
    () => [
      store.currentMapIndex,
      store.zoom,
      store.displayOptions.showTiles,
      store.displayOptions.showCollision,
      store.displayOptions.showWarps,
      store.displayOptions.showSigns,
      store.displayOptions.showNpcs,
      store.displayOptions.showGrid,
      store.displayOptions.showCoordEvents,
      store.displayOptions.showConnections,
      store.tilesetImages,
      store.currentBlocks,
    ],
    () => render(),
    { deep: true },
  )

  watch(
    () => store.selectedEntity,
    (entity) => {
      if (entity) {
        startSelectionAnimation()
      } else {
        stopSelectionAnimation()
        render()
      }
    },
  )

  return {
    render,
    tooltip,
    tooltipPosition,
    hoveringClickable,
    handleCanvasClick,
    handleCanvasMouseMove,
    handleCanvasMouseLeave,
    handleCanvasMouseDown,
    handleCanvasMouseUp,
  }
}
