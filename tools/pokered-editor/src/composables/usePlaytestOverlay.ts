import { ref, type Ref } from 'vue'

// ───────────────────────────────────────────────────────────────────────────
// Shared floating-playtest state. The playtest lives in a draggable overlay
// (PlaytestOverlay.vue) mounted above every editor activity; this module is
// the single source of truth for open/mode/position and the "quick entry"
// requests that editor panels (Pokémon battle/dex, map warp, script preview)
// hand to the overlay.
//
// The `target` + `targetStamp` pair is how a launch request is delivered:
// `launch()` writes the target and bumps the stamp, and the overlay watches
// the stamp to (re)apply the latest request whenever it changes — even to the
// same value (e.g. "battle Pikachu" twice in a row).
// ───────────────────────────────────────────────────────────────────────────

export type PlaytestMode = 'play' | 'test'

export type PlaytestTarget =
  | { kind: 'map'; map: string; x?: number; y?: number }
  | { kind: 'battle'; species: string; level: number }
  | { kind: 'trainerBattle'; class: string; partyIndex: number }
  | { kind: 'moveTest'; move: string }
  | { kind: 'evolution'; from: string; to: string }
  | { kind: 'pokedex'; species: string }
  | { kind: 'playSave'; save: string }

export interface PlaytestOverlayState {
  open: Ref<boolean>
  mode: Ref<PlaytestMode>
  /** The latest quick-entry request; null when opened manually. */
  target: Ref<PlaytestTarget | null>
  /** Monotonic stamp bumped on every launch — watch this, not `target`. */
  targetStamp: Ref<number>
  position: Ref<{ x: number; y: number }>
  persistPosition: () => void
  openOverlay: () => void
  closeOverlay: () => void
  toggleOverlay: () => void
  launch: (target: PlaytestTarget) => void
}

const open = ref(false)
const mode = ref<PlaytestMode>('test')
const target = ref<PlaytestTarget | null>(null)
const targetStamp = ref(0)

const POS_KEY = 'pokered-playtest-overlay-pos'
const DEFAULT_POS = { x: 24, y: 24 }
const position = ref<{ x: number; y: number }>(loadPosition())

function loadPosition(): { x: number; y: number } {
  try {
    const raw = localStorage.getItem(POS_KEY)
    if (raw) {
      const p = JSON.parse(raw) as { x?: unknown; y?: unknown }
      if (typeof p.x === 'number' && typeof p.y === 'number') return { x: p.x, y: p.y }
    }
  } catch {
    /* malformed stored position → default */
  }
  return { ...DEFAULT_POS }
}

function persistPosition() {
  try {
    localStorage.setItem(POS_KEY, JSON.stringify(position.value))
  } catch {
    /* storage full/unavailable — position just won't survive a reload */
  }
}

/** Open the overlay keeping the current mode (manual open from the ActivityBar). */
function openOverlay() {
  open.value = true
}

function closeOverlay() {
  open.value = false
  // Drop the pending quick entry so reopening the overlay doesn't re-trigger
  // the last battle/dex/map request.
  target.value = null
}

function toggleOverlay() {
  if (open.value) closeOverlay()
  else openOverlay()
}

/** Open the overlay and queue a quick-entry target for it to apply. A
 *  `playSave` entry boots into Play mode (persistent save); everything else
 *  uses the scratch Test session. */
function launch(next: PlaytestTarget) {
  mode.value = next.kind === 'playSave' ? 'play' : 'test'
  target.value = next
  targetStamp.value++
  open.value = true
}
export function usePlaytestOverlay(): PlaytestOverlayState {
  return {
    open,
    mode,
    target,
    targetStamp,
    position,
    persistPosition,
    openOverlay,
    closeOverlay,
    toggleOverlay,
    launch,
  }
}
