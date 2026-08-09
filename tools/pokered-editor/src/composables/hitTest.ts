import { TILE_SIZE, BLOCK_TILES } from '../types/constants'
import type { MapJson, Blockset, TileInfo, WarpJson, SignJson, NpcJson, CoordEvent } from '../types'

export function getTileInfoAt(
  canvas: HTMLCanvasElement,
  map: MapJson,
  blockset: Blockset | undefined,
  blocks: number[],
  passableTiles: number[],
  coordEvents: CoordEvent[],
  zoom: number,
  clientX: number,
  clientY: number,
): TileInfo | null {
  const rect = canvas.getBoundingClientRect()
  const x = Math.floor((clientX - rect.left) / (TILE_SIZE * zoom))
  const y = Math.floor((clientY - rect.top) / (TILE_SIZE * zoom))
  const blockX = Math.floor(x / BLOCK_TILES)
  const blockY = Math.floor(y / BLOCK_TILES)

  if (blockX < 0 || blockX >= map.header.width || blockY < 0 || blockY >= map.header.height) {
    return null
  }

  const subX = x % BLOCK_TILES
  const subY = y % BLOCK_TILES
  const blockIndex = blockY * map.header.width + blockX
  const blockId = blocks[blockIndex] ?? 0

  let tileId = 0
  let passable = false
  if (blockset && blockset.blocks[blockId]) {
    tileId = blockset.blocks[blockId][subY * 4 + subX]
    passable = passableTiles.includes(tileId)
  }

  const metaX = Math.floor(x / 2)
  const metaY = Math.floor(y / 2)

  let warp: WarpJson | undefined
  if (map.warps) {
    warp = map.warps.find((w) => w.x === metaX && w.y === metaY)
  }

  let sign: SignJson | undefined
  if (map.signs) {
    sign = map.signs.find((s) => s.x === metaX && s.y === metaY)
  }

  let npc: NpcJson | undefined
  if (map.npcs) {
    npc = map.npcs.find((n) => n.x === metaX && n.y === metaY)
  }

  let coordEvent: CoordEvent | undefined
  if (coordEvents.length > 0) {
    coordEvent = coordEvents.find((ce) => ce.x === metaX && ce.y === metaY)
  }

  return { tileX: x, tileY: y, blockX, blockY, blockIndex, blockId, tileId, passable, warp, sign, npc, coordEvent }
}

export function hasClickableEntity(info: TileInfo): boolean {
  return !!(info.warp || info.sign || info.npc || info.coordEvent)
}
