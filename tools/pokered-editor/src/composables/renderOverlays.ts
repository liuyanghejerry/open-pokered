import { TILE_SIZE, BLOCK_TILES } from '../types/constants'
import type { WarpJson, SignJson, NpcJson, CoordEvent, SelectedEntity, ConnectionsJson } from '../types'

export function renderWarps(
  ctx: CanvasRenderingContext2D,
  warps: WarpJson[],
) {
  warps.forEach((warp) => {
    const tileX = warp.x * 2
    const tileY = warp.y * 2
    const wpx = tileX * TILE_SIZE
    const wpy = tileY * TILE_SIZE
    ctx.fillStyle = 'rgba(52, 152, 219, 0.8)'
    ctx.fillRect(wpx, wpy, TILE_SIZE * 2, TILE_SIZE * 2)
    ctx.strokeStyle = '#3498db'
    ctx.lineWidth = 2
    ctx.strokeRect(wpx, wpy, TILE_SIZE * 2, TILE_SIZE * 2)
  })
}

export function renderSigns(
  ctx: CanvasRenderingContext2D,
  signs: SignJson[],
) {
  signs.forEach((sign) => {
    const tileX = sign.x * 2
    const tileY = sign.y * 2
    const spx = tileX * TILE_SIZE
    const spy = tileY * TILE_SIZE
    ctx.fillStyle = 'rgba(241, 196, 15, 0.8)'
    ctx.fillRect(spx, spy, TILE_SIZE * 2, TILE_SIZE * 2)
    ctx.strokeStyle = '#f39c12'
    ctx.lineWidth = 2
    ctx.strokeRect(spx, spy, TILE_SIZE * 2, TILE_SIZE * 2)
    // "S" label
    ctx.fillStyle = '#000'
    ctx.font = `bold ${TILE_SIZE}px monospace`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText('S', spx + TILE_SIZE, spy + TILE_SIZE)
  })
}

export function renderNpcs(
  ctx: CanvasRenderingContext2D,
  npcs: NpcJson[],
) {
  npcs.forEach((npc) => {
    const tileX = npc.x * 2
    const tileY = npc.y * 2
    const npx = tileX * TILE_SIZE
    const npy = tileY * TILE_SIZE

    if (npc.isTrainer) {
      ctx.fillStyle = 'rgba(231, 76, 60, 0.8)'
      ctx.strokeStyle = '#c0392b'
    } else if (npc.itemId != null) {
      ctx.fillStyle = 'rgba(46, 204, 113, 0.8)'
      ctx.strokeStyle = '#27ae60'
    } else {
      ctx.fillStyle = 'rgba(155, 89, 182, 0.8)'
      ctx.strokeStyle = '#8e44ad'
    }

    ctx.lineWidth = 2
    ctx.fillRect(npx, npy, TILE_SIZE * 2, TILE_SIZE * 2)
    ctx.strokeRect(npx, npy, TILE_SIZE * 2, TILE_SIZE * 2)

    // Label: T=trainer, I=item, N=npc
    const label = npc.isTrainer ? 'T' : npc.itemId != null ? 'I' : 'N'
    ctx.fillStyle = '#fff'
    ctx.font = `bold ${TILE_SIZE}px monospace`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(label, npx + TILE_SIZE, npy + TILE_SIZE)
  })
}

export function renderCoordEvents(
  ctx: CanvasRenderingContext2D,
  coordEvents: CoordEvent[],
) {
  coordEvents.forEach((ce) => {
    const tileX = ce.x * 2
    const tileY = ce.y * 2
    const cpx = tileX * TILE_SIZE
    const cpy = tileY * TILE_SIZE
    ctx.fillStyle = 'rgba(230, 126, 34, 0.8)'
    ctx.fillRect(cpx, cpy, TILE_SIZE * 2, TILE_SIZE * 2)
    ctx.strokeStyle = '#d35400'
    ctx.lineWidth = 2
    ctx.strokeRect(cpx, cpy, TILE_SIZE * 2, TILE_SIZE * 2)
    ctx.fillStyle = '#fff'
    ctx.font = `bold ${TILE_SIZE}px monospace`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText('C', cpx + TILE_SIZE, cpy + TILE_SIZE)
  })
}

export function renderSelectionHighlight(
  ctx: CanvasRenderingContext2D,
  selected: SelectedEntity,
) {
  const x = selected.data.x
  const y = selected.data.y

  const tileX = x * 2
  const tileY = y * 2
  const px = tileX * TILE_SIZE
  const py = tileY * TILE_SIZE
  const size = TILE_SIZE * 2

  const t = (Date.now() % 1000) / 1000
  const alpha = 0.5 + 0.5 * Math.sin(t * Math.PI * 2)

  ctx.save()
  ctx.strokeStyle = `rgba(255, 255, 255, ${alpha})`
  ctx.lineWidth = 3
  ctx.strokeRect(px - 2, py - 2, size + 4, size + 4)

  ctx.strokeStyle = `rgba(255, 255, 100, ${alpha * 0.7})`
  ctx.lineWidth = 1.5
  ctx.strokeRect(px - 1, py - 1, size + 2, size + 2)
  ctx.restore()
}

const ARROW_SIZE = 12

function drawArrow(ctx: CanvasRenderingContext2D, x: number, y: number, direction: 'north' | 'south' | 'west' | 'east') {
  ctx.save()
  ctx.translate(x, y)
  
  ctx.fillStyle = 'rgba(46, 204, 113, 0.9)'
  ctx.strokeStyle = '#27ae60'
  ctx.lineWidth = 2
  
  ctx.beginPath()
  switch (direction) {
    case 'north':
      ctx.moveTo(0, -ARROW_SIZE)
      ctx.lineTo(-ARROW_SIZE/2, 0)
      ctx.lineTo(ARROW_SIZE/2, 0)
      ctx.closePath()
      break
    case 'south':
      ctx.moveTo(0, ARROW_SIZE)
      ctx.lineTo(-ARROW_SIZE/2, 0)
      ctx.lineTo(ARROW_SIZE/2, 0)
      ctx.closePath()
      break
    case 'west':
      ctx.moveTo(-ARROW_SIZE, 0)
      ctx.lineTo(0, -ARROW_SIZE/2)
      ctx.lineTo(0, ARROW_SIZE/2)
      ctx.closePath()
      break
    case 'east':
      ctx.moveTo(ARROW_SIZE, 0)
      ctx.lineTo(0, -ARROW_SIZE/2)
      ctx.lineTo(0, ARROW_SIZE/2)
      ctx.closePath()
      break
  }
  ctx.fill()
  ctx.stroke()
  ctx.restore()
}

export function renderConnections(
  ctx: CanvasRenderingContext2D,
  connections: ConnectionsJson,
  mapWidthBlocks: number,
  mapHeightBlocks: number,
) {
  const mapWidthPx = mapWidthBlocks * BLOCK_TILES * TILE_SIZE
  const mapHeightPx = mapHeightBlocks * BLOCK_TILES * TILE_SIZE
  
  ctx.font = 'bold 10px sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  
  if (connections.north) {
    const offsetTiles = connections.north.offset * 2
    const centerX = offsetTiles * TILE_SIZE + (mapWidthPx / 2)
    const y = 20
    
    ctx.fillStyle = 'rgba(46, 204, 113, 0.7)'
    ctx.fillRect(0, 0, mapWidthPx, 8)
    
    drawArrow(ctx, centerX, y, 'north')
    
    ctx.fillStyle = '#fff'
    const targetName = connections.north.targetMap
    ctx.fillText(targetName, centerX, y + 15)
    ctx.fillStyle = '#aaa'
    ctx.fillText(`off:${connections.north.offset}`, centerX, y + 26)
  }
  
  if (connections.south) {
    const offsetTiles = connections.south.offset * 2
    const centerX = offsetTiles * TILE_SIZE + (mapWidthPx / 2)
    const y = mapHeightPx - 20
    
    ctx.fillStyle = 'rgba(46, 204, 113, 0.7)'
    ctx.fillRect(0, mapHeightPx - 8, mapWidthPx, 8)
    
    drawArrow(ctx, centerX, y, 'south')
    
    ctx.fillStyle = '#fff'
    const targetName = connections.south.targetMap
    ctx.fillText(targetName, centerX, y - 15)
    ctx.fillStyle = '#aaa'
    ctx.fillText(`off:${connections.south.offset}`, centerX, y - 26)
  }
  
  if (connections.west) {
    const offsetTiles = connections.west.offset * 2
    const centerY = offsetTiles * TILE_SIZE + (mapHeightPx / 2)
    const x = 20
    
    ctx.fillStyle = 'rgba(46, 204, 113, 0.7)'
    ctx.fillRect(0, 0, 8, mapHeightPx)
    
    drawArrow(ctx, x, centerY, 'west')
    
    ctx.fillStyle = '#fff'
    const targetName = connections.west.targetMap
    ctx.save()
    ctx.translate(x + 15, centerY)
    ctx.rotate(-Math.PI / 2)
    ctx.fillText(targetName, 0, 0)
    ctx.fillStyle = '#aaa'
    ctx.fillText(`off:${connections.west.offset}`, 0, 11)
    ctx.restore()
  }
  
  if (connections.east) {
    const offsetTiles = connections.east.offset * 2
    const centerY = offsetTiles * TILE_SIZE + (mapHeightPx / 2)
    const x = mapWidthPx - 20
    
    ctx.fillStyle = 'rgba(46, 204, 113, 0.7)'
    ctx.fillRect(mapWidthPx - 8, 0, 8, mapHeightPx)
    
    drawArrow(ctx, x, centerY, 'east')
    
    ctx.fillStyle = '#fff'
    const targetName = connections.east.targetMap
    ctx.save()
    ctx.translate(x - 15, centerY)
    ctx.rotate(Math.PI / 2)
    ctx.fillText(targetName, 0, 0)
    ctx.fillStyle = '#aaa'
    ctx.fillText(`off:${connections.east.offset}`, 0, 11)
    ctx.restore()
  }
}
