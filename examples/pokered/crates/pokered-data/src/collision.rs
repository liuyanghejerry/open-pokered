//! Tileset collision data — passable tiles, tile pair collisions, and ledge tiles.
//!
//! Ported from:
//! - data/tilesets/collision_tile_ids.asm
//! - data/tilesets/pair_collision_tile_ids.asm
//! - data/tilesets/ledge_tiles.asm

use crate::tilesets::{TilesetId, NUM_TILESETS};

/// Passable tile IDs for each tileset.
/// The collision system works by listing which tiles are PASSABLE (walkable).
/// Any tile NOT in the list is impassable.
///
/// In the original game, wTilesetCollisionPtr points to one of these lists,
/// and CanWalkOntoTile / CheckTilePassable iterate through looking for
/// a matching tile ID. The list is terminated by 0xFF.
pub const TILESET_PASSABLE_TILES: [&[u8]; NUM_TILESETS] = [
    // 0: Overworld
    &[
        0x00, 0x10, 0x1B, 0x20, 0x21, 0x23, 0x2C, 0x2D, 0x2E, 0x30, 0x31, 0x33, 0x39, 0x3C, 0x3E,
        0x52, 0x54, 0x58, 0x5B,
    ],
    // 1: RedsHouse1
    &[0x01, 0x02, 0x03, 0x11, 0x12, 0x13, 0x14, 0x1C, 0x1A],
    // 2: Mart
    &[0x11, 0x1A, 0x1C, 0x3C, 0x5E],
    // 3: Forest
    &[
        0x1E, 0x20, 0x2E, 0x30, 0x34, 0x37, 0x39, 0x3A, 0x40, 0x51, 0x52, 0x5A, 0x5C, 0x5E, 0x5F,
    ],
    // 4: RedsHouse2 (same as RedsHouse1)
    &[0x01, 0x02, 0x03, 0x11, 0x12, 0x13, 0x14, 0x1C, 0x1A],
    // 5: Dojo
    &[
        0x11, 0x16, 0x19, 0x2B, 0x3C, 0x3D, 0x3F, 0x4A, 0x4C, 0x4D, 0x03,
    ],
    // 6: Pokecenter (same as Mart)
    &[0x11, 0x1A, 0x1C, 0x3C, 0x5E],
    // 7: Gym (same as Dojo)
    &[
        0x11, 0x16, 0x19, 0x2B, 0x3C, 0x3D, 0x3F, 0x4A, 0x4C, 0x4D, 0x03,
    ],
    // 8: House
    &[0x01, 0x12, 0x14, 0x28, 0x32, 0x37, 0x44, 0x54, 0x5C],
    // 9: ForestGate
    &[0x01, 0x12, 0x14, 0x1A, 0x1C, 0x37, 0x38, 0x3B, 0x3C, 0x5E],
    // 10: Museum (same as ForestGate)
    &[0x01, 0x12, 0x14, 0x1A, 0x1C, 0x37, 0x38, 0x3B, 0x3C, 0x5E],
    // 11: Underground
    &[0x0B, 0x0C, 0x13, 0x15, 0x18],
    // 12: Gate (same as ForestGate)
    &[0x01, 0x12, 0x14, 0x1A, 0x1C, 0x37, 0x38, 0x3B, 0x3C, 0x5E],
    // 13: Ship
    &[0x04, 0x0D, 0x17, 0x1D, 0x1E, 0x23, 0x34, 0x37, 0x39, 0x4A],
    // 14: ShipPort
    &[0x0A, 0x1A, 0x32, 0x3B],
    // 15: Cemetery
    &[0x01, 0x10, 0x13, 0x1B, 0x22, 0x42, 0x52],
    // 16: Interior
    &[0x04, 0x0F, 0x15, 0x1F, 0x3B, 0x45, 0x47, 0x55, 0x56],
    // 17: Cavern
    &[0x05, 0x15, 0x18, 0x1A, 0x20, 0x21, 0x22, 0x2A, 0x2D, 0x30],
    // 18: Lobby
    &[0x14, 0x17, 0x1A, 0x1C, 0x20, 0x38, 0x45],
    // 19: Mansion
    &[0x01, 0x05, 0x11, 0x12, 0x14, 0x1A, 0x1C, 0x2C, 0x53],
    // 20: Lab
    &[0x0C, 0x26, 0x16, 0x1E, 0x34, 0x37],
    // 21: Club
    &[
        0x0F, 0x1A, 0x1F, 0x26, 0x28, 0x29, 0x2C, 0x2D, 0x2E, 0x2F, 0x41,
    ],
    // 22: Facility
    &[
        0x01, 0x10, 0x11, 0x13, 0x1B, 0x20, 0x21, 0x22, 0x30, 0x31, 0x32, 0x42, 0x43, 0x48, 0x52,
        0x55, 0x58, 0x5E,
    ],
    // 23: Plateau
    &[0x1B, 0x23, 0x2C, 0x2D, 0x3B, 0x45],
];

/// Get the passable tile list for a tileset.
/// For `Custom`, returns the registered passable-tile override if any,
/// otherwise the base tileset's list.
pub fn get_passable_tiles(tileset: TilesetId) -> &'static [u8] {
    if let TilesetId::Custom(slot) = tileset {
        if let Some(over) = crate::tilesets::custom::passable_override_for(slot) {
            return over;
        }
    }
    // Built-in (or Custom with no override): index by the resolved base.
    let lookup = if tileset.is_custom() {
        tileset.base()
    } else {
        tileset
    };
    TILESET_PASSABLE_TILES[lookup.to_index()]
}

/// Check if a tile is passable in the given tileset.
/// Returns true if passable, false if impassable.
pub fn is_tile_passable(tileset: TilesetId, tile_id: u8) -> bool {
    get_passable_tiles(tileset).contains(&tile_id)
}

/// A tile pair collision entry.
/// The player cannot cross between tile1 and tile2 when standing on one
/// and trying to walk onto the other (simulates elevation differences).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilePairCollision {
    pub tileset: u8,
    pub tile1: u8,
    pub tile2: u8,
}

/// Tile pair collisions for land movement.
/// From data/tilesets/pair_collision_tile_ids.asm TilePairCollisionsLand.
pub const TILE_PAIR_COLLISIONS_LAND: &[TilePairCollision] = &[
    TilePairCollision {
        tileset: 17,
        tile1: 0x20,
        tile2: 0x05,
    }, // CAVERN
    TilePairCollision {
        tileset: 17,
        tile1: 0x41,
        tile2: 0x05,
    }, // CAVERN
    TilePairCollision {
        tileset: 3,
        tile1: 0x30,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 17,
        tile1: 0x2A,
        tile2: 0x05,
    }, // CAVERN
    TilePairCollision {
        tileset: 17,
        tile1: 0x05,
        tile2: 0x21,
    }, // CAVERN
    TilePairCollision {
        tileset: 3,
        tile1: 0x52,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 3,
        tile1: 0x55,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 3,
        tile1: 0x56,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 3,
        tile1: 0x20,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 3,
        tile1: 0x5E,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 3,
        tile1: 0x5F,
        tile2: 0x2E,
    }, // FOREST
];

/// Tile pair collisions for water movement (surfing).
/// From data/tilesets/pair_collision_tile_ids.asm TilePairCollisionsWater.
pub const TILE_PAIR_COLLISIONS_WATER: &[TilePairCollision] = &[
    TilePairCollision {
        tileset: 3,
        tile1: 0x14,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 3,
        tile1: 0x48,
        tile2: 0x2E,
    }, // FOREST
    TilePairCollision {
        tileset: 17,
        tile1: 0x14,
        tile2: 0x05,
    }, // CAVERN
];

/// Check for tile pair collision.
/// Returns true if movement between `standing_tile` and `target_tile`
/// is blocked by a tile pair collision for the given tileset.
pub fn check_tile_pair_collision(
    tileset: TilesetId,
    standing_tile: u8,
    target_tile: u8,
    on_water: bool,
) -> bool {
    let pairs = if on_water {
        TILE_PAIR_COLLISIONS_WATER
    } else {
        TILE_PAIR_COLLISIONS_LAND
    };
    let tileset_id = tileset.base().to_u8();
    for pair in pairs {
        if pair.tileset != tileset_id {
            continue;
        }
        // Check both directions: standing on tile1 → walking onto tile2, or vice versa
        if (pair.tile1 == standing_tile && pair.tile2 == target_tile)
            || (pair.tile2 == standing_tile && pair.tile1 == target_tile)
        {
            return true;
        }
    }
    false
}

/// Direction constants matching SPRITE_FACING_* values from the original game.
/// Used for ledge tile checks.
pub const SPRITE_FACING_DOWN: u8 = 0x00;
pub const SPRITE_FACING_UP: u8 = 0x04;
pub const SPRITE_FACING_LEFT: u8 = 0x08;
pub const SPRITE_FACING_RIGHT: u8 = 0x0C;

/// Ledge tile entry.
/// When the player is facing `direction`, standing on `standing_tile`,
/// and the tile in front is `ledge_tile`, the player jumps the ledge
/// (only if the correct d-pad button `required_input` is held).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgeTile {
    pub direction: u8,
    pub standing_tile: u8,
    pub ledge_tile: u8,
    pub required_input: u8,
}

/// D-pad input constants for ledge checking.
pub const PAD_DOWN: u8 = 0x80;
pub const PAD_UP: u8 = 0x40;
pub const PAD_LEFT: u8 = 0x20;
pub const PAD_RIGHT: u8 = 0x10;

/// Ledge tiles from data/tilesets/ledge_tiles.asm.
/// These only apply to the Overworld tileset.
pub const LEDGE_TILES: &[LedgeTile] = &[
    LedgeTile {
        direction: SPRITE_FACING_DOWN,
        standing_tile: 0x2C,
        ledge_tile: 0x37,
        required_input: PAD_DOWN,
    },
    LedgeTile {
        direction: SPRITE_FACING_DOWN,
        standing_tile: 0x39,
        ledge_tile: 0x36,
        required_input: PAD_DOWN,
    },
    LedgeTile {
        direction: SPRITE_FACING_DOWN,
        standing_tile: 0x39,
        ledge_tile: 0x37,
        required_input: PAD_DOWN,
    },
    LedgeTile {
        direction: SPRITE_FACING_LEFT,
        standing_tile: 0x2C,
        ledge_tile: 0x27,
        required_input: PAD_LEFT,
    },
    LedgeTile {
        direction: SPRITE_FACING_LEFT,
        standing_tile: 0x39,
        ledge_tile: 0x27,
        required_input: PAD_LEFT,
    },
    LedgeTile {
        direction: SPRITE_FACING_RIGHT,
        standing_tile: 0x2C,
        ledge_tile: 0x0D,
        required_input: PAD_RIGHT,
    },
    LedgeTile {
        direction: SPRITE_FACING_RIGHT,
        standing_tile: 0x2C,
        ledge_tile: 0x1D,
        required_input: PAD_RIGHT,
    },
    LedgeTile {
        direction: SPRITE_FACING_RIGHT,
        standing_tile: 0x39,
        ledge_tile: 0x0D,
        required_input: PAD_RIGHT,
    },
];

/// Check if the player should jump a ledge.
/// Returns Some(direction) if a ledge jump should occur, None otherwise.
pub fn check_ledge_jump(
    facing_direction: u8,
    standing_tile: u8,
    tile_in_front: u8,
    held_input: u8,
) -> bool {
    for ledge in LEDGE_TILES {
        if ledge.direction == facing_direction
            && ledge.standing_tile == standing_tile
            && ledge.ledge_tile == tile_in_front
            && (held_input & ledge.required_input) != 0
        {
            return true;
        }
    }
    false
}
