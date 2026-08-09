// ───────────────────────────────────────────────────────────────────────────
// atlas.ts — sprite-sheet + manifest composition (port of atlas.go).
//
// Lays out each state's frames as one row, and emits a schema-v2 manifest with
// per-frame sheet rects, content trims, and a shared foot pivot (bottom-center).
// ───────────────────────────────────────────────────────────────────────────
import { type Img, alphaThreshold, blit, idx, newImg } from './image'
import type { AnimationEntry, FrameRect, Manifest, StateFrames } from './types'

/** composeAtlas — lay state frames row-by-row into a sheet + build the manifest. */
export function composeAtlas(character: string, states: StateFrames[], cellW: number, cellH: number): { sheet: Img; manifest: Manifest } {
  let maxFrames = 1
  for (const s of states) if (s.frames.length > maxFrames) maxFrames = s.frames.length
  const sheetW = maxFrames * cellW
  const sheetH = states.length * cellH
  const sheet = newImg(sheetW, sheetH)

  const manifest: Manifest = {
    app: 'perfectpixel',
    generator: 'dotzuki-editor/sprite-sheet',
    schema: 'perfectpixel.sprite/2',
    version: 2,
    character,
    sheet: { image: 'sprite-sheet.png', width: sheetW, height: sheetH, cellWidth: cellW, cellHeight: cellH },
    animations: {},
  }

  for (let row = 0; row < states.length; row++) {
    const s = states[row]
    let fps = s.spec.fps
    if (fps <= 0) fps = 8
    const entry: AnimationEntry = {
      row,
      frames: s.frames.length,
      fps,
      loop: s.spec.loop,
      durationMs: Math.trunc(1000 / fps),
      pivot: { x: 0, y: 0 },
      rects: [],
      trims: [],
    }
    let groundY = 0
    for (let col = 0; col < s.frames.length; col++) {
      const frame = s.frames[col]
      const x = col * cellW, y = row * cellH
      blit(sheet, x, y, frame)
      entry.rects.push({ x, y, w: cellW, h: cellH })
      const trim = contentBBox(frame)
      entry.trims.push(trim)
      const bottom = trim.y + trim.h
      if (bottom > groundY) groundY = bottom
    }
    // Shared foot anchor: cell horizontal center + lowest content bottom (ground).
    // If groundY is too high (content at top), fall back to cell bottom so the
    // pivot doesn't poke outside the frame.
    if (groundY < cellH / 2 && s.frames.length > 0) groundY = cellH
    if (groundY > cellH) groundY = cellH
    entry.pivot = { x: Math.trunc(cellW / 2), y: groundY }
    manifest.animations[s.spec.name] = entry
  }
  return { sheet, manifest }
}

/** contentBBox — opaque content bounding rect in cell-local coordinates. */
function contentBBox(f: Img): FrameRect {
  const w = f.width, h = f.height
  let minX = w, minY = h, maxX = -1, maxY = -1
  const d = f.data
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (d[idx(w, x, y) + 3] > alphaThreshold) {
        if (x < minX) minX = x
        if (x > maxX) maxX = x
        if (y < minY) minY = y
        if (y > maxY) maxY = y
      }
    }
  }
  if (maxX < minX) return { x: 0, y: 0, w: 0, h: 0 }
  return { x: minX, y: minY, w: maxX - minX + 1, h: maxY - minY + 1 }
}
