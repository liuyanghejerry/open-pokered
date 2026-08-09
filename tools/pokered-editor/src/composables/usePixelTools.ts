import { type Ref } from 'vue'
import { usePixelStore } from '../stores/pixelStore'

export function usePixelTools(
  canvasRef: Ref<HTMLCanvasElement | null>,
  screenToPixel: (mouseX: number, mouseY: number) => { x: number; y: number } | null,
) {
  const store = usePixelStore()
  let isDrawing = false

  function onPointerDown(e: PointerEvent) {
    if (!canvasRef.value) return
    canvasRef.value.setPointerCapture(e.pointerId)
    const pixel = screenToPixel(e.clientX, e.clientY)
    if (!pixel) return

    const tool = store.activeTool

    if (tool === 'pencil' || tool === 'erase') {
      isDrawing = true
      store.beginStroke()
      if (tool === 'pencil') {
        store.drawPixel(pixel.x, pixel.y)
      } else {
        store.erasePixel(pixel.x, pixel.y)
      }
    } else if (tool === 'eyedropper') {
      store.pickColor(pixel.x, pixel.y)
    } else if (tool === 'fill') {
      store.beginStroke()
      store.fillAt(pixel.x, pixel.y)
    }
  }

  function onPointerMove(e: PointerEvent) {
    if (!isDrawing) return
    const pixel = screenToPixel(e.clientX, e.clientY)
    if (!pixel) return

    if (store.activeTool === 'pencil') {
      store.drawPixel(pixel.x, pixel.y)
    } else if (store.activeTool === 'erase') {
      store.erasePixel(pixel.x, pixel.y)
    }
  }

  function onPointerUp(e: PointerEvent) {
    if (!isDrawing) return
    isDrawing = false
    store.endStroke()
    if (canvasRef.value) {
      canvasRef.value.releasePointerCapture(e.pointerId)
    }
  }

  return { onPointerDown, onPointerMove, onPointerUp }
}
