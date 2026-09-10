//! Generic collision detection for the overworld.
//!
//! This module provides the core collision detection algorithms that are
//! shared across all JRPG engine consumers. Game-specific tile data (passable
//! tiles, ledge tiles, tile pair collisions, counter tiles, etc.) is provided
//! via the [`CollisionProvider`] trait.
//!
//! Implements the classic overworld collision and movement behavior.

use crate::tileset::TilesetTrait;

use super::types::{Direction, MapData, TransportMode};

// ── Sprite Facing Constants ──────────────────────────────────────
/// Direction constants matching SPRITE_FACING_*.
pub const SPRITE_FACING_DOWN: u8 = 0x00;
pub const SPRITE_FACING_UP: u8 = 0x04;
pub const SPRITE_FACING_LEFT: u8 = 0x08;
pub const SPRITE_FACING_RIGHT: u8 = 0x0C;

// ── D-pad Input Constants ────────────────────────────────────────
pub const PAD_DOWN: u8 = 0x80;
pub const PAD_UP: u8 = 0x40;
pub const PAD_LEFT: u8 = 0x20;
pub const PAD_RIGHT: u8 = 0x10;

// ── Collision Provider Trait ─────────────────────────────────────
/// Provides game-specific collision data to the engine.
///
/// Implementations supply tile passability, ledge jump rules, tile-pair
/// collision data, and warp-support metadata for a given tileset type.
pub trait CollisionProvider<T: TilesetTrait> {
    /// Returns `true` if `tile_id` is passable (walkable) in the given tileset.
    fn is_tile_passable(&self, tileset: T, tile_id: u8) -> bool;

    /// Returns `true` if movement between `standing_tile` and `target_tile`
    /// is blocked by a tile-pair collision (elevation difference).
    fn check_tile_pair_collision(
        &self,
        tileset: T,
        standing_tile: u8,
        target_tile: u8,
        on_water: bool,
    ) -> bool;

    /// Returns `true` if the player should jump a ledge.
    ///
    /// `sprite_facing` is one of [`SPRITE_FACING_DOWN`], etc.
    /// `held_input` is a bitmask of pressed d-pad buttons.
    fn check_ledge_jump(
        &self,
        tileset: T,
        sprite_facing: u8,
        standing_tile: u8,
        target_tile: u8,
        held_input: u8,
    ) -> bool;

    /// Returns `true` if `tile_id` is a counter tile (can interact across but not walk onto).
    fn is_counter_tile(&self, tileset: T, tile_id: u8) -> bool;

    /// Resolve the tile ID at a given map position from block data.
    fn get_tile_at_position(&self, tileset: T, blocks: &[u8], map_width: u8, x: u16, y: u16) -> u8;

    /// Returns `true` if `tile_id` is a door tile (for auto-step-out logic).
    fn is_door_tile(&self, tileset: T, tile_id: u8) -> bool;

    /// Returns `true` if `tile_id` is a warp tile (immediate warp trigger).
    fn is_warp_tile(&self, tileset: T, tile_id: u8) -> bool;

    /// Returns `true` if the tile-in-front with the given `facing_idx` (0=Down, 1=Up, 2=Left, 3=Right)
    /// is a warp-carpet tile for ExtraWarpCheck function 2.
    fn is_warp_carpet_tile_in_front(&self, tileset: T, facing_idx: u8, tile_id: u8) -> bool;

    /// Returns `true` if `tile_id` is a water tile in the given tileset —
    /// a tile a surfer can move onto while staying on the water
    /// (CollisionCheckOnWater). Default: no water tiles.
    fn is_water_tile(&self, _tileset: T, _tile_id: u8) -> bool {
        false
    }

    /// Resolve the tile the player would step onto when crossing a map
    /// boundary in `direction` — the connected map's edge tile. The original
    /// reads this tile from the connection strip drawn in the tilemap
    /// (LoadTileBlockMap / GetTileAndCoordsInFrontOfPlayer) and applies the
    /// normal passability rules to it before the player may cross
    /// (CollisionCheckOnLand / CollisionCheckOnWater → CheckTilePassable).
    ///
    /// Returns `None` when the map has no connection in `direction` (or the
    /// connected map has no tile at the arrival position) — the engine then
    /// falls back to the plain map-edge behavior.
    fn get_connection_edge_tile(
        &self,
        _tileset: T,
        _map_width_blocks: u8,
        _map_height_blocks: u8,
        _x: u16,
        _y: u16,
        _direction: Direction,
    ) -> Option<u8> {
        None
    }

    /// Returns `true` if the tileset should use warp-tile-in-front checking
    /// (ExtraWarpCheck function 2) instead of facing-map-edge (function 1).
    fn uses_warp_tile_in_front_check(&self, tileset: T) -> bool;

    /// Handle map-specific extra-warp special cases (e.g. SS_ANNE_BOW tile 0x15).
    /// Returns `Some(true)` if warp should fire, `Some(false)` if not,
    /// or `None` to fall through to normal logic.
    fn check_extra_warp_special(&self, tileset: T, tile_in_front: u8) -> Option<bool>;
}

// ── Collision Result ─────────────────────────────────────────────

/// Result of a collision check when the player tries to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionResult {
    /// Movement is allowed — the target tile is clear.
    Passable,
    /// The tile itself is impassable (wall, obstacle, etc.).
    TileBlocked,
    /// A tile pair collision prevents crossing (elevation difference).
    TilePairBlocked,
    /// An NPC or other sprite is blocking the way.
    SpriteBlocked,
    /// The player is at the edge of the map (triggers map connection).
    MapEdge,
    /// The player should jump a ledge (special movement).
    LedgeJump,
    /// Cannot surf here (water tile but no surf).
    WaterBlocked,
    /// Counter tile — can talk across but not walk onto.
    CounterTile,
    /// Movement is allowed and ends a surf: the surfer steps onto a passable
    /// land tile and returns to walking (CollisionCheckOnWater .stopSurfing).
    StopSurfing,
}

// ── Direction Helpers ────────────────────────────────────────────

/// Convert a Direction to the sprite facing constant.
pub fn direction_to_sprite_facing(dir: Direction) -> u8 {
    match dir {
        Direction::Down => SPRITE_FACING_DOWN,
        Direction::Up => SPRITE_FACING_UP,
        Direction::Left => SPRITE_FACING_LEFT,
        Direction::Right => SPRITE_FACING_RIGHT,
    }
}

/// Convert a Direction to the d-pad input bitmask.
pub fn direction_to_pad_input(dir: Direction) -> u8 {
    match dir {
        Direction::Down => PAD_DOWN,
        Direction::Up => PAD_UP,
        Direction::Left => PAD_LEFT,
        Direction::Right => PAD_RIGHT,
    }
}

// ── Coordinate & Block Helpers ───────────────────────────────────

/// Get the target tile coordinates when moving in a direction.
/// Returns `None` if movement would go out of map bounds.
///
/// Coordinates are in tile space (2× block space).
/// Map dimensions (`map_width_blocks`, `map_height_blocks`) are in blocks.
pub fn get_target_coords(
    x: u16,
    y: u16,
    direction: Direction,
    map_width_blocks: u8,
    map_height_blocks: u8,
) -> Option<(u16, u16)> {
    let max_x = (map_width_blocks as u16) * 2;
    let max_y = (map_height_blocks as u16) * 2;

    match direction {
        Direction::Up => {
            if y == 0 {
                None
            } else {
                Some((x, y - 1))
            }
        }
        Direction::Down => {
            if y + 1 >= max_y {
                None
            } else {
                Some((x, y + 1))
            }
        }
        Direction::Left => {
            if x == 0 {
                None
            } else {
                Some((x - 1, y))
            }
        }
        Direction::Right => {
            if x + 1 >= max_x {
                None
            } else {
                Some((x + 1, y))
            }
        }
    }
}

/// Get the block ID at a position in the map's block data.
///
/// Each block is a 2×2 grid of tiles. Block index = (y/2) * width + (x/2).
pub fn get_block_at(x: u16, y: u16, map_width_blocks: u8, blocks: &[u8]) -> Option<u8> {
    let bx = (x / 2) as usize;
    let by = (y / 2) as usize;
    let w = map_width_blocks as usize;
    let idx = by * w + bx;
    blocks.get(idx).copied()
}

// ── Sprite Collision ─────────────────────────────────────────────

/// Represents the position of a sprite for collision checks.
#[derive(Debug, Clone, Copy)]
pub struct SpritePosition {
    /// Tile X coordinate.
    pub x: u16,
    /// Tile Y coordinate.
    pub y: u16,
}

/// Check if an NPC sprite occupies the target tile.
pub fn check_sprite_collision(
    target_x: u16,
    target_y: u16,
    npc_positions: &[SpritePosition],
) -> bool {
    npc_positions
        .iter()
        .any(|npc| npc.x == target_x && npc.y == target_y)
}

// ── Main Collision Check ─────────────────────────────────────────

/// Full collision check for player movement.
///
/// Checks in order (matching the original game):
/// 1. Map edge (triggers map connection; the tile in front — the connected
///    map's edge tile — must pass the same rules as an in-map move)
/// 2. Ledge jump (special movement)
/// 3. Sprite collision (NPC blocking)
/// 4. Tile pair collision (elevation)
/// 5. Counter tile (can interact across but not walk through)
/// 6. Tile passability (wall/obstacle)
pub fn check_movement_collision<T: TilesetTrait>(
    player_x: u16,
    player_y: u16,
    direction: Direction,
    tileset: T,
    map_width_blocks: u8,
    map_height_blocks: u8,
    standing_tile: u8,
    target_tile: u8,
    transport: TransportMode,
    npc_positions: &[SpritePosition],
    held_input: u8,
    provider: &impl CollisionProvider<T>,
) -> CollisionResult {
    // 1. Check map edge (triggers map connection)
    let target_coords = get_target_coords(
        player_x,
        player_y,
        direction,
        map_width_blocks,
        map_height_blocks,
    );

    if target_coords.is_none() {
        // The player faces a map boundary. The original still checks the tile
        // in front — read from the connection strip, i.e. the connected map's
        // edge tile — and applies the normal rules before allowing the cross
        // (CollisionCheckOnLand/OnWater → CheckTilePassable). An impassable
        // seam bumps like any wall; only a passable (or, when surfing, water)
        // seam lets the player walk into the next map.
        if let Some(edge_tile) = provider.get_connection_edge_tile(
            tileset,
            map_width_blocks,
            map_height_blocks,
            player_x,
            player_y,
            direction,
        ) {
            // Surfing (CollisionCheckOnWater): water keeps surfing; stepping
            // onto a passable land tile ends the surf (the game layer
            // dismounts after the map swap); anything else is a collision.
            if transport == TransportMode::Surfing {
                if provider.check_tile_pair_collision(tileset, standing_tile, edge_tile, true) {
                    return CollisionResult::TilePairBlocked;
                }
                if provider.is_water_tile(tileset, edge_tile) {
                    return CollisionResult::MapEdge;
                }
                if provider.is_tile_passable(tileset, edge_tile) {
                    return CollisionResult::MapEdge;
                }
                return CollisionResult::TileBlocked;
            }

            // Walking / biking: same checks as an in-map move onto `edge_tile`.
            if provider.check_tile_pair_collision(tileset, standing_tile, edge_tile, false) {
                return CollisionResult::TilePairBlocked;
            }
            if provider.is_counter_tile(tileset, edge_tile) {
                return CollisionResult::CounterTile;
            }
            if !provider.is_tile_passable(tileset, edge_tile) {
                return CollisionResult::TileBlocked;
            }
        }
        return CollisionResult::MapEdge;
    }

    let (tx, ty) = target_coords.unwrap();

    // 2. Check ledge jump (only on land, only overworld tileset)
    if transport == TransportMode::Walking || transport == TransportMode::Biking {
        let sprite_facing = direction_to_sprite_facing(direction);
        if provider.check_ledge_jump(
            tileset,
            sprite_facing,
            standing_tile,
            target_tile,
            held_input,
        ) {
            return CollisionResult::LedgeJump;
        }
    }

    // 3. Check sprite collision
    if check_sprite_collision(tx, ty, npc_positions) {
        return CollisionResult::SpriteBlocked;
    }

    // 4. Surfing movement:
    // water tiles are traversable and keep the player surfing; stepping onto
    // a passable land tile ends the surf; everything else is a collision.
    if transport == TransportMode::Surfing {
        if provider.check_tile_pair_collision(tileset, standing_tile, target_tile, true) {
            return CollisionResult::TilePairBlocked;
        }
        if provider.is_water_tile(tileset, target_tile) {
            return CollisionResult::Passable;
        }
        if provider.is_tile_passable(tileset, target_tile) {
            return CollisionResult::StopSurfing;
        }
        return CollisionResult::TileBlocked;
    }

    // 5. Check tile pair collision
    let on_water = transport == TransportMode::Surfing;
    if provider.check_tile_pair_collision(tileset, standing_tile, target_tile, on_water) {
        return CollisionResult::TilePairBlocked;
    }

    // 6. Check counter tile
    if provider.is_counter_tile(tileset, target_tile) {
        return CollisionResult::CounterTile;
    }

    // 7. Check tile passability
    if !provider.is_tile_passable(tileset, target_tile) {
        return CollisionResult::TileBlocked;
    }

    CollisionResult::Passable
}

// ── Edge & Warp Detection ────────────────────────────────────────

/// Check if the player is facing the edge of the map.
pub fn is_facing_map_edge(
    player_x: u16,
    player_y: u16,
    direction: Direction,
    map_width_blocks: u8,
    map_height_blocks: u8,
) -> bool {
    get_target_coords(
        player_x,
        player_y,
        direction,
        map_width_blocks,
        map_height_blocks,
    )
    .is_none()
}

/// Check if the player is standing on a warp tile.
/// Returns the warp index if a match is found.
pub fn check_warp_at_position<M: crate::map::MapTrait, T: TilesetTrait, Mus>(
    x: u16,
    y: u16,
    map: &MapData<M, T, Mus>,
) -> Option<usize> {
    map.warps
        .iter()
        .position(|w| w.x as u16 == x && w.y as u16 == y)
}
