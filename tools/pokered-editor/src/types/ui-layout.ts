/**
 * TypeScript types mirroring the schema_version 2 layout format defined by
 * crates/dotzuki-renderer/src/layout_engine/types.rs (and validated by
 * src/schemas/layout.schema.json).
 *
 * v2 is a flat element tree: a screen owns an ordered `elements` array, each
 * element carrying a `type` discriminator plus type-specific params. There are
 * no `variants`/`boxes`/`labels` — those were the v1 model.
 */

/**
 * Tile-space rectangle with numeric coordinates (1 tile = 8 pixels). Used by the
 * canvas overlay for drag/resize math. See {@link ElementRect} for the on-disk
 * shape, whose `tx`/`ty` may also be `{template}` strings.
 */
export interface TileRect {
  tx: number
  ty: number
  tw: number
  th: number
}

/**
 * A coordinate as stored in JSON: a literal tile index, or a `{template}` string
 * resolved against the data context at render time.
 */
export type Coord = number | string

/** Element rectangle as stored in JSON. `tx`/`ty` default to 0; `tw`/`th` are optional. */
export interface ElementRect {
  tx?: Coord
  ty?: Coord
  tw?: number
  th?: number
}

/** Optional per-screen theme overrides. */
export interface Theme {
  bg_color: string
  default_font?: string
}

/**
 * A single layout element. The `type` field ('border' | 'text' | 'tile' |
 * 'divider' | 'image' | 'list' | 'flex_list' | 'group' | `custom:<name>`)
 * selects which extra params apply, so type-specific fields are carried by the
 * index signature rather than enumerated here.
 */
export interface LayoutElement {
  id?: string
  type: string
  rect: ElementRect
  /** Static bool, or a `{template}` condition string (truthy → visible). */
  visible?: boolean | string
  /** Stacking order; higher draws on top (default 0). */
  z_index?: number
  /** Nested children (only for `group` elements). */
  children?: LayoutElement[]
  /** Catch-all for type-specific params (value, color, style, items, etc.). */
  [key: string]: unknown
}

/** Top-level screen layout JSON structure (schema_version 2). */
export interface ScreenLayout {
  schema_version: number
  screen: string
  theme?: Theme
  elements: LayoutElement[]
  /** Tolerates extra keys such as the migration-leftover `_variants`. */
  [key: string]: unknown
}
