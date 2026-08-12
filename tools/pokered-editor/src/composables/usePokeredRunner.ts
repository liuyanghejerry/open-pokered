// ───────────────────────────────────────────────────────────────────────────
// The pokered WYSIWYG game-preview bridge (crates/pokered-runner-web, built
// by `pnpm build:wasm-pokered` via wasm-pack --target web).
//
// Loading mirrors useWasmPreview: the import URL is built at runtime so Vite's
// import analyzer doesn't pre-resolve it — the request must reach the `/wasm`
// middleware (which falls back to crates/pokered-runner-web/pkg — see
// server/pokeredRoutes.ts). The module is a module-level singleton cache.
//
// WASM contract (see crates/pokered-runner-web/src/lib.rs):
//   new PokeredRunner(save_json?) — save_json is a SaveData JSON from
//     export_save() / localStorage (editor key "pokered-save-editor").
//   tick(input_bitmask) → Uint8Array (160×144×4 RGBA)
//   warp_to(map_name, x, y) / reload_scripts(scenes_json, configs_json)
//   start_wild_battle(species, level) / open_pokedex(species) — editor
//     quick entries that jump the game straight into a battle / dex entry
//   set_wild_data(map_name, json) → boolean / clear_wild_data()
//   set_map_data/set_map_blk/set_trainer/set_move/set_item/set_base_stats
//     → boolean (runtime data overrides) / clear_data_overrides()
//   import_save(json) → boolean / export_save() → string | undefined
//   reset(save_json?) / set_muted(bool) / is_muted()
//   current_map() / player_position() / screen_name()
//   set_text_delay_frames(frames)
//   bitmask: bit0=A, bit1=B, bit2=Select, bit3=Start,
//            bit4=Right, bit5=Left, bit6=Up, bit7=Down
// ───────────────────────────────────────────────────────────────────────────

import { listDeltas } from './useDataStore'
import { fetchWithProgress } from '../utils/fetchWithProgress'

export interface PokeredRunner {
  tick(inputBitmask: number): Uint8Array
  warp_to(mapName: string, x: number, y: number): void
  start_wild_battle(speciesName: string, level: number): void
  start_trainer_battle(className: string, partyIndex: number): void
  start_move_test(moveName: string): void
  play_evolution(fromSpecies: string, toSpecies: string): void
  open_pokedex(speciesName: string): void
  import_editor_save(json: string): boolean
  full_heal(): void
  reload_scripts(scenesJson: string, configsJson: string): void
  set_wild_data(mapName: string, json: string): boolean
  clear_wild_data(): void
  set_map_data(mapName: string, json: string): boolean
  set_map_blk(mapName: string, json: string): boolean
  set_trainer(className: string, json: string): boolean
  set_move(moveName: string, json: string): boolean
  set_item(itemName: string, json: string): boolean
  set_base_stats(speciesName: string, json: string): boolean
  clear_data_overrides(): void
  has_data_overrides(): boolean
  import_save(json: string): boolean
  export_save(): string | undefined
  /** JSON map of the overworld's live script flags (named bits + runtime extras). */
  export_flags(): string
  /** Restore flags from [`export_flags`](PokeredRunner.export_flags) JSON. */
  import_flags(json: string): boolean
  reset(saveJson?: string | null): void
  set_muted(muted: boolean): void
  is_muted(): boolean
  stop_audio(): void
  resume_audio(): void
  current_map(): string
  player_position(): string
  screen_name(): string
  set_text_delay_frames(frames: number): void
  width(): number
  height(): number
  free(): void
}

interface PokeredRunnerModule {
  /** wasm-bindgen init — pass the fetched .wasm bytes to control download progress. */
  default(input?: BufferSource | WebAssembly.Module): Promise<void>
  PokeredRunner: new (saveJson?: string | null) => PokeredRunner
}

let wasmModule: PokeredRunnerModule | null = null
let initPromise: Promise<PokeredRunnerModule> | null = null

/**
 * Load + init the pokered runner WASM module (cached singleton; failures are
 * retryable). The `.wasm` is fetched ourselves (mirroring pokered-web) so the
 * boot loading screen can report byte progress via `onProgress` — wasm-bindgen's
 * `default()` accepts the raw bytes.
 */
export function loadPokeredRunnerModule(
  onProgress?: (loaded: number, total: number) => void,
): Promise<PokeredRunnerModule> {
  if (wasmModule) return Promise.resolve(wasmModule)
  if (initPromise) return initPromise
  initPromise = (async () => {
    try {
      const base = import.meta.env.BASE_URL
      const wasmJsUrl = new URL(
        `${base}wasm/pokered_runner_web.js`,
        window.location.origin,
      ).href
      const mod = (await import(/* @vite-ignore */ wasmJsUrl)) as unknown as PokeredRunnerModule
      const wasmBytes = await fetchWithProgress(
        new URL(`${base}wasm/pokered_runner_web_bg.wasm`, window.location.origin).href,
        onProgress,
      )
      await mod.default(wasmBytes)
      wasmModule = mod
      return mod
    } catch (e) {
      // Allow a retry (e.g. after building the pkg) instead of caching the failure.
      initPromise = null
      throw new Error(
        `Failed to load the pokered runner WASM: ${(e as Error).message} — ` +
          `please run \`pnpm build:wasm-pokered\` first (crates/pokered-runner-web/pkg).`,
      )
    }
  })()
  return initPromise
}

// Input bitmask bits (contract above).
export const BIT_A = 1 << 0
export const BIT_B = 1 << 1
export const BIT_SELECT = 1 << 2
export const BIT_START = 1 << 3
export const BIT_RIGHT = 1 << 4
export const BIT_LEFT = 1 << 5
export const BIT_UP = 1 << 6
export const BIT_DOWN = 1 << 7

/**
 * Map a keyboard event to its input bit (0 = unmapped).
 *
 * Arrows/Enter/Space/Backspace match on `key` (layout-independent values);
 * letters match on `code` (physical key position) so IMEs, Caps Lock, Shift
 * and non-QWERTY layouts never move the mapping off WASD/ZX.
 */
export function keyToBit(key: string, code = ''): number {
  switch (key) {
    case 'ArrowUp': return BIT_UP
    case 'ArrowDown': return BIT_DOWN
    case 'ArrowLeft': return BIT_LEFT
    case 'ArrowRight': return BIT_RIGHT
    case 'Enter': return BIT_START
    case 'Backspace': return BIT_SELECT
  }
  switch (code) {
    case 'KeyW': return BIT_UP
    case 'KeyA': return BIT_LEFT
    case 'KeyS': return BIT_DOWN
    case 'KeyD': return BIT_RIGHT
    case 'KeyZ': return BIT_A
    case 'KeyX': return BIT_B
    case 'Space': return BIT_START
    case 'ShiftRight': return BIT_SELECT
  }
  return 0
}

/** localStorage key the editor uses for its playtest/preview session save. */
export const EDITOR_SAVE_KEY = 'pokered-save-editor'

/** localStorage key for the runtime script-flag snapshot stored alongside the
 *  playtest save (named event bits + runtime extras like `__OBJ_HIDDEN_*`). */
export const EDITOR_FLAGS_KEY = 'pokered-editor-script-flags'

// A single shared runner instance for the whole editor: the wasm game owns
// global (localStorage) state, so multiple instances would fight.
let runnerInstance: PokeredRunner | null = null

/**
 * Get (creating once) the shared PokeredRunner. `saveJson` only applies on
 * first creation; pass null to boot an empty save.
 */
export async function getPokeredRunner(saveJson?: string | null): Promise<PokeredRunner> {
  if (runnerInstance) return runnerInstance
  const mod = await loadPokeredRunnerModule()
  runnerInstance = new mod.PokeredRunner(saveJson ?? null)
  return runnerInstance
}

/** Get the shared runner, creating it headless if it doesn't exist yet. */
export function ensureRunner(saveJson?: string | null): Promise<PokeredRunner> {
  return getPokeredRunner(saveJson)
}

/** Drop the shared instance (e.g. after a project switch) — the wasm module stays cached. */
export function disposePokeredRunner() {
  runnerInstance?.free()
  runnerInstance = null
}

/** Whether the shared runner has been created in this session. */
export function pokeredRunnerActive(): boolean {
  return runnerInstance !== null
}

// ── Editor-side injection bookkeeping ─────────────────────────────────────
// Script injections live on the runner instance (the overworld's script
// loader) and are LOST when the runner resets — the wasm binary embeds the
// scenes it was compiled with, so we replay every injection after a reset.
// Wild-data overrides live in wasm global state and survive resets; the
// front-end still keeps its own list so a deleted override can be removed.

interface InjectedScene {
  /** Raw `.scene` DSL source — the wasm side compiles it (native AST engine). */
  source: string
  config?: string
}

const injectedScenes = new Map<string, InjectedScene>()
const wildOverrides = new Map<string, string>()

/** Inject a `.scene` (raw DSL source + optional script_config.json) for a map key. */
export async function injectSceneScript(
  mapKey: string,
  source: string,
  configJson?: string | null,
): Promise<void> {
  const entry: InjectedScene = { source, config: configJson ?? undefined }
  injectedScenes.set(mapKey, entry)
  const runner = await ensureRunner()
  pushScenesToRunner(runner)
}

/**
 * Set/remove a wild-encounter override for a map. `wildJson` is the editor's
 * `map.json` `wild` block; pass `null` to drop the override. The whole set is
 * re-pushed after each change (the wasm side only supports clear-all).
 */
export async function setWildOverride(
  mapName: string,
  wildJson: string | null,
): Promise<boolean> {
  if (wildJson === null) wildOverrides.delete(mapName)
  else wildOverrides.set(mapName, wildJson)
  const runner = await ensureRunner()
  runner.clear_wild_data()
  let ok = true
  for (const [name, json] of wildOverrides) {
    if (!runner.set_wild_data(name, json)) ok = false
  }
  return ok
}

/** Whether any wild override is currently tracked for a map. */
export function wildOverrideTracked(mapName: string): boolean {
  return wildOverrides.has(mapName)
}

/** Clear all tracked overrides (used when the playtest session is discarded). */
export function clearTrackedOverrides() {
  injectedScenes.clear()
  wildOverrides.clear()
}

// ── Data injection (map/trainer/move/item/pokemon runtime overrides) ──────
// These live in wasm global state (pokered-data::runtime_overrides) and
// survive runner resets, so no replay bookkeeping is needed here.

/** Inject a map's `map.json` (+ optional `map.blk` JSON array) into the game. */
export async function injectMapData(
  mapName: string,
  mapJson: string,
  blkJson?: string | null,
): Promise<void> {
  const runner = await ensureRunner()
  runner.set_map_data(mapName, mapJson)
  if (blkJson) runner.set_map_blk(mapName, blkJson)
}

/** Inject a trainer class's parties into the game. */
export async function injectTrainer(class_name: string, json: string): Promise<boolean> {
  const runner = await ensureRunner()
  return runner.set_trainer(class_name, json)
}

/** Inject a move's data into the game. */
export async function injectMove(move_name: string, json: string): Promise<boolean> {
  const runner = await ensureRunner()
  return runner.set_move(move_name, json)
}

/** Inject an item's data into the game. */
export async function injectItem(item_name: string, json: string): Promise<boolean> {
  const runner = await ensureRunner()
  return runner.set_item(item_name, json)
}

/** Inject a species' base stats into the game. */
export async function injectBaseStats(species_name: string, json: string): Promise<boolean> {
  const runner = await ensureRunner()
  return runner.set_base_stats(species_name, json)
}

/** Replay persisted deltas (IndexedDB) into a fresh runner — used at boot so
 *  edits survive a page reload in static mode. */
export async function replayDataDeltas(): Promise<void> {
  const deltas = await listDeltas()
  if (deltas.length === 0) return
  const runner = await ensureRunner()
  for (const d of deltas) {
    if (typeof d.content !== 'string') continue // binary /gfx deltas aren't replayable here
    if (d.path.startsWith('maps/')) {
      const rest = d.path.slice('maps/'.length)
      const slash = rest.indexOf('/')
      if (slash < 0) continue
      const name = rest.slice(0, slash)
      const file = rest.slice(slash + 1)
      if (file === 'map.json') runner.set_map_data(name, d.content)
      else if (file === 'map.blk') runner.set_map_blk(name, d.content)
    } else if (d.path.startsWith('trainers/')) {
      runner.set_trainer(d.path.slice('trainers/'.length, -'.json'.length), d.content)
    } else if (d.path.startsWith('moves/')) {
      runner.set_move(d.path.slice('moves/'.length, -'.json'.length), d.content)
    } else if (d.path.startsWith('items/')) {
      runner.set_item(d.path.slice('items/'.length, -'.json'.length), d.content)
    } else if (d.path.startsWith('pokemon/')) {
      runner.set_base_stats(d.path.slice('pokemon/'.length, -'.json'.length), d.content)
    }
  }
}

function pushScenesToRunner(runner: PokeredRunner) {
  const scenes: Record<string, string> = {}
  const configs: Record<string, string> = {}
  for (const [key, entry] of injectedScenes) {
    scenes[key] = entry.source
    if (entry.config) configs[key] = entry.config
  }
  runner.reload_scripts(JSON.stringify(scenes), JSON.stringify(configs))
}

/**
 * Reset the game (replay injected scenes after the reboot). Prefer this over
 * calling `runner.reset()` directly from the UI.
 */
export async function resetPokeredRunner(saveJson?: string | null): Promise<void> {
  const runner = await getPokeredRunner(saveJson)
  runner.reset(saveJson ?? null)
  try {
    // A scene that fails to compile shouldn't block the boot — the error is
    // surfaced at inject time and the next successful edit re-pushes the set.
    if (injectedScenes.size > 0) pushScenesToRunner(runner)
  } catch {
    /* best-effort: keep the boot going */
  }
  if (wildOverrides.size > 0) {
    for (const [name, json] of wildOverrides) {
      runner.set_wild_data(name, json)
    }
  }
}
