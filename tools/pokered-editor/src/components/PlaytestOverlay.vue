<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useGameSession } from '../composables/useGameSession'
import { dataFetch } from '../composables/dataAdapter'
import { getPokeredRunner, EDITOR_SAVE_KEY, EDITOR_FLAGS_KEY } from '../composables/usePokeredRunner'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import GameCanvas from './GameCanvas.vue'

/**
 * Floating playtest overlay — the game, available above every editor activity.
 *
 * Owns one shared game session:
 *   ▶ Play — the full game from start to finish (persistent save in
 *            localStorage EDITOR_SAVE_KEY, New Game / Continue / Restart).
 *   🧪 Test — a fresh scratch session (never touches the Play save) with all
 *             editor edits applied, targeted via quick entries: warp to a map,
 *             start a wild battle vs a species, or open a Pokédex entry
 *             (`usePlaytestOverlay.launch`).
 *
 * The panel is draggable by its header; its position persists across reloads.
 */

const { mode, target, targetStamp, position, persistPosition, closeOverlay } =
  usePlaytestOverlay()

const gameCanvas = ref<InstanceType<typeof GameCanvas> | null>(null)
const canvasEl = computed(() => gameCanvas.value?.canvasEl ?? null)

// Scratch (Test) sessions never persist; a Play session writes EDITOR_SAVE_KEY
// while it lives (`playSession`), even if a quick entry temporarily detours it.
let playSession = false
const session = useGameSession(canvasEl, {
  persistSave: (runner) => {
    if (!playSession) return
    try {
      const save = runner.export_save()
      if (save) localStorage.setItem(EDITOR_SAVE_KEY, save)
      // Runtime-only script flags (hidden-object toggles etc.) live outside
      // SaveData — snapshot them alongside the save.
      localStorage.setItem(EDITOR_FLAGS_KEY, runner.export_flags())
    } catch {
      /* best-effort */
    }
  },
})

// ── Play mode ──────────────────────────────────────────────────────────────
const started = ref(false)
const hasSave = ref(!!localStorage.getItem(EDITOR_SAVE_KEY))
let lastSave: string | null = null

function startNewGame() {
  playSession = true
  lastSave = null
  started.value = true
  void session.boot(null)
}

/** Restore the script-flag snapshot saved alongside the playtest save. */
function restoreScriptFlags() {
  const flags = localStorage.getItem(EDITOR_FLAGS_KEY)
  if (!flags) return
  try {
    getPokeredRunner().then((runner) => runner.import_flags(flags))
  } catch {
    /* best-effort */
  }
}

function continueGame() {
  playSession = true
  lastSave = localStorage.getItem(EDITOR_SAVE_KEY)
  started.value = true
  void session.boot(lastSave).then(restoreScriptFlags)
}

function retry() {
  const play = mode.value === 'play'
  void session.boot(play ? lastSave : null).then(() => {
    // Retrying a Play session re-applies the flag snapshot; Test sessions
    // are scratch and keep whatever the runner already holds.
    if (play) restoreScriptFlags()
  })
}

/** Export the current runner state to the Play save. Callers only invoke
 *  this when leaving a Play session (playSession was true), so the shared
 *  runner is guaranteed to exist. */
async function persistPlaySave() {
  try {
    const runner = await getPokeredRunner()
    const save = runner.export_save()
    if (save) localStorage.setItem(EDITOR_SAVE_KEY, save)
    localStorage.setItem(EDITOR_FLAGS_KEY, runner.export_flags())
  } catch {
    /* best-effort */
  }
}

// ── Test mode: target map + coordinates ────────────────────────────────────
const maps = ref<string[]>([])
const selectedMap = ref('')
const x = ref('')
const y = ref('')
const warpBusy = ref(false)

async function loadMaps() {
  try {
    const res = await dataFetch('/api/maps')
    if (res.ok) {
      maps.value = ((await res.json()) as string[]).sort()
    } else {
      throw new Error('no map list')
    }
  } catch {
    // Static hosting (GitHub Pages): no /api backend — read the map list that
    // the deploy workflow generates into <base>/maps.json.
    try {
      const res = await fetch(`${import.meta.env.BASE_URL}maps.json`)
      if (res.ok) maps.value = ((await res.json()) as string[]).sort()
    } catch {
      /* no map list either — target stays empty, game still boots */
    }
  }
  if (selectedMap.value === '' && maps.value.length > 0) {
    selectedMap.value = maps.value[0]
  }
}

/** Parse an optional coordinate input; `undefined` when empty/invalid = auto. */
function coord(v: string): number | undefined {
  const trimmed = v.trim()
  if (trimmed === '') return undefined
  const n = Number(trimmed)
  return Number.isFinite(n) && n >= 0 ? Math.floor(n) : undefined
}

// ── Quick-entry target application ─────────────────────────────────────────
const statusMessage = ref('')

/** Serialized scratch boot: concurrent callers share one in-flight boot so a
 *  quick entry arriving mid-boot waits instead of creating a second runner. */
let bootPromise: Promise<void> | null = null
function scratchBoot(): Promise<void> {
  if (!bootPromise) {
    bootPromise = session.boot(null).finally(() => {
      bootPromise = null
    })
  }
  return bootPromise
}

async function applyTarget() {
  const t = target.value
  if (!t) return
  statusMessage.value = ''
  try {
    if (t.kind === 'map') {
      session.warpTo(t.map, t.x, t.y)
    } else if (t.kind === 'battle') {
      const runner = await getPokeredRunner()
      runner.start_wild_battle(t.species, t.level)
    } else if (t.kind === 'trainerBattle') {
      const runner = await getPokeredRunner()
      runner.start_trainer_battle(t.class, t.partyIndex)
    } else if (t.kind === 'moveTest') {
      const runner = await getPokeredRunner()
      runner.start_move_test(t.move)
    } else if (t.kind === 'evolution') {
      const runner = await getPokeredRunner()
      runner.play_evolution(t.from, t.to)
    } else if (t.kind === 'pokedex') {
      const runner = await getPokeredRunner()
      runner.open_pokedex(t.species)
    } else if (t.kind === 'playSave') {
      // Boot straight into the editor-constructed save — import_editor_save
      // reboots the game itself. Runs as a persistent Play session so the
      // progress is saved back to EDITOR_SAVE_KEY.
      const runner = await getPokeredRunner()
      playSession = true
      started.value = true
      if (!runner.import_editor_save(t.save)) {
        statusMessage.value = 'Failed to apply the editor save (unknown map or malformed snapshot)'
      }
    }
  } catch (e) {
    statusMessage.value = (e as Error).message
  }
}

/**
 * Apply a quick entry issued while the overlay is open. If a Play session is
 * running it is persisted first and the runner is rebooted into a fresh
 * scratch session — a quick test must never mutate the Play save/state.
 * `playSave` entries are the exception: they *are* a Play session and boot
 * their own save (import_editor_save reboots the game).
 */
async function applyQuickEntry() {
  if (target.value?.kind === 'playSave') {
    await applyTarget()
    return
  }
  if (playSession) {
    playSession = false
    await persistPlaySave()
    await scratchBoot()
  } else if (session.status.value !== 'running') {
    await scratchBoot()
  }
  await applyTarget()
}

/** Target banner label, e.g. "⚔ Pikachu Lv5" / "⚔ 训练师 Brock #1" / "🗺 PalletTown". */
const targetLabel = computed(() => {
  const t = target.value
  if (!t) return ''
  if (t.kind === 'battle') return `⚔ 战斗 ${t.species} Lv${t.level}`
  if (t.kind === 'trainerBattle') return `⚔ 训练师 ${t.class} #${t.partyIndex + 1}`
  if (t.kind === 'moveTest') return `⚔ 招式 ${t.move}`
  if (t.kind === 'evolution') return `✨ 进化 ${t.from} → ${t.to}`
  if (t.kind === 'pokedex') return `📖 图鉴 ${t.species}`
  if (t.kind === 'playSave') return '💾 编辑器存档'
  return `🗺 ${t.map}`
})

function clearTarget() {
  target.value = null
}

// ── Boot / request lifecycle ───────────────────────────────────────────────
/** True once the initial mount target has been applied — quick entries that
 *  arrive while the overlay is still booting are applied by onMounted (it
 *  always reads the latest target), so the watcher only re-applies after. */
let initialApplied = false

onMounted(async () => {
  await loadMaps()
  // Pre-fill the map picker from a pending map target.
  if (target.value?.kind === 'map') {
    if (maps.value.includes(target.value.map)) selectedMap.value = target.value.map
    if (target.value.x != null) x.value = String(target.value.x)
    if (target.value.y != null) y.value = String(target.value.y)
  }
  if (target.value?.kind === 'playSave') {
    // Boot straight into the editor-constructed save (no scratch boot).
    await applyTarget()
    initialApplied = true
  } else if (mode.value === 'test') {
    await scratchBoot()
    await applyTarget()
    initialApplied = true
  }
  // Play mode (without a save target) waits for New Game / Continue.
})

// Re-apply a quick entry issued while the overlay is already open (the stamp
// bumps even for identical requests, so "battle Pikachu" twice re-fires).
watch(targetStamp, () => {
  if (!initialApplied) return
  if (mode.value !== 'test' && target.value?.kind !== 'playSave') return
  void applyQuickEntry()
})

/** Fresh scratch boot (Test) — same as TestView's Apply & Warp, plus the
 *  current quick target re-applied (the runner reboots on reset, so the
 *  battle/dex/map request must be pushed again after the boot). */
async function applyAndWarp() {
  if (warpBusy.value) return
  warpBusy.value = true
  try {
    await scratchBoot()
    target.value = { kind: 'map', map: selectedMap.value, x: coord(x.value), y: coord(y.value) }
    await applyTarget()
  } finally {
    warpBusy.value = false
  }
}

// ── Mode switching ─────────────────────────────────────────────────────────
function setMode(next: 'play' | 'test') {
  if (mode.value === next) return
  if (next === 'play') {
    // Leaving the scratch test session; Play mode shows its start screen and
    // reboots with the chosen save on New Game / Continue.
    mode.value = 'play'
    started.value = false
  } else {
    // Leaving a Play session: persist it first, then boot a fresh scratch
    // session and re-apply the current quick target.
    playSession = false
    void persistPlaySave()
    mode.value = 'test'
    started.value = false
    void applyQuickEntry()
  }
}

// ── Debug helpers (P3): text speed / fast-forward / full heal / snapshots ──
const textDelay = ref(3)
watch(textDelay, (v) => {
  void (async () => {
    try {
      const runner = await getPokeredRunner()
      runner.set_text_delay_frames(v)
    } catch {
      /* best-effort */
    }
  })()
})

const SPEEDS = [1, 2, 4]
function cycleSpeed() {
  const i = SPEEDS.indexOf(session.speed.value)
  session.setSpeed(SPEEDS[(i + 1) % SPEEDS.length])
}

async function fullHeal() {
  try {
    const runner = await getPokeredRunner()
    runner.full_heal()
  } catch {
    /* best-effort */
  }
}

async function exportSnapshot() {
  try {
    const runner = await getPokeredRunner()
    const save = runner.export_save()
    if (!save) return
    const blob = new Blob([save], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `pokered-snapshot-${Date.now()}.json`
    a.click()
    URL.revokeObjectURL(url)
  } catch {
    /* best-effort */
  }
}

function importSnapshot() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = '.json'
  input.onchange = async () => {
    const file = input.files?.[0]
    if (!file) return
    try {
      const runner = await getPokeredRunner()
      if (runner.import_save(await file.text())) {
        statusMessage.value = '已载入存档快照'
        // The snapshot carries the save's event bits but not the runtime
        // extras (`__OBJ_HIDDEN_*` etc.) — re-apply the session's flag
        // snapshot so hidden objects stay consistent after the reboot.
        if (playSession) restoreScriptFlags()
      } else {
        statusMessage.value = '存档快照无效'
      }
    } catch {
      /* best-effort */
    }
  }
  input.click()
}

// ── Dragging ───────────────────────────────────────────────────────────────
const dragging = ref(false)
let dragOffset = { x: 0, y: 0 }

function onHeaderPointerDown(e: PointerEvent) {
  // Buttons in the header stop propagation, so a plain press on the header
  // starts the drag.
  dragging.value = true
  dragOffset = { x: e.clientX - position.value.x, y: e.clientY - position.value.y }
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
}

function onHeaderPointerMove(e: PointerEvent) {
  if (!dragging.value) return
  position.value = {
    x: Math.max(0, Math.min(window.innerWidth - 240, e.clientX - dragOffset.x)),
    y: Math.max(0, Math.min(window.innerHeight - 64, e.clientY - dragOffset.y)),
  }
}

function onHeaderPointerUp(e: PointerEvent) {
  if (!dragging.value) return
  dragging.value = false
  persistPosition()
  ;(e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId)
}

const targetBanner = computed(() => targetLabel.value)
</script>

<template>
  <div
    class="fixed flex flex-col rounded-lg shadow-2xl border border-[rgba(255,255,255,0.12)] bg-[#1b1f22] text-text select-none overflow-hidden"
    :style="{ left: `${position.x}px`, top: `${position.y}px`, zIndex: 90, width: '400px' }"
  >
    <!-- Header (drag handle) -->
    <div
      class="flex items-center gap-1.5 px-2 py-1.5 bg-[#262b2f] border-b border-[rgba(255,255,255,0.08)] cursor-grab active:cursor-grabbing shrink-0"
      @pointerdown="onHeaderPointerDown"
      @pointermove="onHeaderPointerMove"
      @pointerup="onHeaderPointerUp"
      @pointercancel="onHeaderPointerUp"
    >
      <span class="text-[12px] font-bold text-accent">🎮 试玩</span>

      <div class="flex items-center gap-0.5 ml-1">
        <button
          class="px-2 py-0.5 text-[11px] font-bold rounded cursor-pointer"
          :class="mode === 'play' ? 'bg-bg-inset text-accent border border-[rgba(255,255,255,0.12)]' : 'text-text-muted hover:text-text border border-transparent'"
          @pointerdown.stop
          @click="setMode('play')"
        >▶ Play</button>
        <button
          class="px-2 py-0.5 text-[11px] font-bold rounded cursor-pointer"
          :class="mode === 'test' ? 'bg-bg-inset text-accent border border-[rgba(255,255,255,0.12)]' : 'text-text-muted hover:text-text border border-transparent'"
          @pointerdown.stop
          @click="setMode('test')"
        >🧪 Test</button>
      </div>

      <span class="ml-auto text-[10px] font-mono text-text-muted">
        {{ session.hudMap.value }} · {{ session.hudPos.value }} · {{ session.hudScreen.value }}
      </span>

      <button
        class="px-1.5 text-[12px] rounded cursor-pointer text-text-muted hover:text-text"
        title="Mute / unmute"
        @pointerdown.stop
        @click="session.toggleMute()"
      >{{ session.muted.value ? '🔇' : '🔊' }}</button>
      <button
        class="px-1.5 text-[12px] rounded cursor-pointer text-text-muted hover:text-danger"
        title="Close the playtest"
        @pointerdown.stop
        @click="closeOverlay()"
      >✕</button>
    </div>

    <!-- Test-mode quick target banner -->
    <div v-if="mode === 'test' && targetBanner" class="flex items-center gap-2 px-2 py-1 bg-[#1d2b26] border-b border-accent/30 shrink-0">
      <span class="text-[11px] text-accent font-bold">{{ targetBanner }}</span>
      <span class="text-[10px] text-text-muted">— 已定位，重新点击编辑器中的入口可再次触发</span>
      <button
        class="ml-auto text-[10px] text-text-muted hover:text-danger cursor-pointer bg-transparent border-none"
        @click="clearTarget"
      >✕ 清除</button>
    </div>

    <div v-if="statusMessage" class="px-2 py-1 bg-danger/10 border-b border-danger/40 text-[10px] text-danger shrink-0">
      {{ statusMessage }}
    </div>

    <!-- Stage -->
    <div class="flex flex-col items-center justify-center gap-2 p-3 overflow-auto">
      <!-- Test controls -->
      <div v-if="mode === 'test'" class="flex items-center gap-2 w-full shrink-0">
        <select
          v-model="selectedMap"
          class="px-1 py-1 text-[11px] rounded border border-accent bg-bg text-text min-w-0 flex-1"
          :disabled="warpBusy || session.status.value === 'loading'"
        >
          <option v-for="m in maps" :key="m" :value="m">{{ m }}</option>
        </select>
        <label class="text-[10px] text-text-muted">X</label>
        <input
          v-model="x"
          type="number"
          min="0"
          placeholder="auto"
          class="w-14 px-1 py-1 text-[11px] rounded border border-accent bg-bg text-text"
          :disabled="warpBusy"
        />
        <label class="text-[10px] text-text-muted">Y</label>
        <input
          v-model="y"
          type="number"
          min="0"
          placeholder="auto"
          class="w-14 px-1 py-1 text-[11px] rounded border border-accent bg-bg text-text"
          :disabled="warpBusy"
        />
        <button
          class="px-2 py-1 text-[11px] rounded bg-accent text-bg border-none hover:bg-accent-hover disabled:opacity-40"
          :disabled="warpBusy || !selectedMap || session.status.value === 'loading'"
          title="Boot a fresh scratch session with all edits applied, then warp — blocked positions snap to the nearest walkable tile"
          @click="applyAndWarp"
        >{{ warpBusy ? '…' : '⟳ 传送' }}</button>
      </div>

      <!-- Play start screen -->
      <div v-if="mode === 'play' && !started" class="flex flex-col items-center gap-2 py-2">
        <div class="text-[13px] font-bold text-accent">PokéRed — Play</div>
        <div class="flex gap-2">
          <button
            class="px-3 py-1.5 text-[12px] rounded bg-accent text-bg font-bold hover:bg-accent-hover"
            @click="startNewGame"
          >▶ New Game</button>
          <button
            class="px-3 py-1.5 text-[12px] rounded bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="!hasSave"
            :title="hasSave ? 'Continue from the saved game' : 'No saved game yet'"
            @click="continueGame"
          >↩ Continue</button>
        </div>
        <p class="text-[10px] text-text-muted text-center">Play the full game — progress auto-saved</p>
      </div>

      <!-- Game canvas -->
      <GameCanvas
        v-show="session.status.value === 'running' && (mode === 'test' || started)"
        ref="gameCanvas"
        height="300px"
        @blur="session.clearKeys()"
      />

      <!-- Loading / error -->
      <div v-if="session.status.value === 'loading'" class="text-[11px] text-text-muted py-6">Loading game…</div>
      <div v-if="session.status.value === 'error'" class="flex flex-col items-center gap-2 py-2 max-w-full">
        <p class="text-[11px] text-danger whitespace-pre-wrap text-center">{{ session.errorMessage.value }}</p>
        <button
          class="px-2 py-1 text-[11px] rounded bg-accent text-bg hover:bg-accent-hover"
          @click="retry"
        >Retry</button>
      </div>

      <!-- Debug toolbar (running only) -->
      <div v-if="session.status.value === 'running'" class="flex items-center gap-1.5 w-full shrink-0">
        <select
          v-model="textDelay"
          class="px-1 py-0.5 text-[10px] rounded border border-accent bg-bg text-text"
          title="In-game text speed"
        >
          <option :value="1">文本: 快</option>
          <option :value="3">文本: 中</option>
          <option :value="5">文本: 慢</option>
        </select>
        <button
          class="px-1.5 py-0.5 text-[10px] rounded bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent cursor-pointer"
          title="Fast-forward (1x/2x/4x)"
          @click="cycleSpeed"
        >⚡ {{ session.speed.value }}x</button>
        <button
          class="px-1.5 py-0.5 text-[10px] rounded bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent cursor-pointer"
          title="Fully restore the player (party + mid-battle state)"
          @click="fullHeal"
        >❤ 恢复</button>
        <button
          class="px-1.5 py-0.5 text-[10px] rounded bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent cursor-pointer"
          title="Export the current game state as a save JSON"
          @click="exportSnapshot"
        >📤 快照</button>
        <button
          class="px-1.5 py-0.5 text-[10px] rounded bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent cursor-pointer"
          title="Import a save JSON into the game"
          @click="importSnapshot"
        >📥 读档</button>
        <span class="ml-auto text-[10px] text-text-muted">
          Arrows / WASD · Z / X = A / B · Enter = Start · Shift = Select
        </span>
      </div>
    </div>
  </div>
</template>
