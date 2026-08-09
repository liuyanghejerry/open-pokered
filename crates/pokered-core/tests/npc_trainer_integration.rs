//! M9.3 Integration tests — NPC interaction + trainer encounter + line-of-sight.

use pokered_core::overworld::npc_interaction::{
    check_sign_interaction, check_trainer_line_of_sight, collect_item, mark_trainer_defeated,
    try_interact, InteractionResult,
};
use pokered_core::overworld::npc_movement::{load_map_npcs, NpcRuntimeState};
use pokered_core::overworld::PokemonNpcData;
use pokered_data::maps::MapId;
use pokered_data::tilesets::TilesetId;

fn provider() -> pokered_core::overworld::collision::PokemonCollisionProvider {
    pokered_core::overworld::collision::PokemonCollisionProvider::new(MapId::PalletTown, TilesetId::Overworld)
}
use pokered_core::overworld::trainer_engine::{
    advance_trainer_battle, TrainerBattleState, TrainerEncounter,
};
use pokered_core::overworld::Direction;
use pokered_data::npc_data::get_map_npcs;
use std::collections::VecDeque;

/// Create a generic NpcRuntimeState and its matching PokemonNpcData.
fn make_npc(
    index: u8,
    x: u16,
    y: u16,
    facing: Direction,
    is_trainer: bool,
    trainer_class: u8,
    range: u8,
    item_id: u8,
) -> (NpcRuntimeState, PokemonNpcData) {
    let npc = NpcRuntimeState {
        npc_index: index,
        sprite_id: 1,
        x,
        y,
        home_x: x,
        home_y: y,
        facing,
        scripted_frame: None,
        movement_type: pokered_core::overworld::NpcMovementType::Stationary,
        range,
        walk_counter: 0,
        delay_counter: 0,
        text_id: index + 1,
        defeated: false,
        visible: true,
        scripted_path: VecDeque::new(),
    };
    let data = PokemonNpcData {
        is_trainer,
        trainer_class,
        trainer_set: if is_trainer { 1 } else { 0 },
        item_id,
        end_battle_text: None,
    };
    (npc, data)
}

/// Helper: make a single NPC with data
fn make_single(
    index: u8,
    x: u16,
    y: u16,
    facing: Direction,
    is_trainer: bool,
    trainer_class: u8,
    range: u8,
    item_id: u8,
) -> (Vec<NpcRuntimeState>, Vec<PokemonNpcData>) {
    let (npc, data) = make_npc(index, x, y, facing, is_trainer, trainer_class, range, item_id);
    (vec![npc], vec![data])
}

// ── NPC Interaction Dispatch ─────────────────────────────────────────

#[test]
fn interact_regular_npc_returns_talk() {
    let (npcs, data) = make_single(0, 5, 4, Direction::Down, false, 0, 0, 0);
    let result = try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider());
    assert_eq!(
        result,
        InteractionResult::Talk {
            npc_index: 0,
            text_id: 1
        }
    );
}

#[test]
fn interact_trainer_npc_returns_battle() {
    let (npcs, data) = make_single(0, 5, 4, Direction::Down, true, 10, 3, 0);
    let result = try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider());
    assert_eq!(
        result,
        InteractionResult::TrainerBattle {
            npc_index: 0,
            trainer_class: 10,
            trainer_set: 1,
        }
    );
}

#[test]
fn interact_item_ball_returns_pickup() {
    let (npcs, data) = make_single(0, 5, 4, Direction::Down, false, 0, 0, 42);
    let result = try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider());
    assert_eq!(
        result,
        InteractionResult::ItemPickup {
            npc_index: 0,
            item_id: 42
        }
    );
}

#[test]
fn interact_defeated_trainer_returns_already_defeated() {
    let (mut npcs, data) = make_single(0, 5, 4, Direction::Down, true, 10, 3, 0);
    npcs[0].defeated = true;
    let result = try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider());
    assert!(matches!(
        result,
        InteractionResult::AlreadyDefeated { npc_index: 0, .. }
    ));
}

#[test]
fn interact_no_npc_in_front_returns_no_target() {
    let (npcs, data) = make_single(0, 10, 10, Direction::Down, false, 0, 0, 0);
    let result = try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider());
    assert_eq!(result, InteractionResult::NoTarget);
}

#[test]
fn interact_invisible_npc_returns_no_target() {
    let (mut npcs, data) = make_single(0, 5, 4, Direction::Down, false, 0, 0, 0);
    npcs[0].visible = false;
    let result = try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider());
    assert_eq!(result, InteractionResult::NoTarget);
}

// ── Trainer Line of Sight ────────────────────────────────────────────

#[test]
fn trainer_sees_player_in_sight_range() {
    let (npcs, data) = make_single(0, 5, 2, Direction::Down, true, 10, 4, 0);
    let sighting = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(sighting.is_some());
    let s = sighting.unwrap();
    assert_eq!(s.npc_index, 0);
    assert_eq!(s.distance, 3);
}

#[test]
fn trainer_does_not_see_player_beyond_range() {
    let (npcs, data) = make_single(0, 5, 2, Direction::Down, true, 10, 2, 0);
    let sighting = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(sighting.is_none(), "player is 3 tiles away but range is 2");
}

#[test]
fn trainer_does_not_see_player_behind() {
    let (npcs, data) = make_single(0, 5, 5, Direction::Down, true, 10, 5, 0);
    let sighting = check_trainer_line_of_sight(&npcs, &data, 5, 2);
    assert!(sighting.is_none(), "player is behind the trainer");
}

#[test]
fn trainer_does_not_see_player_off_axis() {
    let (npcs, data) = make_single(0, 5, 2, Direction::Down, true, 10, 5, 0);
    let sighting = check_trainer_line_of_sight(&npcs, &data, 6, 5);
    assert!(sighting.is_none(), "player is not on the same axis");
}

#[test]
fn defeated_trainer_does_not_see_player() {
    let (mut npcs, data) = make_single(0, 5, 2, Direction::Down, true, 10, 5, 0);
    npcs[0].defeated = true;
    let sighting = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(sighting.is_none());
}

#[test]
fn zero_range_trainer_never_spots_player() {
    let (npcs, data) = make_single(0, 5, 4, Direction::Down, true, 10, 0, 0);
    let sighting = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(sighting.is_none(), "range 0 = talk-only trainer");
}

#[test]
fn multiple_trainers_first_spotter_wins() {
    let (a, da) = make_npc(0, 5, 2, Direction::Down, true, 10, 5, 0);
    let (b, db) = make_npc(1, 8, 5, Direction::Left, true, 20, 5, 0);
    let npcs = vec![a, b];
    let data = vec![da, db];
    let sighting = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(sighting.is_some());
    assert_eq!(sighting.unwrap().npc_index, 0);
}

// ── Item Collection ──────────────────────────────────────────────────

#[test]
fn collect_item_marks_npc_defeated_and_hidden() {
    let (mut npcs, data) = make_single(0, 5, 4, Direction::Down, false, 0, 0, 33);
    let item = collect_item(&mut npcs, &data, 0);
    assert_eq!(item, Some(33));
    assert!(npcs[0].defeated);
    assert!(!npcs[0].visible);
}

#[test]
fn collect_item_already_collected_returns_none() {
    let (mut npcs, data) = make_single(0, 5, 4, Direction::Down, false, 0, 0, 33);
    npcs[0].defeated = true;
    let item = collect_item(&mut npcs, &data, 0);
    assert_eq!(item, None);
}

#[test]
fn mark_trainer_defeated_sets_flag() {
    let (mut npcs, _data) = make_single(0, 5, 4, Direction::Down, true, 10, 3, 0);
    assert!(!npcs[0].defeated);
    mark_trainer_defeated(&mut npcs, 0);
    assert!(npcs[0].defeated);
}

// ── Trainer Battle State Machine ─────────────────────────────────────

#[test]
fn trainer_encounter_full_state_machine() {
    let mut encounter = TrainerEncounter::new(MapId::PewterGym, 0, 1);
    assert_eq!(encounter.state, TrainerBattleState::NotEngaged);

    encounter.engage(5, 5, 3, 5);
    assert_eq!(encounter.state, TrainerBattleState::Spotted);

    for _ in 0..30 {
        advance_trainer_battle(&mut encounter);
    }

    assert_eq!(
        advance_trainer_battle(&mut encounter),
        TrainerBattleState::WalkingToPlayer
    );

    for _ in 0..33 {
        advance_trainer_battle(&mut encounter);
    }

    assert_eq!(
        advance_trainer_battle(&mut encounter),
        TrainerBattleState::ShowBeforeBattleText
    );
    assert_eq!(
        advance_trainer_battle(&mut encounter),
        TrainerBattleState::InBattle
    );
    assert_eq!(
        advance_trainer_battle(&mut encounter),
        TrainerBattleState::ShowEndBattleText
    );
    assert_eq!(
        advance_trainer_battle(&mut encounter),
        TrainerBattleState::Defeated
    );
    assert_eq!(
        advance_trainer_battle(&mut encounter),
        TrainerBattleState::Defeated
    );
}

// ── Sign Interaction ─────────────────────────────────────────────────

#[test]
fn sign_detected_when_facing_it() {
    let signs: Vec<(u8, u8, u8)> = vec![(5, 4, 3), (10, 10, 7)];
    let text = check_sign_interaction(&signs, 5, 5, Direction::Up);
    assert_eq!(text, Some(3));
}

#[test]
fn sign_not_detected_wrong_direction() {
    let signs: Vec<(u8, u8, u8)> = vec![(5, 4, 3)];
    let text = check_sign_interaction(&signs, 5, 5, Direction::Down);
    assert_eq!(text, None);
}

#[test]
fn sign_not_detected_no_sign_at_position() {
    let signs: Vec<(u8, u8, u8)> = vec![(10, 10, 7)];
    let text = check_sign_interaction(&signs, 5, 5, Direction::Up);
    assert_eq!(text, None);
}

// ── Real Map NPC Data Integration ────────────────────────────────────

#[test]
fn pewter_gym_has_real_npcs() {
    let npcs_data = get_map_npcs(MapId::PewterGym);
    assert!(!npcs_data.is_empty(), "Pewter Gym should have NPCs");

    let runtime = load_map_npcs(npcs_data);
    assert_eq!(runtime.len(), npcs_data.len());

    // load_map_npcs no longer copies is_trainer into NpcRuntimeState.
    // Trainer data lives in a separate PokemonNpcData array.
    let trainer_data_count = npcs_data.iter().filter(|n| n.is_trainer).count();
    assert!(
        trainer_data_count > 0,
        "Pewter Gym should have at least one trainer NPC"
    );
}

#[test]
fn oaks_lab_npcs_loaded_correctly() {
    let npcs_data = get_map_npcs(MapId::OaksLab);
    let runtime = load_map_npcs(npcs_data);

    for npc in &runtime {
        assert!(npc.visible, "all NPCs should start visible");
        assert!(!npc.defeated, "all NPCs should start undefeated");
        assert_eq!(npc.walk_counter, 0, "all NPCs should start idle");
    }
}

#[test]
fn trainer_spot_then_defeat_then_recheck() {
    let (mut npcs, data) = make_single(0, 5, 2, Direction::Down, true, 10, 5, 0);

    // Trainer spots player
    let sighting = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(sighting.is_some());

    // Battle happens, mark defeated
    mark_trainer_defeated(&mut npcs, 0);

    // Can no longer spot
    let sighting2 = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(sighting2.is_none());

    // Player at (5,1) facing down → checks (5,2) where defeated trainer is
    let interaction = try_interact(&npcs, &data, 5, 1, Direction::Down, None, &provider());
    assert!(matches!(
        interaction,
        InteractionResult::AlreadyDefeated { .. }
    ));
}
