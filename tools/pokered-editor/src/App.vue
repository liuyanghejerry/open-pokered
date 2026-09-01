<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import PlaytestOverlay from './components/PlaytestOverlay.vue'
import { usePlaytestOverlay } from './composables/usePlaytestOverlay'
import { useMapStore } from './stores/mapStore'
import { useTrainerStore } from './stores/trainerStore'
import { useLayoutStore } from './stores/layoutStore'
import { usePokemonStore } from './stores/pokemonStore'
import { useMoveStore } from './stores/moveStore'
import { usePixelStore } from './stores/pixelStore'
import { useItemStore } from './stores/itemStore'
import { detectBackend } from './composables/dataAdapter'
import { exportDeltasJson, importDeltasJson, clearDeltas } from './composables/useDataStore'
import { staticMode } from './composables/useStaticMode'
import { loadPokeredRunnerModule } from './composables/usePokeredRunner'
import { loadLayoutPreviewModule } from './composables/useWasmPreview'
import { publishGame } from './publish/publish'
import { storeToRefs } from 'pinia'
import ActivityBar from './components/ActivityBar.vue'
import StatusBar from './components/StatusBar.vue'
import MapSidebar from './components/MapSidebar.vue'
import ScriptSidebar from './components/ScriptSidebar.vue'
import SaveSidebar from './components/SaveSidebar.vue'
import TrainerSidebar from './components/TrainerSidebar.vue'
import TrainerEditor from './components/TrainerEditor.vue'
import LayoutSidebar from './components/LayoutSidebar.vue'
import LayoutEditor from './components/LayoutEditor.vue'
import PixelSidebar from './components/PixelSidebar.vue'
import PixelEditor from './components/PixelEditor.vue'
import PokemonSidebar from './components/PokemonSidebar.vue'
import PokemonEditor from './components/PokemonEditor.vue'
import MoveSidebar from './components/MoveSidebar.vue'
import MoveEditor from './components/MoveEditor.vue'
import type { SaveSection } from './components/SaveSidebar.vue'
import EditorToolbar from './components/EditorToolbar.vue'
import MapCanvas from './components/MapCanvas.vue'
import EntityDetailPanel from './components/EntityDetailPanel.vue'
import ScriptEditorPanel from './components/ScriptEditorPanel.vue'
import PartyEditor from './components/PartyEditor.vue'
import FlagEditor from './components/FlagEditor.vue'
import SaveItemsEditor from './components/SaveItemsEditor.vue'
import AssistantPanel from './components/assistant/AssistantPanel.vue'
import {
  type SaveDataSnapshot,
  type PokemonEntry,
  type ItemEntry,
  type PlayerInfo,
  type Facing,
  createDefaultSaveData,
  BADGE_NAMES,
  FACING_DIRECTIONS,
  MAP_NAMES,
} from './types/save-data'
import {
  sharedScriptFunctions,
  sharedActiveFunction,
  sharedDslBlocks,
  sharedActiveDslBlock,
  sharedScriptMode,
} from './composables/useScriptState'
import type { ScriptFunction, DslBlock } from './composables/useCodeMirror'

const store = useMapStore()
const trainerStore = useTrainerStore()
const layoutStore = useLayoutStore()
const pokemonStore = usePokemonStore()
const moveStore = useMoveStore()
const pixelStore = usePixelStore()
const itemStore = useItemStore()
const playtestOverlay = usePlaytestOverlay()
const router = useRouter()
const route = useRoute()
const { currentMap, selectedEntity, scriptEditorOpen } = storeToRefs(store)
const { activeClass: activeTrainerClass } = storeToRefs(trainerStore)
const { activeName: activeLayoutName } = storeToRefs(layoutStore)
const { activeSpecies: activePokemonSpecies } = storeToRefs(pokemonStore)
const { activeMove: activeMoveId } = storeToRefs(moveStore)
const { activeAsset } = storeToRefs(pixelStore)

const sidebarFunctions = computed(() => sharedScriptFunctions.value)
const sidebarActiveFn = computed(() => sharedActiveFunction.value)
const sidebarDslBlocks = computed(() => sharedDslBlocks.value)
const sidebarActiveDslBlock = computed(() => sharedActiveDslBlock.value)
const sidebarIsDslMode = computed(() => sharedScriptMode.value === 'dsl')

type Activity = 'map' | 'script' | 'save' | 'trainer' | 'pokemon' | 'move' | 'layout' | 'pixel' | 'playtest'

// Props from router: read initial activity and sync bidirectional
const props = defineProps<{
  routeActivity?: Activity
  routeQuery?: Record<string, string | string[]>
}>()

const activeActivity = ref<Activity>((props.routeActivity as Activity) ?? 'map')
const saveSubTab = ref<SaveSection>('info')
const saveData = reactive<SaveDataSnapshot>(createDefaultSaveData())
const assistantOpen = ref(false)

function setActivity(activity: Activity) {
  if (activity === 'playtest') {
    // The playtest is a floating overlay available above every activity now —
    // the ActivityBar button toggles it instead of switching activities.
    playtestOverlay.toggleOverlay()
    if (activeActivity.value === 'playtest') {
      // Arrived via a /playtest deep link with no real activity behind it.
      activeActivity.value = 'map'
      router.replace({ path: '/map' })
    }
    return
  }
  activeActivity.value = activity
  if (activity === 'script') {
    store.openScriptEditor()
  } else {
    store.closeScriptEditor()
  }
}

/** The router path for an activity (mirrors the activity→URL sync below). */
function activityPath(act: Activity): string {
  if (act === 'map') return '/map'
  if (act === 'script') return '/script'
  if (act === 'trainer') {
    const cls = activeTrainerClass.value
    return cls ? `/trainer/${cls}` : '/trainer'
  }
  if (act === 'pokemon') {
    const s = activePokemonSpecies.value
    return s ? `/pokemon/${s}` : '/pokemon'
  }
  if (act === 'move') {
    const m = activeMoveId.value
    return m ? `/move/${m}` : '/move'
  }
  if (act === 'layout') {
    const name = activeLayoutName.value
    return name ? `/layout/${name}` : '/layout'
  }
  if (act === 'pixel') {
    const asset = activeAsset.value?.id
    return asset ? `/pixel/${asset}` : '/pixel'
  }
  return `/save/${saveSubTab.value}`
}

// Sync activity → URL
watch(activeActivity, (act) => {
  if (act === 'playtest') return // the overlay has no URL of its own
  router.replace({ path: activityPath(act) })
})

// Sync URL → activity (browser back/forward, direct navigation)
watch(() => route.fullPath, () => {
  if (route.matched.length === 0) return
  const name = route.name as string
  if (name === 'save') {
    if (activeActivity.value !== 'save') setActivity('save')
  } else if (name === 'trainer') {
    if (activeActivity.value !== 'trainer') setActivity('trainer')
    const cls = route.params.className as string | undefined
    if (cls && cls !== activeTrainerClass.value) {
      trainerStore.loadClass(cls)
    }
  } else if (name === 'pokemon') {
    if (activeActivity.value !== 'pokemon') setActivity('pokemon')
    const s = route.params.species as string | undefined
    if (s && s !== activePokemonSpecies.value) {
      pokemonStore.loadSpecies(s)
    }
  } else if (name === 'move') {
    if (activeActivity.value !== 'move') setActivity('move')
    const m = route.params.moveId as string | undefined
    if (m && m !== activeMoveId.value) {
      moveStore.loadMove(m)
    }
  } else if (name === 'layout') {
    if (activeActivity.value !== 'layout') setActivity('layout')
    const layName = route.params.name as string | undefined
    if (layName && layName !== activeLayoutName.value) {
      layoutStore.loadLayout(layName)
    }
  } else if (name === 'pixel') {
    if (activeActivity.value !== 'pixel') setActivity('pixel')
    const asset = route.params.asset as string | undefined
    if (asset && (!activeAsset.value || asset !== activeAsset.value.id)) {
      // Asset loading will be handled by the pixel store when the sidebar loads
    }
  } else if (name === 'playtest') {
    // Legacy full-screen playtest URL: the playtest is a floating overlay
    // now, so open it above the current activity and restore the URL.
    playtestOverlay.openOverlay()
    if (activeActivity.value === 'playtest') activeActivity.value = 'map'
    router.replace({ path: activityPath(activeActivity.value) })
  } else {
    const act = route.params.activity as Activity
    if (act && act !== activeActivity.value) setActivity(act)
  }
})

// Read save section from URL
watch(() => route.params.section, (section) => {
  if (activeActivity.value === 'save' && section && ['info','party','flags','items'].includes(section as string)) {
    saveSubTab.value = section as SaveSection
  }
}, { immediate: true })

// Sync save section → URL (when user clicks sidebar while already in save mode)
watch(saveSubTab, (section) => {
  if (activeActivity.value === 'save') {
    router.replace({ path: `/save/${section}` })
  }
})

const saveSectionLabels: Record<SaveSection, string> = {
  info: 'Player Info',
  party: 'Party',
  flags: 'Flags',
  items: 'Items',
}

// Auto-switch to script mode when jump-to-function is triggered from other panels
watch(scriptEditorOpen, (open) => {
  if (open && activeActivity.value !== 'script') {
    activeActivity.value = 'script'
  }
})

function handleSaveSection(section: SaveSection) {
  saveSubTab.value = section
}

/** Open the floating playtest booted with the Save Editor snapshot — the
 *  constructed player/party/items/flags become the running game's save. */
function playtestSave() {
  playtestOverlay.launch({ kind: 'playSave', save: JSON.stringify(saveData) })
}

function handleScriptFunctionSelect(fn: ScriptFunction) {
  sharedActiveFunction.value = fn.name
}

function handleDslBlockSelect(block: DslBlock) {
  sharedActiveDslBlock.value = block
}

// ---- Save data handlers (lifted from SaveEditor) ----

function updatePlayer<K extends keyof PlayerInfo>(key: K, value: PlayerInfo[K]) {
  saveData.player = { ...saveData.player, [key]: value }
}

function toggleBadge(index: number) {
  const badges = [...saveData.badges]
  badges[index] = !badges[index]
  saveData.badges = badges
}

function importJson() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = '.json'
  input.onchange = () => {
    const file = input.files?.[0]
    if (!file) return
    const reader = new FileReader()
    reader.onload = () => {
      try {
        const data = JSON.parse(reader.result as string)
        if (data) {
          saveData.player = data.player ?? saveData.player
          saveData.badges = data.badges ?? saveData.badges
          saveData.party = data.party ?? saveData.party
          saveData.items = data.items ?? saveData.items
          saveData.flags = data.flags ?? saveData.flags
        }
      } catch (e) {
        alert('Failed to parse JSON: ' + (e as Error).message)
      }
    }
    reader.readAsText(file)
  }
  input.click()
}

function exportJson() {
  const json = JSON.stringify(saveData, null, 2)
  const blob = new Blob([json], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'save-data.json'
  a.click()
  URL.revokeObjectURL(url)
}

function exportJsonPrint() {
  const json = JSON.stringify(saveData, null, 2)
  console.log(json)
}

// ── Static-mode edit export / import / reset (IndexedDB deltas) ──────────

/** Download all local edits (text + binary) as a JSON backup. */
async function exportEdits() {
  try {
    const json = await exportDeltasJson()
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'pokered-editor-edits.json'
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    alert(`Export failed: ${(e as Error).message}`)
  }
}

/** Replace all local edits from an exported JSON backup, then reload. */
function importEdits() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = '.json'
  input.onchange = async () => {
    const file = input.files?.[0]
    if (!file) return
    try {
      const count = await importDeltasJson(await file.text())
      alert(`Imported ${count} edits — reloading…`)
      location.reload()
    } catch (e) {
      alert(`Import failed: ${(e as Error).message}`)
    }
  }
  input.click()
}

/** Wipe every local edit, then reload. */
function resetEdits() {
  if (!confirm('Delete all local edits? This cannot be undone.')) return
  clearDeltas().then(() => location.reload())
}

// ── Static-hosting game publish ───────────────────────────────────────────

/** Build + download the self-contained published game (runner wasm + local
 *  edits in one playable HTML file) — works with no /api backend at all. */
const publishing = ref(false)
async function publishStaticGame() {
  const title = prompt('Published game title:', 'My Pokémon Red')
  if (title === null) return
  publishing.value = true
  try {
    const r = await publishGame(title)
    alert(
      `Published ${r.fileName} (${(r.sizeBytes / 1024 / 1024).toFixed(1)} MB) — ` +
        `${r.deltaCount} local edit(s) embedded. Open the file in any browser to play; ` +
        `progress auto-saves in that browser.`,
    )
  } catch (e) {
    alert(`Publish failed: ${(e as Error).message}`)
  } finally {
    publishing.value = false
  }
}

onMounted(() => {
  // Legacy deep link straight into the old full-screen playtest: open the
  // floating overlay and fall back to the map editor.
  if (props.routeActivity === 'playtest') {
    playtestOverlay.openOverlay()
    activeActivity.value = 'map'
    router.replace({ path: '/map' })
  }
  initialize()
})

watch(activeTrainerClass, (cls) => {
  if (activeActivity.value === 'trainer' && cls) {
    router.replace({ path: `/trainer/${cls}` })
  }
})

watch(activePokemonSpecies, (s) => {
  if (activeActivity.value === 'pokemon' && s) {
    router.replace({ path: `/pokemon/${s}` })
  }
})

watch(activeMoveId, (m) => {
  if (activeActivity.value === 'move' && m) {
    router.replace({ path: `/move/${m}` })
  }
})

watch(activeLayoutName, (name) => {
  if (activeActivity.value === 'layout' && name) {
    router.replace({ path: `/layout/${name}` })
  }
})

watch(activeAsset, (asset) => {
  if (activeActivity.value === 'pixel' && asset) {
    router.replace({ path: `/pixel/${asset.id}` })
  }
})

function handleTrainerSelect(name: string) {
  trainerStore.loadClass(name)
}

/** Assistant "jump to activity" — the payload is a plain string; normalize it
 *  into the Activity union here (script scope, so vue-tsc resolves the type
 *  correctly — an inline template `as Activity` assertion trips TS2719 under
 *  `vue-tsc -b` project references). */
function handleAssistantJump(activity: string) {
  setActivity(activity as Activity)
}

/** Editor is interactive only after all initial data has loaded. */
const appReady = ref(false)
const initStatus = ref('Initializing…')
const initError = ref<string | null>(null)

/** Probe the /api backend; on failure flag static mode (baseline data only). */
async function detectStaticMode() {
  staticMode.value = !(await detectBackend())
}

/**
 * Boot sequence: gate the whole UI behind `appReady`. Every data source
 * (maps, trainers, pokemon, moves, layouts, items) is awaited — success or
 * failure — before the editor becomes interactive, so panels are never shown
 * half-loaded. Failures are surfaced in the init banner (static hosting with
 * a missing baseline) instead of silently rendering empty editors.
 */
async function initialize() {
  initStatus.value = 'Checking data backend…'
  await detectStaticMode()

  const loads: { label: string; run: Promise<unknown> }[] = [
    { label: 'maps', run: store.loadAllMaps() },
    { label: 'trainers', run: trainerStore.loadClassList() },
    { label: 'pokemon', run: pokemonStore.loadSpeciesList() },
    { label: 'moves', run: moveStore.loadMoveList() },
    { label: 'UI layouts', run: layoutStore.loadList() },
    { label: 'items', run: itemStore.loadItemList() },
  ]
  const failed: string[] = []
  const inFlight = loads.map(({ label, run }) =>
    run.then(
      () => { initStatus.value = `Loaded ${label}` },
      () => { failed.push(label); initStatus.value = `Failed to load ${label}` },
    ),
  )
  await Promise.allSettled(inFlight)

  // Preload the WASM bridges so Playtest / Game preview / Layout preview are
  // instant once the editor is ready. On static hosting the binaries are
  // bundled at deploy time — surface their byte progress in the loading
  // screen and treat failure as a (non-blocking) banner. In dev the preloads
  // run in the background: a missing local wasm build must stay silent here
  // (the playtest panel explains how to build it).
  if (staticMode.value) {
    initStatus.value = 'Loading game engine…'
    const mb = (n: number) => (n / 1024 / 1024).toFixed(1)
    const progress = (label: string) => (loaded: number, total: number) => {
      initStatus.value = total > 0
        ? `${label} ${mb(loaded)}/${mb(total)} MB`
        : `${label} ${mb(loaded)} MB`
    }
    try {
      await loadPokeredRunnerModule(progress('Loading game engine…'))
      initStatus.value = 'Game engine ready'
    } catch {
      failed.push('game engine')
      initStatus.value = 'Game engine unavailable'
    }

    initStatus.value = 'Loading layout preview…'
    try {
      await loadLayoutPreviewModule(progress('Loading layout preview…'))
      initStatus.value = 'Layout preview ready'
    } catch {
      failed.push('layout preview')
      initStatus.value = 'Layout preview unavailable'
    }
  } else {
    // Warm the module caches without blocking or reporting (dev/Electron).
    loadPokeredRunnerModule().catch(() => {})
    loadLayoutPreviewModule().catch(() => {})
  }

  // Deep-load the activity selected by the URL, if any (kept from the old
  // onMounted; loadClass etc. also fire from the route watcher on nav).
  if (route.name === 'trainer') {
    const cls = route.params.className as string | undefined
    if (cls) trainerStore.loadClass(cls)
  } else if (route.name === 'pokemon') {
    const s = route.params.species as string | undefined
    if (s) pokemonStore.loadSpecies(s)
  } else if (route.name === 'move') {
    const m = route.params.moveId as string | undefined
    if (m) moveStore.loadMove(m)
  } else if (route.name === 'layout') {
    const layName = route.params.name as string | undefined
    if (layName) layoutStore.loadLayout(layName)
  }
  // pixel: asset loading happens in the pixel sidebar on mount.

  if (failed.length > 0) {
    initError.value = `Some resources failed to load: ${failed.join(', ')}`
  }
  appReady.value = true
}

function handlePokemonSelect(name: string) {
  pokemonStore.loadSpecies(name)
}

function handleMoveSelect(name: string) {
  moveStore.loadMove(name)
}

function handleLayoutSelect(name: string) {
  layoutStore.loadLayout(name)
}

function handlePixelSelect() {
  // Pixel sidebar handles asset selection internally
}
</script>

<template>
  <!-- Loading gate: the editor shell mounts only after all data has loaded -->
  <div
    v-if="!appReady"
    class="h-screen flex flex-col items-center justify-center gap-4 bg-bg text-text"
  >
    <div class="text-2xl font-bold tracking-wide text-accent select-none">PokéRed Editor</div>
    <div class="w-64 h-1.5 rounded bg-bg-inset overflow-hidden">
      <div class="h-full bg-accent rounded animate-pulse" />
    </div>
    <div class="text-xs text-text-muted font-mono">{{ initStatus }}</div>
    <div v-if="initError" class="max-w-md text-center text-[11px] text-danger">{{ initError }}</div>
  </div>

  <div v-else class="h-screen flex flex-col">
    <div v-if="staticMode" class="px-3 py-1.5 text-[11px] bg-[#2c2c15] border-b border-[#e6b422]/40 text-warning shrink-0 flex items-center gap-3 flex-wrap">
      <span>⚠ Static build: edits are saved in this browser (IndexedDB).</span>
      <button
        class="px-2 py-0.5 rounded text-[10px] font-bold cursor-pointer bg-bg-inset text-warning border border-[#e6b422]/40 hover:border-warning"
        @click="exportEdits()"
      >
        💾 Export edits
      </button>
      <button
        class="px-2 py-0.5 rounded text-[10px] font-bold cursor-pointer bg-bg-inset text-warning border border-[#e6b422]/40 hover:border-warning"
        @click="importEdits()"
      >
        📂 Import edits
      </button>
      <button
        class="px-2 py-0.5 rounded text-[10px] font-bold cursor-pointer bg-bg-inset text-warning border border-[#e6b422]/40 hover:border-warning"
        :disabled="publishing"
        title="Download the edited game as one playable HTML file (game engine + your edits embedded)"
        @click="publishStaticGame()"
      >
        🚀 {{ publishing ? 'Publishing…' : 'Publish game' }}
      </button>
      <button
        class="px-2 py-0.5 rounded text-[10px] font-bold cursor-pointer bg-bg-inset text-danger border border-danger/40 hover:border-danger"
        @click="resetEdits()"
      >
        🗑 Reset edits
      </button>
      <span class="text-text-muted">AI 助手在静态模式可用（浏览器直连，需在助手设置中配置 API Key）；精灵图生成仍需要本地后端。</span>
    </div>
    <div class="flex-1 flex min-h-0">
      <!-- Activity Bar -->
      <ActivityBar
        :active="activeActivity"
        :assistant-open="assistantOpen"
        :playtest-open="playtestOverlay.open.value"
        @select="setActivity"
        @toggle-assistant="assistantOpen = !assistantOpen"
      />

      <!-- Sidebar: context-aware per mode -->
      <div
        class="w-80 flex-shrink-0 overflow-y-auto bg-bg-panel border-r border-[rgba(255,255,255,0.06)]"
      >
        <MapSidebar v-if="activeActivity === 'map'" />
        <ScriptSidebar
          v-else-if="activeActivity === 'script'"
          :functions="sidebarFunctions"
          :active-function="sidebarActiveFn"
          :dsl-blocks="sidebarDslBlocks"
          :active-dsl-block="sidebarActiveDslBlock"
          :is-dsl-mode="sidebarIsDslMode"
          :map-name="currentMap?.name ?? ''"
          @select="handleScriptFunctionSelect"
          @select-dsl="handleDslBlockSelect"
        />
        <SaveSidebar
          v-else-if="activeActivity === 'save'"
          :active-section="saveSubTab"
          @select="handleSaveSection"
        />
        <TrainerSidebar
          v-else-if="activeActivity === 'trainer'"
          @select="handleTrainerSelect"
        />
        <PokemonSidebar
          v-else-if="activeActivity === 'pokemon'"
          @select="handlePokemonSelect"
        />
        <MoveSidebar
          v-else-if="activeActivity === 'move'"
          @select="handleMoveSelect"
        />
        <LayoutSidebar
          v-else-if="activeActivity === 'layout'"
          @select="handleLayoutSelect"
        />
        <PixelSidebar
          v-else-if="activeActivity === 'pixel'"
          @select="handlePixelSelect"
        />
        <!-- Playtest has no sidebar: the whole pane is the game. -->
      </div>

      <!-- Main Area -->
      <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
        <!-- Map mode -->
        <template v-if="activeActivity === 'map'">
          <div class="flex-1 flex flex-col overflow-hidden relative">
            <EditorToolbar />
            <div class="flex-1 overflow-auto relative">
              <MapCanvas />

              <!-- Legend Overlay -->
              <div class="absolute top-2 right-2 z-10">
                <details class="bg-bg-panel/90 rounded border border-[rgba(255,255,255,0.1)] shadow-lg">
                  <summary class="px-2 py-1 text-[11px] text-accent font-bold cursor-pointer select-none">
                    📖 Legend
                  </summary>
                  <div class="p-2 text-[10px] space-y-0.5 min-w-[130px]">
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(78,204,163,0.5)"></span> Passable</div>
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(231,76,60,0.5)"></span> Blocked</div>
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(52,152,219,0.8)"></span> Warp</div>
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(241,196,15,0.8)"></span> Sign</div>
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(231,76,60,0.8)"></span> Trainer</div>
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(46,204,113,0.8)"></span> Item NPC</div>
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(230,126,34,0.8)"></span> Coord Event</div>
                    <div class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-sm inline-block" style="background:rgba(155,89,182,0.8)"></span> NPC</div>
                  </div>
                </details>
              </div>
            </div>

            <!-- Entity Detail floating panel -->
            <EntityDetailPanel
              v-if="selectedEntity"
              class="absolute top-12 right-2 z-20 shadow-xl"
            />
          </div>
        </template>

        <!-- Script mode -->
        <template v-else-if="activeActivity === 'script'">
          <ScriptEditorPanel />
        </template>

        <!-- Save mode -->
        <div v-else-if="activeActivity === 'save'" class="flex-1 flex flex-col min-h-0">
          <!-- Save header -->
          <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
            <h2 class="text-accent text-sm font-bold">Save Editor</h2>
            <div class="flex gap-2">
              <button
                class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent"
                title="Open the floating playtest booted with this save — verify the constructed party/items/flags in game"
                @click="playtestSave"
              >
                ▶ 用此存档试玩
              </button>
              <button
                class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent"
                @click="importJson()"
              >
                📂 Import
              </button>
              <button
                class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover"
                @click="exportJson()"
              >
                💾 Export
              </button>
            </div>
          </div>

          <!-- Save content -->
          <div class="flex-1 overflow-y-auto p-3 min-h-0">
            <!-- Info Tab -->
            <div v-show="saveSubTab === 'info'" class="space-y-4">
              <h3 class="text-accent text-[13px] font-bold">Player Info</h3>
              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Player Name</label>
                  <input
                    type="text"
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs font-mono"
                    :value="saveData.player.playerName"
                    maxlength="11"
                    @change="updatePlayer('playerName', ($event.target as HTMLInputElement).value)"
                  />
                </div>
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Rival Name</label>
                  <input
                    type="text"
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs font-mono"
                    :value="saveData.player.rivalName"
                    maxlength="11"
                    @change="updatePlayer('rivalName', ($event.target as HTMLInputElement).value)"
                  />
                </div>
              </div>

              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Map</label>
                  <select
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
                    :value="saveData.player.mapName"
                    @change="updatePlayer('mapName', ($event.target as HTMLSelectElement).value)"
                  >
                    <option v-for="m in MAP_NAMES" :key="m" :value="m">{{ m }}</option>
                  </select>
                </div>
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Facing</label>
                  <select
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
                    :value="saveData.player.facing"
                    @change="updatePlayer('facing', ($event.target as HTMLSelectElement).value as Facing)"
                  >
                    <option v-for="d in FACING_DIRECTIONS" :key="d" :value="d">{{ d }}</option>
                  </select>
                </div>
              </div>

              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Position X</label>
                  <input
                    type="number"
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
                    min="0"
                    max="255"
                    :value="saveData.player.positionX"
                    @change="updatePlayer('positionX', parseInt(($event.target as HTMLInputElement).value, 10))"
                  />
                </div>
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Position Y</label>
                  <input
                    type="number"
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
                    min="0"
                    max="255"
                    :value="saveData.player.positionY"
                    @change="updatePlayer('positionY', parseInt(($event.target as HTMLInputElement).value, 10))"
                  />
                </div>
              </div>

              <div class="grid grid-cols-3 gap-3">
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Play Time (Hours)</label>
                  <input
                    type="number"
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
                    min="0"
                    max="255"
                    :value="saveData.player.playTimeHours"
                    @change="updatePlayer('playTimeHours', parseInt(($event.target as HTMLInputElement).value, 10))"
                  />
                </div>
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Play Time (Minutes)</label>
                  <input
                    type="number"
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
                    min="0"
                    max="59"
                    :value="saveData.player.playTimeMinutes"
                    @change="updatePlayer('playTimeMinutes', parseInt(($event.target as HTMLInputElement).value, 10))"
                  />
                </div>
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Money (₽)</label>
                  <input
                    type="number"
                    class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
                    min="0"
                    max="999999"
                    :value="saveData.player.money"
                    @change="updatePlayer('money', parseInt(($event.target as HTMLInputElement).value, 10))"
                  />
                </div>
              </div>

              <!-- Badges -->
              <div>
                <h3 class="text-accent text-[13px] font-bold mb-2">Badges</h3>
                <div class="bg-bg-inset p-2 rounded">
                  <div class="flex flex-wrap gap-2">
                    <label
                      v-for="(badge, idx) in BADGE_NAMES"
                      :key="idx"
                      class="flex items-center gap-1.5 cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        :checked="saveData.badges[idx]"
                        class="accent-accent"
                        @change="toggleBadge(idx)"
                      />
                      <span
                        class="text-[11px]"
                        :class="saveData.badges[idx] ? 'text-accent' : 'text-text-muted'"
                      >
                        🏅 {{ badge }}
                      </span>
                    </label>
                  </div>
                </div>
              </div>

              <!-- Debug -->
              <div class="pt-3 border-t border-[rgba(255,255,255,0.06)]">
                <p class="text-[10px] text-text-muted mb-2">Debug: Print JSON to console for copy-paste</p>
                <button
                  class="px-3 py-1.5 rounded text-[11px] font-bold cursor-pointer bg-danger text-white border-none hover:opacity-90"
                  @click="exportJsonPrint()"
                >
                  🖨 Print to Console
                </button>
              </div>
            </div>

            <!-- Party Tab -->
            <div v-show="saveSubTab === 'party'" class="h-full">
              <PartyEditor
                :party="saveData.party"
                @update:party="(p: PokemonEntry[]) => (saveData.party = p)"
              />
            </div>

            <!-- Flags Tab -->
            <div v-show="saveSubTab === 'flags'" class="h-full">
              <FlagEditor
                :flags="saveData.flags"
                @update:flags="(f: Record<string, boolean>) => (saveData.flags = f)"
              />
            </div>

            <!-- Items Tab -->
            <div v-show="saveSubTab === 'items'" class="h-full">
              <SaveItemsEditor
                :items="saveData.items"
                @update:items="(i: ItemEntry[]) => (saveData.items = i)"
              />
            </div>
          </div>
        </div>

        <!-- Trainer mode -->
        <TrainerEditor v-else-if="activeActivity === 'trainer'" />

        <!-- Pokemon mode -->
        <PokemonEditor v-else-if="activeActivity === 'pokemon'" />

        <!-- Move mode -->
        <MoveEditor v-else-if="activeActivity === 'move'" />

        <!-- Layout mode -->
        <template v-else-if="activeActivity === 'layout'">
          <LayoutEditor />
        </template>

        <!-- Pixel mode -->
        <PixelEditor v-else-if="activeActivity === 'pixel'" />
      </div>

      <!-- AI Assistant dock (kept mounted with v-show so a running chat survives toggles) -->
      <AssistantPanel
        v-show="assistantOpen"
        :activity="activeActivity"
        @close="assistantOpen = false"
        @jump="handleAssistantJump"
      />
    </div>

    <!-- Status Bar -->
    <StatusBar
      :active-activity="activeActivity"
      :section-name="
        activeActivity === 'save'
          ? saveSectionLabels[saveSubTab]
          : activeActivity === 'trainer'
            ? (activeTrainerClass ?? undefined)
            : activeActivity === 'pokemon'
              ? (activePokemonSpecies ?? undefined)
              : activeActivity === 'move'
                ? (activeMoveId ?? undefined)
                : activeActivity === 'layout'
                  ? (activeLayoutName ?? undefined)
                  : activeActivity === 'pixel'
                    ? (activeAsset?.displayName ?? undefined)
                    : undefined
      "
    />

    <!-- Floating playtest overlay — mounted above every editing activity -->
    <PlaytestOverlay v-if="playtestOverlay.open.value" />
  </div>
</template>
