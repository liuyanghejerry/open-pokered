import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type {
  MapJson, MapScriptConfig, Blockset, EditorTool, DisplayOptions,
  SelectedEntity, ConnectionEntry, ExtraTownMapEntry, TilesetExtra,
} from '../types'
import { TILESET_FILES, MUSIC_LIST } from '../types/constants'
import {
  type TmxMap,
  importTmxToMap,
  exportMapToTmx,
  pickJsonFile,
  downloadJson,
} from '../utils/tmx'
import { setWildOverride, wildOverrideTracked, injectMapData } from '../composables/usePokeredRunner'
import { dataFetch } from '../composables/dataAdapter'
import { gfxRel } from '../utils/assetUrl'

export const useMapStore = defineStore('map', () => {
  const maps = ref<MapJson[]>([])
  const blockData = ref<Record<string, number[]>>({})
  const blocksets = ref<Record<string, Record<number, number[]>>>({})
  const passableTiles = ref<Record<string, number[]>>({})
  const currentMapIndex = ref(0)
  const zoom = ref(2)
  const currentTool = ref<EditorTool>('view')
  const hasUnsavedChanges = ref(false)
  const statusMessage = ref('Loading...')
  const searchQuery = ref('')
  const tilesetImages = ref<Record<string, HTMLImageElement>>({})
  const selectedEntity = ref<SelectedEntity | null>(null)
  const mapHistory = ref<number[]>([])
  const scriptConfigs = ref<Record<string, MapScriptConfig>>({})
  const loading = ref(false)
  const scriptFiles = ref<Record<string, string>>({})
  const scriptEditorOpen = ref(false)
  const scriptJumpTarget = ref<string | null>(null)
  const scriptDirty = ref(false)
  const sceneFiles = ref<Record<string, string>>({})
  const sceneDirty = ref(false)
  const selectedBlockId = ref<number>(0)
  const blkDirty = ref<Set<string>>(new Set())
  const blocksetDirty = ref<Set<string>>(new Set())
  const passableDirty = ref<Set<string>>(new Set())
  const extraTownMapEntries = ref<ExtraTownMapEntry[]>([])
  const tilesetExtras = ref<Record<string, TilesetExtra>>({})

  const displayOptions = ref<DisplayOptions>({
    showTiles: true,
    showCollision: true,
    showWarps: true,
    showSigns: true,
    showNpcs: true,
    showGrid: false,
    showCoordEvents: true,
    showConnections: true,
  })

  const currentMap = computed<MapJson | null>(() => {
    if (maps.value.length === 0) return null
    return maps.value[currentMapIndex.value] ?? null
  })

  const currentBlocks = computed<number[]>(() => {
    const map = currentMap.value
    if (!map) return []
    return blockData.value[map.name] ?? []
  })

  const currentPassableTiles = computed<number[]>(() => {
    const map = currentMap.value
    if (!map) return []
    return passableTiles.value[map.header.tileset] ?? []
  })

  const canGoBack = computed(() => mapHistory.value.length > 0)

  const filteredMaps = computed(() => {
    if (!searchQuery.value) return maps.value.map((m, i) => ({ map: m, index: i }))
    const q = searchQuery.value.toLowerCase()
    return maps.value
      .map((m, i) => ({ map: m, index: i }))
      .filter(({ map }) => map.name.toLowerCase().includes(q))
  })

  function getBlockset(tilesetName: string): Blockset | undefined {
    const blocks = blocksets.value[tilesetName]
    if (!blocks) return undefined
    return { tileset_name: tilesetName, blocks }
  }

  async function loadAllMaps() {
    loading.value = true
    statusMessage.value = 'Loading maps...'
    try {
      const [mapNames, allBlocksets, allPassable, extras, tsExtras] = await Promise.all([
        dataFetch('/api/maps').then(r => r.json()) as Promise<string[]>,
        dataFetch('/api/blocksets').then(r => r.json()) as Promise<Record<string, Record<number, number[]>>>,
        dataFetch('/api/passable-tiles').then(r => r.json()) as Promise<Record<string, number[]>>,
        dataFetch('/api/town-map-extras').then(r => r.ok ? r.json() : {}).catch(() => ({})) as Promise<Record<string, { x: number; y: number; displayName: string }>>,
        dataFetch('/api/tileset-extras').then(r => r.ok ? r.json() : {}).catch(() => ({})) as Promise<Record<string, TilesetExtra>>,
      ])

      tilesetExtras.value = tsExtras
      blocksets.value = allBlocksets
      passableTiles.value = allPassable
      extraTownMapEntries.value = Object.entries(extras).map(([mapName, v]) => ({
        mapName,
        x: v.x,
        y: v.y,
        displayName: v.displayName,
      }))

      const batchSize = 50
      const allMaps: MapJson[] = []
      const allBlocks: Record<string, number[]> = {}
      const allConfigs: Record<string, MapScriptConfig> = {}

      for (let i = 0; i < mapNames.length; i += batchSize) {
        const batch = mapNames.slice(i, i + batchSize)
        const results = await Promise.all(
          batch.map(async (name) => {
            const [mapJson, blk, config] = await Promise.all([
              dataFetch(`/api/maps/${name}/map.json`).then(r => r.json()) as Promise<MapJson>,
              dataFetch(`/api/maps/${name}/map.blk`).then(r => r.json()).catch(() => []) as Promise<number[]>,
              dataFetch(`/api/maps/${name}/script_config.json`).then(r => r.ok ? r.json() : null).catch(() => null) as Promise<MapScriptConfig | null>,
            ])
            return { name, mapJson, blk, config }
          })
        )
        for (const { name, mapJson, blk, config } of results) {
          allMaps.push(mapJson)
          allBlocks[name] = blk
          if (config) {
            allConfigs[name] = config
            applyScriptBindings(mapJson, config)
          }
        }
        statusMessage.value = `Loading maps... ${Math.min(i + batchSize, mapNames.length)}/${mapNames.length}`
      }

      allMaps.sort((a, b) => a.id - b.id)
      maps.value = allMaps
      blockData.value = allBlocks
      scriptConfigs.value = allConfigs

      await loadTilesets()
      currentMapIndex.value = 0
      selectedEntity.value = null
      mapHistory.value = []
      statusMessage.value = `Loaded ${allMaps.length} maps`
    } catch (err) {
      statusMessage.value = `Error: ${(err as Error).message}`
    } finally {
      loading.value = false
    }
  }

  function applyScriptBindings(mapJson: MapJson, config: MapScriptConfig) {
    config.npcs.forEach(({ id, talk, toggleId, scriptId, defaultHidden }) => {
      const npc = mapJson.npcs?.find(n => n.textId === id)
      if (npc) {
        if (talk) npc.talk = talk
        if (toggleId) npc.toggleId = toggleId
        if (scriptId) npc.scriptId = scriptId
        if (defaultHidden != null) npc.defaultHidden = defaultHidden
      }
    })
    config.signs.forEach(({ id, talk }) => {
      const sign = mapJson.signs?.find(s => s.textId === id)
      if (sign) sign.talk = talk
    })
  }

  async function loadTilesets() {
    const loaded: Record<string, HTMLImageElement> = {}
    // Built-in tilesets
    const allFiles: Record<string, string> = { ...TILESET_FILES }
    // Plus user-created ones from tileset_extras.json
    for (const [name, info] of Object.entries(tilesetExtras.value)) {
      allFiles[name] = info.pngFile
    }
    const promises = Object.entries(allFiles).map(async ([name, file]) => {
      try {
        // dataFetch: dev serves /gfx from disk; static mode prefers the
        // IndexedDB delta (an edited tileset) over the bundled baseline.
        const resp = await dataFetch(gfxRel(`tilesets/${file}`))
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
        const blob = await resp.blob()
        const img = new Image()
        img.src = URL.createObjectURL(blob)
        await img.decode()
        loaded[name] = img
      } catch {
        console.warn('Could not load tileset:', name)
      }
    })
    await Promise.all(promises)
    tilesetImages.value = loaded
  }

  function selectMap(index: number) {
    if (index >= 0 && index < maps.value.length) {
      currentMapIndex.value = index
      selectedEntity.value = null
    }
  }

  function navigateToMap(mapName: string) {
    const targetIndex = maps.value.findIndex(m => m.name === mapName)
    if (targetIndex < 0) {
      updateStatus(`Map "${mapName}" not found`)
      return
    }
    mapHistory.value.push(currentMapIndex.value)
    currentMapIndex.value = targetIndex
    selectedEntity.value = null
    updateStatus(`Navigated to ${mapName}`)
  }

  function goBack() {
    const prev = mapHistory.value.pop()
    if (prev != null) {
      currentMapIndex.value = prev
      selectedEntity.value = null
      updateStatus(`Back to ${maps.value[prev]?.name ?? 'unknown'}`)
    }
  }

  function selectEntity(entity: SelectedEntity | null) {
    selectedEntity.value = entity
  }

  function nextMap() {
    if (currentMapIndex.value < maps.value.length - 1) {
      currentMapIndex.value++
      selectedEntity.value = null
    }
  }

  function prevMap() {
    if (currentMapIndex.value > 0) {
      currentMapIndex.value--
      selectedEntity.value = null
    }
  }

  function setTool(tool: EditorTool) {
    currentTool.value = tool
    if (tool === 'edit') {
      selectedEntity.value = null
    }
  }

  function zoomIn() {
    zoom.value = Math.min(4, zoom.value + 1)
  }

  function zoomOut() {
    zoom.value = Math.max(1, zoom.value - 1)
  }

  const currentScriptConfig = computed(() => {
    return currentMap.value ? scriptConfigs.value[currentMap.value.name] : undefined
  })

  function updateNpcTalk(npcIndex: number, talk: string) {
  const map = currentMap.value;
  if (!map || !map.npcs) return;
  const npc = map.npcs[npcIndex];
  if (npc) {
    npc.talk = talk;
    const config = scriptConfigs.value[map.name];
    const configNpc = config?.npcs.find(n => n.id === npc.textId);
    if (configNpc) configNpc.talk = talk;
    hasUnsavedChanges.value = true;
  }
}

function updateNpcToggleId(npcIndex: number, toggleId: string) {
  const map = currentMap.value;
  if (!map || !map.npcs) return;
  const npc = map.npcs[npcIndex];
  if (npc) {
    const config = scriptConfigs.value[map.name];
    let configNpc = config?.npcs.find(n => n.id === npc.textId);
    if (!configNpc) {
      configNpc = { id: npc.textId };
      config?.npcs.push(configNpc);
    }
    configNpc.toggleId = toggleId;
    hasUnsavedChanges.value = true;
  }
}

function updateNpcDefaultHidden(npcIndex: number, hidden: boolean) {
    const map = currentMap.value;
    if (!map || !map.npcs) return;
    const npc = map.npcs[npcIndex];
    if (npc) {
      npc.defaultHidden = hidden;
      const config = scriptConfigs.value[map.name];
      let configNpc = config?.npcs.find(n => n.id === npc.textId);
      if (!configNpc) {
        configNpc = { id: npc.textId };
        config?.npcs.push(configNpc);
      }
      configNpc.defaultHidden = hidden;
      hasUnsavedChanges.value = true;
    }
  }

  /** Add a new NPC to the current map at its center, with a script binding
   *  (textId = next free id), and select it for editing. */
  function addNpc() {
    const map = currentMap.value
    if (!map) return
    if (!map.npcs) map.npcs = []
    const maxTextId = map.npcs.reduce((m, n) => Math.max(m, n.textId || 0), 0)
    const npc = {
      spriteId: 61, // most common overworld NPC sprite
      x: Math.floor(map.header.width / 2),
      y: Math.floor(map.header.height / 2),
      movement: 'Stationary',
      facing: 'Down',
      range: 0,
      textId: maxTextId + 1,
      isTrainer: false,
    }
    map.npcs.push(npc)
    const config = scriptConfigs.value[map.name]
    if (config) config.npcs.push({ id: npc.textId })
    hasUnsavedChanges.value = true
    selectEntity({ type: 'npc', data: npc, index: map.npcs.length - 1 })
  }

  /** Remove an NPC from the current map plus its script binding. */
  function removeNpc(npcIndex: number) {
    const map = currentMap.value
    if (!map || !map.npcs) return
    const npc = map.npcs[npcIndex]
    if (!npc) return
    map.npcs.splice(npcIndex, 1)
    const config = scriptConfigs.value[map.name]
    if (config) config.npcs = config.npcs.filter(n => n.id !== npc.textId)
    if (selectedEntity.value?.type === 'npc' && selectedEntity.value.index === npcIndex) {
      selectedEntity.value = null
    }
    hasUnsavedChanges.value = true
  }

  function updateSignTalk(signIndex: number, talk: string) {
    const map = currentMap.value
    if (!map || !map.signs) return
    const sign = map.signs[signIndex]
    if (sign) {
      sign.talk = talk
      const config = scriptConfigs.value[map.name]
      const configSign = config?.signs.find(s => s.id === sign.textId)
      if (configSign) configSign.talk = talk
      hasUnsavedChanges.value = true
    }
  }

  function addCoordEvent(x: number, y: number, trigger: string) {
    const config = currentScriptConfig.value
    if (config) {
      config.coordEvents.push({ position: [x, y], trigger })
      hasUnsavedChanges.value = true
    }
  }

  function removeCoordEvent(index: number) {
    const config = currentScriptConfig.value
    if (config) {
      config.coordEvents.splice(index, 1)
      hasUnsavedChanges.value = true
      selectedEntity.value = null
    }
  }

  function updateCoordEvent(index: number, updates: { x?: number; y?: number; trigger?: string }) {
    const config = currentScriptConfig.value
    if (!config || index < 0 || index >= config.coordEvents.length) return
    const ce = config.coordEvents[index]
    if (updates.x != null) ce.position[0] = updates.x
    if (updates.y != null) ce.position[1] = updates.y
    if (updates.trigger != null) ce.trigger = updates.trigger
    hasUnsavedChanges.value = true
    if (selectedEntity.value?.type === 'coordEvent' && selectedEntity.value.index === index) {
      selectedEntity.value = {
        type: 'coordEvent',
        data: { x: ce.position[0], y: ce.position[1], trigger: ce.trigger },
        index,
      }
    }
  }

  function setBlockAt(blockIndex: number, blockId: number) {
    const map = currentMap.value
    if (!map) return
    const blocks = blockData.value[map.name]
    if (!blocks) return
    if (blockIndex < 0 || blockIndex >= blocks.length) return
    const id = Math.max(0, Math.min(255, Math.floor(blockId)))
    if (blocks[blockIndex] === id) return
    blocks[blockIndex] = id
    blkDirty.value.add(map.name)
    hasUnsavedChanges.value = true
  }

  function setSelectedBlockId(id: number) {
    selectedBlockId.value = Math.max(0, Math.min(255, Math.floor(id)))
  }

  // Edit a single tile within a block of the given tileset's blockset.
  // tilePos is 0..15 (4×4 grid, row-major). Marks the blockset dirty so the
  // editor will write `.bst` back to disk on save.
  function setBlocksetTile(tilesetName: string, blockId: number, tilePos: number, tileId: number) {
    const bs = blocksets.value[tilesetName]
    if (!bs) return
    const arr = bs[blockId]
    if (!arr || tilePos < 0 || tilePos >= arr.length) return
    const v = Math.max(0, Math.min(255, Math.floor(tileId)))
    if (arr[tilePos] === v) return
    arr[tilePos] = v
    blocksetDirty.value.add(tilesetName)
    hasUnsavedChanges.value = true
  }

  function togglePassableTile(tilesetName: string, tileId: number) {
    const list = passableTiles.value[tilesetName] ?? []
    const v = Math.max(0, Math.min(255, Math.floor(tileId)))
    const idx = list.indexOf(v)
    let next: number[]
    if (idx >= 0) {
      next = list.filter((t) => t !== v)
    } else {
      next = [...list, v].sort((a, b) => a - b)
    }
    passableTiles.value = { ...passableTiles.value, [tilesetName]: next }
    passableDirty.value.add(tilesetName)
    hasUnsavedChanges.value = true
  }

  async function saveCurrentMap() {
    const map = currentMap.value
    if (!map) return

    try {
      const mapCopy = { ...map } as Record<string, unknown>
      if (map.npcs) {
        mapCopy.npcs = map.npcs.map(({ talk, toggleId, scriptId, defaultHidden, ...rest }) => rest)
      }
      if (map.signs) {
        mapCopy.signs = map.signs.map(({ talk, ...rest }) => rest)
      }

      await dataFetch(`/api/maps/${map.name}/map.json`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(mapCopy),
      })

      const config = scriptConfigs.value[map.name]
      if (config) {
        await dataFetch(`/api/maps/${map.name}/script_config.json`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(config),
        })
      }

      // Save .blk if dirty
      if (blkDirty.value.has(map.name)) {
        const blocks = blockData.value[map.name]
        if (blocks) {
          await dataFetch(`/api/maps/${map.name}/map.blk`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(blocks),
          })
          blkDirty.value.delete(map.name)
        }
      }

      // WYSIWYG: push the saved map.json + map.blk into the running game so a
      // saved edit shows up in the playtest without a restart. Best-effort: a
      // not-yet-booted runner is created lazily; failures are silent here.
      try {
        await injectMapData(
          map.name,
          JSON.stringify(mapCopy),
          JSON.stringify(blockData.value[map.name] ?? []),
        )
      } catch {
        /* preview injection is best-effort */
      }

      // Flush dirty blocksets — write the entire .bst back via PUT /api/blocksets/:name
      for (const tilesetName of Array.from(blocksetDirty.value)) {
        const bs = blocksets.value[tilesetName]
        if (!bs) continue
        const r = await dataFetch(`/api/blocksets/${tilesetName}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ blocks: bs }),
        })
        if (r.ok) {
          blocksetDirty.value.delete(tilesetName)
        } else {
          throw new Error(`Failed to save blockset ${tilesetName}: HTTP ${r.status}`)
        }
      }

      // Flush dirty passable-tile lists — written as JSON overrides.
      for (const tilesetName of Array.from(passableDirty.value)) {
        const list = passableTiles.value[tilesetName] ?? []
        const r = await dataFetch(`/api/passable-tiles/${tilesetName}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ tiles: list }),
        })
        if (r.ok) {
          passableDirty.value.delete(tilesetName)
        } else {
          throw new Error(`Failed to save passable tiles for ${tilesetName}: HTTP ${r.status}`)
        }
      }

      hasUnsavedChanges.value = false
      updateStatus(`Saved ${map.name}`)

      // WYSIWYG: push the saved wild-encounter tables into the running game.
      // The wasm build embeds the tables it was compiled with, so without this
      // a saved edit wouldn't show up in the playtest. Best-effort: a
      // not-yet-booted runner is created lazily; failures are silent here.
      try {
        if (map.wild) {
          await setWildOverride(map.name, JSON.stringify(map.wild))
        } else if (wildOverrideTracked(map.name)) {
          await setWildOverride(map.name, null)
        }
      } catch {
        /* preview injection is best-effort */
      }
    } catch (err) {
      updateStatus(`Save error: ${(err as Error).message}`)
    }
  }

  async function loadScriptFile(mapName: string): Promise<string> {
    if (scriptFiles.value[mapName] != null) {
      return scriptFiles.value[mapName]
    }
    const res = await dataFetch(`/api/maps/${mapName}/script.js`)
    const text = res.ok ? await res.text() : ''
    scriptFiles.value[mapName] = text
    return text
  }

  async function saveScriptFile(mapName: string, content: string) {
    try {
      await dataFetch(`/api/maps/${mapName}/script.js`, {
        method: 'PUT',
        headers: { 'Content-Type': 'text/plain' },
        body: content,
      })
      scriptFiles.value[mapName] = content
      scriptDirty.value = false
      updateStatus(`Saved script for ${mapName}`)
    } catch (err) {
      updateStatus(`Script save error: ${(err as Error).message}`)
    }
  }

  function updateScriptContent(mapName: string, content: string) {
    scriptFiles.value[mapName] = content
    scriptDirty.value = true
  }

  async function loadSceneFile(mapName: string): Promise<string> {
    if (sceneFiles.value[mapName] != null) {
      return sceneFiles.value[mapName]
    }
    const res = await dataFetch(`/api/maps/${mapName}/script.scene`)
    const text = res.ok ? await res.text() : ''
    sceneFiles.value[mapName] = text
    return text
  }

  async function saveSceneFile(mapName: string, content: string) {
    try {
      await dataFetch(`/api/maps/${mapName}/script.scene`, {
        method: 'PUT',
        headers: { 'Content-Type': 'text/plain' },
        body: content,
      })
      sceneFiles.value[mapName] = content
      sceneDirty.value = false
      updateStatus(`Saved scene for ${mapName}`)
    } catch (err) {
      updateStatus(`Scene save error: ${(err as Error).message}`)
    }
  }

  function updateSceneContent(mapName: string, content: string) {
    sceneFiles.value[mapName] = content
    sceneDirty.value = true
  }

  function openScriptEditor() {
    scriptEditorOpen.value = true
  }

  function closeScriptEditor() {
    scriptEditorOpen.value = false
    scriptJumpTarget.value = null
  }

  function jumpToFunction(funcName: string) {
    scriptEditorOpen.value = true
    scriptJumpTarget.value = funcName
  }

  function clearJumpTarget() {
    scriptJumpTarget.value = null
  }

  function updateStatus(msg: string) {
    statusMessage.value = msg
  }

  function updateMapMusic(music: string) {
    const map = currentMap.value
    if (!map) return
    map.header.music = music
    hasUnsavedChanges.value = true
  }

  function updateMapConnection(direction: 'north' | 'south' | 'west' | 'east', entry: ConnectionEntry | null) {
    const map = currentMap.value
    if (!map) return
    if (entry) {
      map.connections[direction] = entry
    } else {
      delete map.connections[direction]
    }
    hasUnsavedChanges.value = true
  }

  function getMapNames(): string[] {
    return maps.value.map(m => m.name)
  }

  // ---- Wild encounter editing ----

  type WildVersion = 'red' | 'blue'
  type WildTerrain = 'grass' | 'water'

  function emptyVersionWild() {
    return {
      grass: { encounterRate: 0, mons: [] as { level: number; species: string }[] },
      water: { encounterRate: 0, mons: [] as { level: number; species: string }[] },
    }
  }

  function ensureWildData() {
    const map = currentMap.value
    if (!map) return null
    if (!map.wild) {
      map.wild = { red: emptyVersionWild(), blue: emptyVersionWild() }
      hasUnsavedChanges.value = true
    } else {
      if (!map.wild.red) map.wild.red = emptyVersionWild()
      if (!map.wild.blue) map.wild.blue = emptyVersionWild()
    }
    return map.wild
  }

  function clearWildData() {
    const map = currentMap.value
    if (!map) return
    map.wild = null
    hasUnsavedChanges.value = true
  }

  function getWildTable(version: WildVersion, terrain: WildTerrain) {
    const wild = ensureWildData()
    if (!wild) return null
    const v = wild[version]
    if (!v) return null
    return v[terrain]
  }

  function updateWildEncounterRate(version: WildVersion, terrain: WildTerrain, rate: number) {
    const table = getWildTable(version, terrain)
    if (!table) return
    table.encounterRate = Math.max(0, Math.min(255, Math.floor(rate)))
    hasUnsavedChanges.value = true
  }

  function updateWildMon(
    version: WildVersion,
    terrain: WildTerrain,
    index: number,
    updates: { level?: number; species?: string },
  ) {
    const table = getWildTable(version, terrain)
    if (!table || index < 0 || index >= table.mons.length) return
    const mon = table.mons[index]
    if (updates.level != null) mon.level = Math.max(1, Math.min(100, Math.floor(updates.level)))
    if (updates.species != null) mon.species = updates.species
    hasUnsavedChanges.value = true
  }

  function addWildMon(version: WildVersion, terrain: WildTerrain) {
    const table = getWildTable(version, terrain)
    if (!table) return
    table.mons.push({ level: 1, species: 'None' })
    hasUnsavedChanges.value = true
  }

  function removeWildMon(version: WildVersion, terrain: WildTerrain, index: number) {
    const table = getWildTable(version, terrain)
    if (!table || index < 0 || index >= table.mons.length) return
    table.mons.splice(index, 1)
    hasUnsavedChanges.value = true
  }

  function fillWildMonsTo(version: WildVersion, terrain: WildTerrain, count: number) {
    const table = getWildTable(version, terrain)
    if (!table) return
    while (table.mons.length < count) {
      table.mons.push({ level: 1, species: 'None' })
    }
    hasUnsavedChanges.value = true
  }

  function copyWildTables(srcVersion: WildVersion, dstVersion: WildVersion) {
    if (srcVersion === dstVersion) return
    const wild = ensureWildData()
    if (!wild) return
    const src = wild[srcVersion]
    const dst = wild[dstVersion]
    if (!src || !dst) return
    for (const terrain of ['grass', 'water'] as WildTerrain[]) {
      const srcTable = src[terrain]
      const dstTable = dst[terrain]
      dstTable.encounterRate = srcTable.encounterRate
      dstTable.mons.splice(
        0,
        dstTable.mons.length,
        ...srcTable.mons.map(m => ({ level: m.level, species: m.species })),
      )
    }
    hasUnsavedChanges.value = true
  }

  async function createMap(opts: {
    name: string
    displayName?: string
    tileset?: string
    width?: number
    height?: number
    music?: string
    borderBlock?: number
    townMap?: { x: number; y: number }
  }): Promise<{ ok: boolean; error?: string }> {
    try {
      const res = await dataFetch('/api/maps', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(opts),
      })
      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: `HTTP ${res.status}` }))
        return { ok: false, error: err.error ?? `HTTP ${res.status}` }
      }

      const name = opts.name
      const [mapJson, blk, config] = await Promise.all([
        dataFetch(`/api/maps/${name}/map.json`).then(r => r.json()) as Promise<MapJson>,
        dataFetch(`/api/maps/${name}/map.blk`).then(r => r.json()).catch(() => []) as Promise<number[]>,
        dataFetch(`/api/maps/${name}/script_config.json`).then(r => r.ok ? r.json() : null).catch(() => null) as Promise<MapScriptConfig | null>,
      ])

      maps.value.push(mapJson)
      maps.value.sort((a, b) => a.id - b.id)
      blockData.value[name] = blk
      if (config) {
        scriptConfigs.value[name] = config
        applyScriptBindings(mapJson, config)
      }

      if (opts.townMap) {
        const existing = extraTownMapEntries.value.findIndex(e => e.mapName === name)
        const entry: ExtraTownMapEntry = {
          mapName: name,
          x: opts.townMap.x,
          y: opts.townMap.y,
          displayName: (opts.displayName ?? name).toUpperCase(),
        }
        if (existing >= 0) {
          extraTownMapEntries.value[existing] = entry
        } else {
          extraTownMapEntries.value.push(entry)
        }
      }

      const newIndex = maps.value.findIndex(m => m.name === name)
      if (newIndex >= 0) currentMapIndex.value = newIndex
      updateStatus(`Created map ${name}`)
      return { ok: true }
    } catch (err) {
      return { ok: false, error: (err as Error).message }
    }
  }

  async function createTileset(opts: {
    name: string
    base?: string
    category?: 'outdoor' | 'indoor' | 'cave'
    displayName?: string
  }): Promise<{ ok: boolean; error?: string }> {
    try {
      const res = await dataFetch('/api/tilesets', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(opts),
      })
      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: `HTTP ${res.status}` }))
        return { ok: false, error: err.error ?? `HTTP ${res.status}` }
      }
      const [tsExtras, allBlocksets, allPassable] = await Promise.all([
        dataFetch('/api/tileset-extras').then(r => r.ok ? r.json() : {}) as Promise<Record<string, TilesetExtra>>,
        dataFetch('/api/blocksets').then(r => r.json()) as Promise<Record<string, Record<number, number[]>>>,
        dataFetch('/api/passable-tiles').then(r => r.json()) as Promise<Record<string, number[]>>,
      ])
      tilesetExtras.value = tsExtras
      blocksets.value = allBlocksets
      passableTiles.value = allPassable
      await loadTilesets()
      updateStatus(`Created tileset ${opts.name}`)
      return { ok: true }
    } catch (err) {
      return { ok: false, error: (err as Error).message }
    }
  }

  async function importTmxFromFile(): Promise<string[]> {
    try {
      const raw = await pickJsonFile()
      const tmx = raw as TmxMap

      if (!tmx.layers || !tmx.tilesets) {
        return ['Invalid TMX file: missing layers or tilesets']
      }

      const mapName = `Imported_${String(tmx.properties?.find(p => p.name === 'mapName')?.value ?? 'Map')}_${Date.now()}`
      const tilesetName = String(tmx.properties?.find(p => p.name === 'tileset')?.value ?? 'Overworld')

      const bs = blocksets.value[tilesetName]
      if (!bs) {
        return [`Tileset "${tilesetName}" not found in loaded blocksets. Available: ${Object.keys(blocksets.value).join(', ')}`]
      }

      const maxId = maps.value.reduce((max, m) => Math.max(max, m.id), -1)
      const newId = maxId + 1

      const { mapJson, blockData: blkData, warnings } = importTmxToMap(
        tmx,
        tilesetName,
        bs,
        mapName,
        newId,
        'PalletTown',
        0,
      )

      const map = mapJson as unknown as MapJson
      maps.value.push(map)
      maps.value.sort((a, b) => a.id - b.id)
      blockData.value = { ...blockData.value, [map.name]: blkData }
      scriptConfigs.value = { ...scriptConfigs.value, [map.name]: { npcs: [], signs: [], coordEvents: [] } }

      const newIndex = maps.value.findIndex(m => m.name === map.name)
      if (newIndex >= 0) currentMapIndex.value = newIndex
      selectedEntity.value = null
      updateStatus(`Imported ${map.name}${warnings.length > 0 ? ` (${warnings.length} warning(s))` : ''}`)

      return warnings
    } catch (err) {
      updateStatus(`TMX import error: ${(err as Error).message}`)
      return [(err as Error).message]
    }
  }

  function exportCurrentMapToTmx() {
    const map = currentMap.value
    if (!map) {
      updateStatus('No map loaded to export')
      return
    }

    const blocks = blockData.value[map.name]
    if (!blocks) {
      updateStatus('No block data available for export')
      return
    }

    const tilesetName = map.header.tileset
    const bs = blocksets.value[tilesetName]
    if (!bs) {
      updateStatus(`Blockset "${tilesetName}" not loaded`)
      return
    }

    const pngFile = TILESET_FILES[tilesetName] ?? `${tilesetName.toLowerCase()}.png`
    const tmx = exportMapToTmx(
      map as unknown as Record<string, unknown>,
      blocks,
      bs,
      {
        tilesetName,
        tilesetImage: pngFile,
        tileWidth: 8,
        tileHeight: 8,
        firstGid: 1,
      },
    )

    const filename = `${map.name}.tmx.json`
    downloadJson(tmx, filename)
    updateStatus(`Exported ${filename}`)
  }

  return {
    maps,
    blockData,
    blocksets,
    passableTiles,
    currentMapIndex,
    zoom,
    currentTool,
    hasUnsavedChanges,
    statusMessage,
    searchQuery,
    tilesetImages,
    displayOptions,
    selectedEntity,
    mapHistory,
    loading,
    currentMap,
    currentBlocks,
    currentPassableTiles,
    canGoBack,
    filteredMaps,
    getBlockset,
    loadAllMaps,
    selectMap,
    navigateToMap,
    goBack,
    selectEntity,
    nextMap,
    prevMap,
    setTool,
    zoomIn,
    zoomOut,
    scriptConfigs,
    currentScriptConfig,
    updateNpcTalk,
    updateNpcToggleId,
    updateNpcDefaultHidden,
    addNpc,
    removeNpc,
    updateSignTalk,
    addCoordEvent,
    removeCoordEvent,
    updateCoordEvent,
    saveCurrentMap,
    updateStatus,
    scriptFiles,
    scriptEditorOpen,
    scriptJumpTarget,
    scriptDirty,
    loadScriptFile,
    saveScriptFile,
    updateScriptContent,
    sceneFiles,
    sceneDirty,
    loadSceneFile,
    saveSceneFile,
    updateSceneContent,
    openScriptEditor,
    closeScriptEditor,
    jumpToFunction,
    clearJumpTarget,
    updateMapMusic,
    updateMapConnection,
    getMapNames,
    ensureWildData,
    clearWildData,
    updateWildEncounterRate,
    updateWildMon,
    addWildMon,
    removeWildMon,
    fillWildMonsTo,
    copyWildTables,
    selectedBlockId,
    blkDirty,
    blocksetDirty,
    passableDirty,
    extraTownMapEntries,
    tilesetExtras,
    setBlockAt,
    setSelectedBlockId,
    setBlocksetTile,
    togglePassableTile,
    createMap,
    createTileset,
    importTmxFromFile,
    exportCurrentMapToTmx,
    MUSIC_LIST,
  }
})
