// ───────────────────────────────────────────────────────────────────────────
// aseprite.ts — Aseprite-compatible sprite-sheet JSON (port of aseprite.go).
//
// The de-facto interchange format: Phaser/Pixi/Unity/Godot Aseprite importers
// read it directly. Each animation state becomes a frameTag spanning its frames.
// ───────────────────────────────────────────────────────────────────────────
import type { Manifest } from './types'

interface AseFrame {
  filename: string
  frame: { x: number; y: number; w: number; h: number }
  rotated: boolean
  trimmed: boolean
  spriteSourceSize: { x: number; y: number; w: number; h: number }
  sourceSize: { w: number; h: number }
  duration: number
}
interface AseFrameTag {
  name: string
  from: number
  to: number
  direction: string
  repeat?: string
}

/** buildAsepriteJSON — convert the manifest into Aseprite-compatible sheet JSON. */
export function buildAsepriteJSON(m: Manifest): string {
  // Sort states by row so frame indices match the sheet layout.
  const anims = Object.entries(m.animations).map(([name, anim]) => ({ name, anim }))
  anims.sort((a, b) => a.anim.row - b.anim.row)

  const frames: AseFrame[] = []
  const frameTags: AseFrameTag[] = []
  let idx = 0
  for (const { name, anim } of anims) {
    const fps = anim.fps <= 0 ? 8 : anim.fps
    const duration = Math.trunc(1000 / fps)
    const from = idx
    anim.rects.forEach((r, fi) => {
      frames.push({
        filename: `${name} ${fi}`,
        frame: { x: r.x, y: r.y, w: r.w, h: r.h },
        rotated: false,
        trimmed: false,
        spriteSourceSize: { x: 0, y: 0, w: r.w, h: r.h },
        sourceSize: { w: r.w, h: r.h },
        duration,
      })
      idx++
    })
    const tag: AseFrameTag = { name, from, to: idx - 1, direction: 'forward' }
    if (!anim.loop) tag.repeat = '1'
    frameTags.push(tag)
  }

  const sheet = {
    frames,
    meta: {
      app: 'perfectpixel',
      version: '1.0',
      image: m.sheet.image,
      format: 'RGBA8888',
      size: { w: m.sheet.width, h: m.sheet.height },
      scale: '1',
      frameTags,
    },
  }
  return JSON.stringify(sheet, null, 2)
}
