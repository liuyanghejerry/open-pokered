// ───────────────────────────────────────────────────────────────────────────
// types.ts — shared types for the sprite-sheet pipeline (port of types.go).
// ───────────────────────────────────────────────────────────────────────────
import type { Img } from './image'

/** One animation state (idle, walk, …). `facing` is an 8-direction key or ''. */
export interface StateSpec {
  name: string
  frames: number
  fps: number
  loop: boolean
  action: string
  facing: string
}

/** Result of extracting frames from a strip. */
export interface ExtractResult {
  frames: Img[]
  found: number
  expected: number
  warnings: string[]
}

/** Per-state finished frames going into atlas composition. */
export interface StateFrames {
  spec: StateSpec
  frames: Img[]
}

/** A frame rectangle in sheet coordinates. */
export interface FrameRect {
  x: number
  y: number
  w: number
  h: number
}

/** A 2D integer point (pivot / anchor). */
export interface Point {
  x: number
  y: number
}

/**
 * Per-state manifest entry. `rects` are sheet-absolute cell coords, `trims` are
 * cell-local content bboxes, `pivot` is the shared foot anchor (bottom-center).
 */
export interface AnimationEntry {
  row: number
  frames: number
  fps: number
  loop: boolean
  durationMs: number
  pivot: Point
  rects: FrameRect[]
  trims: FrameRect[]
}

/** Sheet image info. */
export interface SheetInfo {
  image: string
  width: number
  height: number
  cellWidth: number
  cellHeight: number
}

/** Runtime sprite-sheet metadata (schema v2). */
export interface Manifest {
  app: string
  generator: string
  schema: string
  version: number
  character: string
  sheet: SheetInfo
  animations: Record<string, AnimationEntry>
}
