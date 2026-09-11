use dotzuki_engine::render::Rgba;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use dotzuki_engine::hash::HashMap;
use thiserror::Error;

// ============================================================================
// Placeholder type aliases (will be filled in later)
// ============================================================================

pub type FontRegistry = HashMap<String, ()>;

pub type TilesetRegistry = HashMap<String, ()>;

// ============================================================================
// Image registry — full-colour RGBA images the `image` element blits
// ============================================================================

/// A decoded full-colour image: straight RGBA, row-major (`width * height`).
/// The `image` layout element looks one up by its resolved `source` key in the
/// [`ImageRegistry`] carried on [`RenderContext`].
#[derive(Clone, Debug, Default)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Rgba>,
}

impl ImageData {
    pub fn new(width: u32, height: u32, pixels: Vec<Rgba>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.pixels.is_empty()
    }

    /// Pixel at `(x, y)`; transparent if out of range.
    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> Rgba {
        if x >= self.width || y >= self.height {
            return Rgba::TRANSPARENT;
        }
        self.pixels
            .get((y * self.width + x) as usize)
            .copied()
            .unwrap_or(Rgba::TRANSPARENT)
    }

    /// Decode an RGBA image from a PNG (etc.) file. Behind the `image-assets`
    /// feature, mirroring [`crate::walk_sprite::WalkSprite::load`].
    #[cfg(feature = "image-assets")]
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("decode {}: {e}", path.display()))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = img
            .pixels()
            .map(|p| Rgba::new(p.0[0], p.0[1], p.0[2], p.0[3]))
            .collect();
        Ok(Self::new(w, h, pixels))
    }
}

/// Named registry of full-colour images, looked up by an `image` element's
/// resolved `source` key. Empty by default (see [`empty_image_registry`]).
pub type ImageRegistry = HashMap<String, ImageData>;

/// A shared empty image registry — the default [`RenderContext::images`] for
/// callers that render no images (so [`RenderContext::new`] stays 4-arg).
pub fn empty_image_registry() -> &'static ImageRegistry {
    // Const-constructed empty map: no std::sync::OnceLock (unavailable on
    // no_std) and no atomics (unavailable on thumbv4t) required.
    static EMPTY: ImageRegistry =
        dotzuki_engine::hash::HashMap::with_hasher(dotzuki_engine::hash::FxBuildHasher);
    &EMPTY
}

// ============================================================================
// Top-level layout types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ScreenLayout {
    pub schema_version: u8,

    pub screen: String,

    #[serde(default)]
    pub theme: Theme,

    pub elements: Vec<LayoutElement>,
}

// ============================================================================
// Theme
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Theme {
    pub bg_color: String,

    #[serde(default = "default_font_name")]
    pub default_font: String,

    /// How text is positioned. `Tile` (default) keeps the legacy Game Boy 8×8
    /// per-glyph grid; `Proportional` renders at pixel precision with per-glyph
    /// advance (for high-resolution, CJK-correct screens). The proportional path
    /// also requires `Painter::supports_proportional()` — both must hold, so any
    /// layout without an explicit theme (e.g. all pokered screens) stays on Tile.
    #[serde(default)]
    pub text_mode: TextMode,

    /// Default ink colour for text/list/cursor when no element-level `color` is
    /// set. `None` → the legacy `INK_BLACK`.
    #[serde(default)]
    pub ink: Option<String>,

    /// Panel interior fill (for game-themed boxes drawn via `pixel_rect`).
    #[serde(default)]
    pub panel_bg: Option<String>,

    /// Panel border colour.
    #[serde(default)]
    pub panel_border: Option<String>,

    /// Cursor (▶) colour; `None` → the resolved `ink`.
    #[serde(default)]
    pub cursor_color: Option<String>,
}

/// Text positioning strategy for a screen (see [`Theme::text_mode`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextMode {
    /// Legacy 8×8 tile grid — one glyph per tile cell (Game Boy fonts).
    #[default]
    Tile,
    /// Pixel-precise proportional advance (high-resolution / CJK).
    Proportional,
}

fn default_font_name() -> String {
    "default".to_string()
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg_color: "#FFFFFF".to_string(),
            default_font: "default".to_string(),
            text_mode: TextMode::Tile,
            ink: None,
            panel_bg: None,
            panel_border: None,
            cursor_color: None,
        }
    }
}

impl Theme {
    /// Resolve the ink colour: `theme.ink` if set, else the legacy `INK_BLACK`.
    pub fn ink_color(&self) -> dotzuki_engine::render::Rgba {
        self.ink
            .as_deref()
            .map(crate::layout_engine::elements::text::parse_color)
            .unwrap_or(dotzuki_engine::render::Rgba::INK_BLACK)
    }

    /// Resolve the cursor colour: `theme.cursor_color`, else `ink`, else `INK_BLACK`.
    pub fn cursor_ink(&self) -> dotzuki_engine::render::Rgba {
        self.cursor_color
            .as_deref()
            .map(crate::layout_engine::elements::text::parse_color)
            .unwrap_or_else(|| self.ink_color())
    }

    /// Whether to use the proportional path given the painter's capability.
    pub fn proportional(&self, painter_supports: bool) -> bool {
        self.text_mode == TextMode::Proportional && painter_supports
    }
}

// ============================================================================
// Layout element
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct LayoutElement {
    #[serde(default)]
    pub id: String,

    #[serde(rename = "type")]
    pub element_type: String,

    pub rect: ElementRect,

    #[serde(default)]
    pub visible: Visibility,

    #[serde(default)]
    pub z_index: i32,

    #[serde(flatten)]
    pub params: ElementParams,
}

// ============================================================================
// Visibility — static flag or template condition
// ============================================================================

/// Whether an element should render. Either a static `bool` or a
/// `{template}` condition (e.g. `"{show_entry}"`) evaluated against the
/// [`DataContext`] at render time (truthy → visible).
#[derive(Debug, Clone)]
pub enum Visibility {
    /// Static visibility flag.
    Static(bool),
    /// Template condition string; resolved variable's truthiness decides.
    Template(String),
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Static(true)
    }
}

impl Visibility {
    /// Evaluate whether the element should be rendered under `ctx`.
    pub fn eval(&self, ctx: &DataContext) -> bool {
        match self {
            Visibility::Static(b) => *b,
            Visibility::Template(t) => {
                let trimmed = t.trim();
                let key = trimmed
                    .strip_prefix('{')
                    .and_then(|r| r.strip_suffix('}'))
                    .unwrap_or(trimmed)
                    .trim();
                ctx.is_truthy(key)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Visibility {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct VisibilityVisitor;

        impl<'de> serde::de::Visitor<'de> for VisibilityVisitor {
            type Value = Visibility;

            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("a bool or a {template} condition string")
            }

            fn visit_bool<E: de::Error>(self, b: bool) -> Result<Visibility, E> {
                Ok(Visibility::Static(b))
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Visibility, E> {
                match s.trim().to_lowercase().as_str() {
                    "true" => Ok(Visibility::Static(true)),
                    "false" => Ok(Visibility::Static(false)),
                    // `{var}` template or a bare variable name → condition
                    _ => Ok(Visibility::Template(s.to_string())),
                }
            }
        }

        d.deserialize_any(VisibilityVisitor)
    }
}

// ============================================================================
// Coord — template-variable-aware coordinate
// ============================================================================

/// A coordinate value that can be either a literal `u32` or a template variable
/// string (e.g. `"{cursor_0_tx}"`) that must be resolved at render time via
/// [`DataContext`].
#[derive(Debug, Clone)]
pub enum Coord {
    /// A literal numeric coordinate.
    Literal(u32),
    /// A template variable string (including braces), e.g. `"{cursor_y}"`.
    Template(String),
}

impl Coord {
    /// Resolve the coordinate to a `u32`.
    ///
    /// Literal values are returned directly. Template strings are resolved via
    /// [`DataContext::resolve`] and parsed as `u32`. Falls back to `0` if the
    /// variable is missing or unparseable.
    pub fn resolve(&self, ctx: &DataContext) -> u32 {
        match self {
            Coord::Literal(v) => *v,
            Coord::Template(tpl) => {
                let resolved = ctx.resolve(tpl);
                resolved.trim().parse::<u32>().unwrap_or(0)
            }
        }
    }

    /// Returns the literal value if this is a [`Coord::Literal`], or `None`
    /// if it's a template that needs context to resolve.
    pub fn as_literal(&self) -> Option<u32> {
        match self {
            Coord::Literal(v) => Some(*v),
            Coord::Template(_) => None,
        }
    }
}

impl From<u32> for Coord {
    fn from(v: u32) -> Self {
        Coord::Literal(v)
    }
}

impl Default for Coord {
    fn default() -> Self {
        Coord::Literal(0)
    }
}

impl<'de> Deserialize<'de> for Coord {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct CoordVisitor;

        impl<'de> serde::de::Visitor<'de> for CoordVisitor {
            type Value = Coord;

            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("a u32 number or a {template} string")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Coord, E> {
                Ok(Coord::Literal(v as u32))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Coord, E> {
                if v < 0 {
                    return Err(de::Error::custom(format!(
                        "negative coordinate {} not allowed",
                        v
                    )));
                }
                Ok(Coord::Literal(v as u32))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Coord, E> {
                if v.contains('{') {
                    // Template variable — store as-is for runtime resolution
                    Ok(Coord::Template(v.to_string()))
                } else {
                    // Plain numeric string
                    v.parse::<u32>()
                        .map(Coord::Literal)
                        .map_err(de::Error::custom)
                }
            }
        }

        d.deserialize_any(CoordVisitor)
    }
}

// ============================================================================
// ElementRect
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ElementRect {
    #[serde(default)]
    pub tx: Coord,

    #[serde(default)]
    pub ty: Coord,

    #[serde(default)]
    pub tw: Option<u32>,

    #[serde(default)]
    pub th: Option<u32>,
}

// ============================================================================
// Element parameters (untagged union)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ElementParams {
    Border(BorderParams),

    Text(TextParams),

    Tile(TileParams),

    Divider(DividerParams),

    Image(ImageParams),

    List(ListParams),

    FlexList(FlexListParams),

    Group(GroupParams),

    Cursor(CursorParams),

    Bracket(BracketParams),

    PixelRect(PixelRectParams),

    Custom(serde_json::Value),
}

// ============================================================================
// Primitive params (bracket / pixel_rect)
// ============================================================================

/// A partial box border (left/right/top/bottom edges) drawn at the element's
/// `rect`, 1px lines — the original "bracket" decoration used on stats pages.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct BracketParams {
    pub color: Option<String>,
    #[serde(default)]
    pub left: bool,
    #[serde(default)]
    pub right: bool,
    #[serde(default)]
    pub top: bool,
    #[serde(default)]
    pub bottom: bool,
    #[serde(default)]
    pub with_arrow: bool,
}

/// A raw filled rectangle in PIXEL coordinates (not tiles).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PixelRectParams {
    pub color: Option<String>,
    pub px: u32,
    pub py: u32,
    pub pw: u32,
    pub ph: u32,
}

// ============================================================================
// CursorParams
// ============================================================================

/// A selection cursor (e.g. the ▶ arrow). Its base position is the element's
/// `rect.tx`/`rect.ty`; the final position is
/// `base + col*col_step` (x) and `base + row*row_step` (y), so a single cursor
/// expresses a 1-D list, a 2-D grid (battle FIGHT/PKMN/ITEM/RUN), or an
/// enum-offset selector. `col`/`row` are data bindings; place several cursor
/// elements for multi-cursor screens (options/party).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct CursorParams {
    /// Cursor glyph; defaults to ▶ when absent/empty.
    #[serde(default)]
    pub glyph: Option<String>,

    pub color: Option<String>,

    /// Column index (literal or `{template}`); multiplied by `col_step`.
    #[serde(default)]
    pub col: Coord,

    /// Row index (literal or `{template}`); multiplied by `row_step`.
    #[serde(default)]
    pub row: Coord,

    /// Tiles to advance per column.
    #[serde(default)]
    pub col_step: u32,

    /// Tiles to advance per row.
    #[serde(default)]
    pub row_step: u32,
}

impl CursorParams {
    /// The glyph char, defaulting to ▶ (U+25B6).
    pub fn glyph_char(&self) -> char {
        self.glyph
            .as_deref()
            .and_then(|g| g.chars().next())
            .unwrap_or('\u{25B6}')
    }
}

// ============================================================================
// BorderParams
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct BorderParams {
    #[serde(default, deserialize_with = "deserialize_optional_border_style")]
    pub style: Option<BorderStyle>,

    pub tileset: Option<String>,

    /// Elements nested inside the panel (e.g. a text box's contents). Their
    /// rects are absolute screen coordinates — unlike `group`, a border does
    /// not apply layout/offset to its children, it just draws the box and then
    /// renders each child in place.
    #[serde(default)]
    pub children: Vec<LayoutElement>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub enum BorderStyle {
    #[default]
    Single,

    Double,
}

fn deserialize_optional_border_style<'de, D>(d: D) -> Result<Option<BorderStyle>, D::Error>
where
    D: Deserializer<'de>,
{
    // Accept: a style string ("default"/"single"/"double"), an object form
    // (custom per-corner tile ids — treated as Single; the framebuffer painter
    // draws the default box), or null.
    let value = serde_json::Value::deserialize(d)?;
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) => match s.to_lowercase().as_str() {
            "default" | "single" => Ok(Some(BorderStyle::Single)),
            "double" => Ok(Some(BorderStyle::Double)),
            other => Err(de::Error::custom(format!("unknown border style: {other}"))),
        },
        serde_json::Value::Object(_) => Ok(Some(BorderStyle::Single)),
        other => Err(de::Error::custom(format!(
            "border style must be a string or object, got {other}"
        ))),
    }
}

// ============================================================================
// TextParams
// ============================================================================

/// A text value that is either a single string (same in every language) or a
/// per-locale map produced by the GUI DSL's `@t("en", "中文")` literal —
/// `{"en": "TEXT SPEED", "zh": "文字速度"}`. The renderer resolves it against
/// the active language via [`LocalizedValue::get`].
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LocalizedValue {
    Plain(String),
    Localized(alloc::collections::BTreeMap<String, String>),
}

impl LocalizedValue {
    /// Text for `lang`, falling back to `en`, then any present locale, else "".
    pub fn get(&self, lang: &str) -> &str {
        match self {
            LocalizedValue::Plain(s) => s,
            LocalizedValue::Localized(map) => map
                .get(lang)
                .or_else(|| map.get("en"))
                .or_else(|| map.values().next())
                .map(|s| s.as_str())
                .unwrap_or(""),
        }
    }
}

impl Default for LocalizedValue {
    fn default() -> Self {
        LocalizedValue::Plain(String::new())
    }
}

impl From<&str> for LocalizedValue {
    fn from(s: &str) -> Self {
        LocalizedValue::Plain(s.to_string())
    }
}

impl From<String> for LocalizedValue {
    fn from(s: String) -> Self {
        LocalizedValue::Plain(s)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct TextParams {
    pub value: LocalizedValue,

    pub format: Option<String>,

    pub color: Option<String>,

    pub align: Option<TextAlign>,

    pub font: Option<String>,

    pub wrap: Option<String>, // "word" = word wrap, null/none = no wrap

    pub line_spacing: Option<u32>,

    /// Integer text-scale factor for the proportional path — every glyph pixel
    /// becomes a `scale × scale` block. `None`/`1` = normal size. Powers big
    /// title/heading text (e.g. a title-screen logo). Ignored on the tile path.
    pub scale: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum TextAlign {
    Left,

    Center,

    Right,
}

impl<'de> Deserialize<'de> for TextAlign {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.to_lowercase().as_str() {
            "left" => Ok(TextAlign::Left),
            "center" => Ok(TextAlign::Center),
            "right" => Ok(TextAlign::Right),
            _ => Err(de::Error::custom(format!("unknown TextAlign: {s}"))),
        }
    }
}

// ============================================================================
// TileParams
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct TileParams {
    pub tile_id: serde_json::Value,

    #[serde(default)]
    pub flip_x: bool,

    #[serde(default)]
    pub flip_y: bool,

    pub palette: Option<String>,

    pub repeat: Option<u32>,
}

// ============================================================================
// DividerParams
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DividerParams {
    pub tiles: Vec<u16>,

    pub repeat: u32,

    #[serde(default)]
    pub orientation: Direction,
}

// ============================================================================
// ImageParams
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ImageParams {
    /// The image key/template resolved via [`DataContext::resolve`] and looked up
    /// in [`RenderContext::images`]. The `.gui` DSL emits this as `src` (shared
    /// with the scene-`ui` schema), so accept both spellings.
    #[serde(alias = "src")]
    pub source: String,

    #[serde(default)]
    pub flip_x: bool,

    #[serde(default)]
    pub flip_y: bool,

    pub palette: Option<String>,
}

// ============================================================================
// ListParams
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ListParams {
    pub items: String,

    pub item_template: ItemTemplate,

    #[serde(default)]
    pub cursor: ListCursor,

    /// Index of the highlighted item (defaults to the first item). The
    /// cursor glyph is drawn next to this row.
    #[serde(default)]
    pub selected: Option<Coord>,

    pub max_visible: Option<usize>,

    pub footer: Option<String>,
}

// ============================================================================
// ListCursor — cursor style for list / flex_list elements
// ============================================================================

/// Cursor configuration for a scrollable list. Authored either as a bare
/// tile id number (`"cursor": 223`) or as an object
/// (`"cursor": {"tile": 223, "position": "left"}`).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ListCursor {
    /// Tile id used to draw the cursor glyph (default ▶).
    pub tile: Option<u32>,
    /// Position hint (e.g. `"left"`) — currently informational.
    pub position: Option<String>,
}

impl<'de> Deserialize<'de> for ListCursor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct CursorObj {
            #[serde(default)]
            tile: Option<u32>,
            #[serde(default)]
            position: Option<String>,
        }

        struct CursorVisitor;

        impl<'de> serde::de::Visitor<'de> for CursorVisitor {
            type Value = ListCursor;

            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("a tile id number or a {tile, position} object")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<ListCursor, E> {
                Ok(ListCursor {
                    tile: Some(v as u32),
                    position: None,
                })
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<ListCursor, E> {
                Ok(ListCursor {
                    tile: Some(v.max(0) as u32),
                    position: None,
                })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<ListCursor, A::Error> {
                let obj = CursorObj::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(ListCursor {
                    tile: obj.tile,
                    position: obj.position,
                })
            }

            fn visit_unit<E: de::Error>(self) -> Result<ListCursor, E> {
                Ok(ListCursor::default())
            }
        }

        d.deserialize_any(CursorVisitor)
    }
}

// ============================================================================
// FlexListParams
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct FlexListParams {
    pub items: String,

    pub item_layout: Vec<ColumnDef>,

    pub padding: EdgeInsets,

    pub gap: u32,

    #[serde(default)]
    pub cursor: ListCursor,

    /// Index of the highlighted row (defaults to the first row).
    #[serde(default)]
    pub selected: Option<Coord>,
}

// ============================================================================
// GroupParams
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct GroupParams {
    pub layout: LayoutConfig,

    #[serde(default)]
    pub clip: bool,

    pub children: Vec<LayoutElement>,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct LayoutConfig {
    #[serde(default)]
    pub direction: Option<Direction>,

    #[serde(default)]
    pub gap: u32,

    #[serde(default)]
    pub padding: EdgeInsets,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub enum Direction {
    #[default]
    Horizontal,

    Vertical,
}

// ============================================================================
// ItemTemplate / ColumnDef
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ItemTemplate {
    pub height: u32,

    pub gap: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ColumnDef {
    pub field: String,

    pub width: u32,

    pub align: Option<TextAlign>,

    pub prefix: Option<String>,
}

// ============================================================================
// EdgeInsets
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct EdgeInsets {
    #[serde(default)]
    pub top: u32,

    #[serde(default)]
    pub bottom: u32,

    #[serde(default)]
    pub left: u32,

    #[serde(default)]
    pub right: u32,
}

impl Default for EdgeInsets {
    fn default() -> Self {
        Self {
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        }
    }
}

// ============================================================================
// RenderError
// ============================================================================

#[derive(Debug, Clone, Deserialize, Error)]
pub enum RenderError {
    #[error("invalid layout")]
    InvalidLayout,

    #[error("unknown element type")]
    UnknownElement,

    #[error("missing variable")]
    MissingVariable,

    #[error("render failed")]
    RenderFailed,
}

// ============================================================================
// DataValue
// ============================================================================

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum DataValue {
    Str(String),

    Int(i64),

    Float(f64),

    Bool(bool),

    List(Vec<DataValue>),

    TileId(u16),
}

// ============================================================================
// From conversions for DataValue
// ============================================================================

impl From<String> for DataValue {
    fn from(s: String) -> Self {
        DataValue::Str(s)
    }
}

impl From<&str> for DataValue {
    fn from(s: &str) -> Self {
        DataValue::Str(s.to_string())
    }
}

impl From<i64> for DataValue {
    fn from(n: i64) -> Self {
        DataValue::Int(n)
    }
}

impl From<u16> for DataValue {
    fn from(n: u16) -> Self {
        DataValue::TileId(n)
    }
}

impl From<bool> for DataValue {
    fn from(b: bool) -> Self {
        DataValue::Bool(b)
    }
}

impl From<Vec<DataValue>> for DataValue {
    fn from(v: Vec<DataValue>) -> Self {
        DataValue::List(v)
    }
}

// ============================================================================
// RenderContext
// ============================================================================

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RenderContext<'a> {
    pub screen: &'a str,

    pub theme: &'a Theme,

    pub fonts: &'a FontRegistry,

    pub tilesets: &'a TilesetRegistry,

    /// Full-colour images the `image` element blits, keyed by its resolved
    /// `source`. Defaults to a shared empty registry; attach with
    /// [`RenderContext::with_images`].
    pub images: &'a ImageRegistry,
}

impl<'a> RenderContext<'a> {
    pub fn new(
        screen: &'a str,
        theme: &'a Theme,
        fonts: &'a FontRegistry,
        tilesets: &'a TilesetRegistry,
    ) -> Self {
        Self {
            screen,
            theme,
            fonts,
            tilesets,
            images: empty_image_registry(),
        }
    }

    /// Attach an image registry the `image` element blits from.
    pub fn with_images(mut self, images: &'a ImageRegistry) -> Self {
        self.images = images;
        self
    }
}

// ============================================================================
// DataContext
// ============================================================================

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DataContext {
    pub(crate) values: HashMap<String, DataValue>,
}
