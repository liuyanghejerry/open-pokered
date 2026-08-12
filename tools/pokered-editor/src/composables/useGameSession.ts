import { ref, type Ref, onBeforeUnmount } from 'vue'
import {
  getPokeredRunner,
  resetPokeredRunner,
  keyToBit,
  replayDataDeltas,
  pokeredRunnerActive,
  type PokeredRunner,
} from './usePokeredRunner'

// ───────────────────────────────────────────────────────────────────────────
// Shared game-session layer for the Play (full playthrough) and Test (targeted
// change verification) views. Owns the rAF loop, keyboard input and the
// single shared runner instance; the views own their boot policy and save
// strategy via the `persistSave` option (Play writes EDITOR_SAVE_KEY, Test
// never persists).
// ───────────────────────────────────────────────────────────────────────────

/** The runner's fixed framebuffer (160×144 GB screen). */
export const GAME_WIDTH = 160
export const GAME_HEIGHT = 144
/** Game Boy frame rate — the runner advances one frame per tick. */
const STEP_MS = 1000 / 59.7275
/** Save persistence cadence (plus on tab-hide and unmount). */
const SAVE_INTERVAL_MS = 2000
/** HUD status refresh cadence (cheap wasm getters, but no need for per-frame). */
const HUD_INTERVAL_MS = 500

export interface GameSession {
  status: Ref<'loading' | 'running' | 'error'>
  errorMessage: Ref<string>
  muted: Ref<boolean>
  hudMap: Ref<string>
  hudPos: Ref<string>
  hudScreen: Ref<string>
  /** Current emulation speed multiplier (1 = normal, 2/4 = fast-forward). */
  speed: Ref<number>
  /** Boot (or reboot) the shared runner with the given save; applies IndexedDB
   *  deltas on first creation and replays injected scenes on reset. */
  boot: (saveJson?: string | null) => Promise<void>
  /** Teleport to a map at optional coordinates. */
  warpTo: (map: string, x?: number, y?: number) => void
  /** Set the emulation speed multiplier (1–8). */
  setSpeed: (multiplier: number) => void
  toggleMute: () => void
  clearKeys: () => void
  /** Stop the loop, persist (per policy) and detach listeners. */
  dispose: () => void
}

export function useGameSession(
  canvasEl: Ref<HTMLCanvasElement | null>,
  opts: { persistSave?: (runner: PokeredRunner) => void } = {},
): GameSession {
  const persistSave = opts.persistSave ?? (() => {})

  const status = ref<'loading' | 'running' | 'error'>('loading')
  const errorMessage = ref('')
  const muted = ref(false)
  const speed = ref(1)
  const hudMap = ref('')
  const hudPos = ref('')
  const hudScreen = ref('')

  let runner: PokeredRunner | null = null
  let ctx: CanvasRenderingContext2D | null = null
  let rafId = 0
  let lastTime = 0
  let acc = 0
  let hudTimer: number | undefined
  let saveTimer: number | undefined
  const pressed = new Set<number>()

  function inputMask(): number {
    let mask = 0
    for (const bit of pressed) mask |= bit
    return mask
  }

  function clearKeys() {
    pressed.clear()
  }

  /** Don't steal keys while the user types in an input (or navigates a
   *  dropdown/select inside the overlay). */
  function isTypingTarget(): boolean {
    const el = document.activeElement as HTMLElement | null
    if (!el) return false
    return (
      el.tagName === 'INPUT' ||
      el.tagName === 'TEXTAREA' ||
      el.tagName === 'SELECT' ||
      el.isContentEditable
    )
  }

  function onKeyDown(e: KeyboardEvent) {
    // Modifier combos (Ctrl/Cmd/Alt) are editor shortcuts (save, undo, …) —
    // never game input. Without this, Cmd+S would walk down (KeyS) and Cmd+Z
    // would press A (KeyZ) inside the playtest.
    if (e.ctrlKey || e.metaKey || e.altKey) return
    const bit = keyToBit(e.key, e.code)
    if (!bit || status.value !== 'running' || isTypingTarget()) return
    e.preventDefault()
    // The playtest owns its keys exclusively while the game runs: swallow the
    // event in the capture phase so the editor's document/window shortcut
    // handlers behind the overlay (map navigation, tool switching, …) never
    // fire for the same press.
    e.stopImmediatePropagation()
    pressed.add(bit)
  }

  function onKeyUp(e: KeyboardEvent) {
    const bit = keyToBit(e.key, e.code)
    if (!bit) return
    if (!isTypingTarget()) e.preventDefault()
    pressed.delete(bit)
  }

  function frame(now: number) {
    rafId = requestAnimationFrame(frame)
    if (!runner || !ctx) return
    if (!lastTime) lastTime = now
    acc += Math.min(now - lastTime, 250) // cap long tab-switch gaps
    lastTime = now
    const mask = inputMask()
    // Fast-forward: step the game more than once per real frame.
    const step = STEP_MS / speed.value
    let bytes: Uint8Array | null = null
    while (acc >= step) {
      bytes = runner.tick(mask)
      acc -= step
    }
    if (bytes) {
      ctx.putImageData(new ImageData(new Uint8ClampedArray(bytes), GAME_WIDTH, GAME_HEIGHT), 0, 0)
    }
  }

  /** Clamp the speed multiplier to the supported 1–8 range. */
  function setSpeed(multiplier: number) {
    speed.value = Math.max(1, Math.min(8, Math.round(multiplier)))
  }

  function startLoop() {
    cancelAnimationFrame(rafId)
    lastTime = 0
    acc = 0
    rafId = requestAnimationFrame(frame)
  }

  function stopLoop() {
    cancelAnimationFrame(rafId)
    rafId = 0
  }

  function persist() {
    if (runner) persistSave(runner)
  }

  function refreshHud() {
    if (!runner) return
    try {
      hudMap.value = runner.current_map()
      hudPos.value = runner.player_position()
      hudScreen.value = runner.screen_name()
    } catch {
      /* ignore */
    }
  }

  async function boot(saveJson?: string | null): Promise<void> {
    status.value = 'loading'
    errorMessage.value = ''
    try {
      const firstBoot = !pokeredRunnerActive()
      if (firstBoot) {
        // Fresh runner: create with the save, then replay IndexedDB deltas so
        // data edits (static-mode) are applied to the new wasm instance.
        runner = await getPokeredRunner(saveJson ?? null)
        await replayDataDeltas()
      } else {
        // Reboot the shared instance: resets the game and re-pushes injected
        // scenes / wild overrides; data overrides live in wasm global state
        // and survive the reset.
        await resetPokeredRunner(saveJson ?? null)
        runner = await getPokeredRunner(saveJson ?? null)
      }
      muted.value = runner.is_muted()
      ctx = canvasEl.value?.getContext('2d') ?? null
      status.value = 'running'
      // If the runner was previously stopped with stop_audio() (leftover from
      // an earlier playtest exit), re-queue the current map's music — the
      // request is consumed by the first tick below.
      runner.resume_audio()
      startLoop()
      hudTimer = window.setInterval(refreshHud, HUD_INTERVAL_MS)
      saveTimer = window.setInterval(persist, SAVE_INTERVAL_MS)
    } catch (e) {
      status.value = 'error'
      errorMessage.value = (e as Error).message
    }
  }

  function warpTo(map: string, x = 0, y = 0) {
    if (!runner) return
    try {
      runner.warp_to(map, x, y)
      refreshHud()
    } catch (e) {
      errorMessage.value = (e as Error).message
    }
  }

  function toggleMute() {
    if (!runner) return
    muted.value = !muted.value
    runner.set_muted(muted.value)
  }

  function onVisibilityChange() {
    if (document.visibilityState === 'hidden') persist()
  }

  function dispose() {
    stopLoop()
    if (hudTimer) clearInterval(hudTimer)
    if (saveTimer) clearInterval(saveTimer)
    persist()
    // The shared runner survives the view; stop the game audio so the frozen
    // APU state doesn't keep droning after the playtest closes (the runner
    // would otherwise keep rendering the last note through Web Audio).
    runner?.stop_audio()
    ctx = null
    window.removeEventListener('keydown', onKeyDown, true)
    window.removeEventListener('keyup', onKeyUp, true)
    document.removeEventListener('visibilitychange', onVisibilityChange)
  }

  // Attach keyboard + visibility listeners for this session's lifetime.
  // Keyboard uses the window capture phase so the game sees its keys before —
  // and, once consumed, exclusively of — the editor's shortcut handlers.
  window.addEventListener('keydown', onKeyDown, true)
  window.addEventListener('keyup', onKeyUp, true)
  document.addEventListener('visibilitychange', onVisibilityChange)
  onBeforeUnmount(dispose)

  return {
    status,
    errorMessage,
    muted,
    speed,
    hudMap,
    hudPos,
    hudScreen,
    boot,
    warpTo,
    setSpeed,
    toggleMute,
    clearKeys,
    dispose,
  }
}
