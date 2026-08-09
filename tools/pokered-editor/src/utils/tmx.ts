/**
 * TMX (Tiled JSON) import/export for the game-editor.
 *
 * Game maps use **blocks** (4×4 tiles = 16 tiles per block). Each block ID
 * maps to 16 tile IDs via the blockset (*.bst file).  The block data is stored
 * in `map.blk` as a flat byte array.
 *
 * Tiled maps work at the **tile** level.  This module converts between the
 * two representations:
 *
 *   Import:  Tiled tile layer → group 4×4 tiles → match against blockset → block IDs
 *   Export:  block IDs → expand through blockset → Tiled tile layer
 *
 * Flip flags in Tiled GIDs (bits 29-31) are stripped before matching since the
 * game does not support flipped tiles within blocks.
 */

// ---------------------------------------------------------------------------
// Tiled JSON types (mirrors jrpg-engine-tiled/src/lib.rs)
// ---------------------------------------------------------------------------

/** A single custom property from Tiled. */
export interface TmxProperty {
  name: string
  value: string | number | boolean
}

/** A tile with custom properties (from a tileset's "tiles" array). */
export interface TmxTile {
  id: number
  properties?: TmxProperty[]
}

/** A tileset reference as it appears in the Tiled map JSON. */
export interface TmxTilesetRef {
  firstgid: number
  source?: string
  name: string
  tilewidth: number
  tileheight: number
  tilecount: number
  tiles?: TmxTile[]
}

/** A single tile layer from a Tiled map. */
export interface TmxLayer {
  name: string
  data: number[]
  width: number
  height: number
  visible?: boolean
  opacity?: number
  properties?: TmxProperty[]
  /** Tiled object layers have "objects" instead of "data". */
  objects?: TmxObject[]
  type?: string
}

/** A Tiled object (for warps, NPCs, signs). */
export interface TmxObject {
  id: number
  name?: string
  type?: string
  x: number
  y: number
  width: number
  height: number
  properties?: TmxProperty[]
  gid?: number
  visible?: boolean
}

/** The root Tiled map document. */
export interface TmxMap {
  width: number
  height: number
  tilewidth: number
  tileheight: number
  layers: TmxLayer[]
  tilesets: TmxTilesetRef[]
  backgroundcolor?: string
  properties?: TmxProperty[]
  nextlayerid?: number
  nextobjectid?: number
}

// ---------------------------------------------------------------------------
// GID helpers (from jrpg-engine-tiled)
// ---------------------------------------------------------------------------

/** Mask to strip flip flags from a Tiled GID, leaving the raw tile ID. */
export const GID_TILE_MASK = 0x1fff_ffff

/** Horizontal flip flag (bit 31). */
export const GID_FLIP_H = 0x8000_0000
/** Vertical flip flag (bit 30). */
export const GID_FLIP_V = 0x4000_0000
/** Diagonal flip flag (bit 29). */
export const GID_FLIP_D = 0x2000_0000

/** Strip all flip flags from a GID, returning the clean tile ID. */
export function cleanGid(gid: number): number {
  return gid & GID_TILE_MASK
}

/** Extract flip flags from a GID. */
export function gidFlipFlags(gid: number): { flipH: boolean; flipV: boolean; flipD: boolean } {
  return {
    flipH: (gid & GID_FLIP_H) !== 0,
    flipV: (gid & GID_FLIP_V) !== 0,
    flipD: (gid & GID_FLIP_D) !== 0,
  }
}

// ---------------------------------------------------------------------------
// Block ↔ tile conversion
// ---------------------------------------------------------------------------

/** Number of tiles per block (4×4 grid). */
export const BLOCK_TILES = 16
export const BLOCK_DIM = 4 // 4 tiles per block side

/**
 * Build a reverse lookup: serialised 16-tile pattern → block ID.
 *
 * Used during import to match 4×4 tile groups from Tiled back to game blocks.
 */
export function buildBlockLookup(
  blockset: Record<number, number[]>,
): Map<string, number> {
  const lookup = new Map<string, number>()
  for (const [blockIdStr, tiles] of Object.entries(blockset)) {
    const blockId = Number(blockIdStr)
    if (tiles.length === BLOCK_TILES) {
      lookup.set(tiles.join(','), blockId)
    }
  }
  return lookup
}

/**
 * Find the best-matching block ID for a 4×4 tile arrangement.
 * Returns the block ID or -1 if no match found.
 */
export function findBlockId(
  tiles: number[],
  blockset: Record<number, number[]>,
): number {
  if (tiles.length !== BLOCK_TILES) return -1
  const key = tiles.join(',')
  for (const [blockIdStr, pattern] of Object.entries(blockset)) {
    if (pattern.length === BLOCK_TILES && pattern.join(',') === key) {
      return Number(blockIdStr)
    }
  }
  return -1
}

// ---------------------------------------------------------------------------
// Tiled JSON → game-editor conversion
// ---------------------------------------------------------------------------

export interface TmxImportResult {
  mapJson: Record<string, unknown>
  blockData: number[]
  warnings: string[]
}

/**
 * Import a Tiled JSON map into the game-editor format.
 *
 * Converts the first visible tile layer into block data by grouping 4×4 tiles
 * and matching against the given blockset.  Tile layers with type "objectgroup"
 * are scanned for warp/NPC/sign objects.
 *
 * @param tmx     - Parsed Tiled map JSON
 * @param tilesetName - Game tileset name to use in the map header
 * @param blockset - The blockset definition (block ID → 16 tile IDs)
 * @param mapName  - Name for the generated map (defaults to "ImportedMap")
 * @param mapId    - Numeric ID for the generated map (defaults to 0)
 * @param music    - BGM music name (defaults to "PalletTown")
 * @param borderBlock - Border block ID (defaults to 0)
 */
export function importTmxToMap(
  tmx: TmxMap,
  tilesetName: string,
  blockset: Record<number, number[]>,
  mapName = 'ImportedMap',
  mapId = 0,
  music = 'PalletTown',
  borderBlock = 0,
): TmxImportResult {
  const warnings: string[] = []

  // Find the first tile layer (type 'tilelayer' or no type specified)
  const tileLayers = tmx.layers.filter(
    (l) => (l.type ?? 'tilelayer') === 'tilelayer',
  )
  const objectLayers = tmx.layers.filter(
    (l) => l.type === 'objectgroup',
  )

  if (tileLayers.length === 0) {
    return {
      mapJson: createEmptyMapJson(mapName, mapId, tilesetName, music, 1, 1, borderBlock),
      blockData: [borderBlock],
      warnings: ['No tile layers found in TMX. Created empty map.'],
    }
  }

  // Use the first visible tile layer, or first layer if none visible
  const layer =
    tileLayers.find((l) => l.visible !== false) ?? tileLayers[0]

  const layerW = layer.width
  const layerH = layer.height

  // Convert tile dimensions to block dimensions
  // Each block is 4×4 tiles, so blockW = ceil(layerW / 4), blockH = ceil(layerH / 4)
  const blockW = Math.ceil(layerW / BLOCK_DIM)
  const blockH = Math.ceil(layerH / BLOCK_DIM)

  // Warn if tile dimensions are not multiples of 4
  if (layerW % BLOCK_DIM !== 0 || layerH % BLOCK_DIM !== 0) {
    warnings.push(
      `Tile layer dimensions (${layerW}×${layerH}) are not multiples of ${BLOCK_DIM}. ` +
        `Map will be padded to ${blockW * BLOCK_DIM}×${blockH * BLOCK_DIM} tiles (${blockW}×${blockH} blocks).`,
    )
  }

  // Build block data
  const blockData: number[] = []
  let unmatchedBlocks = 0

  for (let by = 0; by < blockH; by++) {
    for (let bx = 0; bx < blockW; bx++) {
      // Collect 16 tiles for this block (row-major within the 4×4 grid)
      const tiles: number[] = []
      for (let ty = 0; ty < BLOCK_DIM; ty++) {
        for (let tx = 0; tx < BLOCK_DIM; tx++) {
          const tileX = bx * BLOCK_DIM + tx
          const tileY = by * BLOCK_DIM + ty
          if (tileX < layerW && tileY < layerH) {
            const idx = tileY * layerW + tileX
            const gid = layer.data[idx] ?? 0
            // Convert GID to local tile ID by subtracting firstgid
            // We use the first tileset's firstgid; in practice Tiled maps
            // often use a single tileset or shared GID range.
            const localId = tiledGidToLocal(gid, tmx.tilesets)
            tiles.push(localId)
          } else {
            tiles.push(0) // pad with tile 0
          }
        }
      }

      // Match against blockset
      const blockId = findBlockId(tiles, blockset)
      if (blockId >= 0) {
        blockData.push(blockId)
      } else {
        // No exact match — use borderBlock as fallback
        blockData.push(borderBlock)
        unmatchedBlocks++
      }
    }
  }

  if (unmatchedBlocks > 0) {
    warnings.push(
      `${unmatchedBlocks} block(s) could not be matched to the blockset and were replaced with borderBlock (${borderBlock}).`,
    )
  }

  // Extract objects from object layers
  const warps: Record<string, unknown>[] = []
  const npcs: Record<string, unknown>[] = []
  const signs: Record<string, unknown>[] = []
  let npcTextId = 1
  let signTextId = 1
  let warpId = 0

  for (const objLayer of objectLayers) {
    const layerName = (objLayer.name ?? '').toLowerCase()
    for (const obj of objLayer.objects ?? []) {
      const props = propertiesToMap(obj.properties ?? [])
      // Convert pixel coords to block coords (Tiled objects use pixels)
      const objBlockX = Math.floor(obj.x / (tmx.tilewidth || 16) / BLOCK_DIM)
      const objBlockY = Math.floor(obj.y / (tmx.tileheight || 16) / BLOCK_DIM)

      // Classify by layer name or object type
      const objType = (obj.type ?? layerName).toLowerCase()

      if (objType.includes('warp')) {
        warps.push({
          x: objBlockX,
          y: objBlockY,
          destMap: props['destMap'] ?? '',
          destWarpId: Number(props['destWarpId'] ?? 0),
        })
        warpId++
      } else if (objType.includes('npc') || objType.includes('trainer')) {
        npcs.push(tmxObjectToNpc(obj, objBlockX, objBlockY, npcTextId++, props))
      } else if (objType.includes('sign')) {
        signs.push({
          x: objBlockX,
          y: objBlockY,
          textId: signTextId++,
        })
      }
    }
  }

  // Extract map-level properties
  const mapProps = propertiesToMap(tmx.properties ?? [])

  const mapJson = {
    id: mapId,
    name: mapName,
    header: {
      tileset: tilesetName,
      music: mapProps['music'] || music,
      connectionFlags: 0,
      width: blockW,
      height: blockH,
      borderBlock,
    },
    connections: {},
    warps,
    npcs,
    signs,
    text: { npc: {}, sign: {} },
    wild: null,
  }

  return { mapJson, blockData, warnings }
}

/** Convert a Tiled object to a game-editor NPC entry. */
function tmxObjectToNpc(
  obj: TmxObject,
  x: number,
  y: number,
  textId: number,
  props: Record<string, string>,
): Record<string, unknown> {
  const isTrainer = props['isTrainer'] === 'true' || (obj.type ?? '').includes('trainer')
  const npc: Record<string, unknown> = {
    spriteId: Number(props['spriteId'] ?? 1),
    spriteName: props['spriteName'] ?? obj.name ?? 'Unknown',
    x,
    y,
    movement: props['movement'] ?? 'Stationary',
    facing: props['facing'] ?? 'Down',
    range: Number(props['range'] ?? 0),
    textId,
    isTrainer,
  }
  if (isTrainer) {
    if (props['trainerClass']) npc.trainerClass = props['trainerClass']
    if (props['trainerSet'] != null) npc.trainerSet = Number(props['trainerSet'])
  }
  return npc
}

/** Convert a Tiled GID to a local tile ID by subtracting the matching tileset's firstgid. */
function tiledGidToLocal(gid: number, tilesets: TmxTilesetRef[]): number {
  if (gid === 0) return 0
  const clean = cleanGid(gid)
  // Find which tileset this GID belongs to (tilesets are sorted by firstgid)
  let localId = clean
  for (let i = tilesets.length - 1; i >= 0; i--) {
    if (clean >= tilesets[i].firstgid) {
      localId = clean - tilesets[i].firstgid
      // Clamp to valid range
      if (localId >= tilesets[i].tilecount) {
        localId = clean % tilesets[i].tilecount
      }
      break
    }
  }
  return localId
}

/** Convert Tiled properties array to a flat key-value map. */
function propertiesToMap(props: TmxProperty[]): Record<string, string> {
  const map: Record<string, string> = {}
  for (const p of props) {
    map[p.name] = String(p.value)
  }
  return map
}

// ---------------------------------------------------------------------------
// Game-editor → Tiled JSON conversion
// ---------------------------------------------------------------------------

export interface TmxExportOptions {
  /** Game tileset name (e.g. "Overworld") */
  tilesetName: string
  /** Path to the tileset PNG (relative, e.g. "../gfx/tilesets/overworld.png") */
  tilesetImage: string
  /** Tile width in pixels (default: 8) */
  tileWidth?: number
  /** Tile height in pixels (default: 8) */
  tileHeight?: number
  /** First GID for the tileset (default: 1) */
  firstGid?: number
}

/**
 * Convert a game-editor map to Tiled JSON format.
 *
 * Expands blocks into 4×4 tile grids using the given blockset.  Warps, NPCs,
 * and signs are exported as Tiled object layers.
 */
export function exportMapToTmx(
  mapJson: Record<string, unknown>,
  blockData: number[],
  blockset: Record<number, number[]>,
  options: TmxExportOptions,
): TmxMap {
  const header = (mapJson.header ?? {}) as Record<string, unknown>
  const blockW = (header.width as number) ?? 10
  const blockH = (header.height as number) ?? 9
  const tileW = options.tileWidth ?? 8
  const tileH = options.tileHeight ?? 8
  const firstGid = options.firstGid ?? 1
  const mapName = (mapJson.name as string) ?? 'ExportedMap'
  const borderBlock = (header.borderBlock as number) ?? 0
  const music = (header.music as string) ?? 'PalletTown'

  // Expand blocks into tiles
  const tileLayerW = blockW * BLOCK_DIM
  const tileLayerH = blockH * BLOCK_DIM
  const tileData: number[] = []

  for (let by = 0; by < blockH; by++) {
    for (let ty = 0; ty < BLOCK_DIM; ty++) {
      for (let bx = 0; bx < blockW; bx++) {
        const blockIdx = by * blockW + bx
        const blockId = blockData[blockIdx] ?? borderBlock
        const tiles = blockset[blockId]
        for (let tx = 0; tx < BLOCK_DIM; tx++) {
          const tileId = tiles?.[ty * BLOCK_DIM + tx] ?? 0
          // Convert local tile ID to GID: add firstgid
          tileData.push(tileId > 0 ? tileId + firstGid : 0)
        }
      }
    }
  }

  // Build tile layer
  const tileLayer: TmxLayer = {
    name: 'ground',
    type: 'tilelayer',
    data: tileData,
    width: tileLayerW,
    height: tileLayerH,
    visible: true,
    opacity: 1,
    properties: [],
  }

  // Build object layers for map entities
  const layers: TmxLayer[] = [tileLayer]

  // Warps → object layer
  const warps = (mapJson.warps as Record<string, unknown>[]) ?? []
  if (warps.length > 0) {
    const warpObjects: TmxObject[] = warps.map((w, i) => ({
      id: i + 1,
      name: `Warp ${i + 1}`,
      type: 'warp',
      x: ((w.x as number) ?? 0) * BLOCK_DIM * tileW,
      y: ((w.y as number) ?? 0) * BLOCK_DIM * tileH,
      width: tileW,
      height: tileH,
      properties: [
        { name: 'destMap', value: (w.destMap as string) ?? '' },
        { name: 'destWarpId', value: (w.destWarpId as number) ?? 0 },
      ],
    }))
    layers.push({
      name: 'warps',
      type: 'objectgroup',
      data: [],
      width: tileLayerW,
      height: tileLayerH,
      visible: true,
      opacity: 1,
      objects: warpObjects,
    })
  }

  // Signs → object layer
  const signs = (mapJson.signs as Record<string, unknown>[]) ?? []
  if (signs.length > 0) {
    const signObjects: TmxObject[] = signs.map((s, i) => ({
      id: i + 1,
      name: `Sign ${(s.textId as number) ?? i + 1}`,
      type: 'sign',
      x: ((s.x as number) ?? 0) * BLOCK_DIM * tileW,
      y: ((s.y as number) ?? 0) * BLOCK_DIM * tileH,
      width: tileW,
      height: tileH,
      properties: [
        { name: 'textId', value: (s.textId as number) ?? 1 },
      ],
    }))
    layers.push({
      name: 'signs',
      type: 'objectgroup',
      data: [],
      width: tileLayerW,
      height: tileLayerH,
      visible: true,
      opacity: 1,
      objects: signObjects,
    })
  }

  // NPCs → object layer
  const npcs = (mapJson.npcs as Record<string, unknown>[]) ?? []
  if (npcs.length > 0) {
    const npcObjects: TmxObject[] = npcs.map((n, i) => ({
      id: i + 1,
      name: (n.spriteName as string) ?? `NPC ${i + 1}`,
      type: (n.isTrainer as boolean) ? 'trainer' : 'npc',
      x: ((n.x as number) ?? 0) * BLOCK_DIM * tileW,
      y: ((n.y as number) ?? 0) * BLOCK_DIM * tileH,
      width: tileW,
      height: tileH,
      properties: [
        { name: 'spriteId', value: (n.spriteId as number) ?? 1 },
        { name: 'spriteName', value: (n.spriteName as string) ?? 'Unknown' },
        { name: 'movement', value: (n.movement as string) ?? 'Stationary' },
        { name: 'facing', value: (n.facing as string) ?? 'Down' },
        { name: 'range', value: (n.range as number) ?? 0 },
        { name: 'textId', value: (n.textId as number) ?? i + 1 },
        { name: 'isTrainer', value: String(n.isTrainer ?? false) },
      ],
    }))
    layers.push({
      name: 'npcs',
      type: 'objectgroup',
      data: [],
      width: tileLayerW,
      height: tileLayerH,
      visible: true,
      opacity: 1,
      objects: npcObjects,
    })
  }

  // Compute tile count from the tileset PNG dimensions
  // Game tilesets have 128×?? tiles (usually 128×48 = 96 tiles for 8×8 tiles)
  // We use a conservative estimate
  const tileCount = 256 // up to 256 unique tiles

  const tmxMap: TmxMap = {
    width: tileLayerW,
    height: tileLayerH,
    tilewidth: tileW,
    tileheight: tileH,
    layers,
    tilesets: [
      {
        firstgid: firstGid,
        name: options.tilesetName,
        tilewidth: tileW,
        tileheight: tileH,
        tilecount: tileCount,
        source: undefined, // embedded tileset reference
      },
    ],
    properties: [
      { name: 'mapName', value: mapName },
      { name: 'music', value: music },
      { name: 'tileset', value: options.tilesetName },
    ],
    nextlayerid: layers.length + 1,
    nextobjectid:
      warps.length + signs.length + npcs.length + 1,
  }

  return tmxMap
}

/** Create an empty map.json structure. */
function createEmptyMapJson(
  name: string,
  id: number,
  tileset: string,
  music: string,
  width: number,
  height: number,
  borderBlock: number,
): Record<string, unknown> {
  return {
    id,
    name,
    header: {
      tileset,
      music,
      connectionFlags: 0,
      width,
      height,
      borderBlock,
    },
    connections: {},
    warps: [],
    npcs: [],
    signs: [],
    text: { npc: {}, sign: {} },
    wild: null,
  }
}

// ---------------------------------------------------------------------------
// File I/O helpers (browser-side)
// ---------------------------------------------------------------------------

/** Trigger a file download in the browser. */
export function downloadJson(data: unknown, filename: string): void {
  const json = JSON.stringify(data, null, 2)
  const blob = new Blob([json], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

/** Prompt user to select a JSON file and return parsed content. */
export function pickJsonFile(): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = '.json,.tmx'
    input.onchange = () => {
      const file = input.files?.[0]
      if (!file) {
        reject(new Error('No file selected'))
        return
      }
      const reader = new FileReader()
      reader.onload = () => {
        try {
          const data = JSON.parse(reader.result as string)
          resolve(data)
        } catch (e) {
          reject(new Error(`Failed to parse JSON: ${(e as Error).message}`))
        }
      }
      reader.onerror = () => reject(new Error('Failed to read file'))
      reader.readAsText(file)
    }
    input.click()
  })
}
