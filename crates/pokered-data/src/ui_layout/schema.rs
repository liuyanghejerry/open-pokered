use std::borrow::Cow;
use std::collections::BTreeMap;

use super::types::{BracketSides, InkColor, TileRect};

// ── Schema types for JSON ↔ codegen ↔ runtime layout system ──
//
// All string fields use `Cow<'static, str>` and all slice fields use
// `Cow<'static, [T]>` so the same type works for both:
// 1. Compile-time generated statics (build.rs emits `Cow::Borrowed("…")`)
// 2. Runtime-parsed layouts (wasm preview parser emits `Cow::Owned(…)`)

// ── Box (unified container) ───────────────────────────────────────────

/// Layout mode for a box — controls how children are positioned inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum LayoutMode {
    /// Children positioned by explicit tile coordinates (labels[].tx/ty).
    #[default]
    Absolute,
    /// Children flow vertically with gap/padding/alignment.
    Flex,
    /// Scrollable list with fixed row_step.
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BoxDef {
    pub id: Cow<'static, str>,
    pub rect: TileRect,
    pub color: InkColor,
    #[cfg_attr(feature = "serde", serde(default = "default_true"))]
    pub border: bool,
    /// Stacking order. Higher values draw on top.
    #[cfg_attr(feature = "serde", serde(default))]
    pub z_index: u32,
    /// Layout mode: "absolute" (default), "flex", or "list".
    #[cfg_attr(feature = "serde", serde(default))]
    pub layout: LayoutMode,
    // ── absolute layout fields ──
    #[cfg_attr(feature = "serde", serde(default))]
    pub labels: Cow<'static, [LabelDef]>,
    // ── flex layout fields ──
    #[cfg_attr(feature = "serde", serde(default = "default_gap"))]
    pub gap: u32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub padding: EdgeInsets,
    #[cfg_attr(feature = "serde", serde(default))]
    pub justify: Justify,
    #[cfg_attr(feature = "serde", serde(default))]
    pub align: Align,
    #[cfg_attr(feature = "serde", serde(default))]
    pub items: Cow<'static, [FlexItem]>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub cursor: Option<CursorDef>,
    #[cfg_attr(feature = "serde", serde(default = "default_item_name_width"))]
    pub item_name_width: u32,
    #[cfg_attr(feature = "serde", serde(default = "default_qty_width"))]
    pub qty_width: u32,
    #[cfg_attr(feature = "serde", serde(default, rename = "width"))]
    pub width_mode: SizeMode,
    #[cfg_attr(feature = "serde", serde(default, rename = "height"))]
    pub height_mode: SizeMode,
    #[cfg_attr(feature = "serde", serde(default))]
    pub min_width: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_width: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub min_height: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_height: Option<u32>,
    // ── list layout fields ──
    #[cfg_attr(feature = "serde", serde(default))]
    pub item_start_ty: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub row_step: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_visible_rows: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub dynamic_height: Option<DynamicHeight>,
    // ── text layout fields (for text_box absolute-mode text rendering) ──
    /// X tile offset of the first text character, relative to the box interior origin.
    /// `None` means use the renderer's built-in default (1 for dialog, 0 for battle_text).
    #[cfg_attr(feature = "serde", serde(default))]
    pub text_start_tx: Option<u32>,
    /// Y tile offset of the first text line, relative to the box interior origin.
    /// `None` means use the renderer's built-in default (typically 1).
    #[cfg_attr(feature = "serde", serde(default))]
    pub text_start_ty: Option<u32>,
    /// Tile rows between consecutive text lines. `None` = renderer default (2).
    #[cfg_attr(feature = "serde", serde(default))]
    pub line_height: Option<u32>,
    /// Maximum number of text lines to display. `None` = renderer default (2).
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_lines: Option<u32>,
    /// Legacy: max characters per line for Latin text. No longer consulted —
    /// wrapping is measured in pixels from the box interior width (Fusion
    /// Pixel font: Latin 5px, CJK 10px advance). Kept for editor/JSON compat.
    #[cfg_attr(feature = "serde", serde(default))]
    pub line_width: Option<u32>,
    /// Legacy: max characters per line for CJK text. No longer consulted —
    /// wrapping is measured in pixels from the box interior width. Kept for
    /// editor/JSON compat.
    #[cfg_attr(feature = "serde", serde(default))]
    pub line_width_zh: Option<u32>,
}

fn default_true() -> bool { true }
fn default_gap() -> u32 { 1 }
fn default_item_name_width() -> u32 { 14 }
fn default_qty_width() -> u32 { 2 }

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct LabelDef {
    pub tx: u32,
    pub ty: u32,
    pub text: Cow<'static, str>,
    pub color: InkColor,
    /// Optional Game Boy tile IDs to render in place of `text`. When present
    /// (non-empty), each tile id is drawn at consecutive horizontal positions
    /// starting at (tx, ty); `text` is used as the per-tile fallback when the
    /// painter has no game font (e.g. wasm preview without font assets).
    /// Required for composite glyphs like the "Pk"+"Mn" pair (tiles 0xE1/0xE2)
    /// used in the battle menu's "PKMN" entry — the original game renders
    /// these as 2 special tiles, not 4 ASCII characters.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "<[u8]>::is_empty"))]
    pub tile_ids: Cow<'static, [u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DynamicHeight {
    pub extra_per_row: u32,
}

// ── Cursor ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CursorDef {
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub tx: u32,
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub base_ty: u32,
    #[cfg_attr(feature = "serde", serde(default = "default_cursor_row_step"))]
    pub row_step: u32,
    #[cfg_attr(feature = "serde", serde(default = "default_cursor_glyph"))]
    pub glyph: char,
    pub color: InkColor,
    /// Horizontal step for 2D grid cursors (e.g. battle_main's 2×2 FIGHT/PKMN/ITEM/RUN).
    /// When `col_step` is present, `tx = base_tx + col*col_step`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub col_step: Option<u32>,
    /// Base x-coordinate for 2D grid cursors. Falls back to `tx` when absent.
    #[cfg_attr(feature = "serde", serde(default))]
    pub base_tx: Option<u32>,
}

#[cfg_attr(not(feature = "serde"), allow(dead_code))]
fn default_cursor_glyph() -> char {
    '\u{25B6}' // ▶ — matches engine.rs Frame::cursor_at
}

#[cfg_attr(not(feature = "serde"), allow(dead_code))]
fn default_cursor_row_step() -> u32 {
    2
}

// ── Flex layout types (shared by Flex layout mode in BoxDef) ──────────

/// Sizing mode for a flex container dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SizeMode {
    /// Use `rect.tw` / `rect.th` directly.
    #[default]
    Fixed,
    /// Auto-size to content, bounded by min/max.
    Auto,
}

/// Main-axis (column direction: vertical) alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
}

/// Cross-axis (column direction: horizontal) alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// Padding inside a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EdgeInsets {
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub top: u32,
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub bottom: u32,
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub left: u32,
    #[cfg_attr(feature = "serde", serde(default = "one"))]
    pub right: u32,
}

fn one() -> u32 { 1 }

impl Default for EdgeInsets {
    fn default() -> Self {
        Self { top: 1, bottom: 1, left: 1, right: 1 }
    }
}

/// A static item inside a flex container.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FlexItem {
    pub text: Cow<'static, str>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub color: InkColor,
}

/// Flexbox-like container. Replaces the `BoxDef` + `ListParams` + `CursorDef`
/// combo with a layout model where items flow vertically and the container

// ── Primitives ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PrimitiveDef {
    pub id: Cow<'static, str>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub parent_id: Option<Cow<'static, str>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub color: InkColor,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub kind: PrimitiveKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", rename_all = "snake_case")
)]
pub enum PrimitiveKind {
    /// Horizontal HP bar (e.g. stats page1).
    HpBar {
        tx: u32,
        ty: u32,
        width_tiles: u32,
    },
    /// Decorative bracket box drawn with tilemap border glyphs.
    #[cfg_attr(feature = "serde", serde(alias = "bracket_box"))]
    BracketBox {
        rect: TileRect,
        #[cfg_attr(feature = "serde", serde(default))]
        sides: BracketSides,
        #[cfg_attr(feature = "serde", serde(default))]
        with_arrow: bool,
    },
    /// Vertical line of `length_tiles` tiles starting at (tx, ty).
    #[cfg_attr(feature = "serde", serde(alias = "vline"))]
    Vline {
        tx: u32,
        ty: u32,
        length_tiles: u32,
    },
    /// Horizontal line of `length_tiles` tiles starting at (tx, ty).
    #[cfg_attr(feature = "serde", serde(alias = "hline"))]
    Hline {
        tx: u32,
        ty: u32,
        length_tiles: u32,
    },
    /// Raw pixel rectangle (px/py = top-left pixel, pw/ph = pixel extent).
    #[cfg_attr(feature = "serde", serde(alias = "pixel_rect"))]
    PixelRect {
        px: u32,
        py: u32,
        pw: u32,
        ph: u32,
    },
}

// ── Dynamic Labels ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DynamicLabelDef {
    /// Parent box/region id, or "screen" for screen-absolute positioning.
    pub parent: Cow<'static, str>,
    pub tx: u32,
    pub ty: u32,
    /// If present: static label. If absent: code supplies text at draw time.
    #[cfg_attr(feature = "serde", serde(default))]
    pub text: Option<Cow<'static, str>>,
    pub color: InkColor,
}

// ── Container types (serde ↔ codegen) ─────────────────────────────────
//
// These types own their collections (Vec, BTreeMap) and are used for:
// 1. Serde deserialization of JSON files in tests / runtime parsing
// 2. As an intermediate representation the build.rs codegen can target
//
// The build.rs codegen emits *per-screen named structs* (e.g.
// YesNoDefaultLayout) that mirror the same field set but as statics with
// Cow::Borrowed. The per-scene structs are the compile-time path;
// VariantDef/ScreenLayout are the runtime/serde path.
// schema_version 2 layouts skip codegen and are loaded at runtime.

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ScreenLayout {
    pub schema_version: u32,
    #[cfg_attr(feature = "serde", serde(rename = "screen"))]
    pub screen_id: String,
    pub variants: BTreeMap<String, VariantDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct VariantDef {
    /// Unified container array. Each child is a box with its own layout mode
    /// (absolute/flex/list) and z_index for stacking order.
    #[cfg_attr(feature = "serde", serde(default))]
    pub children: Option<Vec<BoxDef>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub primitives: Option<Vec<PrimitiveDef>>,
    /// Single cursor (syntactic sugar for single-element cursors). Both
    /// `cursor` and `cursors` may be present simultaneously; consumers should
    /// check `cursors` first and fall back to `cursor`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub cursor: Option<CursorDef>,
    /// Multiple cursors for variants that need more than one independently
    /// positioned cursor (e.g. party entry: ▶ + ◆, options: per-row cursors).
    #[cfg_attr(feature = "serde", serde(default))]
    pub cursors: Option<Vec<CursorDef>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub dynamic_labels: Option<BTreeMap<String, DynamicLabelDef>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub enum_position_map: Option<BTreeMap<String, i32>>,
}

// ── Per-screen layout structs ──
//
// Generated by build.rs codegen from ui_layouts/*.json (schema_version 1 only).
// schema_version 2 layouts skip codegen and are loaded at runtime.

// Generated layout statics and registry:
include!(concat!(env!("OUT_DIR"), "/ui_layouts_gen.rs"));

// ── Mart layouts ──────────────────────────────────────────────────────
//
// Mart was migrated to schema_version 2 (element-based layout).
// The layout engine parses the JSON at runtime — no struct generation needed.

// ── DialogDefaultLayout conversion ────────────────────────────────────

impl DialogDefaultLayout {
    /// Build a `DialogDefaultLayout` from the `"default"` variant.
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let box_0 = v.children.as_ref()?.first()?.clone();
        let cursor = v.cursor?;
        Some(Self { box_0, cursor })
    }
}

// ── BattleTextDefaultLayout conversion ────────────────────────────────

impl BattleTextDefaultLayout {
    /// Build a `BattleTextDefaultLayout` from the `"default"` variant.
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let box_0 = v.children.as_ref()?.first()?.clone();
        let cursor = v.cursor?;
        Some(Self { box_0, cursor })
    }
}

// ── BattlePartyDefaultLayout conversion ───────────────────────────────

impl BattlePartyDefaultLayout {
    /// Build a `BattlePartyDefaultLayout` from the `"default"` variant.
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let box_0 = v.children.as_ref()?.first()?.clone();
        let cursor = v.cursor?;
        Some(Self { box_0, cursor })
    }
}

// ── BattleBagDefaultLayout conversion ─────────────────────────────────

impl BattleBagDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let list_child = v.children.as_ref()?.iter().find(|b| b.id == "list")?.clone();
        Some(Self { list: list_child })
    }
}

// ── MainDefaultLayout conversion ──────────────────────────────────────

impl MainDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let menu_child = v.children.as_ref()?.iter().find(|b| b.id == "menu")?.clone();
        Some(Self { menu: menu_child })
    }
}

// ── BattleMoveDefaultLayout conversion ────────────────────────────────

impl BattleMoveDefaultLayout {
    /// Build a `BattleMoveDefaultLayout` from the `"default"` variant.
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let base = children.iter().find(|b| b.id == "base")?.clone();
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let box_1 = children.iter().find(|b| b.id == "box_1")?.clone();
        let list_default = children.iter().find(|b| b.id == "list_default")?.clone();
        Some(Self { base, box_0, box_1, list_default })
    }
}

// ── StartDefaultLayout conversion ─────────────────────────────────────

impl StartDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let menu_child = v.children.as_ref()?.iter().find(|b| b.id == "menu")?.clone();
        Some(Self { menu: menu_child })
    }
}

// ── YesNoDefaultLayout conversion ──────────────────────────────────────

impl YesNoDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let box_0 = v.children.as_ref()?.first()?.clone();
        let cursor = v.cursor?;
        Some(Self { box_0, cursor })
    }
}

// ── OakSpeechTextPhaseLayout conversion ────────────────────────────────

impl OakSpeechTextPhaseLayout {
    pub fn from_text_phase_variant(v: &VariantDef) -> Option<Self> {
        let dialog_box = v.children.as_ref()?.first()?.clone();
        let cursor = v.cursor?;
        Some(Self { dialog_box, cursor })
    }
}

// ── OakSpeechNameChoiceLayout conversion ───────────────────────────────

impl OakSpeechNameChoiceLayout {
    pub fn from_name_choice_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let name_list = children.iter().find(|b| b.id == "name_list")?.clone();
        let prompt_box = children.iter().find(|b| b.id == "prompt_box")?.clone();
        let cursor = v.cursor?;
        Some(Self { name_list, prompt_box, cursor })
    }
}

// ── BagDefaultLayout conversion ──────────────────────────────────────

impl BagDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let list = children.iter().find(|b| b.id == "list")?.clone();
        Some(Self { box_0, list })
    }
}

// ── BattleMainDefaultLayout conversion ───────────────────────────────

impl BattleMainDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let base = children.iter().find(|b| b.id == "base")?.clone();
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let cursor = v.cursor?;
        Some(Self { base, box_0, cursor })
    }
}

// ── NamingDefaultLayout conversion ───────────────────────────────────

impl NamingDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let region_0 = children.iter().find(|b| b.id == "region_0")?.clone();
        Some(Self { box_0, region_0 })
    }
}

// ── OptionsDefaultLayout conversion ──────────────────────────────────

impl OptionsDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let box_1 = children.iter().find(|b| b.id == "box_1")?.clone();
        let box_2 = children.iter().find(|b| b.id == "box_2")?.clone();
        let region_0 = children.iter().find(|b| b.id == "region_0")?.clone();
        let cursors_vec = v.cursors.clone()?;
        let epm_map = v.enum_position_map.as_ref()?;
        let enum_position_map: Vec<_> = epm_map.iter().map(|(k, v)| (Cow::Owned(k.clone()), *v)).collect();
        Some(Self { box_0, box_1, box_2, region_0, cursors: Cow::Owned(cursors_vec), enum_position_map: Cow::Owned(enum_position_map) })
    }
}

// ── PartyDefaultLayout conversion ────────────────────────────────────

impl PartyDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let region_0 = children.iter().find(|b| b.id == "region_0")?.clone();
        let region_1 = children.iter().find(|b| b.id == "region_1")?.clone();
        Some(Self { region_0, region_1 })
    }
}

// ── SaveDefaultLayout conversion ─────────────────────────────────────

impl SaveDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let box_1 = children.iter().find(|b| b.id == "box_1")?.clone();
        let box_2 = children.iter().find(|b| b.id == "box_2")?.clone();
        Some(Self { box_0, box_1, box_2 })
    }
}

// ── SaveAskPromptLayout conversion ───────────────────────────────────

impl SaveAskPromptLayout {
    pub fn from_ask_prompt_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let box_1 = children.iter().find(|b| b.id == "box_1")?.clone();
        let cursor = v.cursor?;
        let epm_map = v.enum_position_map.as_ref()?;
        let enum_position_map: Vec<_> = epm_map.iter().map(|(k, v)| (Cow::Owned(k.clone()), *v)).collect();
        Some(Self { box_0, box_1, cursor, enum_position_map: Cow::Owned(enum_position_map) })
    }
}

// ── StatsPage1Layout conversion ──────────────────────────────────────

impl StatsPage1Layout {
    pub fn from_page1_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let region_0 = children.iter().find(|b| b.id == "region_0")?.clone();
        let primitives = v.primitives.as_ref()?;
        let prim_0 = primitives.iter().find(|p| p.id == "prim_0")?.clone();
        let bracket_0 = primitives.iter().find(|p| p.id == "bracket_0")?.clone();
        let bracket_1 = primitives.iter().find(|p| p.id == "bracket_1")?.clone();
        Some(Self { box_0, region_0, prim_0, bracket_0, bracket_1 })
    }
}

// ── StatsPage2Layout conversion ──────────────────────────────────────

impl StatsPage2Layout {
    pub fn from_page2_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let box_0 = children.iter().find(|b| b.id == "box_0")?.clone();
        let region_0 = children.iter().find(|b| b.id == "region_0")?.clone();
        let list_page2 = children.iter().find(|b| b.id == "list_page2")?.clone();
        Some(Self { box_0, region_0, list_page2 })
    }
}

// ── PokedexDefaultLayout conversion ──────────────────────────────────

impl PokedexDefaultLayout {
    pub fn from_default_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let frame = children.iter().find(|b| b.id == "frame")?.clone();
        let cursor = v.cursor?;
        Some(Self { frame, cursor })
    }
}

// ── MartMainMenuLayout conversion ────────────────────────────────────

impl MartMainMenuLayout {
    pub fn from_main_menu_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let menu_box = children.iter().find(|b| b.id == "menu_box")?.clone();
        let money_box = children.iter().find(|b| b.id == "money_box")?.clone();
        let cursor = v.cursor?;
        let dl_map = v.dynamic_labels.as_ref()?;
        let dynamic_labels: Vec<_> = dl_map.iter().map(|(k, v)| (Cow::Owned(k.clone()), v.clone())).collect();
        Some(Self { menu_box, money_box, cursor, dynamic_labels: Cow::Owned(dynamic_labels) })
    }
}

// ── MartResultDialogLayout conversion ────────────────────────────────

impl MartResultDialogLayout {
    pub fn from_result_dialog_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let result_box = children.iter().find(|b| b.id == "result_box")?.clone();
        let dl_map = v.dynamic_labels.as_ref()?;
        let dynamic_labels: Vec<_> = dl_map.iter().map(|(k, v)| (Cow::Owned(k.clone()), v.clone())).collect();
        Some(Self { result_box, dynamic_labels: Cow::Owned(dynamic_labels) })
    }
}

// ── MartConfirmLayout conversion ─────────────────────────────────────

impl MartConfirmLayout {
    pub fn from_confirm_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let message_region = children.iter().find(|b| b.id == "message_region")?.clone();
        let choice_box = children.iter().find(|b| b.id == "choice_box")?.clone();
        let cursor = v.cursor?;
        Some(Self { message_region, choice_box, cursor })
    }
}

// ── MartQuantityLayout conversion ────────────────────────────────────

impl MartQuantityLayout {
    pub fn from_quantity_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let detail_box = children.iter().find(|b| b.id == "detail_box")?.clone();
        let money_box = children.iter().find(|b| b.id == "money_box")?.clone();
        let dl_map = v.dynamic_labels.as_ref()?;
        let dynamic_labels: Vec<_> = dl_map.iter().map(|(k, v)| (Cow::Owned(k.clone()), v.clone())).collect();
        Some(Self { detail_box, money_box, dynamic_labels: Cow::Owned(dynamic_labels) })
    }
}

// ── MartBuyItemsWithMoneyLayout conversion ───────────────────────────

impl MartBuyItemsWithMoneyLayout {
    pub fn from_buy_items_with_money_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let list_box = children.iter().find(|b| b.id == "list_box")?.clone();
        let money_box = children.iter().find(|b| b.id == "money_box")?.clone();
        let cursor = v.cursor?;
        let dl_map = v.dynamic_labels.as_ref()?;
        let dynamic_labels: Vec<_> = dl_map.iter().map(|(k, v)| (Cow::Owned(k.clone()), v.clone())).collect();
        Some(Self { list_box, money_box, cursor, dynamic_labels: Cow::Owned(dynamic_labels) })
    }
}

// ── MartSellItemsWithMoneyLayout conversion ──────────────────────────

impl MartSellItemsWithMoneyLayout {
    pub fn from_sell_items_with_money_variant(v: &VariantDef) -> Option<Self> {
        let children = v.children.as_ref()?;
        let list_box = children.iter().find(|b| b.id == "list_box")?.clone();
        let money_box = children.iter().find(|b| b.id == "money_box")?.clone();
        let cursor = v.cursor?;
        let dl_map = v.dynamic_labels.as_ref()?;
        let dynamic_labels: Vec<_> = dl_map.iter().map(|(k, v)| (Cow::Owned(k.clone()), v.clone())).collect();
        Some(Self { list_box, money_box, cursor, dynamic_labels: Cow::Owned(dynamic_labels) })
    }
}
