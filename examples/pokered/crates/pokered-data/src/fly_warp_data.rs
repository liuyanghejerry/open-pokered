use crate::maps::MapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlyWarpCoords {
    pub map_width: u8,
    pub x: u8,
    pub y: u8,
    pub sub_block_y: u8,
    pub sub_block_x: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlyDestination {
    pub map_id: MapId,
    pub coords: FlyWarpCoords,
}

// From data/maps/special_warps.asm FlyWarpDataPtr
// 13 fly destinations: all cities + Route 4 + Route 10
pub const FLY_DESTINATIONS: &[FlyDestination] = &[
    FlyDestination {
        map_id: MapId::PalletTown,
        coords: FlyWarpCoords {
            map_width: 10,
            x: 5,
            y: 6,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::ViridianCity,
        coords: FlyWarpCoords {
            map_width: 20,
            x: 23,
            y: 26,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::PewterCity,
        coords: FlyWarpCoords {
            map_width: 20,
            x: 13,
            y: 26,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::CeruleanCity,
        coords: FlyWarpCoords {
            map_width: 20,
            x: 19,
            y: 18,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::LavenderTown,
        coords: FlyWarpCoords {
            map_width: 10,
            x: 3,
            y: 6,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::VermilionCity,
        coords: FlyWarpCoords {
            map_width: 20,
            x: 11,
            y: 4,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::CeladonCity,
        coords: FlyWarpCoords {
            map_width: 25,
            x: 41,
            y: 10,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::FuchsiaCity,
        coords: FlyWarpCoords {
            map_width: 20,
            x: 19,
            y: 28,
            sub_block_y: 0,
            sub_block_x: 0,
        },
    },
    FlyDestination {
        map_id: MapId::CinnabarIsland,
        coords: FlyWarpCoords {
            map_width: 10,
            x: 11,
            y: 12,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::IndigoPlateau,
        coords: FlyWarpCoords {
            map_width: 10,
            x: 9,
            y: 6,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::SaffronCity,
        coords: FlyWarpCoords {
            map_width: 20,
            x: 9,
            y: 30,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::Route4,
        coords: FlyWarpCoords {
            map_width: 45,
            x: 11,
            y: 6,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    FlyDestination {
        map_id: MapId::Route10,
        coords: FlyWarpCoords {
            map_width: 10,
            x: 11,
            y: 20,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
];

pub fn fly_destination_for_map(map_id: MapId) -> Option<&'static FlyDestination> {
    FLY_DESTINATIONS.iter().find(|d| d.map_id == map_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonWarpEntry {
    pub map_id: MapId,
    pub warp_id: u8,
    pub coords: FlyWarpCoords,
}

// From data/maps/special_warps.asm DungeonWarpList + DungeonWarpData
pub const DUNGEON_WARPS: &[DungeonWarpEntry] = &[
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB1F,
        warp_id: 1,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 18,
            y: 7,
            sub_block_y: 1,
            sub_block_x: 0,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB1F,
        warp_id: 2,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 23,
            y: 7,
            sub_block_y: 1,
            sub_block_x: 1,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB2F,
        warp_id: 1,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 19,
            y: 7,
            sub_block_y: 1,
            sub_block_x: 1,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB2F,
        warp_id: 2,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 22,
            y: 7,
            sub_block_y: 1,
            sub_block_x: 0,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB3F,
        warp_id: 1,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 18,
            y: 7,
            sub_block_y: 1,
            sub_block_x: 0,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB3F,
        warp_id: 2,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 19,
            y: 7,
            sub_block_y: 1,
            sub_block_x: 1,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB4F,
        warp_id: 1,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 4,
            y: 14,
            sub_block_y: 0,
            sub_block_x: 0,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::SeafoamIslandsB4F,
        warp_id: 2,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 5,
            y: 14,
            sub_block_y: 0,
            sub_block_x: 1,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::VictoryRoad2F,
        warp_id: 2,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 22,
            y: 16,
            sub_block_y: 0,
            sub_block_x: 0,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::PokemonMansion1F,
        warp_id: 1,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 16,
            y: 14,
            sub_block_y: 0,
            sub_block_x: 0,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::PokemonMansion1F,
        warp_id: 2,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 16,
            y: 14,
            sub_block_y: 0,
            sub_block_x: 0,
        },
    },
    DungeonWarpEntry {
        map_id: MapId::PokemonMansion2F,
        warp_id: 3,
        coords: FlyWarpCoords {
            map_width: 15,
            x: 18,
            y: 14,
            sub_block_y: 0,
            sub_block_x: 0,
        },
    },
];

pub fn dungeon_warp_for(map_id: MapId, warp_id: u8) -> Option<&'static DungeonWarpEntry> {
    DUNGEON_WARPS
        .iter()
        .find(|e| e.map_id == map_id && e.warp_id == warp_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecialWarpEntry {
    pub map_id: MapId,
    pub coords: FlyWarpCoords,
    pub tileset_id: u8,
}

// From data/maps/special_warps.asm NewGameWarp
pub const NEW_GAME_WARP: SpecialWarpEntry = SpecialWarpEntry {
    map_id: MapId::RedsHouse2F,
    coords: FlyWarpCoords {
        map_width: 4,
        x: 3,
        y: 6,
        sub_block_y: 0,
        sub_block_x: 1,
    },
    tileset_id: 2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkWarpType {
    TradeCenterPlayer,
    TradeCenterFriend,
    ColosseumPlayer,
    ColosseumFriend,
}

pub fn link_warp(warp_type: LinkWarpType) -> SpecialWarpEntry {
    match warp_type {
        LinkWarpType::TradeCenterPlayer => SpecialWarpEntry {
            map_id: MapId::TradeCenter,
            coords: FlyWarpCoords {
                map_width: 5,
                x: 3,
                y: 4,
                sub_block_y: 0,
                sub_block_x: 1,
            },
            tileset_id: 8,
        },
        LinkWarpType::TradeCenterFriend => SpecialWarpEntry {
            map_id: MapId::TradeCenter,
            coords: FlyWarpCoords {
                map_width: 5,
                x: 6,
                y: 4,
                sub_block_y: 0,
                sub_block_x: 0,
            },
            tileset_id: 8,
        },
        LinkWarpType::ColosseumPlayer => SpecialWarpEntry {
            map_id: MapId::Colosseum,
            coords: FlyWarpCoords {
                map_width: 5,
                x: 3,
                y: 4,
                sub_block_y: 0,
                sub_block_x: 1,
            },
            tileset_id: 8,
        },
        LinkWarpType::ColosseumFriend => SpecialWarpEntry {
            map_id: MapId::Colosseum,
            coords: FlyWarpCoords {
                map_width: 5,
                x: 6,
                y: 4,
                sub_block_y: 0,
                sub_block_x: 0,
            },
            tileset_id: 8,
        },
    }
}

// Player spin direction cycle during teleport animation
// From engine/overworld/player_animations.asm PlayerSpinningFacingOrder
// Down -> Left -> Up -> Right (then wraps)
pub const TELEPORT_SPIN_ORDER: [u8; 4] = [
    0,  // SPRITE_FACING_DOWN
    8,  // SPRITE_FACING_LEFT
    4,  // SPRITE_FACING_UP
    12, // SPRITE_FACING_RIGHT
];
