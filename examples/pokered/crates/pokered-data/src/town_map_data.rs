//! Town map data — coordinates for the town map screen display.
//!
//! From data/maps/town_map_entries.asm and data/maps/town_map_order.asm.
//! - 37 outdoor map entries (cities, routes)
//! - 60 indoor map group entries (buildings, dungeons)
//! - 48 town map order entries (fly destination ordering)

use crate::map_names::{map_to_name_id, MapNameId};
use crate::maps::MapId;

/// Town map coordinate entry for an outdoor map
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutdoorTownMapEntry {
    pub x: u8,
    pub y: u8,
    pub name: MapNameId,
}

/// Town map coordinate entry for an indoor map group
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndoorTownMapEntry {
    pub group_id: u8,
    pub x: u8,
    pub y: u8,
    pub name: MapNameId,
}

/// Number of outdoor map entries (PALLET_TOWN through ROUTE_25 = FIRST_INDOOR_MAP)
pub const NUM_OUTDOOR_ENTRIES: usize = 37;

/// Number of indoor map group entries
pub const NUM_INDOOR_ENTRIES: usize = 60;

/// Number of town map order entries (fly destinations)
pub const NUM_TOWN_MAP_ORDER: usize = 47;

/// Town map tilemap dimensions (full Game Boy screen, in 8×8 tiles).
pub const TOWN_MAP_WIDTH: usize = 20;
pub const TOWN_MAP_HEIGHT: usize = 18;

/// RLE-compressed 20×18 town-map tilemap (`gfx/town_map/town_map.rle`). Each
/// byte packs a tile index in the high nibble and a run length in the low nibble;
/// a `0x00` byte terminates the stream.
const TOWN_MAP_RLE: &[u8] = include_bytes!("../../../gfx/town_map/town_map.rle");

/// Decode the town map into `TOWN_MAP_WIDTH * TOWN_MAP_HEIGHT` tile indices
/// (row-major). Each index selects one of the 16 tiles in `town_map.png`.
pub fn decode_town_map_tilemap() -> Vec<u8> {
    let mut out = Vec::with_capacity(TOWN_MAP_WIDTH * TOWN_MAP_HEIGHT);
    for &b in TOWN_MAP_RLE {
        if b == 0 {
            break;
        }
        let tile = b >> 4;
        let run = b & 0x0f;
        for _ in 0..run {
            out.push(tile);
        }
    }
    out
}

/// Outdoor map entries — indexed by map ID (0x00..0x24)
/// From ExternalMapEntries in data/maps/town_map_entries.asm
pub const OUTDOOR_TOWN_MAP_ENTRIES: [OutdoorTownMapEntry; NUM_OUTDOOR_ENTRIES] = [
    OutdoorTownMapEntry {
        x: 2,
        y: 11,
        name: MapNameId::PalletTown,
    }, // PALLET_TOWN
    OutdoorTownMapEntry {
        x: 2,
        y: 8,
        name: MapNameId::ViridianCity,
    }, // VIRIDIAN_CITY
    OutdoorTownMapEntry {
        x: 2,
        y: 3,
        name: MapNameId::PewterCity,
    }, // PEWTER_CITY
    OutdoorTownMapEntry {
        x: 10,
        y: 2,
        name: MapNameId::CeruleanCity,
    }, // CERULEAN_CITY
    OutdoorTownMapEntry {
        x: 14,
        y: 5,
        name: MapNameId::LavenderTown,
    }, // LAVENDER_TOWN
    OutdoorTownMapEntry {
        x: 10,
        y: 9,
        name: MapNameId::VermilionCity,
    }, // VERMILION_CITY
    OutdoorTownMapEntry {
        x: 7,
        y: 5,
        name: MapNameId::CeladonCity,
    }, // CELADON_CITY
    OutdoorTownMapEntry {
        x: 8,
        y: 13,
        name: MapNameId::FuchsiaCity,
    }, // FUCHSIA_CITY
    OutdoorTownMapEntry {
        x: 2,
        y: 15,
        name: MapNameId::CinnabarIsland,
    }, // CINNABAR_ISLAND
    OutdoorTownMapEntry {
        x: 0,
        y: 2,
        name: MapNameId::IndigoPlateau,
    }, // INDIGO_PLATEAU
    OutdoorTownMapEntry {
        x: 10,
        y: 5,
        name: MapNameId::SaffronCity,
    }, // SAFFRON_CITY
    OutdoorTownMapEntry {
        x: 0,
        y: 0,
        name: MapNameId::PalletTown,
    }, // UNUSED_MAP_0B
    OutdoorTownMapEntry {
        x: 2,
        y: 10,
        name: MapNameId::Route1,
    }, // ROUTE_1
    OutdoorTownMapEntry {
        x: 2,
        y: 6,
        name: MapNameId::Route2,
    }, // ROUTE_2
    OutdoorTownMapEntry {
        x: 4,
        y: 3,
        name: MapNameId::Route3,
    }, // ROUTE_3
    OutdoorTownMapEntry {
        x: 8,
        y: 2,
        name: MapNameId::Route4,
    }, // ROUTE_4
    OutdoorTownMapEntry {
        x: 10,
        y: 3,
        name: MapNameId::Route5,
    }, // ROUTE_5
    OutdoorTownMapEntry {
        x: 10,
        y: 8,
        name: MapNameId::Route6,
    }, // ROUTE_6
    OutdoorTownMapEntry {
        x: 8,
        y: 5,
        name: MapNameId::Route7,
    }, // ROUTE_7
    OutdoorTownMapEntry {
        x: 13,
        y: 5,
        name: MapNameId::Route8,
    }, // ROUTE_8
    OutdoorTownMapEntry {
        x: 13,
        y: 2,
        name: MapNameId::Route9,
    }, // ROUTE_9
    OutdoorTownMapEntry {
        x: 14,
        y: 4,
        name: MapNameId::Route10,
    }, // ROUTE_10
    OutdoorTownMapEntry {
        x: 12,
        y: 9,
        name: MapNameId::Route11,
    }, // ROUTE_11
    OutdoorTownMapEntry {
        x: 14,
        y: 9,
        name: MapNameId::Route12,
    }, // ROUTE_12
    OutdoorTownMapEntry {
        x: 13,
        y: 11,
        name: MapNameId::Route13,
    }, // ROUTE_13
    OutdoorTownMapEntry {
        x: 11,
        y: 12,
        name: MapNameId::Route14,
    }, // ROUTE_14
    OutdoorTownMapEntry {
        x: 10,
        y: 13,
        name: MapNameId::Route15,
    }, // ROUTE_15
    OutdoorTownMapEntry {
        x: 5,
        y: 5,
        name: MapNameId::Route16,
    }, // ROUTE_16
    OutdoorTownMapEntry {
        x: 4,
        y: 8,
        name: MapNameId::Route17,
    }, // ROUTE_17
    OutdoorTownMapEntry {
        x: 6,
        y: 13,
        name: MapNameId::Route18,
    }, // ROUTE_18
    OutdoorTownMapEntry {
        x: 6,
        y: 15,
        name: MapNameId::SeaRoute19,
    }, // ROUTE_19
    OutdoorTownMapEntry {
        x: 4,
        y: 15,
        name: MapNameId::SeaRoute20,
    }, // ROUTE_20
    OutdoorTownMapEntry {
        x: 2,
        y: 13,
        name: MapNameId::SeaRoute21,
    }, // ROUTE_21
    OutdoorTownMapEntry {
        x: 0,
        y: 8,
        name: MapNameId::Route22,
    }, // ROUTE_22
    OutdoorTownMapEntry {
        x: 0,
        y: 6,
        name: MapNameId::Route23,
    }, // ROUTE_23
    OutdoorTownMapEntry {
        x: 10,
        y: 1,
        name: MapNameId::Route24,
    }, // ROUTE_24
    OutdoorTownMapEntry {
        x: 11,
        y: 0,
        name: MapNameId::Route25,
    }, // ROUTE_25
];

/// Indoor map group entries — indexed by indoor group ID (0..59)
/// From InternalMapEntries in data/maps/town_map_entries.asm
pub const INDOOR_TOWN_MAP_ENTRIES: [IndoorTownMapEntry; NUM_INDOOR_ENTRIES] = [
    IndoorTownMapEntry {
        group_id: 0,
        x: 2,
        y: 11,
        name: MapNameId::PalletTown,
    }, // PALLET_TOWN
    IndoorTownMapEntry {
        group_id: 1,
        x: 2,
        y: 8,
        name: MapNameId::ViridianCity,
    }, // VIRIDIAN_CITY
    IndoorTownMapEntry {
        group_id: 2,
        x: 2,
        y: 6,
        name: MapNameId::Route2,
    }, // ROUTE_2
    IndoorTownMapEntry {
        group_id: 3,
        x: 2,
        y: 4,
        name: MapNameId::ViridianForest,
    }, // VIRIDIAN_FOREST
    IndoorTownMapEntry {
        group_id: 4,
        x: 2,
        y: 3,
        name: MapNameId::PewterCity,
    }, // PEWTER_CITY
    IndoorTownMapEntry {
        group_id: 5,
        x: 6,
        y: 2,
        name: MapNameId::MountMoon,
    }, // MT_MOON
    IndoorTownMapEntry {
        group_id: 6,
        x: 10,
        y: 2,
        name: MapNameId::CeruleanCity,
    }, // CERULEAN_CITY
    IndoorTownMapEntry {
        group_id: 7,
        x: 5,
        y: 2,
        name: MapNameId::Route4,
    }, // ROUTE_4
    IndoorTownMapEntry {
        group_id: 8,
        x: 10,
        y: 2,
        name: MapNameId::CeruleanCity,
    }, // CERULEAN_CITY_2
    IndoorTownMapEntry {
        group_id: 9,
        x: 10,
        y: 4,
        name: MapNameId::Route5,
    }, // ROUTE_5
    IndoorTownMapEntry {
        group_id: 10,
        x: 10,
        y: 6,
        name: MapNameId::Route6,
    }, // ROUTE_6
    IndoorTownMapEntry {
        group_id: 11,
        x: 9,
        y: 5,
        name: MapNameId::Route7,
    }, // ROUTE_7
    IndoorTownMapEntry {
        group_id: 12,
        x: 11,
        y: 5,
        name: MapNameId::Route8,
    }, // ROUTE_8
    IndoorTownMapEntry {
        group_id: 13,
        x: 14,
        y: 3,
        name: MapNameId::RockTunnel,
    }, // ROCK_TUNNEL
    IndoorTownMapEntry {
        group_id: 14,
        x: 15,
        y: 4,
        name: MapNameId::PowerPlant,
    }, // POWER_PLANT
    IndoorTownMapEntry {
        group_id: 15,
        x: 13,
        y: 9,
        name: MapNameId::Route11,
    }, // ROUTE_11
    IndoorTownMapEntry {
        group_id: 16,
        x: 14,
        y: 7,
        name: MapNameId::Route12,
    }, // ROUTE_12
    IndoorTownMapEntry {
        group_id: 17,
        x: 12,
        y: 0,
        name: MapNameId::SeaCottage,
    }, // SEA_COTTAGE
    IndoorTownMapEntry {
        group_id: 18,
        x: 10,
        y: 9,
        name: MapNameId::VermilionCity,
    }, // VERMILION_CITY
    IndoorTownMapEntry {
        group_id: 19,
        x: 9,
        y: 10,
        name: MapNameId::SSAnne,
    }, // SS_ANNE
    IndoorTownMapEntry {
        group_id: 20,
        x: 0,
        y: 4,
        name: MapNameId::VictoryRoad,
    }, // VICTORY_ROAD
    IndoorTownMapEntry {
        group_id: 21,
        x: 0,
        y: 2,
        name: MapNameId::PokemonLeague,
    }, // POKEMON_LEAGUE
    IndoorTownMapEntry {
        group_id: 22,
        x: 10,
        y: 5,
        name: MapNameId::UndergroundPath,
    }, // UNDERGROUND_PATH
    IndoorTownMapEntry {
        group_id: 23,
        x: 0,
        y: 2,
        name: MapNameId::PokemonLeague,
    }, // POKEMON_LEAGUE_2
    IndoorTownMapEntry {
        group_id: 24,
        x: 10,
        y: 5,
        name: MapNameId::UndergroundPath,
    }, // UNDERGROUND_PATH_2
    IndoorTownMapEntry {
        group_id: 25,
        x: 7,
        y: 5,
        name: MapNameId::CeladonCity,
    }, // CELADON_CITY
    IndoorTownMapEntry {
        group_id: 26,
        x: 14,
        y: 5,
        name: MapNameId::LavenderTown,
    }, // LAVENDER_TOWN
    IndoorTownMapEntry {
        group_id: 27,
        x: 15,
        y: 5,
        name: MapNameId::PokemonTower,
    }, // POKEMON_TOWER
    IndoorTownMapEntry {
        group_id: 28,
        x: 14,
        y: 5,
        name: MapNameId::LavenderTown,
    }, // LAVENDER_TOWN_2
    IndoorTownMapEntry {
        group_id: 29,
        x: 8,
        y: 13,
        name: MapNameId::FuchsiaCity,
    }, // FUCHSIA_CITY
    IndoorTownMapEntry {
        group_id: 30,
        x: 8,
        y: 12,
        name: MapNameId::SafariZone,
    }, // SAFARI_ZONE
    IndoorTownMapEntry {
        group_id: 31,
        x: 8,
        y: 13,
        name: MapNameId::FuchsiaCity,
    }, // FUCHSIA_CITY_2
    IndoorTownMapEntry {
        group_id: 32,
        x: 5,
        y: 15,
        name: MapNameId::SeafoamIslands,
    }, // SEAFOAM_ISLANDS
    IndoorTownMapEntry {
        group_id: 33,
        x: 10,
        y: 9,
        name: MapNameId::VermilionCity,
    }, // VERMILION_CITY_2
    IndoorTownMapEntry {
        group_id: 34,
        x: 8,
        y: 13,
        name: MapNameId::FuchsiaCity,
    }, // FUCHSIA_CITY_3
    IndoorTownMapEntry {
        group_id: 35,
        x: 2,
        y: 15,
        name: MapNameId::PokemonMansion,
    }, // POKEMON_MANSION
    IndoorTownMapEntry {
        group_id: 36,
        x: 2,
        y: 15,
        name: MapNameId::CinnabarIsland,
    }, // CINNABAR_ISLAND
    IndoorTownMapEntry {
        group_id: 37,
        x: 0,
        y: 2,
        name: MapNameId::IndigoPlateau,
    }, // INDIGO_PLATEAU
    IndoorTownMapEntry {
        group_id: 38,
        x: 10,
        y: 5,
        name: MapNameId::SaffronCity,
    }, // SAFFRON_CITY
    IndoorTownMapEntry {
        group_id: 39,
        x: 9,
        y: 13,
        name: MapNameId::Route15,
    }, // ROUTE_15
    IndoorTownMapEntry {
        group_id: 40,
        x: 4,
        y: 5,
        name: MapNameId::Route16,
    }, // ROUTE_16
    IndoorTownMapEntry {
        group_id: 41,
        x: 14,
        y: 10,
        name: MapNameId::Route12,
    }, // ROUTE_12_2
    IndoorTownMapEntry {
        group_id: 42,
        x: 7,
        y: 13,
        name: MapNameId::Route18,
    }, // ROUTE_18
    IndoorTownMapEntry {
        group_id: 43,
        x: 5,
        y: 15,
        name: MapNameId::SeafoamIslands,
    }, // SEAFOAM_ISLANDS_2
    IndoorTownMapEntry {
        group_id: 44,
        x: 0,
        y: 7,
        name: MapNameId::Route22,
    }, // ROUTE_22
    IndoorTownMapEntry {
        group_id: 45,
        x: 0,
        y: 4,
        name: MapNameId::VictoryRoad,
    }, // VICTORY_ROAD_2
    IndoorTownMapEntry {
        group_id: 46,
        x: 14,
        y: 7,
        name: MapNameId::Route12,
    }, // ROUTE_12_3
    IndoorTownMapEntry {
        group_id: 47,
        x: 10,
        y: 9,
        name: MapNameId::VermilionCity,
    }, // VERMILION_CITY_3
    IndoorTownMapEntry {
        group_id: 48,
        x: 3,
        y: 4,
        name: MapNameId::DiglettsCave,
    }, // DIGLETTS_CAVE
    IndoorTownMapEntry {
        group_id: 49,
        x: 0,
        y: 4,
        name: MapNameId::VictoryRoad,
    }, // VICTORY_ROAD_3
    IndoorTownMapEntry {
        group_id: 50,
        x: 7,
        y: 5,
        name: MapNameId::RocketHQ,
    }, // ROCKET_HQ
    IndoorTownMapEntry {
        group_id: 51,
        x: 10,
        y: 5,
        name: MapNameId::SilphCo,
    }, // SILPH_CO
    IndoorTownMapEntry {
        group_id: 52,
        x: 2,
        y: 15,
        name: MapNameId::PokemonMansion,
    }, // POKEMON_MANSION_2
    IndoorTownMapEntry {
        group_id: 53,
        x: 8,
        y: 12,
        name: MapNameId::SafariZone,
    }, // SAFARI_ZONE_2
    IndoorTownMapEntry {
        group_id: 54,
        x: 9,
        y: 1,
        name: MapNameId::CeruleanCave,
    }, // CERULEAN_CAVE
    IndoorTownMapEntry {
        group_id: 55,
        x: 14,
        y: 5,
        name: MapNameId::LavenderTown,
    }, // LAVENDER_TOWN_3
    IndoorTownMapEntry {
        group_id: 56,
        x: 10,
        y: 2,
        name: MapNameId::CeruleanCity,
    }, // CERULEAN_CITY_3
    IndoorTownMapEntry {
        group_id: 57,
        x: 14,
        y: 3,
        name: MapNameId::RockTunnel,
    }, // ROCK_TUNNEL_2
    IndoorTownMapEntry {
        group_id: 58,
        x: 10,
        y: 5,
        name: MapNameId::SilphCo,
    }, // SILPH_CO_2
    IndoorTownMapEntry {
        group_id: 59,
        x: 0,
        y: 2,
        name: MapNameId::PokemonLeague,
    }, // POKEMON_LEAGUE_3
];

/// Resolve any map to its town-map marker: the `(x, y)` tile on the 20×18 map
/// and the landmark name to display. Works for dungeon/indoor floors too — they
/// resolve to their parent landmark via [`map_to_name_id`]. Outdoor entries take
/// priority (canonical coords); indoor group entries are the fallback for maps
/// that only exist inside (Mt. Moon, Power Plant, …).
pub fn town_map_position(map: MapId) -> Option<(u8, u8, MapNameId)> {
    let name = map_to_name_id(map);
    OUTDOOR_TOWN_MAP_ENTRIES
        .iter()
        .find(|e| e.name == name)
        .map(|e| (e.x, e.y, e.name))
        .or_else(|| {
            INDOOR_TOWN_MAP_ENTRIES
                .iter()
                .find(|e| e.name == name)
                .map(|e| (e.x, e.y, e.name))
        })
}

/// Town map order — the order maps appear in the fly destination list.
/// From TownMapOrder in data/maps/town_map_order.asm.
/// 48 entries (TownMapOrder to TownMapOrderEnd).
pub const TOWN_MAP_ORDER: [MapId; NUM_TOWN_MAP_ORDER] = [
    MapId::PalletTown,
    MapId::Route1,
    MapId::ViridianCity,
    MapId::Route2,
    MapId::ViridianForest,
    MapId::DiglettsCave,
    MapId::PewterCity,
    MapId::Route3,
    MapId::MtMoon1F,
    MapId::Route4,
    MapId::CeruleanCity,
    MapId::Route24,
    MapId::Route25,
    MapId::BillsHouse,
    MapId::Route5,
    MapId::Route6,
    MapId::VermilionCity,
    MapId::SSAnne1F,
    MapId::Route9,
    MapId::RockTunnelPokecenter,
    MapId::Route10,
    MapId::LavenderTown,
    MapId::PokemonTower2F,
    MapId::Route8,
    MapId::Route7,
    MapId::CeladonCity,
    MapId::SaffronCity,
    MapId::Route11,
    MapId::Route12,
    MapId::Route13,
    MapId::Route14,
    MapId::Route15,
    MapId::Route16,
    MapId::Route17,
    MapId::Route18,
    MapId::FuchsiaCity,
    MapId::SafariZoneEast,
    MapId::Route19,
    MapId::SeafoamIslandsB1F,
    MapId::Route20,
    MapId::CinnabarIsland,
    MapId::Route21,
    MapId::Route22,
    MapId::Route23,
    MapId::VictoryRoad3F,
    MapId::IndigoPlateau,
    MapId::PowerPlant,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilemap_decodes_to_full_screen() {
        let tm = decode_town_map_tilemap();
        assert_eq!(tm.len(), TOWN_MAP_WIDTH * TOWN_MAP_HEIGHT);
        // Every index must be a valid tile in the 16-tile sheet.
        assert!(tm.iter().all(|&t| t < 16));
    }

    #[test]
    fn resolves_outdoor_city() {
        // Pallet Town is an outdoor entry at (2, 11).
        assert_eq!(
            town_map_position(MapId::PalletTown),
            Some((2, 11, MapNameId::PalletTown))
        );
    }

    #[test]
    fn resolves_indoor_dungeon_to_parent_landmark() {
        // Mt. Moon floors only exist indoors; they resolve to the MT.MOON
        // landmark marker (indoor group 5 at (6, 2)).
        let (x, y, name) = town_map_position(MapId::MtMoon1F).expect("resolves");
        assert_eq!(name, MapNameId::MountMoon);
        assert_eq!((x, y), (6, 2));
    }
}
