import path from 'path'
import fs from 'fs'

export function copyDir(src: string, dest: string) {
  if (!fs.existsSync(src)) return
  fs.mkdirSync(dest, { recursive: true })
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const srcPath = path.join(src, entry.name)
    const destPath = path.join(dest, entry.name)
    if (entry.isDirectory()) {
      copyDir(srcPath, destPath)
    } else {
      fs.copyFileSync(srcPath, destPath)
    }
  }
}

// ── Sprite Studio: per-character multi-category sprite sheets ──
// Game-agnostic defaults mirroring the wuxia character-sprite-gen pipeline
// (data/gfx/<category>/<id>/sheet.png, RGBA, row = facing, col = walk frame).
// A project may override via the story activity's `sprite.categories`. The
// overworld frame set includes distinct run frames (cols 3/4) — they animate
// in-game once the engine actor gains a run state.
export const DEFAULT_SPRITE_CATEGORIES = [
  {
    id: 'overworld', label: { en: 'Overworld', zh: '行走图' }, dir: 'data/gfx/overworld',
    rows: 4, cols: 5, cellW: 24, cellH: 32,
    rowNames: ['down', 'up', 'left', 'right'],
    colNames: ['stand', 'walk1', 'walk2', 'run1', 'run2'],
    animated: true, footAnchor: true, standCol: 0, walkCols: [1, 2], runCols: [3, 4],
  },
  {
    id: 'portrait', label: { en: 'Battle portrait', zh: '战斗立绘' }, dir: 'data/gfx/portrait',
    rows: 1, cols: 1, cellW: 64, cellH: 64, animated: false,
  },
  {
    id: 'dex', label: { en: 'Bestiary', zh: '图鉴立绘' }, dir: 'data/gfx/dex',
    rows: 1, cols: 1, cellW: 64, cellH: 64, animated: false,
  },
  {
    id: 'head', label: { en: 'Dialogue head', zh: '对话头像' }, dir: 'data/gfx/head',
    rows: 1, cols: 1, cellW: 32, cellH: 32, animated: false,
  },
]

/** Read a PNG's pixel dimensions from its IHDR header (no image decoder needed). */
export function pngSize(buf: Buffer): { w: number; h: number } | null {
  if (!buf || buf.length < 24) return null
  if (buf.readUInt32BE(12) !== 0x49484452) return null // 'IHDR'
  return { w: buf.readUInt32BE(16), h: buf.readUInt32BE(20) }
}
