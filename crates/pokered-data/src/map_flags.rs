use crate::maps::MapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonMapRange {
    pub first: MapId,
    pub last: MapId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForceBikeSurfEntry {
    pub map: MapId,
    pub x: u8,
    pub y: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeMapEntry {
    pub gym_map: MapId,
    pub badge_bit: u8,
}

pub const DUNGEON_MAPS_EXACT: [MapId; 4] = [
    MapId::ViridianForest,
    MapId::RockTunnel1F,
    MapId::SeafoamIslands1F,
    MapId::RockTunnelB1F,
];

pub const DUNGEON_MAPS_RANGES: [DungeonMapRange; 4] = [
    DungeonMapRange {
        first: MapId::MtMoon1F,
        last: MapId::MtMoonB2F,
    },
    DungeonMapRange {
        first: MapId::SSAnne1F,
        last: MapId::HallOfFame,
    },
    DungeonMapRange {
        first: MapId::LavenderPokecenter,
        last: MapId::LavenderCuboneHouse,
    },
    DungeonMapRange {
        first: MapId::SilphCo2F,
        last: MapId::CeruleanCave1F,
    },
];

pub const FORCED_BIKE_SURF_ENTRIES: [ForceBikeSurfEntry; 8] = [
    ForceBikeSurfEntry {
        map: MapId::Route16,
        x: 10,
        y: 17,
    },
    ForceBikeSurfEntry {
        map: MapId::Route16,
        x: 11,
        y: 17,
    },
    ForceBikeSurfEntry {
        map: MapId::Route18,
        x: 8,
        y: 33,
    },
    ForceBikeSurfEntry {
        map: MapId::Route18,
        x: 9,
        y: 33,
    },
    ForceBikeSurfEntry {
        map: MapId::SeafoamIslandsB3F,
        x: 7,
        y: 18,
    },
    ForceBikeSurfEntry {
        map: MapId::SeafoamIslandsB3F,
        x: 7,
        y: 19,
    },
    ForceBikeSurfEntry {
        map: MapId::SeafoamIslandsB4F,
        x: 14,
        y: 4,
    },
    ForceBikeSurfEntry {
        map: MapId::SeafoamIslandsB4F,
        x: 14,
        y: 5,
    },
];

pub const SAFARI_ZONE_REST_HOUSES: [MapId; 3] = [
    MapId::SafariZoneWestRestHouse,
    MapId::SafariZoneEastRestHouse,
    MapId::SafariZoneNorthRestHouse,
];

pub const BIT_BOULDERBADGE: u8 = 0;
pub const BIT_CASCADEBADGE: u8 = 1;
pub const BIT_THUNDERBADGE: u8 = 2;
pub const BIT_RAINBOWBADGE: u8 = 3;
pub const BIT_SOULBADGE: u8 = 4;
pub const BIT_MARSHBADGE: u8 = 5;
pub const BIT_VOLCANOBADGE: u8 = 6;
pub const BIT_EARTHBADGE: u8 = 7;

pub const BADGE_MAP_FLAGS: [BadgeMapEntry; 8] = [
    BadgeMapEntry {
        gym_map: MapId::PewterGym,
        badge_bit: BIT_BOULDERBADGE,
    },
    BadgeMapEntry {
        gym_map: MapId::CeruleanGym,
        badge_bit: BIT_CASCADEBADGE,
    },
    BadgeMapEntry {
        gym_map: MapId::VermilionGym,
        badge_bit: BIT_THUNDERBADGE,
    },
    BadgeMapEntry {
        gym_map: MapId::CeladonGym,
        badge_bit: BIT_RAINBOWBADGE,
    },
    BadgeMapEntry {
        gym_map: MapId::FuchsiaGym,
        badge_bit: BIT_SOULBADGE,
    },
    BadgeMapEntry {
        gym_map: MapId::SaffronGym,
        badge_bit: BIT_MARSHBADGE,
    },
    BadgeMapEntry {
        gym_map: MapId::CinnabarGym,
        badge_bit: BIT_VOLCANOBADGE,
    },
    BadgeMapEntry {
        gym_map: MapId::ViridianGym,
        badge_bit: BIT_EARTHBADGE,
    },
];

pub fn is_dungeon_map(map: MapId) -> bool {
    let map_id = map as u8;
    for &exact in &DUNGEON_MAPS_EXACT {
        if map_id == exact as u8 {
            return true;
        }
    }
    for range in &DUNGEON_MAPS_RANGES {
        if map_id >= range.first as u8 && map_id <= range.last as u8 {
            return true;
        }
    }
    false
}

/// First route map ID ($0B in the original — city/town maps are $00-$0A).
/// From constants/map_constants.asm FIRST_ROUTE_MAP.
pub const FIRST_ROUTE_MAP: u8 = 0x0B;

/// Is this map a city/town (visited-tracking + FLY destination candidate)?
/// From MarkTownVisitedAndLoadToggleableObjects: `cp FIRST_ROUTE_MAP`.
pub fn is_city_map(map: MapId) -> bool {
    (map as u8) < FIRST_ROUTE_MAP
}

pub fn check_force_bike_surf(map: MapId, x: u8, y: u8) -> bool {
    for entry in &FORCED_BIKE_SURF_ENTRIES {
        if entry.map == map && entry.x == x && entry.y == y {
            return true;
        }
    }
    false
}

pub fn is_safari_rest_house(map: MapId) -> bool {
    SAFARI_ZONE_REST_HOUSES.contains(&map)
}

/// True for the walkable Safari Zone maps where the 500-step counter runs
/// (everything except the entrance gate). Matches the original's
/// `SAFARI_ZONE_EAST`..=`SAFARI_ZONE_NORTH_REST_HOUSE` map-id range.
pub fn is_safari_zone_map(map: MapId) -> bool {
    let id = map as u8;
    id >= MapId::SafariZoneEast as u8 && id <= MapId::SafariZoneNorthRestHouse as u8
}

pub fn badge_for_gym(map: MapId) -> Option<u8> {
    for entry in &BADGE_MAP_FLAGS {
        if entry.gym_map == map {
            return Some(1 << entry.badge_bit);
        }
    }
    None
}
