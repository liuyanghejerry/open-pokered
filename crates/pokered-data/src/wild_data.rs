//! Wild encounter data for all maps.
//!
//! `wild_data()` and `wild_data_for_map()` bodies are generated at build time
//! from `maps/{Name}/map.json` by `build.rs::generate_wild_data()`.
//! Fishing data (`good_rod_data`, `super_rod_*`) remains hand-coded since it
//! lives outside the per-map JSON schema.
//!
//! The browser editor's WYSIWYG playtest can inject *runtime* overrides
//! ([`set_wild_data_override`]) which shadow the build-time tables — a saved
//! wild-encounter edit shows up in the running game without a rebuild.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::map_names::{map_to_name_id, MapNameId};
use crate::maps::MapId;
use crate::species::Species;
use serde::{Deserialize, Serialize};

/// Owned form of [`MapWildData`] for editor-injected overrides (the static
/// variant's `name` is `&'static str`, which can't be deserialized from JSON).
#[derive(Debug, Clone)]
pub struct MapWildDataOverride {
    pub name: String,
    pub grass: WildEncounterTable,
    pub water: WildEncounterTable,
}

/// Runtime wild-encounter overrides, keyed by `"{Version}:{MapName}"`
/// (`"Red:Route1"`). Populated via [`set_wild_data_override`]; empty by
/// default so the build-time generated tables are always the baseline.
static WILD_OVERRIDES: OnceLock<Mutex<HashMap<String, MapWildDataOverride>>> = OnceLock::new();

fn wild_overrides() -> &'static Mutex<HashMap<String, MapWildDataOverride>> {
    WILD_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Inject a wild-encounter override for a map, parsed from the editor's
/// `map.json` `wild` block shape (camelCase, both versions):
///
/// ```json
/// { "red":  { "grass": { "encounterRate": 25, "mons": [{"level":3,"species":"PIDGEY"}] }, "water": { ... } },
///   "blue": { "grass": { ... }, "water": { ... } } }
/// ```
///
/// Species names accept SCREAMING_SNAKE or PascalCase (see
/// [`Species::from_scene_name`]). Returns `false` when the JSON can't be
/// parsed or contains no usable tables — the override is left unchanged.
pub fn set_wild_data_override(map_name: &str, json: &str) -> bool {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut inserted = false;
    let mut overrides = wild_overrides().lock().unwrap();
    for (version_key, version_obj) in [("red", &value["red"]), ("blue", &value["blue"])] {
        let Some(table) = parse_table_json(version_obj.get("grass")) else {
            continue;
        };
        let Some(water) = parse_table_json(version_obj.get("water")) else {
            continue;
        };
        let key = format!("{}:{}", version_key_owned(version_key), map_name);
        overrides.insert(
            key,
            MapWildDataOverride {
                name: map_name.to_string(),
                grass: table,
                water,
            },
        );
        inserted = true;
    }
    inserted
}

fn version_key_owned(v: &str) -> String {
    match v {
        "red" => "Red".to_string(),
        _ => "Blue".to_string(),
    }
}

fn parse_table_json(v: Option<&serde_json::Value>) -> Option<WildEncounterTable> {
    let v = v?;
    let encounter_rate = v.get("encounterRate")?.as_u64()? as u8;
    let mons = v.get("mons")?.as_array()?;
    let mut parsed = Vec::with_capacity(mons.len());
    for m in mons {
        let level = m.get("level")?.as_u64()? as u8;
        let species = Species::from_scene_name(m.get("species")?.as_str()?)?;
        parsed.push(WildMon { level, species });
    }
    Some(WildEncounterTable {
        encounter_rate,
        mons: parsed,
    })
}

/// Remove every editor-injected wild override (used when the editor discards
/// its playtest session).
pub fn clear_wild_data_overrides() {
    wild_overrides().lock().unwrap().clear();
}

/// `true` when a runtime override is currently active for the given map.
pub fn has_wild_data_override(map_name: &str) -> bool {
    let overrides = wild_overrides().lock().unwrap();
    overrides.keys().any(|k| k.ends_with(&format!(":{map_name}")))
}

/// Encounter slot probabilities (out of 256)
/// 10 slots with decreasing probability
pub const ENCOUNTER_SLOT_CHANCES: [u8; 10] = [51, 51, 39, 25, 25, 25, 13, 13, 11, 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WildMon {
    pub level: u8,
    pub species: Species,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WildEncounterTable {
    pub encounter_rate: u8,
    pub mons: Vec<WildMon>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapWildData {
    pub name: &'static str,
    pub grass: WildEncounterTable,
    pub water: WildEncounterTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameVersion {
    Red,
    Blue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FishingGroup {
    pub mons: Vec<WildMon>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperRodMapEntry {
    pub map_name: &'static str,
    pub group_index: usize,
}


pub fn wild_data(version: GameVersion) -> Vec<MapWildData> {
    match version {
        GameVersion::Red => include!(concat!(env!("OUT_DIR"), "/wild_data_red_gen.rs")),
        GameVersion::Blue => include!(concat!(env!("OUT_DIR"), "/wild_data_blue_gen.rs")),
    }
}

/// Good Rod encounters (same for all maps, random choice of these)
pub fn good_rod_data() -> Vec<WildMon> {
    vec![
        WildMon {
            level: 10,
            species: Species::Goldeen,
        },
        WildMon {
            level: 10,
            species: Species::Poliwag,
        },
    ]
}

/// Super Rod fishing groups
pub fn super_rod_groups() -> Vec<FishingGroup> {
    vec![
        // Group1
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 15,
                    species: Species::Tentacool,
                },
                WildMon {
                    level: 15,
                    species: Species::Poliwag,
                },
            ],
        },
        // Group2
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 15,
                    species: Species::Goldeen,
                },
                WildMon {
                    level: 15,
                    species: Species::Poliwag,
                },
            ],
        },
        // Group3
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 15,
                    species: Species::Psyduck,
                },
                WildMon {
                    level: 15,
                    species: Species::Goldeen,
                },
                WildMon {
                    level: 15,
                    species: Species::Krabby,
                },
            ],
        },
        // Group4
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 15,
                    species: Species::Krabby,
                },
                WildMon {
                    level: 15,
                    species: Species::Shellder,
                },
            ],
        },
        // Group5
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 23,
                    species: Species::Poliwhirl,
                },
                WildMon {
                    level: 15,
                    species: Species::Slowpoke,
                },
            ],
        },
        // Group6
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 15,
                    species: Species::Dratini,
                },
                WildMon {
                    level: 15,
                    species: Species::Krabby,
                },
                WildMon {
                    level: 15,
                    species: Species::Psyduck,
                },
                WildMon {
                    level: 15,
                    species: Species::Slowpoke,
                },
            ],
        },
        // Group7
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 5,
                    species: Species::Tentacool,
                },
                WildMon {
                    level: 15,
                    species: Species::Krabby,
                },
                WildMon {
                    level: 15,
                    species: Species::Goldeen,
                },
                WildMon {
                    level: 15,
                    species: Species::Magikarp,
                },
            ],
        },
        // Group8
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 15,
                    species: Species::Staryu,
                },
                WildMon {
                    level: 15,
                    species: Species::Horsea,
                },
                WildMon {
                    level: 15,
                    species: Species::Shellder,
                },
                WildMon {
                    level: 15,
                    species: Species::Goldeen,
                },
            ],
        },
        // Group9
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 23,
                    species: Species::Slowbro,
                },
                WildMon {
                    level: 23,
                    species: Species::Seaking,
                },
                WildMon {
                    level: 23,
                    species: Species::Kingler,
                },
                WildMon {
                    level: 23,
                    species: Species::Seadra,
                },
            ],
        },
        // Group10
        FishingGroup {
            mons: vec![
                WildMon {
                    level: 23,
                    species: Species::Seaking,
                },
                WildMon {
                    level: 15,
                    species: Species::Krabby,
                },
                WildMon {
                    level: 15,
                    species: Species::Goldeen,
                },
                WildMon {
                    level: 15,
                    species: Species::Magikarp,
                },
            ],
        },
    ]
}

/// Super Rod map-to-group mappings
pub fn super_rod_map_entries() -> Vec<SuperRodMapEntry> {
    vec![
        SuperRodMapEntry {
            map_name: "PALLET_TOWN",
            group_index: 0,
        },
        SuperRodMapEntry {
            map_name: "VIRIDIAN_CITY",
            group_index: 0,
        },
        SuperRodMapEntry {
            map_name: "CERULEAN_CITY",
            group_index: 2,
        },
        SuperRodMapEntry {
            map_name: "VERMILION_CITY",
            group_index: 3,
        },
        SuperRodMapEntry {
            map_name: "CELADON_CITY",
            group_index: 4,
        },
        SuperRodMapEntry {
            map_name: "FUCHSIA_CITY",
            group_index: 9,
        },
        SuperRodMapEntry {
            map_name: "CINNABAR_ISLAND",
            group_index: 7,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_4",
            group_index: 2,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_6",
            group_index: 3,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_10",
            group_index: 4,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_11",
            group_index: 3,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_12",
            group_index: 6,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_13",
            group_index: 6,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_17",
            group_index: 6,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_18",
            group_index: 6,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_19",
            group_index: 7,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_20",
            group_index: 7,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_21",
            group_index: 7,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_22",
            group_index: 1,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_23",
            group_index: 8,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_24",
            group_index: 2,
        },
        SuperRodMapEntry {
            map_name: "ROUTE_25",
            group_index: 2,
        },
        SuperRodMapEntry {
            map_name: "CERULEAN_GYM",
            group_index: 2,
        },
        SuperRodMapEntry {
            map_name: "VERMILION_DOCK",
            group_index: 3,
        },
        SuperRodMapEntry {
            map_name: "SEAFOAM_ISLANDS_B3F",
            group_index: 7,
        },
        SuperRodMapEntry {
            map_name: "SEAFOAM_ISLANDS_B4F",
            group_index: 7,
        },
        SuperRodMapEntry {
            map_name: "SAFARI_ZONE_EAST",
            group_index: 5,
        },
        SuperRodMapEntry {
            map_name: "SAFARI_ZONE_NORTH",
            group_index: 5,
        },
        SuperRodMapEntry {
            map_name: "SAFARI_ZONE_WEST",
            group_index: 5,
        },
        SuperRodMapEntry {
            map_name: "SAFARI_ZONE_CENTER",
            group_index: 5,
        },
        SuperRodMapEntry {
            map_name: "CERULEAN_CAVE_2F",
            group_index: 8,
        },
        SuperRodMapEntry {
            map_name: "CERULEAN_CAVE_B1F",
            group_index: 8,
        },
        SuperRodMapEntry {
            map_name: "CERULEAN_CAVE_1F",
            group_index: 8,
        },
    ]
}

/// Super Rod map-to-group lookup keyed by `MapId` — the same table as
/// [`super_rod_map_entries`] (data/wild/super_rod.asm `SuperRodData`), in a
/// form the fishing logic can use without string conversion.
///
/// Returns the index into [`super_rod_groups`], or `None` when the map has no
/// fishing group (the original's `ReadSuperRodData` reports `$2` — "no
/// fishing groups found" — which prints "Looks like there's nothing here.").
pub fn super_rod_group_index_for_map(map: MapId) -> Option<usize> {
    Some(match map {
        // .Group1
        MapId::PalletTown | MapId::ViridianCity => 0,
        // .Group2
        MapId::Route22 => 1,
        // .Group3
        MapId::CeruleanCity
        | MapId::Route4
        | MapId::Route24
        | MapId::Route25
        | MapId::CeruleanGym => 2,
        // .Group4
        MapId::VermilionCity | MapId::Route6 | MapId::Route11 | MapId::VermilionDock => 3,
        // .Group5
        MapId::CeladonCity | MapId::Route10 => 4,
        // .Group6
        MapId::SafariZoneEast
        | MapId::SafariZoneNorth
        | MapId::SafariZoneWest
        | MapId::SafariZoneCenter => 5,
        // .Group7
        MapId::Route12 | MapId::Route13 | MapId::Route17 | MapId::Route18 => 6,
        // .Group8
        MapId::CinnabarIsland
        | MapId::Route19
        | MapId::Route20
        | MapId::Route21
        | MapId::SeafoamIslandsB3F
        | MapId::SeafoamIslandsB4F => 7,
        // .Group9
        MapId::Route23 | MapId::CeruleanCave2F | MapId::CeruleanCaveB1F | MapId::CeruleanCave1F => 8,
        // .Group10
        MapId::FuchsiaCity => 9,
        _ => return None,
    })
}

/// Look up wild encounter data for a specific map and game version.
///
/// Returns None if the map has no wild encounters.
///
/// # Example
/// ```
/// use pokered_data::maps::MapId;
/// use pokered_data::wild_data::{wild_data_for_map, GameVersion};
///
/// let data = wild_data_for_map(MapId::Route1, GameVersion::Red);
/// assert!(data.is_some());
/// assert_eq!(data.unwrap().name, "Route1");
///
/// let no_data = wild_data_for_map(MapId::PalletTown, GameVersion::Red);
/// assert!(no_data.is_none());
/// ```
pub fn wild_data_for_map(map_id: MapId, version: GameVersion) -> Option<MapWildData> {
    // Editor-injected runtime override shadows the build-time generated table
    // for this (version, map) pair. `name` is leaked once per injected
    // override — bounded by the editor's injected map count, and only on
    // builds that actually use the override feature.
    let key = format!("{:?}:{:?}", version, map_id);
    if let Some(ov) = wild_overrides().lock().unwrap().get(&key) {
        let name: &'static str = Box::leak(ov.name.clone().into_boxed_str());
        return Some(MapWildData {
            name,
            grass: ov.grass.clone(),
            water: ov.water.clone(),
        });
    }
    include!(concat!(env!("OUT_DIR"), "/wild_data_for_map_gen.rs"))
}

/// Map the `name` field of a [`MapWildData`] (the map directory name, e.g.
/// `"Route1"` — generated from `maps/{Name}/map.json`) back to its [`MapId`].
pub fn map_id_for_wild_table_name(name: &str) -> Option<MapId> {
    include!(concat!(env!("OUT_DIR"), "/wild_map_id_gen.rs"))
}

/// Wild-encounter locations of a species, in the order the Pokédex AREA page
/// shows them.
///
/// Ports `FindWildLocationsOfMon` (engine/items/item_effects.asm) — which
/// scans every map's *land* (grass) and *water* encounter tables for the
/// species (10 slots each, `NUM_WILDMONS`) — followed by the nest-icon
/// placement of `DisplayWildLocations` (engine/items/town_map.asm), which
/// skips the location whose town-map coordinates are $19 (y=1, x=9):
/// Cerulean Cave. Its species (Kadabra, Hypno, Electrode, Rhydon, Raichu,
/// Wigglytuff, Dodrio in Red) therefore report "AREA UNKNOWN", exactly like
/// species with no wild encounters at all (starters, trade/gift-only mons,
/// fossils, legendaries, fishing-only mons such as Magikarp).
///
/// Only grass+water tables are considered — roof/tree encounter groups and
/// the fishing rod tables (`FishingGroup`) are not part of the original's
/// AREA scan.
///
/// Returns the maps sorted by map ID (the original iterates `WildDataPointers`
/// in map-ID order).
pub fn area_locations(species: Species, version: GameVersion) -> Vec<MapId> {
    let mut maps: Vec<MapId> = wild_data(version)
        .into_iter()
        .filter(|m| {
            m.grass.mons.iter().any(|w| w.species == species)
                || m.water.mons.iter().any(|w| w.species == species)
        })
        .filter_map(|m| map_id_for_wild_table_name(m.name))
        // DisplayWildLocations skips the $19-coords nest (Cerulean Cave).
        .filter(|m| map_to_name_id(*m) != MapNameId::CeruleanCave)
        .collect();
    maps.sort_by_key(|m| *m as u8);
    maps.dedup();
    maps
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `MapId` for each name in [`super_rod_map_entries`] (test-side mirror of
    /// the asm `SuperRodData` index, used to prove the two representations
    /// of the same table agree).
    fn map_for_entry_name(name: &str) -> MapId {
        match name {
            "PALLET_TOWN" => MapId::PalletTown,
            "VIRIDIAN_CITY" => MapId::ViridianCity,
            "CERULEAN_CITY" => MapId::CeruleanCity,
            "VERMILION_CITY" => MapId::VermilionCity,
            "CELADON_CITY" => MapId::CeladonCity,
            "FUCHSIA_CITY" => MapId::FuchsiaCity,
            "CINNABAR_ISLAND" => MapId::CinnabarIsland,
            "ROUTE_4" => MapId::Route4,
            "ROUTE_6" => MapId::Route6,
            "ROUTE_10" => MapId::Route10,
            "ROUTE_11" => MapId::Route11,
            "ROUTE_12" => MapId::Route12,
            "ROUTE_13" => MapId::Route13,
            "ROUTE_17" => MapId::Route17,
            "ROUTE_18" => MapId::Route18,
            "ROUTE_19" => MapId::Route19,
            "ROUTE_20" => MapId::Route20,
            "ROUTE_21" => MapId::Route21,
            "ROUTE_22" => MapId::Route22,
            "ROUTE_23" => MapId::Route23,
            "ROUTE_24" => MapId::Route24,
            "ROUTE_25" => MapId::Route25,
            "CERULEAN_GYM" => MapId::CeruleanGym,
            "VERMILION_DOCK" => MapId::VermilionDock,
            "SEAFOAM_ISLANDS_B3F" => MapId::SeafoamIslandsB3F,
            "SEAFOAM_ISLANDS_B4F" => MapId::SeafoamIslandsB4F,
            "SAFARI_ZONE_EAST" => MapId::SafariZoneEast,
            "SAFARI_ZONE_NORTH" => MapId::SafariZoneNorth,
            "SAFARI_ZONE_WEST" => MapId::SafariZoneWest,
            "SAFARI_ZONE_CENTER" => MapId::SafariZoneCenter,
            "CERULEAN_CAVE_2F" => MapId::CeruleanCave2F,
            "CERULEAN_CAVE_B1F" => MapId::CeruleanCaveB1F,
            "CERULEAN_CAVE_1F" => MapId::CeruleanCave1F,
            other => panic!("unmapped SuperRodData name: {other}"),
        }
    }

    /// data/wild/super_rod.asm `SuperRodData`: both representations of the
    /// map→group table must agree, entry for entry.
    #[test]
    fn super_rod_group_index_matches_name_keyed_table() {
        let entries = super_rod_map_entries();
        assert_eq!(entries.len(), 33, "asm SuperRodData has 33 map entries");
        for entry in &entries {
            let map = map_for_entry_name(entry.map_name);
            assert_eq!(
                super_rod_group_index_for_map(map),
                Some(entry.group_index),
                "{}",
                entry.map_name
            );
        }
    }

    /// Spot-checks against the asm (`dbw MAP, .GroupN` pairs).
    #[test]
    fn super_rod_group_index_spot_checks() {
        assert_eq!(super_rod_group_index_for_map(MapId::PalletTown), Some(0)); // .Group1
        assert_eq!(super_rod_group_index_for_map(MapId::Route22), Some(1)); // .Group2
        assert_eq!(super_rod_group_index_for_map(MapId::CeruleanCity), Some(2)); // .Group3
        assert_eq!(super_rod_group_index_for_map(MapId::VermilionDock), Some(3)); // .Group4
        assert_eq!(super_rod_group_index_for_map(MapId::Route12), Some(6)); // .Group7
        assert_eq!(super_rod_group_index_for_map(MapId::SafariZoneCenter), Some(5)); // .Group6
        assert_eq!(super_rod_group_index_for_map(MapId::CeruleanCave1F), Some(8)); // .Group9
        assert_eq!(super_rod_group_index_for_map(MapId::FuchsiaCity), Some(9)); // .Group10
    }

    /// Maps absent from `SuperRodData` (e.g. Route 1) have no fishing group.
    #[test]
    fn super_rod_group_index_none_for_unlisted_maps() {
        assert_eq!(super_rod_group_index_for_map(MapId::Route1), None);
        assert_eq!(super_rod_group_index_for_map(MapId::RedsHouse1F), None);
    }

    /// data/wild/good_rod.asm `GoodRodMons`: 10 GOLDEEN / 10 POLIWAG.
    #[test]
    fn good_rod_table_matches_asm() {
        let mons = good_rod_data();
        assert_eq!(mons.len(), 2);
        assert_eq!(mons[0], WildMon { level: 10, species: Species::Goldeen });
        assert_eq!(mons[1], WildMon { level: 10, species: Species::Poliwag });
    }

    /// data/wild/super_rod.asm `.Group7` (Routes 12/13/17/18):
    /// 5 TENTACOOL / 15 KRABBY / 15 GOLDEEN / 15 MAGIKARP.
    #[test]
    fn super_rod_group7_matches_asm() {
        let group = &super_rod_groups()[6];
        assert_eq!(
            group.mons,
            vec![
                WildMon { level: 5, species: Species::Tentacool },
                WildMon { level: 15, species: Species::Krabby },
                WildMon { level: 15, species: Species::Goldeen },
                WildMon { level: 15, species: Species::Magikarp },
            ]
        );
    }

    /// data/wild/super_rod.asm `.Group9` (Route 23, Cerulean Cave): all L23.
    #[test]
    fn super_rod_group9_matches_asm() {
        let group = &super_rod_groups()[8];
        assert_eq!(
            group.mons,
            vec![
                WildMon { level: 23, species: Species::Slowbro },
                WildMon { level: 23, species: Species::Seaking },
                WildMon { level: 23, species: Species::Kingler },
                WildMon { level: 23, species: Species::Seadra },
            ]
        );
    }

    /// Every `map.json` wild-table map resolves back to a `MapId`.
    #[test]
    fn all_wild_table_names_resolve_to_map_ids() {
        for m in wild_data(GameVersion::Red) {
            assert!(
                map_id_for_wild_table_name(m.name).is_some(),
                "no MapId for wild table map {}",
                m.name
            );
        }
        assert_eq!(map_id_for_wild_table_name("Route1"), Some(MapId::Route1));
        assert_eq!(map_id_for_wild_table_name("MtMoonB2F"), Some(MapId::MtMoonB2F));
        assert_eq!(map_id_for_wild_table_name("Nope"), None);
    }

    /// Reference table for `area_locations` (Red), generated from the original
    /// disassembly by parsing `WildDataPointers` + `data/wild/maps/*.asm`:
    /// each species' map list = union of the grass and water tables containing
    /// it, minus Cerulean Cave (whose nest icon `DisplayWildLocations` skips
    /// — town-map coords $19). The two tables agree only if the Rust wild
    /// data (from `maps/*/map.json`) is faithful to the asm.
    ///
    /// `MapId` values below are the asm map numbers (Route1=12, Route19=30,
    /// Route21=32, ViridianForest=51, MtMoon1F=59, RockTunnel1F=82,
    /// PowerPlant=83, PokemonTower3F=144, SeafoamIslandsB1F=159,
    /// SeafoamIslands1F=192, DiglettsCave=197, SafariZoneEast=217,
    /// SafariZoneNorth=218, SafariZoneWest=219, SafariZoneCenter=220,
    /// CeruleanCave2F=226, CeruleanCaveB1F=227, CeruleanCave1F=228).
    const RED_AREA_EXPECTED: [(u8, &[u8]); 151] = [
        //   1 Bulbasaur
        (1, &[]),
        //   2 Ivysaur
        (2, &[]),
        //   3 Venusaur
        (3, &[]),
        //   4 Charmander
        (4, &[]),
        //   5 Charmeleon
        (5, &[]),
        //   6 Charizard
        (6, &[]),
        //   7 Squirtle
        (7, &[]),
        //   8 Wartortle
        (8, &[]),
        //   9 Blastoise
        (9, &[]),
        //  10 Caterpie
        (10, &[36, 51]),
        //  11 Metapod
        (11, &[36, 51]),
        //  12 Butterfree
        (12, &[]),
        //  13 Weedle
        (13, &[13, 35, 36, 51]),
        //  14 Kakuna
        (14, &[35, 36, 51]),
        //  15 Beedrill
        (15, &[]),
        //  16 Pidgey
        (16, &[12, 13, 14, 16, 17, 18, 19, 23, 24, 25, 26, 32, 35, 36]),
        //  17 Pidgeotto
        (17, &[25, 26, 32]),
        //  18 Pidgeot
        (18, &[]),
        //  19 Rattata
        (19, &[12, 13, 15, 20, 27, 32, 33]),
        //  20 Raticate
        (20, &[27, 28, 29, 32]),
        //  21 Spearow
        (21, &[14, 15, 20, 21, 22, 27, 28, 29, 33, 34]),
        //  22 Fearow
        (22, &[28, 29, 34]),
        //  23 Ekans
        (23, &[15, 19, 20, 21, 22, 34]),
        //  24 Arbok
        (24, &[34]),
        //  25 Pikachu
        (25, &[51, 83]),
        //  26 Raichu
        (26, &[]),
        //  27 Sandshrew
        (27, &[]),
        //  28 Sandslash
        (28, &[]),
        //  29 NidoranF
        (29, &[33, 217, 219]),
        //  30 Nidorina
        (30, &[218, 220]),
        //  31 Nidoqueen
        (31, &[]),
        //  32 NidoranM
        (32, &[33, 217, 218, 219, 220]),
        //  33 Nidorino
        (33, &[217, 218, 219, 220]),
        //  34 Nidoking
        (34, &[]),
        //  35 Clefairy
        (35, &[59, 60, 61]),
        //  36 Clefable
        (36, &[]),
        //  37 Vulpix
        (37, &[]),
        //  38 Ninetales
        (38, &[]),
        //  39 Jigglypuff
        (39, &[14]),
        //  40 Wigglytuff
        (40, &[]),
        //  41 Zubat
        (41, &[59, 60, 61, 82, 108, 192, 194, 198, 232]),
        //  42 Golbat
        (42, &[108, 160, 162, 192, 194, 198]),
        //  43 Oddish
        (43, &[16, 17, 18, 23, 24, 25, 26, 35, 36]),
        //  44 Gloom
        (44, &[23, 24, 25, 26]),
        //  45 Vileplume
        (45, &[]),
        //  46 Paras
        (46, &[59, 60, 61, 217, 218]),
        //  47 Parasect
        (47, &[217, 220]),
        //  48 Venonat
        (48, &[23, 24, 25, 26, 219, 220]),
        //  49 Venomoth
        (49, &[198, 218, 219]),
        //  50 Diglett
        (50, &[197]),
        //  51 Dugtrio
        (51, &[197]),
        //  52 Meowth
        (52, &[]),
        //  53 Persian
        (53, &[]),
        //  54 Psyduck
        (54, &[192]),
        //  55 Golduck
        (55, &[192]),
        //  56 Mankey
        (56, &[16, 17, 18, 19]),
        //  57 Primeape
        (57, &[]),
        //  58 Growlithe
        (58, &[18, 19, 165, 214, 215, 216]),
        //  59 Arcanine
        (59, &[]),
        //  60 Poliwag
        (60, &[]),
        //  61 Poliwhirl
        (61, &[]),
        //  62 Poliwrath
        (62, &[]),
        //  63 Abra
        (63, &[35, 36]),
        //  64 Kadabra
        (64, &[]),
        //  65 Alakazam
        (65, &[]),
        //  66 Machop
        (66, &[82, 108, 194, 198, 232]),
        //  67 Machoke
        (67, &[108, 194, 198]),
        //  68 Machamp
        (68, &[]),
        //  69 Bellsprout
        (69, &[]),
        //  70 Weepinbell
        (70, &[]),
        //  71 Victreebel
        (71, &[]),
        //  72 Tentacool
        (72, &[30, 31, 32]),
        //  73 Tentacruel
        (73, &[]),
        //  74 Geodude
        (74, &[59, 60, 61, 82, 108, 194, 198, 232]),
        //  75 Graveler
        (75, &[108, 194, 198]),
        //  76 Golem
        (76, &[]),
        //  77 Ponyta
        (77, &[165, 214, 215, 216]),
        //  78 Rapidash
        (78, &[]),
        //  79 Slowpoke
        (79, &[159, 160, 161, 162, 192]),
        //  80 Slowbro
        (80, &[160, 162]),
        //  81 Magnemite
        (81, &[83]),
        //  82 Magneton
        (82, &[83]),
        //  83 Farfetchd
        (83, &[]),
        //  84 Doduo
        (84, &[27, 28, 29, 217, 219]),
        //  85 Dodrio
        (85, &[]),
        //  86 Seel
        (86, &[159, 160, 161, 162, 192]),
        //  87 Dewgong
        (87, &[159, 161]),
        //  88 Grimer
        (88, &[165, 214, 215, 216]),
        //  89 Muk
        (89, &[165, 214, 215, 216]),
        //  90 Shellder
        (90, &[159, 160, 161, 162, 192]),
        //  91 Cloyster
        (91, &[]),
        //  92 Gastly
        (92, &[144, 145, 146, 147, 148]),
        //  93 Haunter
        (93, &[144, 145, 146, 147, 148]),
        //  94 Gengar
        (94, &[]),
        //  95 Onix
        (95, &[82, 108, 194, 198, 232]),
        //  96 Drowzee
        (96, &[22]),
        //  97 Hypno
        (97, &[]),
        //  98 Krabby
        (98, &[]),
        //  99 Kingler
        (99, &[]),
        // 100 Voltorb
        (100, &[21, 83]),
        // 101 Electrode
        (101, &[]),
        // 102 Exeggcute
        (102, &[217, 218, 219, 220]),
        // 103 Exeggutor
        (103, &[]),
        // 104 Cubone
        (104, &[144, 145, 146, 147, 148]),
        // 105 Marowak
        (105, &[108, 194]),
        // 106 Hitmonlee
        (106, &[]),
        // 107 Hitmonchan
        (107, &[]),
        // 108 Lickitung
        (108, &[]),
        // 109 Koffing
        (109, &[165, 214, 215, 216]),
        // 110 Weezing
        (110, &[165, 214, 215, 216]),
        // 111 Rhyhorn
        (111, &[218, 220]),
        // 112 Rhydon
        (112, &[]),
        // 113 Chansey
        (113, &[218, 220]),
        // 114 Tangela
        (114, &[32]),
        // 115 Kangaskhan
        (115, &[217, 219]),
        // 116 Horsea
        (116, &[159, 160, 161, 162, 192]),
        // 117 Seadra
        (117, &[159, 161]),
        // 118 Goldeen
        (118, &[]),
        // 119 Seaking
        (119, &[]),
        // 120 Staryu
        (120, &[159, 160]),
        // 121 Starmie
        (121, &[]),
        // 122 MrMime
        (122, &[]),
        // 123 Scyther
        (123, &[217, 220]),
        // 124 Jynx
        (124, &[]),
        // 125 Electabuzz
        (125, &[83]),
        // 126 Magmar
        (126, &[]),
        // 127 Pinsir
        (127, &[]),
        // 128 Tauros
        (128, &[218, 219]),
        // 129 Magikarp
        (129, &[]),
        // 130 Gyarados
        (130, &[]),
        // 131 Lapras
        (131, &[]),
        // 132 Ditto
        (132, &[24, 25, 26, 34]),
        // 133 Eevee
        (133, &[]),
        // 134 Vaporeon
        (134, &[]),
        // 135 Jolteon
        (135, &[]),
        // 136 Flareon
        (136, &[]),
        // 137 Porygon
        (137, &[]),
        // 138 Omanyte
        (138, &[]),
        // 139 Omastar
        (139, &[]),
        // 140 Kabuto
        (140, &[]),
        // 141 Kabutops
        (141, &[]),
        // 142 Aerodactyl
        (142, &[]),
        // 143 Snorlax
        (143, &[]),
        // 144 Articuno
        (144, &[]),
        // 145 Zapdos
        (145, &[]),
        // 146 Moltres
        (146, &[]),
        // 147 Dratini
        (147, &[]),
        // 148 Dragonair
        (148, &[]),
        // 149 Dragonite
        (149, &[]),
        // 150 Mewtwo
        (150, &[]),
        // 151 Mew
        (151, &[]),
    ];

    /// The full Red area table must match the asm-derived reference — every
    /// species, every map (`FindWildLocationsOfMon` + the Cerulean Cave
    /// exclusion in `DisplayWildLocations`).
    #[test]
    fn area_locations_matches_asm_for_all_red_species() {
        for (dex, expected) in RED_AREA_EXPECTED {
            let species = Species::from_index_id(dex);
            let got: Vec<u8> = area_locations(species, GameVersion::Red)
                .into_iter()
                .map(|m| m as u8)
                .collect();
            assert_eq!(
                got, expected,
                "area_locations({}) mismatch vs asm",
                species.pascal_name()
            );
        }
    }

    /// Area lists are sorted, deduplicated, and never contain Cerulean Cave.
    #[test]
    fn area_locations_are_sorted_unique_and_exclude_cerulean_cave() {
        for version in [GameVersion::Red, GameVersion::Blue] {
            for dex in 1..=151u8 {
                let maps = area_locations(Species::from_index_id(dex), version);
                let mut sorted = maps.clone();
                sorted.sort_by_key(|m| *m as u8);
                sorted.dedup();
                assert_eq!(maps, sorted, "unsorted/duplicate area list");
                for m in &maps {
                    assert_ne!(
                        map_to_name_id(*m),
                        MapNameId::CeruleanCave,
                        "Cerulean Cave must not appear in area lists"
                    );
                }
            }
        }
    }

    /// Spot checks against the asm (see the reference table above):
    /// grass+water union, fishing-only mons, and the Cerulean Cave exclusion.
    #[test]
    fn area_locations_spot_checks() {
        use GameVersion::Red as R;
        // Pidgey: grass on Routes 1-3, 5-8, 12-15, 21, 24, 25 (Red).
        assert_eq!(
            area_locations(Species::Pidgey, R),
            vec![
                MapId::Route1, MapId::Route2, MapId::Route3, MapId::Route5,
                MapId::Route6, MapId::Route7, MapId::Route8, MapId::Route12,
                MapId::Route13, MapId::Route14, MapId::Route15, MapId::Route21,
                MapId::Route24, MapId::Route25,
            ]
        );
        // Tentacool: water on Routes 19/20/21 (SeaRoutes + Route 21).
        assert_eq!(
            area_locations(Species::Tentacool, R),
            vec![MapId::Route19, MapId::Route20, MapId::Route21]
        );
        // Ditto: Routes 13/14/15/23 grass — Cerulean Cave is skipped, so the
        // original's AREA page shows "AREA UNKNOWN"-free locations but not the
        // cave. (Ditto IS findable in Red — Routes 13-15 and 23.)
        assert_eq!(
            area_locations(Species::Ditto, R),
            vec![
                MapId::Route13, MapId::Route14, MapId::Route15, MapId::Route23
            ]
        );
        // Kadabra: only in Cerulean Cave → no locations ("AREA UNKNOWN").
        assert!(area_locations(Species::Kadabra, R).is_empty());
        // Fishing-only (Magikarp), starter (Bulbasaur) and event-only (Mewtwo,
        // Mew) species have no grass/water tables → "AREA UNKNOWN".
        assert!(area_locations(Species::Magikarp, R).is_empty());
        assert!(area_locations(Species::Bulbasaur, R).is_empty());
        assert!(area_locations(Species::Mewtwo, R).is_empty());
        assert!(area_locations(Species::Mew, R).is_empty());
        // Zubat: Mt. Moon 1F/B1F/B2F, Rock Tunnel 1F/B1F, Victory Road 1F/2F,
        // Seafoam Islands 1F.
        assert_eq!(
            area_locations(Species::Zubat, R),
            vec![
                MapId::MtMoon1F, MapId::MtMoonB1F, MapId::MtMoonB2F,
                MapId::RockTunnel1F, MapId::VictoryRoad1F, MapId::SeafoamIslands1F,
                MapId::VictoryRoad2F, MapId::VictoryRoad3F, MapId::RockTunnelB1F,
            ]
        );
        // Chansey: Safari Zone North + Center (Red).
        assert_eq!(
            area_locations(Species::Chansey, R),
            vec![MapId::SafariZoneNorth, MapId::SafariZoneCenter]
        );
    }
}
