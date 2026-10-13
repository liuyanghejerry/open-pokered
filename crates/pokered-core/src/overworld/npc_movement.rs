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

/// STAY reaches TryWalking's direction selection but cannot translate.
/// Two random draws select direction and the subsequent delay separately.
/// Legacy stationary range zero retains its existing fixed-facing behavior.
pub fn advance_stationary_facing(npc: &mut NpcRuntimeState, player_moving: bool,
    direction_random: u8, delay_random: u8) {
    if !npc.visible || npc.movement_type != NpcMovementType::Stationary
        || npc.walk_counter != 0 || !npc.scripted_path.is_empty()
        || !matches!(npc.range, 0xD0..=0xD3 | 0xFC..=0xFF) { return; }
    if npc.delay_counter != 0 {
        npc.delay_counter -= 1;
        return;
    }
    if player_moving { return; }
    let index = if matches!(npc.range, 0xD0..=0xD3) {npc.range - 0xD0}
        else {direction_random >> 6};
    npc.facing = match npc.range {
        0xFC => [Direction::Down, Direction::Up, Direction::Up, Direction::Down][index as usize],
        0xFD => [Direction::Left, Direction::Right, Direction::Left, Direction::Right][index as usize],
        _ => [Direction::Down, Direction::Up, Direction::Left, Direction::Right][index as usize],
    };
    let delay = delay_random & 0x7f;
    npc.delay_counter = if delay == 0 {256} else {u16::from(delay)};
}

pub fn stationary_range(movement: NpcMovementType, range: u8, facing: Direction) -> u8 {
    if movement == NpcMovementType::Stationary && range == 0 {
        0xD0 + match facing {Direction::Down=>0, Direction::Up=>1, Direction::Left=>2, Direction::Right=>3}
    } else if movement == NpcMovementType::Stationary && range == 1 {0xFC}
    else if movement == NpcMovementType::Stationary && range == 2 {0xFD}
    else {range}
}

/// CheckSpriteAvailability's map rectangle (stored sprite coordinates add 4).
/// Hidden/off-screen STAY objects do not decrement their delay or draw RNG.
pub fn stationary_available(npc: &NpcRuntimeState, player: (u16, u16)) -> bool {
    let dx = i32::from(npc.x) - i32::from(player.0);
    let dy = i32::from(npc.y) - i32::from(player.1);
    npc.visible && (-4..=5).contains(&dx) && (-4..=4).contains(&dy)
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
            range: stationary_range(convert_movement(npc.movement), npc.range, convert_facing(npc.facing)),
            walk_counter: 0,
            delay_counter: 0,
            text_id: npc.text_id,
            defeated: false,
            visible: true,
            scripted_path: VecDeque::new(),
        })
        .collect()
}

#[cfg(test)]
mod stationary_tests {
    use super::*;
    #[test]
    fn stationary_facing_matches_all_sixty_four_original_routine_probes() {
        let d: serde_json::Value = serde_json::from_str(include_str!("stationary_facing_golden.json")).unwrap();
        for row in d["cases"].as_array().unwrap() {
            let mut n = load_map_npcs(pokered_data::npc_data::get_map_npcs(pokered_data::maps::MapId::PalletTown))[0].clone();
            n.visible=true;n.movement_type=NpcMovementType::Stationary;
            n.range=match row["movement_byte2"].as_u64().unwrap() {0=>254,1=>252,2=>253,v=>v as u8};
            n.facing=Direction::Up;n.walk_counter=0;n.delay_counter=0;n.scripted_path.clear();
            let origin=(n.x,n.y);let delay=row["delay"].as_u64().unwrap() as u8;
            advance_stationary_facing(&mut n,false,row["random_direction_byte"].as_u64().unwrap() as u8,delay);
            let expected=match row["facing"].as_u64().unwrap() {0=>Direction::Down,4=>Direction::Up,8=>Direction::Left,12=>Direction::Right,_=>unreachable!()};
            assert_eq!(n.facing,expected);assert_eq!((n.x,n.y),origin);assert_eq!(n.walk_counter,0);
            assert_eq!(n.delay_counter,if delay==0 {256} else {u16::from(delay)});
        }
    }
    #[test]
    fn stationary_delay_ticks_during_player_walk_but_ready_turn_waits() {
        let mut n=load_map_npcs(pokered_data::npc_data::get_map_npcs(pokered_data::maps::MapId::PalletTown))[0].clone();
        n.visible=true;n.range=255;n.delay_counter=0;n.facing=Direction::Down;
        advance_stationary_facing(&mut n,false,192,0);
        assert_eq!(n.facing,Direction::Right);assert_eq!(n.delay_counter,256);
        for _ in 0..256 {advance_stationary_facing(&mut n,true,0,1);}
        assert_eq!(n.delay_counter,0);assert_eq!(n.facing,Direction::Right);
        advance_stationary_facing(&mut n,true,0,1);assert_eq!(n.facing,Direction::Right);
        advance_stationary_facing(&mut n,false,0,1);assert_eq!(n.facing,Direction::Down);
        n.visible=false;advance_stationary_facing(&mut n,false,64,1);assert_eq!(n.delay_counter,1);
    }
}
