import { TILE_SIZE, BLOCK_TILES } from '../types/constants'
import type { Blockset } from '../types'

export function renderTilesAndCollision(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  blocks: number[],
  passableTiles: number[],
  blockset: Blockset | undefined,
  tilesetImg: HTMLImageElement | undefined,
  showTiles: boolean,
  showCollision: boolean,
) {
  for (let by = 0; by < height; by++) {
    for (let bx = 0; bx < width; bx++) {
      const blockIdx = by * width + bx
      const blockId = blocks[blockIdx]
      const px = bx * BLOCK_TILES * TILE_SIZE
      const py = by * BLOCK_TILES * TILE_SIZE

      if (showTiles && blockset && tilesetImg) {
        const blockTiles = blockset.blocks[blockId]
        if (blockTiles) {
          const tilesPerRow = Math.floor(tilesetImg.width / TILE_SIZE)
          for (let ty = 0; ty < 4; ty++) {
            for (let tx = 0; tx < 4; tx++) {
              const tileId = blockTiles[ty * 4 + tx]
              const srcX = (tileId % tilesPerRow) * TILE_SIZE
              const srcY = Math.floor(tileId / tilesPerRow) * TILE_SIZE
              ctx.drawImage(
                tilesetImg,
                srcX, srcY, TILE_SIZE, TILE_SIZE,
                px + tx * TILE_SIZE, py + ty * TILE_SIZE, TILE_SIZE, TILE_SIZE,
              )
            }
          }
        }
      } else {
        ctx.fillStyle = `hsl(${blockId * 7}, 50%, 40%)`
        ctx.fillRect(px, py, BLOCK_TILES * TILE_SIZE, BLOCK_TILES * TILE_SIZE)
      }

      if (showCollision && blockset && blockset.blocks[blockId]) {
        const blockTiles = blockset.blocks[blockId]
        for (let ty = 0; ty < 4; ty++) {
          for (let tx = 0; tx < 4; tx++) {
            const tileId = blockTiles[ty * 4 + tx]
            const passable = passableTiles.includes(tileId)
            ctx.fillStyle = passable
              ? 'rgba(78, 204, 163, 0.25)'
              : 'rgba(231, 76, 60, 0.35)'
            ctx.fillRect(
              px + tx * TILE_SIZE,
              py + ty * TILE_SIZE,
              TILE_SIZE,
              TILE_SIZE,
            )
          }
        }
      }
    }
  }
}

export function renderGrid(
  ctx: CanvasRenderingContext2D,
  pw: number,
  ph: number,
) {
  ctx.strokeStyle = 'rgba(255, 255, 0, 0.3)'
  ctx.lineWidth = 0.5
  for (let x = 0; x <= pw; x += BLOCK_TILES * TILE_SIZE) {
    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, ph)
    ctx.stroke()
  }
  for (let y = 0; y <= ph; y += BLOCK_TILES * TILE_SIZE) {
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(pw, y)
    ctx.stroke()
  }
}
