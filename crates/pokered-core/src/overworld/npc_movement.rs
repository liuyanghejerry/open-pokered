//! Pokémon-specific NPC movement helpers and engine re-exports.
//!
//! Generic NPC movement (NpcRuntimeState, update_npc_movement, etc.) is
//! provided by `dotzuki_engine::overworld::npc_movement`. This module adds
//! Pokémon-specific data conversion functions.

use std::collections::VecDeque;

use dotzuki_engine::overworld::{Direction, NpcMovementType};
use pokered_data::npc_data::{NpcEntry, NpcFacing, NpcMovement};

// Re-export engine NPC movement types and functions
pub use dotzuki_engine::overworld::npc_movement::{
    direction_toward, get_npc_positions, is_scripted_move_done, npc_at_position,
    npc_at_position_mut, npc_in_front_of_player, start_scripted_move, update_npc_movement,
    NpcRuntimeState, NPC_MAX_DELAY, NPC_WALK_FRAMES,
};

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
