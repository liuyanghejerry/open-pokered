//! Pokémon-specific NPC movement helpers and engine re-exports.
//!
//! Generic NPC movement (NpcRuntimeState, update_npc_movement, etc.) is
//! provided by `dotzuki_engine::overworld::npc_movement`. This module adds
//! Pokémon-specific data conversion functions.

use crate::alloc_prelude::*;
use alloc::collections::VecDeque;

use dotzuki_engine::overworld::{Direction, NpcMovementType};
use pokered_data::npc_data::{NpcEntry, NpcFacing, NpcMovement};

// Re-export engine NPC movement types and functions
pub use dotzuki_engine::overworld::npc_movement::{
    direction_toward, get_npc_positions, is_scripted_move_done, npc_at_position,
    npc_at_position_mut, npc_in_front_of_player, start_scripted_move, NpcRuntimeState,
    NPC_MAX_DELAY, NPC_WALK_FRAMES,
};

/// Use the original four direction intervals, axis remapping and zero-delay
/// wrap without changing the generic engine's default wandering policy.
pub fn update_npc_movement<T: dotzuki_engine::tileset::TilesetTrait>(
    npcs: &mut [NpcRuntimeState],
    player_x: u16,
    player_y: u16,
    player_dest: Option<(u16, u16)>,
    map_width_blocks: u8,
    map_height_blocks: u8,
    rng_value: u8,
    blocks: &[u8],
    tileset: T,
    provider: &impl dotzuki_engine::overworld::CollisionProvider<T>,
) {
    dotzuki_engine::overworld::update_npc_movement_with_policy(
        npcs,
        player_x,
        player_y,
        player_dest,
        map_width_blocks,
        map_height_blocks,
        rng_value,
        blocks,
        tileset,
        provider,
        dotzuki_engine::overworld::NpcWanderPolicy::Classic,
    );
}

// ── Pokémon-Specific Conversions ───────────────────────────────────

pub fn convert_movement(m: NpcMovement) -> NpcMovementType {
    match m.0 {
        0 => NpcMovementType::Stationary,
        1 => NpcMovementType::Wander,
        2 => NpcMovementType::FixedPath,
        3 => NpcMovementType::FacePlayer,
        _ => NpcMovementType::Stationary,
    }
}

/// The generator's `range` field carries the classic movement byte 2 for
/// Wander NPCs (parse_npcs.py DIRECTION_MAP: 0 = ANY_DIR, 1 = UP_DOWN,
/// 2 = LEFT_RIGHT). Map it onto the engine's axis restriction.
pub fn convert_wander_axis(range_code: u8) -> dotzuki_engine::overworld::NpcWanderAxis {
    use dotzuki_engine::overworld::NpcWanderAxis;
    match range_code {
        1 => NpcWanderAxis::Vertical,
        2 => NpcWanderAxis::Horizontal,
        _ => NpcWanderAxis::Any,
    }
}

pub fn convert_facing(f: NpcFacing) -> Direction {
    match f.0 {
        0 => Direction::Down,
        1 => Direction::Up,
        2 => Direction::Left,
        3 => Direction::Right,
        _ => Direction::Down,
    }
}

/// Load NPC runtime states from Pokémon data entries.
///
/// Pokémon-specific fields (is_trainer, trainer_class, trainer_set, item_id)
/// are stored in the parallel `PokemonNpcData` array at the same index.
pub fn load_map_npcs(npcs: &[NpcEntry]) -> Vec<NpcRuntimeState> {
    npcs.iter()
        .enumerate()
        .map(|(i, npc)| NpcRuntimeState {
            npc_index: i as u8,
            sprite_id: npc.sprite_id,
            x: npc.x as u16,
            y: npc.y as u16,
            home_x: npc.x as u16,
            home_y: npc.y as u16,
            facing: convert_facing(npc.facing),
            scripted_frame: None,
            movement_type: convert_movement(npc.movement),
            wander_axis: convert_wander_axis(npc.range),
            range: npc.range,
            walk_counter: 0,
            delay_counter: 0,
            text_id: npc.text_id,
            defeated: false,
            visible: true,
            scripted_path: VecDeque::new(),
        })
        .collect()
}
