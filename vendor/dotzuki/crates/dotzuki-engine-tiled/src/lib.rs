//! Tiled `.tmx` map parser for the JRPG engine framework.
//!
//! This crate parses [Tiled](https://www.mapeditor.org/) map files in JSON
//! format and converts them into [`dotzuki_engine`] types suitable for rendering
//! and game logic.
//!
//! # GID flipping
//!
//! Tiled encodes horizontal, vertical, and diagonal flip flags in the upper
//! bits of each tile GID.  The conversion functions in this crate strip those
//! flags and translate them into [`TilemapEntry`] fields.
//!
//! # Example
//!
//! ```rust
//! use dotzuki_engine_tiled::{parse_tmx, tmx_to_map_state};
//!
//! let json = r#"{
//!   "width": 2, "height": 2, "tilewidth": 16, "tileheight": 16,
//!   "layers": [{
//!     "name": "ground", "width": 2, "height": 2,
//!     "data": [1, 2, 3, 4],
//!     "visible": true, "opacity": 1.0
//!   }],
//!   "tilesets": [{
//!     "firstgid": 1, "name": "overworld",
//!     "tilewidth": 16, "tileheight": 16, "tilecount": 64
//!   }]
//! }"#;
//!
//! let tmx = parse_tmx(json).expect("valid Tiled JSON");
//! let state = tmx_to_map_state(&tmx);
//! assert_eq!(state.layers.len(), 1);
//! ```

use dotzuki_engine::metatile::TriggerType;
use dotzuki_engine::render::{BlendMode, MapLayer, MapRenderState};
use dotzuki_engine::tile_meta::CollisionType;
use dotzuki_engine::tilemap::{Tilemap, TilemapEntry};
use dotzuki_engine::trigger_manager::Trigger;
use serde::Deserialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// GID constants
// ---------------------------------------------------------------------------

/// Mask that strips all Tiled flip flags, leaving only the tile ID.
pub const GID_TILE_MASK: u32 = 0x1FFF_FFFF;

/// Horizontal flip flag (bit 31).
pub const GID_FLIP_H: u32 = 0x8000_0000;
/// Vertical flip flag (bit 30).
pub const GID_FLIP_V: u32 = 0x4000_0000;
/// Diagonal flip flag (bit 29).
pub const GID_FLIP_D: u32 = 0x2000_0000;

// ---------------------------------------------------------------------------
// Property mapping — Tiled custom properties → engine metadata
// ---------------------------------------------------------------------------

/// Parsed tile properties from Tiled custom properties.
///
/// Each field is `Option` because a Tiled map may not define every property.
/// Use the `PropertyConfig` type to customise which Tiled property names map
/// to which fields.
#[derive(Debug, Default, Clone)]
pub struct TileProperties {
    /// Collision behaviour derived from the `"collision"` property.
    pub collision: Option<CollisionType>,
    /// Name of a script/event to trigger (from the `"trigger"` property).
    pub trigger: Option<String>,
    /// When the trigger fires (from the `"trigger_type"` property).
    pub trigger_type: Option<TriggerType>,
    /// Animation group index (from the `"animation_group"` property).
    pub animation_group: Option<u8>,
    /// Vertical pixel offset (from the `"z_offset"` property).
    pub z_offset: Option<i8>,
    /// Palette-swap variant name (from the `"palette_swap"` property).
    pub palette_swap: Option<String>,
}

/// Configurable property name mappings.
///
/// Allows users of the library to use different Tiled custom property names
/// without hard-coding the defaults.  Use [`PropertyConfig::default`] for the
/// standard names.
///
/// # Example
///
/// ```rust
/// use dotzuki_engine_tiled::PropertyConfig;
///
/// let config = PropertyConfig {
///     collision_key: "collision_type".into(),
///     animation_key: "anim_group".into(),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct PropertyConfig {
    pub collision_key: String,
    pub trigger_key: String,
    pub trigger_type_key: String,
    pub animation_key: String,
    pub z_offset_key: String,
    pub palette_swap_key: String,
}

impl Default for PropertyConfig {
    fn default() -> Self {
        Self {
            collision_key: "collision".into(),
            trigger_key: "trigger".into(),
            trigger_type_key: "trigger_type".into(),
            animation_key: "animation_group".into(),
            z_offset_key: "z_offset".into(),
            palette_swap_key: "palette_swap".into(),
        }
    }
}

/// Convert a slice of [`TmxProperty`] into a `HashMap` keyed by property name.
///
/// String, number and boolean values are converted to their string
/// representation.  `null`, array and object values are silently skipped.
pub fn properties_to_map(props: &[TmxProperty]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for prop in props {
        let value = match &prop.value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => continue,
        };
        map.insert(prop.name.clone(), value);
    }
    map
}

/// Parse Tiled custom string properties into typed [`TileProperties`].
///
/// Uses the default [`PropertyConfig`] key names.  Unknown or unparsable
/// properties are silently ignored (the corresponding field stays `None`).
///
/// # Example
///
/// ```rust
/// use std::collections::HashMap;
/// use dotzuki_engine_tiled::{TileProperties, parse_tile_properties};
///
/// let mut props = HashMap::new();
/// props.insert("collision".into(), "impassable".into());
/// let tp = parse_tile_properties(&props);
/// assert!(tp.collision.is_some());
/// ```
pub fn parse_tile_properties(props: &HashMap<String, String>) -> TileProperties {
    parse_tile_properties_with_config(props, &PropertyConfig::default())
}

/// Like [`parse_tile_properties`] but accepts a custom [`PropertyConfig`].
pub fn parse_tile_properties_with_config(
    props: &HashMap<String, String>,
    config: &PropertyConfig,
) -> TileProperties {
    let collision = props
        .get(&config.collision_key)
        .and_then(|s| match s.as_str() {
            "passable" => Some(CollisionType::Passable),
            "impassable" => Some(CollisionType::Impassable),
            "ledge" => Some(CollisionType::Ledge { direction: 0 }),
            "water" => Some(CollisionType::Water),
            "grass" => Some(CollisionType::Grass(None)),
            "door" => Some(CollisionType::Door),
            "warp" => Some(CollisionType::Warp),
            "counter" => Some(CollisionType::Counter),
            _ => None,
        });

    let trigger = props.get(&config.trigger_key).cloned();

    let trigger_type = props
        .get(&config.trigger_type_key)
        .and_then(|s| match s.as_str() {
            "on_step" => Some(TriggerType::OnStep),
            "on_enter" => Some(TriggerType::OnEnter),
            "on_interact" => Some(TriggerType::OnInteract),
            _ => None,
        });

    let animation_group = props
        .get(&config.animation_key)
        .and_then(|s| s.parse::<u8>().ok());

    let z_offset = props
        .get(&config.z_offset_key)
        .and_then(|s| s.parse::<i8>().ok());

    let palette_swap = props.get(&config.palette_swap_key).cloned();

    TileProperties {
        collision,
        trigger,
        trigger_type,
        animation_group,
        z_offset,
        palette_swap,
    }
}

/// Extracts [`Trigger`] entries from a Tiled map by scanning every tileset tile
/// for `trigger` / `trigger_type` custom properties and cross-referencing layer
/// GID data to determine tile positions.
///
/// Each matching tile produces one [`Trigger`] per occurrence in the map layers.
/// The caller must provide a `map_id` string (e.g. `"StartTown"`) that will be
/// stamped on every returned trigger.
pub fn extract_triggers_from_tmx(tmx: &TmxMap, map_id: &str) -> Vec<Trigger> {
    let mut triggers = Vec::new();

    // Build a lookup: global_tile_id → (trigger_name, trigger_type)
    // Tiled's "firstgid" is 1-based, but local tile IDs inside tilesets are 0-based.
    let mut tile_trigger_map: Vec<(u32, String, TriggerType)> = Vec::new();

    for ts_ref in &tmx.tilesets {
        for tile in &ts_ref.tiles {
            let props = properties_to_map(&tile.properties);
            let tp = parse_tile_properties(&props);
            if let (Some(script_name), Some(trigger_type)) = (tp.trigger, tp.trigger_type) {
                let global_id = ts_ref.first_gid.saturating_add(tile.id);
                tile_trigger_map.push((global_id, script_name, trigger_type));
            }
        }
    }

    if tile_trigger_map.is_empty() {
        return triggers;
    }

    // Walk each layer and check each GID against the trigger map.
    for layer in &tmx.layers {
        if !layer.visible {
            continue;
        }
        let w = layer.width.max(1) as usize;
        for (idx, &gid) in layer.data.iter().enumerate() {
            let clean = clean_gid(gid);
            if let Some((_, ref script_name, ref trigger_type)) =
                tile_trigger_map.iter().find(|(tgid, _, _)| *tgid == clean)
            {
                let x = (idx % w) as u32;
                let y = (idx / w) as u32;
                let id = format!("tiled_{}_{}_{}", map_id, x, y);
                triggers.push(Trigger::single_tile(
                    id,
                    map_id.to_string(),
                    *trigger_type,
                    x,
                    y,
                    script_name.clone(),
                    true, // one_shot: tile triggers fire once
                ));
            }
        }
    }

    triggers
}

impl TmxLayer {
    /// Parse this layer's custom Tiled properties into typed [`TileProperties`].
    ///
    /// Uses the default [`PropertyConfig`] key names.
    pub fn tile_properties(&self) -> TileProperties {
        let map = properties_to_map(&self.properties);
        parse_tile_properties(&map)
    }

    /// Like [`tile_properties`](Self::tile_properties) but accepts a custom
    /// [`PropertyConfig`].
    pub fn tile_properties_with_config(&self, config: &PropertyConfig) -> TileProperties {
        let map = properties_to_map(&self.properties);
        parse_tile_properties_with_config(&map, config)
    }
}

/// The elevation level of a Tiled layer: the integer custom property
/// `level` (default 0 when absent, non-integer or out of `i32` range).
/// Multi-level maps use it to split rendering at the player's elevation
/// (see [`MapLayer::level`]).
pub fn layer_level(layer: &TmxLayer) -> i32 {
    layer
        .properties
        .iter()
        .find(|p| p.name == "level")
        .and_then(|p| p.value.as_i64())
        .and_then(|v| i32::try_from(v).ok())
        .unwrap_or(0)
}

/// Errors that can occur during Tiled map parsing.
#[derive(Debug, Clone)]
pub enum TmxError {
    /// The JSON could not be deserialized.
    Json(String),
    /// An empty tileset list was encountered – the map has no tileset references.
    NoTilesets,
    /// A layer's data array is the wrong length for its width×height.
    LayerDataSize {
        layer: String,
        expected: usize,
        actual: usize,
    },
}

impl std::fmt::Display for TmxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(msg) => write!(f, "JSON parse error: {msg}"),
            Self::NoTilesets => write!(f, "map contains no tilesets"),
            Self::LayerDataSize {
                layer,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "layer '{layer}' data size mismatch: expected {expected}, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for TmxError {}

// ---------------------------------------------------------------------------
// Raw Tiled JSON types (serde)
// ---------------------------------------------------------------------------

/// A single tile's custom data extracted from a tileset's `tiles` dictionary.
#[derive(Debug, Clone, Deserialize)]
pub struct TmxTile {
    /// Local tile ID within the tileset.
    pub id: u32,
    /// Custom string properties attached to this tile (e.g., terrain, collision).
    #[serde(default)]
    pub properties: Vec<TmxProperty>,
}

/// A key-value custom property from Tiled.
#[derive(Debug, Clone, Deserialize)]
pub struct TmxProperty {
    pub name: String,
    pub value: serde_json::Value,
}

/// A tileset reference as it appears in the map JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct TmxTilesetRef {
    /// The first (lowest) GID that maps into this tileset.
    #[serde(rename = "firstgid")]
    pub first_gid: u32,

    /// If the tileset is stored in an external `.tsx` file, this is the
    /// relative path.  When `None` the tileset is embedded in the map.
    pub source: Option<String>,

    /// Human-readable name.
    pub name: String,

    /// Pixel width of each tile in the tileset.
    #[serde(rename = "tilewidth")]
    pub tile_width: u32,

    /// Pixel height of each tile in the tileset.
    #[serde(rename = "tileheight")]
    pub tile_height: u32,

    /// Total number of tiles in this tileset.
    #[serde(rename = "tilecount")]
    pub tile_count: u32,

    /// Per-tile custom properties (keyed by local tile ID).
    #[serde(default)]
    pub tiles: Vec<TmxTile>,
}

/// A single layer from a Tiled map.
#[derive(Debug, Clone, Deserialize)]
pub struct TmxLayer {
    /// Human-readable name for the layer (e.g. "ground", "collision").
    pub name: String,

    /// Row-major GID array.  Length must equal `width * height`.
    pub data: Vec<u32>,

    /// Tile count horizontally for this layer.
    pub width: u32,

    /// Tile count vertically for this layer.
    pub height: u32,

    /// Whether the layer should be drawn.
    #[serde(default = "default_true")]
    pub visible: bool,

    /// Global opacity (0.0 = transparent, 1.0 = opaque).
    #[serde(default = "default_one")]
    pub opacity: f32,

    /// Custom string properties attached to the layer.
    #[serde(default)]
    pub properties: Vec<TmxProperty>,
}

fn default_true() -> bool {
    true
}
fn default_one() -> f32 {
    1.0
}

/// The root Tiled map document, deserialised from JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct TmxMap {
    /// Tile count horizontally.
    pub width: u32,

    /// Tile count vertically.
    pub height: u32,

    /// Pixel width of each tile.
    #[serde(rename = "tilewidth")]
    pub tile_width: u32,

    /// Pixel height of each tile.
    #[serde(rename = "tileheight")]
    pub tile_height: u32,

    /// Ordered layer list (bottom first).
    #[serde(default)]
    pub layers: Vec<TmxLayer>,

    /// Referenced tilesets (in order of ascending `first_gid`).
    #[serde(default)]
    pub tilesets: Vec<TmxTilesetRef>,

    /// Map-level background colour in `#RRGGBB` or `#AARRGGBB` format.
    #[serde(rename = "backgroundcolor")]
    pub background_color: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a Tiled `.tmx` file from a JSON string.
///
/// # Errors
///
/// Returns [`TmxError::Json`] if the input is not valid JSON or does not
/// conform to the expected schema.
pub fn parse_tmx(json: &str) -> Result<TmxMap, TmxError> {
    serde_json::from_str(json).map_err(|e| TmxError::Json(e.to_string()))
}

// ---------------------------------------------------------------------------
// GID conversion
// ---------------------------------------------------------------------------

/// Convert a raw Tiled GID into a [`TilemapEntry`].
///
/// Tiled stores flip flags in the upper bits of the GID:
///
/// | Bit  | Meaning              |
/// |------|----------------------|
/// | 31   | Horizontal flip      |
/// | 30   | Vertical flip        |
/// | 29   | Diagonal (anti-diagonal) flip |
/// | 0-28 | Tile ID              |
///
/// The diagonal flip is not mapped to a `TilemapEntry` field — it is
/// effectively a 180° rotation that can be expressed as combined H+V flips.
/// If you need to preserve it, inspect the raw GID before calling this
/// function.
///
/// # Examples
///
/// ```rust
/// use dotzuki_engine_tiled::gid_to_tilemap_entry;
///
/// let entry = gid_to_tilemap_entry(42);
/// assert_eq!(entry.tile_id, 42);
/// assert!(!entry.flip_h);
/// assert!(!entry.flip_v);
///
/// let entry = gid_to_tilemap_entry(0x8000_0042);
/// assert_eq!(entry.tile_id, 0x42);
/// assert!(entry.flip_h);
/// assert!(!entry.flip_v);
/// ```
pub fn gid_to_tilemap_entry(gid: u32) -> TilemapEntry {
    let tile_id = (gid & GID_TILE_MASK) as u16;
    let flip_h = (gid & GID_FLIP_H) != 0;
    let flip_v = (gid & GID_FLIP_V) != 0;

    TilemapEntry {
        tile_id,
        flip_h,
        flip_v,
        ..Default::default()
    }
}

/// Returns the clean tile ID (stripped of all flip flags).
pub fn clean_gid(gid: u32) -> u32 {
    gid & GID_TILE_MASK
}

// ---------------------------------------------------------------------------
// Conversion to dotzuki-engine types
// ---------------------------------------------------------------------------

/// Convert a parsed [`TmxMap`] into a [`MapRenderState`] suitable for
/// rendering.
///
/// Each Tiled layer becomes a [`MapLayer`] whose `z_index` is set to its
/// position in the layer stack (0 = bottom) and whose `level` comes from
/// the layer's integer custom property `level` (default 0 — see
/// [`layer_level`]).  Layers marked `visible: false` are still included
/// but have `visible` set accordingly.
///
/// The map's background colour, if present, is used to fill the
/// `background_color` field of `MapRenderState`.  Otherwise opaque black is
/// used.
pub fn tmx_to_map_state(tmx: &TmxMap) -> MapRenderState {
    let bg = parse_hex_color(tmx.background_color.as_deref()).unwrap_or((0, 0, 0, 255));

    let layers: Vec<MapLayer> = tmx
        .layers
        .iter()
        .enumerate()
        .map(|(i, layer)| {
            let w = layer.width as u16;
            let h = layer.height as u16;
            let tile_count = w as usize * h as usize;

            let entries: Vec<TilemapEntry> = layer
                .data
                .iter()
                .take(tile_count)
                .map(|&gid| gid_to_tilemap_entry(gid))
                .collect();

            let tilemap = Tilemap {
                width: w,
                height: h,
                entries,
            };

            MapLayer {
                tilemap,
                visible: layer.visible,
                opacity: layer.opacity.clamp(0.0, 1.0),
                scroll_factor: (1.0, 1.0),
                blend_mode: BlendMode::Normal,
                z_index: i as i32,
                no_animation: false,
                level: layer_level(layer),
            }
        })
        .collect();

    MapRenderState {
        layers,
        background_color: bg,
    }
}

/// Parse a hex colour string like `"#FF8000"`, `"#FFF"`, or `"#80FF0000"`
/// into `(r, g, b, a)`.
fn parse_hex_color(hex: Option<&str>) -> Option<(u8, u8, u8, u8)> {
    let hex = hex?;
    let hex = hex.strip_prefix('#')?;

    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some((r, g, b, 255))
        }
        4 => {
            let a = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let r = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
            Some((r, g, b, a))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b, 255))
        }
        8 => {
            let a = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let r = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let g = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let b = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some((r, g, b, a))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // GID helpers
    // ---------------------------------------------------------------

    #[test]
    fn clean_gid_strips_flags() {
        assert_eq!(clean_gid(42), 42);
        assert_eq!(clean_gid(GID_FLIP_H | 99), 99);
        assert_eq!(clean_gid(GID_FLIP_V | 99), 99);
        assert_eq!(clean_gid(GID_FLIP_D | 99), 99);
        assert_eq!(clean_gid(GID_FLIP_H | GID_FLIP_V | GID_FLIP_D | 99), 99);
        assert_eq!(clean_gid(0x1FFF_FFFF), 0x1FFF_FFFF);
    }

    #[test]
    fn gid_no_flips() {
        let e = gid_to_tilemap_entry(42);
        assert_eq!(e.tile_id, 42);
        assert!(!e.flip_h);
        assert!(!e.flip_v);
    }

    #[test]
    fn gid_horizontal_flip() {
        let e = gid_to_tilemap_entry(GID_FLIP_H | 7);
        assert_eq!(e.tile_id, 7);
        assert!(e.flip_h);
        assert!(!e.flip_v);
    }

    #[test]
    fn gid_vertical_flip() {
        let e = gid_to_tilemap_entry(GID_FLIP_V | 13);
        assert_eq!(e.tile_id, 13);
        assert!(!e.flip_h);
        assert!(e.flip_v);
    }

    #[test]
    fn gid_both_flips() {
        let e = gid_to_tilemap_entry(GID_FLIP_H | GID_FLIP_V | 255);
        assert_eq!(e.tile_id, 255);
        assert!(e.flip_h);
        assert!(e.flip_v);
    }

    #[test]
    fn gid_diagonal_ignored() {
        let e = gid_to_tilemap_entry(GID_FLIP_D | 10);
        assert_eq!(e.tile_id, 10);
        assert!(!e.flip_h);
        assert!(!e.flip_v);
    }

    #[test]
    fn gid_large_tile_id() {
        // 16-bit tile IDs are supported
        let e = gid_to_tilemap_entry(0xFFFF);
        assert_eq!(e.tile_id, 0xFFFF);
    }

    // ---------------------------------------------------------------
    // Hex colour parsing
    // ---------------------------------------------------------------

    #[test]
    fn hex_6_digit() {
        assert_eq!(parse_hex_color(Some("#FF8000")), Some((255, 128, 0, 255)));
    }

    #[test]
    fn hex_8_digit() {
        assert_eq!(parse_hex_color(Some("#80FF0000")), Some((255, 0, 0, 128)));
    }

    #[test]
    fn hex_3_digit() {
        assert_eq!(parse_hex_color(Some("#F00")), Some((255, 0, 0, 255)));
    }

    #[test]
    fn hex_none() {
        assert_eq!(parse_hex_color(None), None);
    }

    #[test]
    fn hex_no_hash() {
        assert_eq!(parse_hex_color(Some("FF8000")), None);
    }

    // ---------------------------------------------------------------
    // parse_tmx — minimal valid map
    // ---------------------------------------------------------------

    fn minimal_tmx_json() -> String {
        r#"{
  "width": 2,
  "height": 2,
  "tilewidth": 16,
  "tileheight": 16,
  "layers": [
    {
      "name": "ground",
      "width": 2,
      "height": 2,
      "data": [1, 2, 3, 4],
      "visible": true,
      "opacity": 1.0
    }
  ],
  "tilesets": [
    {
      "firstgid": 1,
      "name": "overworld",
      "tilewidth": 16,
      "tileheight": 16,
      "tilecount": 64
    }
  ]
}"#
        .to_string()
    }

    #[test]
    fn parse_minimal_tmx() {
        let tmx = parse_tmx(&minimal_tmx_json()).expect("parse");
        assert_eq!(tmx.width, 2);
        assert_eq!(tmx.height, 2);
        assert_eq!(tmx.tile_width, 16);
        assert_eq!(tmx.tile_height, 16);
        assert_eq!(tmx.layers.len(), 1);
        assert_eq!(tmx.tilesets.len(), 1);
    }

    #[test]
    fn parse_layer_properties() {
        let tmx = parse_tmx(&minimal_tmx_json()).unwrap();
        let layer = &tmx.layers[0];
        assert_eq!(layer.name, "ground");
        assert_eq!(layer.data, vec![1, 2, 3, 4]);
        assert_eq!(layer.width, 2);
        assert_eq!(layer.height, 2);
        assert!(layer.visible);
        assert!((layer.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_tileset_properties() {
        let tmx = parse_tmx(&minimal_tmx_json()).unwrap();
        let ts = &tmx.tilesets[0];
        assert_eq!(ts.first_gid, 1);
        assert_eq!(ts.name, "overworld");
        assert_eq!(ts.tile_width, 16);
        assert_eq!(ts.tile_height, 16);
        assert_eq!(ts.tile_count, 64);
    }

    #[test]
    fn parse_invalid_json() {
        let result = parse_tmx("not json at all");
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // tmx_to_map_state
    // ---------------------------------------------------------------

    #[test]
    fn tmx_to_map_state_single_layer() {
        let tmx = parse_tmx(&minimal_tmx_json()).unwrap();
        let state = tmx_to_map_state(&tmx);

        assert_eq!(state.layers.len(), 1);
        assert_eq!(state.layers[0].z_index, 0);
        assert!(state.layers[0].visible);
        assert_eq!(state.layers[0].tilemap.width, 2);
        assert_eq!(state.layers[0].tilemap.height, 2);
        assert_eq!(state.layers[0].tilemap.entries.len(), 4);

        // Entries should match the GID data
        assert_eq!(state.layers[0].tilemap.entries[0].tile_id, 1);
        assert_eq!(state.layers[0].tilemap.entries[1].tile_id, 2);
        assert_eq!(state.layers[0].tilemap.entries[2].tile_id, 3);
        assert_eq!(state.layers[0].tilemap.entries[3].tile_id, 4);
    }

    #[test]
    fn tmx_to_map_state_multi_layer_z_index() {
        let json = r#"{
  "width": 2, "height": 2, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "bottom", "width": 2, "height": 2, "data": [1,1,1,1] },
    { "name": "middle", "width": 2, "height": 2, "data": [2,2,2,2] },
    { "name": "top",    "width": 2, "height": 2, "data": [3,3,3,3] }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 16 }
  ]
}"#;

        let tmx = parse_tmx(json).unwrap();
        let state = tmx_to_map_state(&tmx);
        assert_eq!(state.layers.len(), 3);
        assert_eq!(state.layers[0].z_index, 0);
        assert_eq!(state.layers[1].z_index, 1);
        assert_eq!(state.layers[2].z_index, 2);

        assert_eq!(state.layers[0].tilemap.entries[0].tile_id, 1);
        assert_eq!(state.layers[1].tilemap.entries[0].tile_id, 2);
        assert_eq!(state.layers[2].tilemap.entries[0].tile_id, 3);
    }

    #[test]
    fn tmx_to_map_state_hidden_layer() {
        let json = r#"{
  "width": 1, "height": 1, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "hidden", "width": 1, "height": 1, "data": [1], "visible": false },
    { "name": "shown",  "width": 1, "height": 1, "data": [2], "visible": true }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#;

        let tmx = parse_tmx(json).unwrap();
        let state = tmx_to_map_state(&tmx);
        assert!(!state.layers[0].visible);
        assert!(state.layers[1].visible);
        assert_eq!(state.visible_layer_count(), 1);
    }

    #[test]
    fn tmx_to_map_state_with_opacity() {
        let json = r#"{
  "width": 1, "height": 1, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "fade", "width": 1, "height": 1, "data": [1], "opacity": 0.5 }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#;

        let tmx = parse_tmx(json).unwrap();
        let state = tmx_to_map_state(&tmx);
        assert!((state.layers[0].opacity - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn tmx_to_map_state_flipped_gids() {
        // GID with H-flip: 0x80000000 | 5 = horizontal flip on tile 5
        // GID with V-flip: 0x40000000 | 3 = vertical flip on tile 3
        let json = format!(
            r#"{{
  "width": 2, "height": 1, "tilewidth": 16, "tileheight": 16,
  "layers": [
    {{ "name": "flip", "width": 2, "height": 1, "data": [{hflip}, {vflip}] }}
  ],
  "tilesets": [
    {{ "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 16 }}
  ]
}}"#,
            hflip = GID_FLIP_H | 5,
            vflip = GID_FLIP_V | 3,
        );

        let tmx = parse_tmx(&json).unwrap();
        let state = tmx_to_map_state(&tmx);

        let entries = &state.layers[0].tilemap.entries;
        assert_eq!(entries[0].tile_id, 5);
        assert!(entries[0].flip_h);
        assert!(!entries[0].flip_v);

        assert_eq!(entries[1].tile_id, 3);
        assert!(!entries[1].flip_h);
        assert!(entries[1].flip_v);
    }

    #[test]
    fn tmx_to_map_state_layer_level_property() {
        let json = r#"{
  "width": 1, "height": 1, "tilewidth": 16, "tileheight": 16,
  "layers": [
    { "name": "ground", "width": 1, "height": 1, "data": [1] },
    { "name": "walltop", "width": 1, "height": 1, "data": [2],
      "properties": [{ "name": "level", "type": "int", "value": 1 }] },
    { "name": "bad", "width": 1, "height": 1, "data": [3],
      "properties": [{ "name": "level", "type": "string", "value": "high" }] }
  ],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#;

        let tmx = parse_tmx(json).unwrap();
        assert_eq!(layer_level(&tmx.layers[0]), 0, "absent ⇒ ground");
        assert_eq!(layer_level(&tmx.layers[1]), 1, "integer property read");
        assert_eq!(layer_level(&tmx.layers[2]), 0, "non-integer ignored");

        let state = tmx_to_map_state(&tmx);
        assert_eq!(state.layers[0].level, 0);
        assert_eq!(state.layers[1].level, 1);
        assert_eq!(state.layers[2].level, 0);
    }

    #[test]
    fn tmx_to_map_state_background_color() {
        let json = r###"{
  "width": 1, "height": 1, "tilewidth": 16, "tileheight": 16,
  "backgroundcolor": "#123456",
  "layers": [],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"###;

        let tmx = parse_tmx(json).unwrap();
        let state = tmx_to_map_state(&tmx);
        assert_eq!(state.background_color, (0x12, 0x34, 0x56, 0xFF));
    }

    #[test]
    fn tmx_to_map_state_default_bg() {
        let json = r#"{
  "width": 1, "height": 1, "tilewidth": 16, "tileheight": 16,
  "layers": [],
  "tilesets": [
    { "firstgid": 1, "name": "ts", "tilewidth": 16, "tileheight": 16, "tilecount": 4 }
  ]
}"#;

        let tmx = parse_tmx(json).unwrap();
        let state = tmx_to_map_state(&tmx);
        assert_eq!(state.background_color, (0, 0, 0, 255));
    }

    // ---------------------------------------------------------------
    // Embedded .tmx test file (2×2 tiles, 1 layer, 1 tileset)
    // ---------------------------------------------------------------

    const EMBEDDED_2X2: &str = r###"{
  "width": 2,
  "height": 2,
  "tilewidth": 16,
  "tileheight": 16,
  "backgroundcolor": "#306850",
  "layers": [
    {
      "name": "ground",
      "width": 2,
      "height": 2,
      "data": [1, 2, 3, 4],
      "visible": true,
      "opacity": 1.0
    }
  ],
  "tilesets": [
    {
      "firstgid": 1,
      "name": "overworld",
      "tilewidth": 16,
      "tileheight": 16,
      "tilecount": 64,
      "tiles": [
        {
          "id": 0,
          "properties": [
            { "name": "terrain", "value": "grass" }
          ]
        },
        {
          "id": 1,
          "properties": [
            { "name": "terrain", "value": "water" }
          ]
        },
        {
          "id": 2,
          "properties": [
            { "name": "collision", "value": "block" }
          ]
        }
      ]
    }
  ]
}"###;

    #[test]
    fn embedded_2x2_parse() {
        let tmx = parse_tmx(EMBEDDED_2X2).expect("should parse embedded map");

        assert_eq!(tmx.width, 2);
        assert_eq!(tmx.height, 2);
        assert_eq!(tmx.tile_width, 16);
        assert_eq!(tmx.tile_height, 16);
        assert_eq!(tmx.background_color.as_deref(), Some("#306850"));
        assert_eq!(tmx.layers.len(), 1);
        assert_eq!(tmx.tilesets.len(), 1);
    }

    #[test]
    fn embedded_2x2_tileset_tiles() {
        let tmx = parse_tmx(EMBEDDED_2X2).unwrap();
        let ts = &tmx.tilesets[0];
        assert_eq!(ts.tiles.len(), 3);

        // Tile 0 has terrain=grass
        assert_eq!(ts.tiles[0].id, 0);
        assert_eq!(ts.tiles[0].properties.len(), 1);
        assert_eq!(ts.tiles[0].properties[0].name, "terrain");
        assert_eq!(ts.tiles[0].properties[0].value, "grass");

        // Tile 1 has terrain=water
        assert_eq!(ts.tiles[1].id, 1);
        assert_eq!(ts.tiles[1].properties[0].name, "terrain");
        assert_eq!(ts.tiles[1].properties[0].value, "water");

        // Tile 2 has collision=block
        assert_eq!(ts.tiles[2].id, 2);
        assert_eq!(ts.tiles[2].properties[0].name, "collision");
        assert_eq!(ts.tiles[2].properties[0].value, "block");
    }

    #[test]
    fn embedded_2x2_to_map_state() {
        let tmx = parse_tmx(EMBEDDED_2X2).unwrap();
        let state = tmx_to_map_state(&tmx);

        assert_eq!(state.layers.len(), 1);
        assert_eq!(state.background_color, (0x30, 0x68, 0x50, 0xFF));

        let layer = &state.layers[0];
        assert_eq!(layer.tilemap.width, 2);
        assert_eq!(layer.tilemap.height, 2);
        assert!(layer.visible);

        let entries = &layer.tilemap.entries;
        assert_eq!(entries[0].tile_id, 1);
        assert_eq!(entries[1].tile_id, 2);
        assert_eq!(entries[2].tile_id, 3);
        assert_eq!(entries[3].tile_id, 4);
    }

    // ---------------------------------------------------------------
    // Empty map
    // ---------------------------------------------------------------

    #[test]
    fn empty_map_no_layers() {
        let json = r#"{
  "width": 10, "height": 10, "tilewidth": 8, "tileheight": 8,
  "layers": [],
  "tilesets": [
    { "firstgid": 1, "name": "empty", "tilewidth": 8, "tileheight": 8, "tilecount": 1 }
  ]
}"#;

        let tmx = parse_tmx(json).unwrap();
        let state = tmx_to_map_state(&tmx);
        assert!(state.layers.is_empty());
    }

    // ---------------------------------------------------------------
    // Property mapping — TileProperties
    // ---------------------------------------------------------------

    fn make_props(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn collision_impassable() {
        let props = make_props(&[("collision", "impassable")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Impassable));
    }

    #[test]
    fn collision_passable() {
        let props = make_props(&[("collision", "passable")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Passable));
    }

    #[test]
    fn collision_ledge() {
        let props = make_props(&[("collision", "ledge")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Ledge { direction: 0 }));
    }

    #[test]
    fn collision_water() {
        let props = make_props(&[("collision", "water")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Water));
    }

    #[test]
    fn collision_grass() {
        let props = make_props(&[("collision", "grass")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Grass(None)));
    }

    #[test]
    fn collision_door() {
        let props = make_props(&[("collision", "door")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Door));
    }

    #[test]
    fn collision_warp() {
        let props = make_props(&[("collision", "warp")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Warp));
    }

    #[test]
    fn collision_counter() {
        let props = make_props(&[("collision", "counter")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Counter));
    }

    #[test]
    fn collision_unknown_value_returns_none() {
        let props = make_props(&[("collision", "bouncy")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, None);
    }

    #[test]
    fn trigger_script_name() {
        let props = make_props(&[("trigger", "prof_lab_door")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.trigger.as_deref(), Some("prof_lab_door"));
    }

    #[test]
    fn trigger_type_on_step() {
        let props = make_props(&[("trigger_type", "on_step")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.trigger_type, Some(TriggerType::OnStep));
    }

    #[test]
    fn trigger_type_on_enter() {
        let props = make_props(&[("trigger_type", "on_enter")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.trigger_type, Some(TriggerType::OnEnter));
    }

    #[test]
    fn trigger_type_on_interact() {
        let props = make_props(&[("trigger_type", "on_interact")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.trigger_type, Some(TriggerType::OnInteract));
    }

    #[test]
    fn trigger_type_unknown_returns_none() {
        let props = make_props(&[("trigger_type", "on_hover")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.trigger_type, None);
    }

    #[test]
    fn animation_group_parsed() {
        let props = make_props(&[("animation_group", "3")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.animation_group, Some(3));
    }

    #[test]
    fn z_offset_negative() {
        let props = make_props(&[("z_offset", "-1")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.z_offset, Some(-1));
    }

    #[test]
    fn z_offset_positive() {
        let props = make_props(&[("z_offset", "5")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.z_offset, Some(5));
    }

    #[test]
    fn palette_swap_name() {
        let props = make_props(&[("palette_swap", "night")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.palette_swap.as_deref(), Some("night"));
    }

    #[test]
    fn all_properties_together() {
        let props = make_props(&[
            ("collision", "water"),
            ("trigger", "surf_check"),
            ("trigger_type", "on_step"),
            ("animation_group", "1"),
            ("z_offset", "-2"),
            ("palette_swap", "dungeon"),
        ]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, Some(CollisionType::Water));
        assert_eq!(tp.trigger.as_deref(), Some("surf_check"));
        assert_eq!(tp.trigger_type, Some(TriggerType::OnStep));
        assert_eq!(tp.animation_group, Some(1));
        assert_eq!(tp.z_offset, Some(-2));
        assert_eq!(tp.palette_swap.as_deref(), Some("dungeon"));
    }

    #[test]
    fn unknown_property_ignored() {
        let props = make_props(&[("unknown_key", "whatever")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, None);
        assert_eq!(tp.trigger, None);
        assert_eq!(tp.trigger_type, None);
        assert_eq!(tp.animation_group, None);
        assert_eq!(tp.z_offset, None);
        assert_eq!(tp.palette_swap, None);
    }

    #[test]
    fn custom_property_config_changes_key_names() {
        let config = PropertyConfig {
            collision_key: "collision_type".into(),
            trigger_key: "script".into(),
            animation_key: "anim".into(),
            ..Default::default()
        };

        let mut props = HashMap::new();
        props.insert("collision_type".into(), "door".into());
        props.insert("script".into(), "open_door".into());
        props.insert("anim".into(), "2".into());

        let tp = parse_tile_properties_with_config(&props, &config);
        assert_eq!(tp.collision, Some(CollisionType::Door));
        assert_eq!(tp.trigger.as_deref(), Some("open_door"));
        assert_eq!(tp.animation_group, Some(2));
        // Default keys that weren't set should be absent
        assert_eq!(tp.z_offset, None);
        assert_eq!(tp.palette_swap, None);
    }

    #[test]
    fn empty_properties_returns_default() {
        let props = HashMap::new();
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.collision, None);
        assert_eq!(tp.trigger, None);
        assert_eq!(tp.trigger_type, None);
        assert_eq!(tp.animation_group, None);
        assert_eq!(tp.z_offset, None);
        assert_eq!(tp.palette_swap, None);
    }

    #[test]
    fn tmx_layer_tile_properties_with_custom_props() {
        // Build a TmxLayer with a custom property and verify tile_properties()
        let layer = TmxLayer {
            name: "test".into(),
            data: vec![1],
            width: 1,
            height: 1,
            visible: true,
            opacity: 1.0,
            properties: vec![TmxProperty {
                name: "collision".into(),
                value: serde_json::Value::String("impassable".into()),
            }],
        };
        let tp = layer.tile_properties();
        assert_eq!(tp.collision, Some(CollisionType::Impassable));
    }

    #[test]
    fn properties_to_map_converts_strings() {
        let props = vec![
            TmxProperty {
                name: "a".into(),
                value: serde_json::Value::String("hello".into()),
            },
            TmxProperty {
                name: "b".into(),
                value: serde_json::Value::Number(42.into()),
            },
            TmxProperty {
                name: "c".into(),
                value: serde_json::Value::Bool(true),
            },
        ];
        let map = properties_to_map(&props);
        assert_eq!(map.get("a").map(String::as_str), Some("hello"));
        assert_eq!(map.get("b").map(String::as_str), Some("42"));
        assert_eq!(map.get("c").map(String::as_str), Some("true"));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn invalid_number_returns_none() {
        let props = make_props(&[("z_offset", "not_a_number")]);
        let tp = parse_tile_properties(&props);
        assert_eq!(tp.z_offset, None);
    }

    #[test]
    fn collision_case_sensitive() {
        let props = make_props(&[("collision", "IMPASSABLE")]);
        let tp = parse_tile_properties(&props);
        // Only lowercase exact matches work
        assert_eq!(tp.collision, None);
    }
}
