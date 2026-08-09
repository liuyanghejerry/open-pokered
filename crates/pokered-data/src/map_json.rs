//! JSON-serializable map data types.
//!
//! These types define the schema for `map.json` files in `maps/{MapName}/`.
//! Used by both the generator tool (serialization) and the runtime loader (deserialization).

use serde::{Deserialize, Serialize};

/// Complete map data as stored in `map.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapJson {
    /// Numeric map ID (0x00-0xF7)
    pub id: u8,
    /// Map name matching MapId variant (e.g. "PalletTown")
    pub name: String,
    /// Map header data
    pub header: MapHeaderJson,
    /// Cardinal connections to adjacent maps
    #[serde(default)]
    pub connections: ConnectionsJson,
    /// Warp points (doors, stairs, etc.)
    #[serde(default)]
    pub warps: Vec<WarpJson>,
    /// NPC definitions
    #[serde(default)]
    pub npcs: Vec<NpcJson>,
    /// Sign definitions
    #[serde(default)]
    pub signs: Vec<SignJson>,
    /// Dialog text for NPCs and signs
    #[serde(default)]
    pub text: MapTextJson,
    /// Wild encounter data (per game version)
    #[serde(default)]
    pub wild: Option<WildDataJson>,
}

/// Map header — tileset, music, dimensions, connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapHeaderJson {
    /// Tileset name (e.g. "Overworld", "House")
    pub tileset: String,
    /// Music track name (e.g. "PalletTown", "Cities1")
    pub music: String,
    /// Bitfield: bit3=north, bit2=south, bit1=west, bit0=east
    pub connection_flags: u8,
    /// Map width in blocks
    pub width: u8,
    /// Map height in blocks
    pub height: u8,
    /// Border block ID
    pub border_block: u8,
}

/// Cardinal connections to adjacent maps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionsJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub north: Option<ConnectionEntryJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub south: Option<ConnectionEntryJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub west: Option<ConnectionEntryJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub east: Option<ConnectionEntryJson>,
}

/// A single map connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionEntryJson {
    /// Target map name (e.g. "Route1")
    pub target_map: String,
    /// Offset for alignment
    pub offset: i8,
}

/// A warp point (door, stairs, cave entrance, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarpJson {
    pub x: u8,
    pub y: u8,
    /// Destination map name, or null for LAST_MAP / dynamic destination
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest_map: Option<String>,
    pub dest_warp_id: u8,
}

/// An NPC definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcJson {
    pub sprite_id: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_name: Option<String>,
    pub x: u8,
    pub y: u8,
    /// Movement type: "Stationary", "Wander", "FixedPath", "FacePlayer"
    pub movement: String,
    /// Facing direction: "Down", "Up", "Left", "Right"
    pub facing: String,
    pub range: u8,
    pub text_id: u8,
    pub is_trainer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_set: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<u8>,
    /// One-shot victory quip shown after this trainer (a sight/talk trainer,
    /// `isTrainer: true`) is beaten, converted from the original per-map
    /// `TrainerHeader` `TextEndBattle`. Absent for non-trainers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_battle_text: Option<String>,
}

/// A sign definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignJson {
    pub x: u8,
    pub y: u8,
    pub text_id: u8,
}

/// Dialog text for all NPCs and signs on a map.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapTextJson {
    /// NPC dialog: key = text_id (string), value = array of text pages
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub npc: std::collections::HashMap<String, Vec<TextPageJson>>,
    /// Sign dialog: key = text_id (string), value = array of text pages
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub sign: std::collections::HashMap<String, Vec<TextPageJson>>,
}

/// A single dialog page (two lines displayed in the text box).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextPageJson {
    pub line1: String,
    #[serde(default)]
    pub line2: String,
}

/// Wild encounter data for both game versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WildDataJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub red: Option<VersionWildJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blue: Option<VersionWildJson>,
}

/// Wild encounters for a single version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionWildJson {
    pub grass: WildEncounterTableJson,
    pub water: WildEncounterTableJson,
}

/// A wild encounter table (rate + 10 slots).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WildEncounterTableJson {
    pub encounter_rate: u8,
    pub mons: Vec<WildMonJson>,
}

/// A single wild encounter slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WildMonJson {
    pub level: u8,
    pub species: String,
}

// ─── Compile-time mirror types ──────────────────────────────────────────────
//
// These `Static*` types parallel the owned `*Json` types above but use
// `&'static str` / `&'static [T]` everywhere. They exist solely so
// `build.rs::generate_map_data()` can emit a `static MAP_TABLE: &[(&str,
// &StaticMapJson)] = &[...]` literal — fully placed in `.rodata` with zero
// runtime allocation.
//
// At init time, the embedded loader walks `MAP_TABLE` and uses the
// `From<&'static StaticMapJson> for MapJson` cascade defined below to
// materialize owned `MapJson` values into the existing
// `HashMap<String, MapJson>` store. This preserves the existing public API
// (`get_map_json -> Option<&'static MapJson>`) without disturbing any caller.
//
// Types whose runtime representation contains no `String`/`Vec`/`HashMap`
// fields (currently `SignJson`) reuse the owned type directly inside `&'static
// [_]` slices; no mirror is needed for those.

/// Static-friendly mirror of [`MapJson`]; emitted by `build.rs`.
#[derive(Debug)]
pub struct StaticMapJson {
    pub id: u8,
    pub name: &'static str,
    pub header: &'static StaticMapHeaderJson,
    pub connections: &'static StaticConnectionsJson,
    pub warps: &'static [StaticWarpJson],
    pub npcs: &'static [StaticNpcJson],
    pub signs: &'static [SignJson],
    pub text: &'static StaticMapTextJson,
    pub wild: Option<&'static StaticWildDataJson>,
}

/// Static-friendly mirror of [`MapHeaderJson`].
#[derive(Debug)]
pub struct StaticMapHeaderJson {
    pub tileset: &'static str,
    pub music: &'static str,
    pub connection_flags: u8,
    pub width: u8,
    pub height: u8,
    pub border_block: u8,
}

/// Static-friendly mirror of [`ConnectionsJson`].
#[derive(Debug)]
pub struct StaticConnectionsJson {
    pub north: Option<&'static StaticConnectionEntryJson>,
    pub south: Option<&'static StaticConnectionEntryJson>,
    pub west: Option<&'static StaticConnectionEntryJson>,
    pub east: Option<&'static StaticConnectionEntryJson>,
}

/// Static-friendly mirror of [`ConnectionEntryJson`].
#[derive(Debug)]
pub struct StaticConnectionEntryJson {
    pub target_map: &'static str,
    pub offset: i8,
}

/// Static-friendly mirror of [`WarpJson`].
#[derive(Debug)]
pub struct StaticWarpJson {
    pub x: u8,
    pub y: u8,
    pub dest_map: Option<&'static str>,
    pub dest_warp_id: u8,
}

/// Static-friendly mirror of [`NpcJson`].
#[derive(Debug)]
pub struct StaticNpcJson {
    pub sprite_id: u8,
    pub sprite_name: Option<&'static str>,
    pub x: u8,
    pub y: u8,
    pub movement: &'static str,
    pub facing: &'static str,
    pub range: u8,
    pub text_id: u8,
    pub is_trainer: bool,
    pub trainer_class: Option<&'static str>,
    pub trainer_set: Option<u8>,
    pub item_id: Option<u8>,
    pub end_battle_text: Option<&'static str>,
}

/// Static-friendly mirror of [`MapTextJson`].
///
/// HashMap entries are stored as sorted-by-key slices of `(text_id, pages)`
/// pairs; lookups are O(N) linear scans (typical N ≈ 5–10 per map). Avoids
/// pulling `phf` into the build dependencies.
#[derive(Debug)]
pub struct StaticMapTextJson {
    pub npc: &'static [(&'static str, &'static [StaticTextPageJson])],
    pub sign: &'static [(&'static str, &'static [StaticTextPageJson])],
}

/// Static-friendly mirror of [`TextPageJson`].
#[derive(Debug)]
pub struct StaticTextPageJson {
    pub line1: &'static str,
    pub line2: &'static str,
}

/// Static-friendly mirror of [`WildDataJson`].
#[derive(Debug)]
pub struct StaticWildDataJson {
    pub red: Option<&'static StaticVersionWildJson>,
    pub blue: Option<&'static StaticVersionWildJson>,
}

/// Static-friendly mirror of [`VersionWildJson`].
///
/// Tables are stored by value because `StaticWildEncounterTableJson` is
/// already a small POD-like struct (one `u8` + one `&'static [_]` fat pointer).
#[derive(Debug)]
pub struct StaticVersionWildJson {
    pub grass: StaticWildEncounterTableJson,
    pub water: StaticWildEncounterTableJson,
}

/// Static-friendly mirror of [`WildEncounterTableJson`].
#[derive(Debug)]
pub struct StaticWildEncounterTableJson {
    pub encounter_rate: u8,
    pub mons: &'static [StaticWildMonJson],
}

/// Static-friendly mirror of [`WildMonJson`].
#[derive(Debug)]
pub struct StaticWildMonJson {
    pub level: u8,
    pub species: &'static str,
}

// ─── From conversions: Static* → owned ──────────────────────────────────────
//
// Used at embedded-init time to materialize each `&'static StaticMapJson`
// from the build-script-emitted table into the owned `HashMap` value the
// loader stores in its `OnceLock<MapDataStore>`.

impl From<&'static StaticMapJson> for MapJson {
    fn from(s: &'static StaticMapJson) -> Self {
        MapJson {
            id: s.id,
            name: s.name.to_string(),
            header: MapHeaderJson::from(s.header),
            connections: ConnectionsJson::from(s.connections),
            warps: s.warps.iter().map(WarpJson::from).collect(),
            npcs: s.npcs.iter().map(NpcJson::from).collect(),
            signs: s.signs.to_vec(),
            text: MapTextJson::from(s.text),
            wild: s.wild.map(WildDataJson::from),
        }
    }
}

impl From<&'static StaticMapHeaderJson> for MapHeaderJson {
    fn from(s: &'static StaticMapHeaderJson) -> Self {
        MapHeaderJson {
            tileset: s.tileset.to_string(),
            music: s.music.to_string(),
            connection_flags: s.connection_flags,
            width: s.width,
            height: s.height,
            border_block: s.border_block,
        }
    }
}

impl From<&'static StaticConnectionsJson> for ConnectionsJson {
    fn from(s: &'static StaticConnectionsJson) -> Self {
        ConnectionsJson {
            north: s.north.map(ConnectionEntryJson::from),
            south: s.south.map(ConnectionEntryJson::from),
            west: s.west.map(ConnectionEntryJson::from),
            east: s.east.map(ConnectionEntryJson::from),
        }
    }
}

impl From<&'static StaticConnectionEntryJson> for ConnectionEntryJson {
    fn from(s: &'static StaticConnectionEntryJson) -> Self {
        ConnectionEntryJson {
            target_map: s.target_map.to_string(),
            offset: s.offset,
        }
    }
}

impl From<&StaticWarpJson> for WarpJson {
    fn from(s: &StaticWarpJson) -> Self {
        WarpJson {
            x: s.x,
            y: s.y,
            dest_map: s.dest_map.map(str::to_string),
            dest_warp_id: s.dest_warp_id,
        }
    }
}

impl From<&StaticNpcJson> for NpcJson {
    fn from(s: &StaticNpcJson) -> Self {
        NpcJson {
            sprite_id: s.sprite_id,
            sprite_name: s.sprite_name.map(str::to_string),
            x: s.x,
            y: s.y,
            movement: s.movement.to_string(),
            facing: s.facing.to_string(),
            range: s.range,
            text_id: s.text_id,
            is_trainer: s.is_trainer,
            trainer_class: s.trainer_class.map(str::to_string),
            trainer_set: s.trainer_set,
            item_id: s.item_id,
            end_battle_text: s.end_battle_text.map(str::to_string),
        }
    }
}

impl From<&'static StaticMapTextJson> for MapTextJson {
    fn from(s: &'static StaticMapTextJson) -> Self {
        let convert_section = |entries: &'static [(&'static str, &'static [StaticTextPageJson])]| {
            entries
                .iter()
                .map(|(key, pages)| {
                    (
                        key.to_string(),
                        pages.iter().map(TextPageJson::from).collect(),
                    )
                })
                .collect()
        };
        MapTextJson {
            npc: convert_section(s.npc),
            sign: convert_section(s.sign),
        }
    }
}

impl From<&StaticTextPageJson> for TextPageJson {
    fn from(s: &StaticTextPageJson) -> Self {
        TextPageJson {
            line1: s.line1.to_string(),
            line2: s.line2.to_string(),
        }
    }
}

impl From<&'static StaticWildDataJson> for WildDataJson {
    fn from(s: &'static StaticWildDataJson) -> Self {
        WildDataJson {
            red: s.red.map(VersionWildJson::from),
            blue: s.blue.map(VersionWildJson::from),
        }
    }
}

impl From<&'static StaticVersionWildJson> for VersionWildJson {
    fn from(s: &'static StaticVersionWildJson) -> Self {
        VersionWildJson {
            grass: WildEncounterTableJson::from(&s.grass),
            water: WildEncounterTableJson::from(&s.water),
        }
    }
}

impl From<&StaticWildEncounterTableJson> for WildEncounterTableJson {
    fn from(s: &StaticWildEncounterTableJson) -> Self {
        WildEncounterTableJson {
            encounter_rate: s.encounter_rate,
            mons: s.mons.iter().map(WildMonJson::from).collect(),
        }
    }
}

impl From<&StaticWildMonJson> for WildMonJson {
    fn from(s: &StaticWildMonJson) -> Self {
        WildMonJson {
            level: s.level,
            species: s.species.to_string(),
        }
    }
}
