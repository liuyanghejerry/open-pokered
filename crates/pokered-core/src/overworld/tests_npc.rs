use super::collision::PokemonCollisionProvider;
use super::npc_interaction::*;
use super::npc_movement::*;
use super::*;
use dotzuki_engine::overworld::MapData;
use pokered_data::maps::MapId;
use pokered_data::music::MusicId;
use pokered_data::npc_data::{get_map_npcs, NpcFacing, NpcMovement};
use pokered_data::sign_data::get_map_signs;
use pokered_data::tilesets::TilesetId;
use std::collections::VecDeque;

fn provider() -> PokemonCollisionProvider {
    PokemonCollisionProvider::new(MapId::PalletTown, TilesetId::Overworld)
}

// ── Data Integrity Tests ───────────────────────────────────────────

#[test]
fn pallet_town_has_npcs() {
    let npcs = get_map_npcs(MapId::PalletTown);
    assert!(!npcs.is_empty(), "PalletTown should have NPCs");
}

#[test]
fn pallet_town_npc_count() {
    let npcs = get_map_npcs(MapId::PalletTown);
    assert_eq!(npcs.len(), 3, "PalletTown has 3 NPCs: Oak, Girl, Fisher");
}

#[test]
fn oaks_lab_has_npcs() {
    let npcs = get_map_npcs(MapId::OaksLab);
    assert!(
        npcs.len() >= 3,
        "Oak's Lab should have at least Oak + 2 NPCs"
    );
}

#[test]
fn viridian_city_has_signs() {
    let signs = get_map_signs(MapId::ViridianCity);
    assert!(!signs.is_empty(), "ViridianCity should have signs");
}

#[test]
fn pewter_city_gym_has_trainers() {
    let npcs = get_map_npcs(MapId::PewterGym);
    let trainers: Vec<_> = npcs.iter().filter(|n| n.is_trainer).collect();
    assert!(!trainers.is_empty(), "Pewter Gym should have trainer NPCs");
}

#[test]
fn cerulean_city_has_npcs() {
    let npcs = get_map_npcs(MapId::CeruleanCity);
    assert!(!npcs.is_empty());
}

#[test]
fn viridian_forest_has_trainers() {
    let npcs = get_map_npcs(MapId::ViridianForest);
    let trainers: Vec<_> = npcs.iter().filter(|n| n.is_trainer).collect();
    assert!(
        !trainers.is_empty(),
        "Viridian Forest should have bug catcher trainers"
    );
}

#[test]
fn empty_map_returns_empty_npcs() {
    let npcs = get_map_npcs(MapId::DiglettsCave);
    assert!(npcs.is_empty(), "Diglett's Cave has no NPCs");
}

#[test]
fn total_npc_count_918() {
    let mut total = 0usize;
    for i in 0..248u8 {
        if let Some(map) = MapId::from_u8(i) {
            total += get_map_npcs(map).len();
        }
    }
    assert_eq!(total, 918, "Total NPCs across all maps should be 918");
}

#[test]
fn total_sign_count() {
    let mut total = 0usize;
    for i in 0..248u8 {
        if let Some(map) = MapId::from_u8(i) {
            total += get_map_signs(map).len();
        }
    }
    // 234 + 106: the hidden-event fill (tools/fill_hidden_events.py) ported
    // the remaining sign-like hidden events (gym statues, bench guys, PCs,
    // trash texts, slot machines, house flavor texts, ...) as signs at the
    // reference coordinates, plus the PokemonMansion2F switch sign, the two
    // FightingDojo wall texts, the CeladonMansionRoofHouse blackboard (×2)
    // and the ViridianSchoolHouse blackboard.
    assert_eq!(total, 340, "Total signs across all maps should be 340");
}

#[test]
fn npc_sprite_ids_nonzero() {
    for i in 0..248u8 {
        if let Some(map) = MapId::from_u8(i) {
            for npc in get_map_npcs(map) {
                assert!(npc.sprite_id > 0, "NPC in {:?} has sprite_id 0", map);
            }
        }
    }
}

#[test]
fn npc_text_ids_nonzero() {
    for i in 0..248u8 {
        if let Some(map) = MapId::from_u8(i) {
            for npc in get_map_npcs(map) {
                assert!(npc.text_id > 0, "NPC in {:?} has text_id 0", map);
            }
        }
    }
}

#[test]
fn trainer_npcs_have_class() {
    for i in 0..248u8 {
        if let Some(map) = MapId::from_u8(i) {
            for npc in get_map_npcs(map) {
                if npc.is_trainer {
                    assert!(
                        npc.trainer_class > 0,
                        "Trainer in {:?} has trainer_class 0",
                        map
                    );
                }
            }
        }
    }
}

#[test]
fn item_npcs_have_item_id() {
    for i in 0..248u8 {
        if let Some(map) = MapId::from_u8(i) {
            for npc in get_map_npcs(map) {
                if npc.item_id != 0 {
                    assert!(
                        !npc.is_trainer,
                        "NPC in {:?} has both is_trainer and item_id set",
                        map
                    );
                }
            }
        }
    }
}

// ── Movement Conversion Tests ──────────────────────────────────────

#[test]
fn convert_movement_stationary() {
    assert_eq!(
        convert_movement(NpcMovement::STATIONARY),
        NpcMovementType::Stationary
    );
}

#[test]
fn convert_movement_wander() {
    assert_eq!(
        convert_movement(NpcMovement::WANDER),
        NpcMovementType::Wander
    );
}

#[test]
fn convert_movement_fixed_path() {
    assert_eq!(
        convert_movement(NpcMovement::FIXED_PATH),
        NpcMovementType::FixedPath
    );
}

#[test]
fn convert_movement_face_player() {
    assert_eq!(
        convert_movement(NpcMovement::FACE_PLAYER),
        NpcMovementType::FacePlayer
    );
}

#[test]
fn convert_facing_all_directions() {
    assert_eq!(convert_facing(NpcFacing::DOWN), Direction::Down);
    assert_eq!(convert_facing(NpcFacing::UP), Direction::Up);
    assert_eq!(convert_facing(NpcFacing::LEFT), Direction::Left);
    assert_eq!(convert_facing(NpcFacing::RIGHT), Direction::Right);
}

// ── NPC Loading Tests ──────────────────────────────────────────────

#[test]
fn load_map_npcs_pallet_town() {
    let data = get_map_npcs(MapId::PalletTown);
    let runtime = load_map_npcs(data);
    assert_eq!(runtime.len(), data.len());
    for (i, npc) in runtime.iter().enumerate() {
        assert_eq!(npc.npc_index, i as u8);
        assert_eq!(npc.x, data[i].x as u16);
        assert_eq!(npc.y, data[i].y as u16);
        assert_eq!(npc.home_x, data[i].x as u16);
        assert_eq!(npc.home_y, data[i].y as u16);
        assert!(!npc.defeated);
        assert!(npc.visible);
    }
}

#[test]
fn get_npc_positions_filters_invisible() {
    let data = get_map_npcs(MapId::PalletTown);
    let mut runtime = load_map_npcs(data);
    let full_count = get_npc_positions(&runtime).len();

    runtime[0].visible = false;
    let reduced = get_npc_positions(&runtime);
    assert_eq!(reduced.len(), full_count - 1);
}

// ── NPC Movement Tests ─────────────────────────────────────────────

fn make_test_npc(x: u16, y: u16, movement: NpcMovementType) -> NpcRuntimeState {
    NpcRuntimeState {
        npc_index: 0,
        sprite_id: 1,
        x,
        y,
        home_x: x,
        home_y: y,
        facing: Direction::Down,
        scripted_frame: None,
        movement_type: movement,
        wander_axis: dotzuki_engine::overworld::NpcWanderAxis::Any,
        range: 2,
        walk_counter: 0,
        delay_counter: 0,
        text_id: 1,
        defeated: false,
        visible: true,
        scripted_path: VecDeque::new(),
    }
}

#[test]
fn stationary_npc_does_not_move() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    for rng in 0..=255u8 {
        update_npc_movement(
            &mut npcs,
            0,
            0,
            None,
            10,
            10,
            rng,
            &[],
            TilesetId::Overworld,
            &provider(),
        );
    }
    assert_eq!(npcs[0].x, 5);
    assert_eq!(npcs[0].y, 5);
}

#[test]
fn face_player_npc_turns_toward_player() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::FacePlayer)];

    update_npc_movement(&mut npcs, 8, 5, None, 10, 10, 0, &[], TilesetId::Overworld, &provider());
    assert_eq!(npcs[0].facing, Direction::Right);

    update_npc_movement(&mut npcs, 2, 5, None, 10, 10, 0, &[], TilesetId::Overworld, &provider());
    assert_eq!(npcs[0].facing, Direction::Left);

    update_npc_movement(&mut npcs, 5, 8, None, 10, 10, 0, &[], TilesetId::Overworld, &provider());
    assert_eq!(npcs[0].facing, Direction::Down);

    update_npc_movement(&mut npcs, 5, 2, None, 10, 10, 0, &[], TilesetId::Overworld, &provider());
    assert_eq!(npcs[0].facing, Direction::Up);
}

#[test]
fn wander_npc_has_no_radial_leash() {
    // Classic random walkers have NO leash: they walk until blocked
    // (movement.asm:195-251). The axis byte is the restriction — see
    // wander_npc_respects_axis.
    let mut npcs = vec![make_test_npc(10, 10, NpcMovementType::Wander)];
    npcs[0].range = 2;

    for frame in 0..1000u32 {
        let rng = (frame * 7 + 13) as u8;
        update_npc_movement(
            &mut npcs,
            0,
            0,
            None,
            20,
            20,
            rng,
            &[],
            TilesetId::Overworld,
            &provider(),
        );
    }

    // No radial assertion: an unleashed walker may be anywhere the map
    // allowed. Sanity: it stays on the map.
    assert!(npcs[0].x < 20 && npcs[0].y < 20);
}

#[test]
fn wander_npc_respects_axis() {
    // UP_DOWN (movement byte 2 = $01): vertical-only wandering — the x
    // coordinate NEVER changes regardless of the rng stream.
    let mut npcs = vec![make_test_npc(10, 10, NpcMovementType::Wander)];
    npcs[0].wander_axis = dotzuki_engine::overworld::NpcWanderAxis::Vertical;
    for frame in 0..1000u32 {
        let rng = (frame * 7 + 13) as u8;
        update_npc_movement(
            &mut npcs, 0, 0, None, 20, 20, rng, &[], TilesetId::Overworld, &provider(),
        );
    }
    assert_eq!(npcs[0].x, 10, "a vertical walker never changes column");

    // LEFT_RIGHT ($02): horizontal-only — y never changes.
    let mut npcs = vec![make_test_npc(10, 10, NpcMovementType::Wander)];
    npcs[0].wander_axis = dotzuki_engine::overworld::NpcWanderAxis::Horizontal;
    for frame in 0..1000u32 {
        let rng = (frame * 7 + 13) as u8;
        update_npc_movement(
            &mut npcs, 0, 0, None, 20, 20, rng, &[], TilesetId::Overworld, &provider(),
        );
    }
    assert_eq!(npcs[0].y, 10, "a horizontal walker never changes row");
}

#[test]
fn walking_npc_completes_step() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Wander)];
    npcs[0].walk_counter = NPC_WALK_FRAMES;
    npcs[0].facing = Direction::Right;

    for _ in 0..NPC_WALK_FRAMES {
        update_npc_movement(&mut npcs, 0, 0, None, 10, 10, 0, &[], TilesetId::Overworld, &provider());
    }

    assert_eq!(npcs[0].x, 6, "NPC should have moved one tile right");
    assert_eq!(npcs[0].walk_counter, 0);
}

#[test]
fn npc_at_position_finds_visible() {
    let npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    assert!(npc_at_position(&npcs, 5, 5).is_some());
    assert!(npc_at_position(&npcs, 5, 6).is_none());
}

#[test]
fn npc_at_position_skips_invisible() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    npcs[0].visible = false;
    assert!(npc_at_position(&npcs, 5, 5).is_none());
}

#[test]
fn npc_in_front_of_player_test() {
    let npcs = vec![make_test_npc(5, 4, NpcMovementType::Stationary)];
    assert!(npc_in_front_of_player(&npcs, 5, 5, Direction::Up, None::<&MapData<MapId, TilesetId, MusicId>>, &provider()).is_some());
    assert!(npc_in_front_of_player(&npcs, 5, 5, Direction::Down, None::<&MapData<MapId, TilesetId, MusicId>>, &provider()).is_none());
}

// ── NPC Interaction Tests ──────────────────────────────────────────

fn make_trainer_extra() -> PokemonNpcData {
    PokemonNpcData {
        is_trainer: true,
        trainer_class: 9,
        trainer_set: 1,
        item_id: 0,
        end_battle_text: None,
    }
}

fn make_item_extra(item_id: u8) -> PokemonNpcData {
    PokemonNpcData {
        is_trainer: false,
        trainer_class: 0,
        trainer_set: 0,
        item_id,
        end_battle_text: None,
    }
}

fn no_extra() -> PokemonNpcData {
    PokemonNpcData {
        is_trainer: false,
        trainer_class: 0,
        trainer_set: 0,
        item_id: 0,
        end_battle_text: None,
    }
}

#[test]
fn interact_no_target() {
    let npcs = vec![];
    let data: Vec<PokemonNpcData> = vec![];
    assert_eq!(
        try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider()),
        InteractionResult::NoTarget
    );
}

#[test]
fn interact_talk_regular_npc() {
    let npcs = vec![make_test_npc(5, 4, NpcMovementType::Stationary)];
    let data = vec![no_extra()];
    match try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider()) {
        InteractionResult::Talk { text_id, .. } => assert_eq!(text_id, 1),
        other => panic!("Expected Talk, got {:?}", other),
    }
}

#[test]
fn interact_trainer_battle() {
    let npcs = vec![make_test_npc(5, 4, NpcMovementType::Stationary)];
    let data = vec![make_trainer_extra()];
    match try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider()) {
        InteractionResult::TrainerBattle {
            trainer_class,
            trainer_set,
            ..
        } => {
            assert_eq!(trainer_class, 9);
            assert_eq!(trainer_set, 1);
        }
        other => panic!("Expected TrainerBattle, got {:?}", other),
    }
}

#[test]
fn interact_item_pickup() {
    let npcs = vec![make_test_npc(5, 4, NpcMovementType::Stationary)];
    let data = vec![make_item_extra(0x14)];
    match try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider()) {
        InteractionResult::ItemPickup { item_id, .. } => assert_eq!(item_id, 0x14),
        other => panic!("Expected ItemPickup, got {:?}", other),
    }
}

#[test]
fn interact_defeated_trainer() {
    let mut npcs = vec![make_test_npc(5, 4, NpcMovementType::Stationary)];
    npcs[0].defeated = true;
    let data = vec![make_trainer_extra()];
    match try_interact(&npcs, &data, 5, 5, Direction::Up, None, &provider()) {
        InteractionResult::AlreadyDefeated { text_id, .. } => assert_eq!(text_id, 1),
        other => panic!("Expected AlreadyDefeated, got {:?}", other),
    }
}

#[test]
fn collect_item_marks_defeated_and_invisible() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    let data = vec![make_item_extra(0x20)];
    let item = collect_item(&mut npcs, &data, 0);
    assert_eq!(item, Some(0x20));
    assert!(npcs[0].defeated);
    assert!(!npcs[0].visible);
}

#[test]
fn collect_item_already_taken() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    npcs[0].defeated = true;
    let data = vec![make_item_extra(0x20)];
    assert_eq!(collect_item(&mut npcs, &data, 0), None);
}

#[test]
fn mark_trainer_defeated_test() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    assert!(!npcs[0].defeated);
    mark_trainer_defeated(&mut npcs, 0);
    assert!(npcs[0].defeated);
}

// ── Trainer Line of Sight Tests ────────────────────────────────────

#[test]
fn trainer_sees_player_in_range() {
    let mut npcs = vec![make_test_npc(5, 2, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].range = 4;
    let data = vec![make_trainer_extra()];

    let result = check_trainer_line_of_sight(&npcs, &data, 5, 5);
    assert!(result.is_some());
    let sighting = result.unwrap();
    assert_eq!(sighting.distance, 3);
    assert_eq!(sighting.trainer_class, 9);
}

#[test]
fn trainer_does_not_see_behind() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].range = 4;
    let data = vec![make_trainer_extra()];

    let result = check_trainer_line_of_sight(&npcs, &data, 5, 2);
    assert!(
        result.is_none(),
        "Trainer facing down should not see player above"
    );
}

#[test]
fn trainer_does_not_see_out_of_range() {
    let mut npcs = vec![make_test_npc(5, 2, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].range = 2;
    let data = vec![make_trainer_extra()];

    let result = check_trainer_line_of_sight(&npcs, &data, 5, 8);
    assert!(result.is_none(), "Player at distance 6 exceeds range 2");
}

#[test]
fn defeated_trainer_does_not_see() {
    let mut npcs = vec![make_test_npc(5, 2, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].range = 4;
    npcs[0].defeated = true;
    let data = vec![make_trainer_extra()];

    assert!(check_trainer_line_of_sight(&npcs, &data, 5, 5).is_none());
}

#[test]
fn trainer_range_zero_never_triggers() {
    let mut npcs = vec![make_test_npc(5, 4, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].range = 0;
    let data = vec![make_trainer_extra()];

    assert!(check_trainer_line_of_sight(&npcs, &data, 5, 5).is_none());
}

// ── Sign Interaction Tests ─────────────────────────────────────────

#[test]
fn sign_interaction_found() {
    let signs = vec![(5u8, 4u8, 3u8)];
    assert_eq!(check_sign_interaction(&signs, 5, 5, Direction::Up), Some(3));
}

#[test]
fn sign_interaction_wrong_direction() {
    let signs = vec![(5u8, 4u8, 3u8)];
    assert_eq!(check_sign_interaction(&signs, 5, 5, Direction::Down), None);
}

#[test]
fn sign_interaction_no_sign() {
    let signs: Vec<(u8, u8, u8)> = vec![];
    assert_eq!(check_sign_interaction(&signs, 5, 5, Direction::Up), None);
}

// ── Real-map trainer sight regression ──────────────────────────────

/// Regression: the trainer-LOS engage path creates the "!" emotion bubble
/// without a script effect, and the bubble countdown used to tick only
/// inside tick_active_effect — so it never moved, the engage intro waited
/// on `frames_remaining == 0` forever, and no battle ever started.
#[test]
fn viridian_forest_trainer_engages_on_sight_line() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;

    let mut screen = OverworldScreen::new(MapId::ViridianForest, None, PokemonRedData);
    // Bug Catcher (textId 2) stands at (30,33) facing Left with range 2:
    // his sight line covers (28,33) and (29,33).
    screen.state.player.x = 29;
    screen.state.player.y = 33;
    for _ in 0..120 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
        if let Some(ref pending) = screen.pending_trainer_battle {
            assert_eq!(pending.trainer_id, "OPP_BUG_CATCHER1");
            // The intro is done: bubble fully counted down (cleared to None
            // on the following frame, same as the script-effect path), and
            // the engage state consumed.
            assert!(screen
                .pending_emotion_bubble
                .as_ref()
                .map_or(true, |b| b.frames_remaining == 0));
            assert!(screen.trainer_encounter_intro.is_none());
            return;
        }
    }
    panic!("trainer never engaged on sight tile (29,33)");
}
