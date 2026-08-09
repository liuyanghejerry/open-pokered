//! Pokémon Red/Blue player movement — re-exports from jrpg-engine.

pub use jrpg_engine::overworld::player_movement::{
    advance_step, direction_delta, frames_per_step, get_tile_at_position, opposite_direction,
    process_frame, try_move, InputState, MoveResult, WALK_COUNTER_INIT,
};

use jrpg_engine::tileset::TilesetTrait;
use pokered_data::tilesets::TilesetId;

/// Check if the player is currently on a grass tile.
/// Used to determine if wild encounters should be checked.
pub fn is_on_grass<T: TilesetTrait>(standing_tile: u8, tileset: T) -> bool {
    let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
    pokered_data::tileset_data::get_tileset_header(concrete).is_grass_tile(standing_tile)
}
