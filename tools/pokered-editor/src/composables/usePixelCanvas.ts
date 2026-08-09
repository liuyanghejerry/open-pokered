import { watch, type Ref } from 'vue'
import { usePixelStore } from '../stores/pixelStore'

export function usePixelCanvas(canvasRef: Ref<HTMLCanvasElement | null>) {
  const store = usePixelStore()

  function render() {
    const canvas = canvasRef.value
    if (!canvas || !store.imageData) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const pixelW = store.imageData.width
    const pixelH = store.imageData.height
    const z = store.zoom

    canvas.width = pixelW * z
    canvas.height = pixelH * z
    canvas.style.width = `${canvas.width}px`
    canvas.style.height = `${canvas.height}px`

    ctx.imageSmoothingEnabled = false

    ctx.fillStyle = '#1a1a2e'
    ctx.fillRect(0, 0, canvas.width, canvas.height)

    const imgData = store.imageData.data
    for (let y = 0; y < pixelH; y++) {
      for (let x = 0; x < pixelW; x++) {
        const idx = (y * pixelW + x) * 4
        const r = imgData[idx]
        const g = imgData[idx + 1]
        const b = imgData[idx + 2]
        const a = imgData[idx + 3]

        if (a === 0) continue

        ctx.fillStyle = `rgb(${r},${g},${b})`
        ctx.fillRect(x * z, y * z, z, z)
      }
    }

    if (store.showGrid) {
      ctx.strokeStyle = 'rgba(255,255,255,0.2)'
      ctx.lineWidth = 1
      for (let x = 0; x <= pixelW; x++) {
        ctx.beginPath()
        ctx.moveTo(x * z, 0)
        ctx.lineTo(x * z, canvas.height)
        ctx.stroke()
      }
      for (let y = 0; y <= pixelH; y++) {
        ctx.beginPath()
        ctx.moveTo(0, y * z)
        ctx.lineTo(canvas.width, y * z)
        ctx.stroke()
      }
    }
  }

  function screenToPixel(mouseX: number, mouseY: number): { x: number; y: number } | null {
    const canvas = canvasRef.value
    if (!canvas || !store.imageData) return null
    const rect = canvas.getBoundingClientRect()
    const px = mouseX - rect.left
    const py = mouseY - rect.top
    const z = store.zoom
    const x = Math.floor(px / z)
    const y = Math.floor(py / z)
    if (x < 0 || x >= store.imageData.width || y < 0 || y >= store.imageData.height) return null
    return { x, y }
  }

  function pixelToScreen(x: number, y: number): { x: number; y: number } | null {
    if (!store.imageData) return null
    const z = store.zoom
    return { x: x * z, y: y * z }
  }

  watch(
    () => store.renderVersion,
    () => { render() },
  )

  watch(
    () => [store.zoom, store.showGrid],
    () => { render() },
  )

  return { render, screenToPixel, pixelToScreen }
}
