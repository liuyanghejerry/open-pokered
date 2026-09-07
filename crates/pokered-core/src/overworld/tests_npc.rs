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
    // 340 -> 339: the audit removed BillsHouse's bogus (5,4) sign — the port
    // had duplicated the PC interaction onto an empty tile; the original has
    // ONE BillsHousePC hidden event at (1,4) (hidden_events.asm BILLS_HOUSE),
    // which remains as the surviving sign.
    assert_eq!(total, 339, "Total signs across all maps should be 339");
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
// The engage distance comes from the trainer-header table (original
// `def_trainers` view range), not the NPC's map range byte.

fn los_headers(views: &[u8]) -> Vec<pokered_data::trainer_headers::TrainerHeaderData> {
    use pokered_data::event_flags::EventFlag;
    views
        .iter()
        .map(|&v| pokered_data::trainer_headers::TrainerHeaderData {
            event_flag: EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0,
            sight_range: v,
        })
        .collect()
}

#[test]
fn trainer_sees_player_in_range() {
    let mut npcs = vec![make_test_npc(5, 2, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].range = 0; // STAY trainer: map range byte is NOT the sight range
    let data = vec![make_trainer_extra()];

    let result = check_trainer_line_of_sight(
        &npcs, &data, &los_headers(&[4]), &Default::default(), 5, 5,
    );
    assert!(result.is_some());
    let sighting = result.unwrap();
    assert_eq!(sighting.distance, 3);
    assert_eq!(sighting.trainer_class, 9);
}

#[test]
fn trainer_does_not_see_behind() {
    let mut npcs = vec![make_test_npc(5, 5, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    let data = vec![make_trainer_extra()];

    let result = check_trainer_line_of_sight(
        &npcs, &data, &los_headers(&[4]), &Default::default(), 5, 2,
    );
    assert!(
        result.is_none(),
        "Trainer facing down should not see player above"
    );
}

#[test]
fn trainer_does_not_see_out_of_range() {
    let mut npcs = vec![make_test_npc(5, 2, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    let data = vec![make_trainer_extra()];

    let result = check_trainer_line_of_sight(
        &npcs, &data, &los_headers(&[2]), &Default::default(), 5, 8,
    );
    assert!(result.is_none(), "Player at distance 6 exceeds view range 2");
}

#[test]
fn defeated_trainer_does_not_see() {
    let mut npcs = vec![make_test_npc(5, 2, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].defeated = true;
    let data = vec![make_trainer_extra()];

    assert!(check_trainer_line_of_sight(
        &npcs, &data, &los_headers(&[4]), &Default::default(), 5, 5
    )
    .is_none());
}

#[test]
fn trainer_view_zero_never_triggers() {
    // View range 0 = talk-only trainer (e.g. PokemonMansion scientists):
    // never engages by sight, even though the map range byte says 2.
    let mut npcs = vec![make_test_npc(5, 4, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    npcs[0].range = 2;
    let data = vec![make_trainer_extra()];

    assert!(check_trainer_line_of_sight(
        &npcs, &data, &los_headers(&[0]), &Default::default(), 5, 5
    )
    .is_none());
}

#[test]
fn beaten_trainer_flag_blocks_sight() {
    // CheckForEngagingTrainers skips trainers whose EVENT_BEAT_* flag is
    // set — our LOS must test the same flag, not just npc.defeated.
    use pokered_data::event_flags::EventFlag;
    let mut npcs = vec![make_test_npc(5, 2, NpcMovementType::Stationary)];
    npcs[0].facing = Direction::Down;
    let data = vec![make_trainer_extra()];

    let mut flags = super::event_flags::EventFlags::new();
    flags.set(EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0);
    assert!(check_trainer_line_of_sight(
        &npcs, &data, &los_headers(&[4]), &flags, 5, 5
    )
    .is_none());
}

#[test]
fn header_matches_kth_trainer_npc() {
    // The k-th header belongs to the k-th trainer NPC in object order:
    // npc 0 is a plain talker, npc 1 a short-sight trainer, npc 2 the
    // header-1 trainer (view 5). Player stands 3 tiles under npc 2 —
    // beyond npc 1's view but within npc 2's.
    let mut talker = make_test_npc(5, 2, NpcMovementType::Stationary);
    talker.facing = Direction::Down;
    talker.npc_index = 0;
    let mut a = make_test_npc(5, 7, NpcMovementType::Stationary);
    a.facing = Direction::Down;
    a.npc_index = 1;
    let mut b = make_test_npc(5, 9, NpcMovementType::Stationary);
    b.facing = Direction::Down;
    b.npc_index = 2;
    let npcs = vec![talker, a, b];
    let data = vec![
        PokemonNpcData {
            is_trainer: false,
            trainer_class: 0,
            trainer_set: 0,
            item_id: 0,
            end_battle_text: None,
        },
        make_trainer_extra(),
        make_trainer_extra(),
    ];

    let result = check_trainer_line_of_sight(
        &npcs, &data, &los_headers(&[1, 5]), &Default::default(), 5, 12,
    );
    let sighting = result.expect("npc 2 (header 1, view 5) sees the player");
    assert_eq!(sighting.npc_index, 2);
    assert_eq!(sighting.distance, 3);
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

// ── Stale edge detection on re-entry from a sub-screen ─────────────

/// The E4 exit doors are flag-gated tile swaps re-applied on every load
/// (BrunoShowOrHideExitBlock / AgathaShowOrHideExitBlock). The .blk ships the
/// Bruno exit OPEN, so the unbeaten `@else` branch must close it — the audit
/// walked out of BrunosRoom without fighting (bruno-skipped-without-battle).
#[test]
fn brunos_room_exit_door_blocked_until_beaten() {
    use super::screen::OverworldScreen;
    use pokered_data::event_flags::EventFlag;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::BrunosRoom, None, PokemonRedData);
    screen.run_on_load();
    let door = 2; // replaceTileBlock(2, 0, …) → block (x=2, y=0)
    // The @load autowalk suspends the script; the door write lands after the
    // walk drains through the update loop.
    for _ in 0..200 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        36,
        "unbeaten exit must be closed ($24) despite the open .blk default"
    );
    screen.set_event_flag_live(EventFlag::EVENT_BEAT_BRUNOS_ROOM_TRAINER_0);
    screen.rerun_map_on_load_script();
    for _ in 0..30 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        5,
        "beaten exit re-applied open ($5)"
    );
}

/// Agatha's exit opens the moment her battle is won — the writeback re-runs
/// the map `@load` (the original hands back to the per-frame script after
/// EndTrainerBattle), so no extra re-talk is needed. Regression for the
/// audit's agatha-door-after-extra-talk evidence.
#[test]
fn agathas_room_exit_opens_after_victory_without_retalk() {
    use super::screen::OverworldScreen;
    use pokered_data::event_flags::EventFlag;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::AgathasRoom, None, PokemonRedData);
    screen.run_on_load();
    let door = 2;
    // Drain the @load autowalk so the unbeaten door write lands.
    for _ in 0..200 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        59,
        "unbeaten exit closed ($3b)"
    );
    screen.set_event_flag_live(EventFlag::EVENT_BEAT_AGATHAS_ROOM_TRAINER_0);
    screen.rerun_map_on_load_script();
    for _ in 0..30 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        14,
        "victory re-applies the open exit ($0e) without a re-talk"
    );
    // A later re-entry keeps it open (the @load re-apply the audit relied on).
    // Two async warps × (fade-out + commit + fade-in) — pump enough frames.
    screen.warp_to_map(MapId::PalletTown, 5, 6);
    screen.warp_to_map(MapId::AgathasRoom, 4, 11);
    for _ in 0..160 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        14,
        "re-entry keeps the beaten exit open"
    );
}

#[test]
fn rocket_hideout_talk_only_guard_battles_after_scene_dialogue() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::RocketHideoutB4F, None, PokemonRedData);
    screen.run_on_load();
    screen.state.player.x = 26;
    screen.state.player.y = 13;
    screen.state.player.facing = Direction::Up;
    let input = |a| super::OverworldInput::new(false, false, false, false, a, false, false, false);
    for _ in 0..20 { screen.update_frame(input(false)); }
    screen.update_frame(input(true));
    let mut saw_dialogue = false;
    for frame in 0..600 {
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            saw_dialogue = true;
            dialogue.skip_to_full_page();
            assert!(screen.pending_trainer_battle.is_none(), "battle must await the dialogue");
        }
        screen.update_frame(input(frame % 2 == 1));
        if let Some(pending) = &screen.pending_trainer_battle {
            assert!(saw_dialogue);
            assert_eq!(pending.trainer_id, "OPP_ROCKET17");
            assert_eq!(pending.npc_index, 2);
            return;
        }
    }
    panic!("talk-only guard never started battle after dialogue");
}

#[test]
fn silph_boardroom_door_opens_from_corridor_only_with_card_key() {
    use pokered_data::impl_traits::PokemonRedData;
    for has_key in [false, true] {
        for x in [6, 7] {
            let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
            screen.warp_to_map(MapId::SilphCo11F, x, 14);
            let bag = if has_key { vec!["CARD_KEY".into()] } else { vec![] };
            screen.seed_script_query_state(0, &bag, 0, 0, 0, 0, &[], 0, 0, 0);
            let input = |a| super::OverworldInput::new(false, false, false, false, a, false, false, false);
            for _ in 0..100 {
                screen.seed_script_query_state(0, &bag, 0, 0, 0, 0, &[], 0, 0, 0);
                screen.update_frame(input(false));
            }
            screen.state.player.facing = Direction::Up;
            let door = 6 * screen.map_data.as_ref().unwrap().width as usize + 3;
            assert_eq!(screen.map_data.as_ref().unwrap().blocks[door], 32);
            screen.update_frame(input(true));
            for frame in 0..180 {
                if let Some(dialogue) = screen.pending_dialogue.as_mut() {
                    dialogue.skip_to_full_page();
                }
                screen.update_frame(input(frame % 2 == 1));
            }
            assert_eq!(screen.script_flags().get("EVENT_SILPH_CO_11_UNLOCKED_DOOR").copied().unwrap_or(false), has_key);
            assert_eq!(screen.map_data.as_ref().unwrap().blocks[door], if has_key { 3 } else { 32 });
        }
    }
}

#[test]
fn silph_third_floor_door_requires_key_and_stays_open_after_reentry() {
    use pokered_data::impl_traits::PokemonRedData;
    for has_key in [false, true] {
        let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        screen.warp_to_map(MapId::SilphCo3F, 18, 9);
        let bag = if has_key { vec!["CARD_KEY".into()] } else { vec![] };
        let input = |a| super::OverworldInput::new(false, false, false, false, a, false, false, false);
        for _ in 0..100 {
            screen.seed_script_query_state(0, &bag, 0, 0, 0, 0, &[], 0, 0, 0);
            screen.update_frame(input(false));
        }
        let door = 4 * screen.map_data.as_ref().unwrap().width as usize + 8;
        assert_eq!(screen.map_data.as_ref().unwrap().blocks[door], 95);
        screen.state.player.facing = Direction::Left;
        screen.update_frame(input(true));
        for frame in 0..180 {
            if let Some(dialogue) = screen.pending_dialogue.as_mut() {
                dialogue.skip_to_full_page();
            }
            screen.update_frame(input(frame % 2 == 1));
        }
        assert_eq!(screen.script_flags().get("EVENT_SILPH_CO_3_UNLOCKED_DOOR2").copied().unwrap_or(false), has_key);
        assert_eq!(screen.map_data.as_ref().unwrap().blocks[door], if has_key { 14 } else { 95 });
        screen.warp_to_map(MapId::PalletTown, 5, 6);
        screen.warp_to_map(MapId::SilphCo3F, 18, 9);
        for _ in 0..100 { screen.update_frame(input(false)); }
        assert_eq!(screen.map_data.as_ref().unwrap().blocks[door], if has_key { 14 } else { 95 });
    }
}

#[test]
fn saffron_liberation_clears_gym_guard_and_restores_citizens() {
    use pokered_data::impl_traits::PokemonRedData;
    for liberated in [false, true] {
        let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        screen.set_flag_live("EVENT_RESCUED_MR_FUJI", true);
        screen.set_flag_live("EVENT_BEAT_SILPH_CO_GIOVANNI", liberated);
        screen.warp_to_map(MapId::SaffronCity, 35, 4);
        for _ in 0..120 {
            screen.update_frame(super::OverworldInput::new(false, false, false, false, false, false, false, false));
        }
        for npc in &screen.npc_states {
            let expected = match npc.text_id {
                1..=7 => !liberated,
                8..=13 => liberated,
                14..=15 => false,
                _ => continue,
            };
            assert_eq!(npc.visible, expected, "NPC {} with liberated={liberated}", npc.text_id);
        }
    }
}

#[test]
fn mansion_statues_offer_and_apply_switch_from_adjacent_floor() {
    use pokered_data::impl_traits::PokemonRedData;
    for restored in [true, false] {
    for (map, x, y) in [(MapId::PokemonMansion3F, 10, 6),
                        (MapId::PokemonMansion1F, 2, 6),
                        (MapId::PokemonMansionB1F, 20, 4),
                        (MapId::PokemonMansionB1F, 18, 26)] {
        let mut screen = OverworldScreen::new(if restored { map } else { MapId::PalletTown }, None, PokemonRedData);
        if restored {
            screen.state.player.x = x as u16;
            screen.state.player.y = y as u16;
            screen.run_on_load();
        } else {
            screen.warp_to_map(map, x, y);
        }
        let input = |a| super::OverworldInput::new(false, false, false, false, a, false, false, false);
        for _ in 0..100 { screen.update_frame(input(false)); }
        screen.state.player.facing = Direction::Up;
        screen.seed_script_query_state(0, &[], 0, 0, 0, 0, &[], 0, 0, 0);
        screen.update_frame(input(true));
        for frame in 0..180 {
            if screen.pending_choice.is_some() { break; }
            if let Some(dialogue) = screen.pending_dialogue.as_mut() { dialogue.skip_to_full_page(); }
            screen.update_frame(input(frame % 2 == 1));
        }
        assert!(screen.pending_choice.is_some(), "statue on {map:?} above ({x},{y}) must offer YES/NO");
        screen.update_frame(input(false));
        screen.update_frame(input(true));
        for frame in 0..180 {
            if screen.script_flags().get("EVENT_MANSION_SWITCH_ON") == Some(&true) { break; }
            if let Some(dialogue) = screen.pending_dialogue.as_mut() { dialogue.skip_to_full_page(); }
            screen.update_frame(input(frame % 2 == 1));
        }
        assert_eq!(screen.script_flags().get("EVENT_MANSION_SWITCH_ON"), Some(&true));
    }
    }
}

/// Regression: returning to the overworld from the START menu with the A
/// button still held (the press that confirmed EXIT) used to re-fire as a
/// fresh A press on the first frame back — instantly talking to the facing
/// NPC. The original never does this: home/joypad.asm recomputes hJoyPressed
/// against hJoyReleased every frame, so a press consumed by the menu loop
/// can't re-fire in the overworld loop. Frontends now call `sync_prev_input`
/// on re-entry to re-baseline the overworld's edge detectors.
#[test]
fn reentry_with_a_still_held_does_not_talk_to_facing_npc() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;

    let a_input = super::OverworldInput::new(
        false, false, false, false, true, false, false, false,
    );
    let neutral = super::OverworldInput::new(
        false, false, false, false, false, false, false, false,
    );

    // Daisy sits at (2,3); stand beside her facing her.
    let mut screen = OverworldScreen::new(MapId::BluesHouse, None, PokemonRedData);
    screen.state.player.x = 1;
    screen.state.player.y = 3;
    screen.state.player.facing = Direction::Right;

    // Let the map settle (load effects, NPC ticks) — the overworld ran for a
    // while before the START menu was opened.
    for _ in 0..10 {
        screen.update_frame(neutral);
    }
    assert!(screen.pending_dialogue.is_none());
    assert!(screen.active_script_effect.is_none());

    // The START menu consumed the A press; on re-entry the frontend
    // re-baselines the edge detectors to the still-held A...
    screen.sync_prev_input(true, false, false, false);

    // ...so the first overworld frame with A held must not talk.
    screen.update_frame(a_input);
    assert!(
        screen.pending_dialogue.is_none(),
        "held A re-fired after menu close and talked to the facing NPC"
    );
    assert!(
        screen.active_script_effect.is_none(),
        "held A re-fired after menu close and started a script"
    );

    // Release, then a genuine press → talks to Daisy as usual.
    screen.update_frame(neutral);
    screen.update_frame(a_input);
    assert!(
        screen.pending_dialogue.is_some() || screen.active_script_effect.is_some(),
        "a genuine A press must still talk to the facing NPC"
    );
}

/// Regression guard for map switches: every in-game warp (doors, script
/// WarpTo, blackout, debug/editor warp) commits through `pending_warp`
/// inside `update_frame`, whose edge detectors run at the top of the frame
/// — before the fade early-returns — so a button held across the whole
/// transition never re-fires as a fresh press in the destination map (the
/// map-switch analog of the START-menu re-entry leak above). If a future
/// change recreates the screen or resets `prev_*` at warp commit, this
/// catches it.
#[test]
fn held_a_across_map_warp_does_not_talk_to_facing_npc() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;

    let a_input = super::OverworldInput::new(
        false, false, false, false, true, false, false, false,
    );
    let neutral = super::OverworldInput::new(
        false, false, false, false, false, false, false, false,
    );

    // Start in PalletTown at the top-left corner facing up: the tile in
    // front is outside the map, so the initial held-A frame below can never
    // talk to an NPC or sign there (wandering NPCs cannot leave bounds).
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    screen.state.player.x = 0;
    screen.state.player.y = 0;
    screen.state.player.facing = Direction::Up;
    for _ in 0..10 {
        screen.update_frame(neutral);
    }
    assert!(screen.pending_dialogue.is_none());
    assert!(screen.active_script_effect.is_none());

    // Press A and keep it held: prev_a_pressed becomes true, and the player
    // then warps into BluesHouse at (1,3) facing Right — Daisy sits at
    // (2,3), directly in front of the landing spot.
    screen.update_frame(a_input);
    screen.warp_to_map(MapId::BluesHouse, 1, 3);
    screen.state.player.facing = Direction::Right;

    // Hold A across fade-out, black screen, commit, fade-in and the first
    // settled frames of the new map (~57 fade frames in total).
    for _ in 0..90 {
        screen.update_frame(a_input);
        assert!(
            screen.pending_dialogue.is_none() && screen.active_script_effect.is_none(),
            "held A re-fired after the warp and talked to the facing NPC"
        );
    }
    assert_eq!(screen.state.current_map, MapId::BluesHouse);
    assert_eq!(screen.state.player.x, 1);
    assert_eq!(screen.state.player.y, 3);

    // Release, then a genuine press → talks to Daisy as usual.
    screen.update_frame(neutral);
    screen.update_frame(a_input);
    assert!(
        screen.pending_dialogue.is_some() || screen.active_script_effect.is_some(),
        "a genuine A press must still talk to the facing NPC after a warp"
    );
}

#[test]
fn elite_four_talk_triggers_battle_after_map_trigger_setup() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    for (map, trainer_id, entered) in [
        (MapId::BrunosRoom, "OPP_BRUNO1", "EVENT_AUTOWALKED_INTO_BRUNOS_ROOM"),
        (MapId::AgathasRoom, "OPP_AGATHA1", "EVENT_AUTOWALKED_INTO_AGATHAS_ROOM"),
    ] {
        let mut screen = OverworldScreen::new(map, None, PokemonRedData);
        screen.script_engine.set_flag(entered, true);
        screen.sync_flags_from_engine();
        screen.run_on_load();
        screen.state.player.x = 5;
        screen.state.player.y = 3;
        screen.state.player.facing = Direction::Up;
        let input = |a| super::OverworldInput::new(false, false, false, false, a, false, false, false);
        for _ in 0..20 { screen.update_frame(input(false)); }
        screen.update_frame(input(true));
        let mut saw_dialogue = false;
        for frame in 0..1800 {
            if let Some(dialogue) = screen.pending_dialogue.as_mut() {
                saw_dialogue = true;
                dialogue.skip_to_full_page();
            }
            screen.update_frame(input(frame % 2 == 1));
            if screen.pending_trainer_battle.is_some() { break; }
        }
        assert!(saw_dialogue, "{map:?}");
        assert_eq!(screen.pending_trainer_battle.as_ref().map(|t| t.trainer_id.as_str()), Some(trainer_id), "{map:?}");
    }
}

/// The B4F boss door is a flag-gated tile swap re-applied on every load
/// (RocketHideoutB4F.asm:11-35): closed ($2d=45) until BOTH guards are beaten,
/// then floor ($0e=14) + unlock latch. The map.blk ships the tile OPEN, so the
/// unbeaten `@else` must write the closed block — the audit walked through
/// twice with both flags unset (door-passed-0/1).
#[test]
fn rocket_hideout_b4f_door_closed_until_both_guards_beaten() {
    use super::screen::OverworldScreen;
    use pokered_data::event_flags::EventFlag;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::RocketHideoutB4F, None, PokemonRedData);
    screen.run_on_load();
    for _ in 0..30 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    let width = screen.map_data.as_ref().unwrap().width as usize;
    let door = 5 * width + 12; // replaceTileBlock(12, 5, …) → block (x=12, y=5)
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        45,
        "unbeaten door must be closed ($2d) despite the open .blk default"
    );
    // One guard down: still locked.
    screen.set_event_flag_live(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_0);
    screen.rerun_map_on_load_script();
    for _ in 0..30 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        45,
        "one guard beaten is not enough"
    );
    // Both down: unlock latch + open door.
    screen.set_event_flag_live(EventFlag::EVENT_BEAT_ROCKET_HIDEOUT_4_TRAINER_1);
    screen.rerun_map_on_load_script();
    for _ in 0..30 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[door],
        14,
        "both guards beaten re-applies the open floor ($0e)"
    );
    assert!(
        screen
            .script_flags()
            .get("EVENT_ROCKET_HIDEOUT_4_DOOR_UNLOCKED")
            .copied()
            .unwrap_or(false),
        "unlock flag latched"
    );
}

/// The six Cinnabar Gym gate blocks are re-applied from their quiz flags on
/// every load (UpdateCinnabarGymGateTileBlocks_): closed HORIZONTAL $54 /
/// VERTICAL $5f until the matching machine is answered correctly, open $0e
/// afterwards. The .blk ships every gate OPEN — the audit walked through gate 1
/// twice with all flags unset (cinnabar-gate-without-quiz-0/1).
#[test]
fn cinnabar_gym_gates_initialized_per_quiz_flags() {
    use super::screen::OverworldScreen;
    use pokered_data::event_flags::EventFlag;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::CinnabarGym, None, PokemonRedData);
    screen.run_on_load();
    for _ in 0..30 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    let width = screen.map_data.as_ref().unwrap().width as usize;
    let gates = [(9usize, 3usize), (6, 3), (6, 6), (3, 8), (2, 6), (2, 3)];
    for (i, &(x, y)) in gates.iter().enumerate() {
        let closed = if i == 3 { 95 } else { 84 }; // gate 4 is the VERTICAL gate
        assert_eq!(
            screen.map_data.as_ref().unwrap().blocks[y * width + x],
            closed,
            "gate {i} at ({x},{y}) must start closed"
        );
    }
    for flag in [
        EventFlag::EVENT_CINNABAR_GYM_GATE0_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE1_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE2_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE3_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE4_UNLOCKED,
        EventFlag::EVENT_CINNABAR_GYM_GATE5_UNLOCKED,
    ] {
        screen.set_event_flag_live(flag);
    }
    screen.rerun_map_on_load_script();
    for _ in 0..30 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    for &(x, y) in gates.iter() {
        assert_eq!(
            screen.map_data.as_ref().unwrap().blocks[y * width + x],
            14,
            "unlocked gate at ({x},{y}) re-applied open"
        );
    }
}

/// Machine 1 (tile 15,7, facing up): answering the original question correctly
/// opens its gate WITHOUT touching any trainer flag — gates and trainers are
/// independent in the original (the audit's port marked trainers beaten and
/// hid them on quiz answers). This drives the real OnInteract trigger through
/// the real choice UI.
#[test]
fn cinnabar_quiz_machine_correct_answer_opens_gate_only() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::CinnabarGym, None, PokemonRedData);
    screen.state.player.x = 15;
    screen.state.player.y = 8;
    screen.run_on_load();
    let input = |a: bool| super::OverworldInput::new(false, false, false, false, a, false, false, false);
    for _ in 0..100 {
        screen.update_frame(input(false));
    }
    screen.state.player.facing = Direction::Up;
    screen.seed_script_query_state(0, &[], 0, 0, 0, 0, &[], 0, 0, 0);
    screen.update_frame(input(true));
    for frame in 0..300 {
        if screen.pending_choice.is_some() {
            break;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    assert!(screen.pending_choice.is_some(), "machine 1 must offer YES/NO");
    // Correct answer is NO — move down to the second option, then confirm.
    screen.update_frame(input(false));
    screen.update_frame(super::OverworldInput::new(false, true, false, false, false, false, false, false));
    screen.update_frame(input(false));
    screen.update_frame(input(true));
    for frame in 0..300 {
        if screen.script_flags().get("EVENT_CINNABAR_GYM_GATE0_UNLOCKED") == Some(&true) {
            break;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    assert_eq!(
        screen.script_flags().get("EVENT_CINNABAR_GYM_GATE0_UNLOCKED"),
        Some(&true),
        "correct answer sets the gate flag"
    );
    // The gate swap is the effect after the flag write — give it frames.
    for _ in 0..30 {
        screen.update_frame(input(false));
    }
    let width = screen.map_data.as_ref().unwrap().width as usize;
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[3 * width + 9],
        14,
        "gate 1 re-applied open right after the correct answer"
    );
    for f in 0..=6 {
        assert_neq_trainer_flag(&screen, f);
    }
}

fn assert_neq_trainer_flag(screen: &super::screen::OverworldScreen<pokered_data::impl_traits::PokemonRedData>, f: usize) {
    assert_ne!(
        screen.script_flags().get(&format!("EVENT_BEAT_CINNABAR_GYM_TRAINER_{f}")),
        Some(&true),
        "trainer {f} must stay unbeaten — quiz answers never set trainer flags"
    );
}

/// Answering WRONG pits the gate-linked trainer against you (the original's
/// wOpponentAfterWrongAnswer = gate index + 2) and leaves the gate closed.
#[test]
fn cinnabar_quiz_wrong_answer_sends_gate_trainer() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::CinnabarGym, None, PokemonRedData);
    screen.state.player.x = 15;
    screen.state.player.y = 8;
    screen.run_on_load();
    let input = |a: bool| super::OverworldInput::new(false, false, false, false, a, false, false, false);
    for _ in 0..100 {
        screen.update_frame(input(false));
    }
    screen.state.player.facing = Direction::Up;
    screen.seed_script_query_state(0, &[], 0, 0, 0, 0, &[], 0, 0, 0);
    screen.update_frame(input(true));
    for frame in 0..300 {
        if screen.pending_choice.is_some() {
            break;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    assert!(screen.pending_choice.is_some());
    // YES is the first option: confirm directly.
    screen.update_frame(input(false));
    screen.update_frame(input(true));
    for frame in 0..300 {
        if screen.pending_trainer_battle.is_some() {
            break;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    let pending = screen
        .pending_trainer_battle
        .as_ref()
        .expect("wrong answer must pit the gate-linked trainer against the player");
    assert_eq!(pending.trainer_id, "OPP_BURGLAR4", "gate 1 links to trainer npc 3");
    assert_ne!(
        screen.script_flags().get("EVENT_CINNABAR_GYM_GATE0_UNLOCKED"),
        Some(&true),
        "a wrong answer never opens the gate"
    );
}

/// Interacting with ZAPDOS queues the real wild battle; the scripted outcome
/// path must hide the bird on win/caught (home/trainers.asm:185-212) — the
/// audit talked to the captured bird and heard the cry again
/// (caught-zapdos-cries-after-continue). Hidden state must survive re-entry.
#[test]
fn zapdos_hidden_after_capture_and_stays_hidden_on_reentry() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::PowerPlant, None, PokemonRedData);
    // Bird at (4,9); stand below and face up.
    screen.state.player.x = 4;
    screen.state.player.y = 10;
    screen.run_on_load();
    let input = |a: bool| super::OverworldInput::new(false, false, false, false, a, false, false, false);
    for _ in 0..100 {
        screen.update_frame(input(false));
    }
    screen.state.player.facing = Direction::Up;
    screen.seed_script_query_state(0, &[], 0, 0, 0, 0, &[], 0, 0, 0);
    screen.update_frame(input(true));
    for frame in 0..300 {
        if screen.pending_wild_encounter.is_some() {
            break;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    assert!(screen.pending_wild_encounter.is_some(), "interacting with the bird starts the static battle");

    // The app settles the battle as a capture and resumes the script.
    screen.pending_wild_encounter = None;
    screen.resume_script_after_battle("caught");
    for _ in 0..60 {
        screen.update_frame(input(false));
    }
    let bird = &screen.npc_states[8]; // npc 9 in object order
    assert!(!bird.visible, "captured ZAPDOS must be hidden");

    // Hidden state survives leaving and re-entering the plant.
    screen.warp_to_map(MapId::PalletTown, 5, 6);
    screen.warp_to_map(MapId::PowerPlant, 4, 10);
    for _ in 0..160 {
        screen.update_frame(input(false));
    }
    let bird = &screen.npc_states[8];
    assert!(!bird.visible, "captured ZAPDOS stays hidden on re-entry");
}

/// Beaten Tower 7F Rockets walk to the down-stairs and vanish
/// (PokemonTower7FEndBattleScript → RocketLeaveMovement → HideNPC): the
/// writeback's post-battle OnLoad re-run plays the leave once, and the hidden
/// state persists across re-entries (audit: tower-rockets-path — all three
/// flags true, all three still visible with no movement left).
#[test]
fn tower_rockets_leave_and_hide_after_victory() {
    use super::screen::OverworldScreen;
    use pokered_data::event_flags::EventFlag;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::PokemonTower7F, None, PokemonRedData);
    screen.run_on_load();
    let input = |a: bool| super::OverworldInput::new(false, false, false, false, a, false, false, false);
    for _ in 0..50 {
        screen.update_frame(input(false));
    }
    for flag in [
        EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_0,
        EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_1,
        EventFlag::EVENT_BEAT_POKEMONTOWER_7_TRAINER_2,
    ] {
        screen.set_event_flag_live(flag);
    }
    // Post-battle OnLoad re-run (what the writeback fires after each win).
    screen.rerun_map_on_load_script();
    for _ in 0..300 {
        screen.update_frame(input(false));
    }
    for (i, rocket) in screen.npc_states.iter().take(3).enumerate() {
        assert!(!rocket.visible, "beaten rocket {} must be hidden", i + 1);
    }
    for rocket in screen.npc_states.iter().take(2) {
        assert_eq!((rocket.x, rocket.y), (9, 16), "rockets 1/2 leave down the stairs");
    }
    // Latch: another load must not replay the walk (rockets stay hidden).
    screen.warp_to_map(MapId::PalletTown, 5, 6);
    screen.warp_to_map(MapId::PokemonTower7F, 9, 15);
    for _ in 0..160 {
        screen.update_frame(input(false));
    }
    for (i, rocket) in screen.npc_states.iter().take(3).enumerate() {
        assert!(!rocket.visible, "rocket {} stays hidden on re-entry", i + 1);
    }
}

/// Catching the Route12 SNORLAX must NOT show the "returned to the mountains"
/// dialogue (wBattleResult == 2 skips it, scripts/Route12.asm:50) — the audit
/// heard the mountains line with a Snorlax already in the party
/// (snorlax-caught-but-mountains). Defeating it still shows the line.
#[test]
fn route12_snorlax_caught_skips_mountain_dialogue() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    for (caught, expect_dialogue) in [(true, false), (false, true)] {
        let mut screen = OverworldScreen::new(MapId::Route12, None, PokemonRedData);
        // Snorlax at (10,62) facing down; stand below and face up.
        screen.state.player.x = 10;
        screen.state.player.y = 63;
        screen.run_on_load();
        let input = |a: bool| super::OverworldInput::new(false, false, false, false, a, false, false, false);
        for _ in 0..100 {
            screen.update_frame(input(false));
        }
        screen.seed_script_query_state(0, &["POKE_FLUTE".to_string()], 0, 0, 0, 0, &[], 0, 0, 0);
        screen.state.player.facing = Direction::Up;
        screen.update_frame(input(true));
        for frame in 0..300 {
            if screen.pending_wild_encounter.is_some() {
                break;
            }
            if let Some(dialogue) = screen.pending_dialogue.as_mut() {
                dialogue.skip_to_full_page();
            }
            screen.update_frame(input(frame % 2 == 1));
        }
        assert!(screen.pending_wild_encounter.is_some(), "snorlax battle queued");
        screen.pending_wild_encounter = None;
        let outcome = if caught { "caught" } else { "win" };
        screen.resume_script_after_battle(outcome);
        if !expect_dialogue {
            // Caught: no narration at all — the flag flips without any text.
            for _ in 0..30 {
                screen.update_frame(input(false));
            }
            assert!(
                screen.pending_dialogue.is_none(),
                "caught snorlax must not narrate returning to the mountains"
            );
        } else {
            // Win: the mountains dialogue shows, THEN the flag flips.
            screen.update_frame(input(false));
            assert!(
                screen.pending_dialogue.is_some(),
                "defeated snorlax narrates returning to the mountains"
            );
        }
        for frame in 0..120 {
            if screen.script_flags().get("EVENT_BEAT_ROUTE12_SNORLAX") == Some(&true) {
                break;
            }
            if let Some(dialogue) = screen.pending_dialogue.as_mut() {
                dialogue.skip_to_full_page();
            }
            screen.update_frame(input(frame % 2 == 1));
        }
        assert_eq!(
            screen.script_flags().get("EVENT_BEAT_ROUTE12_SNORLAX"),
            Some(&true),
            "snorlax cleared on {outcome}"
        );
        // The hide lands as the effect after the flag write.
        for _ in 0..30 {
            screen.update_frame(input(false));
        }
        assert!(!screen.npc_states[0].visible, "snorlax hidden on {outcome}");
    }
}

/// The hidden/shown mutex keys must resolve HIDDEN-first on re-entry. The
/// audit's reappearing-rival bug: `__OBJ_HIDDEN_CERULEAN_RIVAL` and
/// `__OBJ_SHOWN_CERULEAN_RIVAL` were BOTH true (the runner's pre-battle shown
/// state merges back through sync), and the default-hidden restore pass then
/// re-showed the beaten rival over the hidden pass.
#[test]
fn hidden_object_stays_hidden_when_stale_shown_key_exists() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::CeruleanCity, None, PokemonRedData);
    screen.run_on_load();
    // Post-battle conflict state exactly as the audit observed it.
    screen.set_flag_live("__OBJ_SHOWN_CERULEAN_RIVAL", true);
    screen.set_flag_live("__OBJ_HIDDEN_CERULEAN_RIVAL", true);
    // Re-entry rebuilds npc_states and re-applies the flags.
    screen.warp_to_map(MapId::PalletTown, 5, 6);
    screen.warp_to_map(MapId::CeruleanCity, 20, 8);
    let input = |a: bool| super::OverworldInput::new(false, false, false, false, a, false, false, false);
    for _ in 0..160 {
        screen.update_frame(input(false));
    }
    assert!(
        !screen.npc_states[0].visible,
        "beaten rival must stay hidden when a stale shown key also exists"
    );
}

/// The VictoryRoad 1F switch flag (and its open-floor block) may only be set
/// by a STRENGTH boulder actually pushed onto the switch tile (17,13) —
/// CheckBoulderCoords + SetEvent (VictoryRoad1F.asm:28-41). The audit's port
/// set it by merely TALKING to the boulder. This drives the real push:
/// boulder placed one tile north of the switch, STRENGTH active, d-pad held.
#[test]
fn victory_road_switch_requires_boulder_pushed_onto_it() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::VictoryRoad1F, None, PokemonRedData);
    screen.run_on_load();
    let input_hold_down = super::OverworldInput::new(false, true, false, false, false, false, false, false);
    // Find the puzzle boulder (npc 5) and stage it one tile north of the switch.
    let boulder_idx = screen
        .npc_states
        .iter()
        .position(|n| n.sprite_id == pokered_data::sprites::SpriteId::Boulder as u8)
        .expect("1F has boulders");
    screen.npc_states[boulder_idx].x = 17;
    screen.npc_states[boulder_idx].y = 12;
    // Player stands north of the boulder, facing it, STRENGTH active.
    screen.state.player.x = 17;
    screen.state.player.y = 11;
    screen.state.player.facing = Direction::Down;
    screen.strength_active = true;
    for _ in 0..30 {
        screen.update_frame(input_hold_down);
    }
    assert_eq!(
        (screen.npc_states[boulder_idx].x, screen.npc_states[boulder_idx].y),
        (17, 13),
        "the push slides the boulder onto the switch"
    );
    assert_eq!(
        screen.script_flags().get("EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH"),
        Some(&true),
        "boulder on the switch sets the flag"
    );
    let width = screen.map_data.as_ref().unwrap().width as usize;
    assert_eq!(
        screen.map_data.as_ref().unwrap().blocks[6 * width + 4],
        29,
        "the path block re-opens ($1d at X=4, Y=6)"
    );

    // Talking to the boulder alone (no push) must NOT set the flag: a fresh
    // screen with the boulder nowhere near the switch.
    let mut screen = OverworldScreen::new(MapId::VictoryRoad1F, None, PokemonRedData);
    screen.run_on_load();
    for _ in 0..30 {
        screen.update_frame(input_hold_down);
    }
    assert_ne!(
        screen.script_flags().get("EVENT_VICTORY_ROAD_1_BOULDER_ON_SWITCH"),
        Some(&true),
        "the switch flag starts unset"
    );
}

/// The Bill subplot runs at the REAL PC tile (1,4): agreeing walks the monster
/// into the machine (hidden), and interacting with the PC runs the separation,
/// after which the human Bill walks OUT of the machine to (4,4). The audit's
/// port bound the separation to a bogus (5,4) empty tile, skipped the walks,
/// and left the real PC showing only the monitor text (bill-visible-pc vs
/// bill-invisible-pc evidence).
#[test]
fn bills_pc_and_cutscene_run_at_real_pc_tile() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::BillsHouse, None, PokemonRedData);
    screen.state.player.x = 1;
    screen.state.player.y = 5; // below the PC at (1,4)
    screen.run_on_load();
    let input = |a: bool| super::OverworldInput::new(false, false, false, false, a, false, false, false);
    for _ in 0..50 {
        screen.update_frame(input(false));
    }

    // 1. Fresh PC interaction: monitor text only, no subplot progress.
    screen.update_frame(input(true));
    for frame in 0..120 {
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    assert!(
        screen.script_flags().get("EVENT_BILL_SAID_USE_CELL_SEPARATOR") != Some(&true),
        "fresh PC use must not run the separation"
    );

    // 2. Talk to the monster Bill (6,5) and agree: he walks INTO the machine
    //    ((6,2)) and is hidden there (A09 walk choreography).
    screen.state.player.x = 6;
    screen.state.player.y = 6;
    screen.state.player.facing = Direction::Up;
    screen.seed_script_query_state(0, &[], 0, 0, 0, 0, &[], 0, 0, 0);
    screen.update_frame(input(true));
    let mut chose_yes = false;
    for frame in 0..400 {
        if screen.pending_choice.is_some() && !chose_yes {
            screen.update_frame(input(false));
            screen.update_frame(input(true)); // YES
            chose_yes = true;
            continue;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    assert!(chose_yes, "the help choice must appear");
    assert_eq!(
        screen.script_flags().get("EVENT_BILL_SAID_USE_CELL_SEPARATOR"),
        Some(&true),
        "agreeing latches the separator request"
    );
    assert!(!screen.npc_states[0].visible, "monster Bill hidden inside the machine");
    assert_eq!(
        (screen.npc_states[0].x, screen.npc_states[0].y),
        (6, 2),
        "monster Bill walked up into the machine"
    );

    // 3. Interact with the PC (1,4): separation runs, human Bill walks out.
    screen.state.player.x = 1;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;
    screen.update_frame(input(true));
    for frame in 0..300 {
        if screen.script_flags().get("EVENT_MET_BILL") == Some(&true) {
            break;
        }
        if let Some(dialogue) = screen.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        screen.update_frame(input(frame % 2 == 1));
    }
    assert_eq!(
        screen.script_flags().get("EVENT_USED_CELL_SEPARATOR_ON_BILL"),
        Some(&true)
    );
    assert_eq!(screen.script_flags().get("EVENT_MET_BILL_2"), Some(&true));
    assert!(screen.npc_states[1].visible, "human Bill shown");
    assert_eq!(
        (screen.npc_states[1].x, screen.npc_states[1].y),
        (4, 4),
        "human Bill walked out of the machine to his PC"
    );
}

#[test]
fn warp_into_safari_zone_arms_safari_game() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    screen.warp_to_map(MapId::SafariZoneWest, 14, 7);
    for _ in 0..160 {
        screen.update_frame(super::OverworldInput::new(
            false, false, false, false, false, false, false, false,
        ));
    }
    assert!(screen.is_safari_game_active(), "warp commit must arm the safari game");
}

/// Ending the Safari run (timeout eject or leaving the zone) must clear
/// EVENT_IN_SAFARI_ZONE — the gate scene's "Leaving early?" branch keys off
/// it, so a stale flag wrongly asked the player to leave early on re-entry
/// (audit: §狩猎地带 START).
#[test]
fn safari_end_clears_in_safari_zone_flag() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::SafariZoneWest, None, PokemonRedData);
    screen.start_safari_game();
    screen.set_flag_live("EVENT_IN_SAFARI_ZONE", true);
    screen.end_safari_game();
    assert!(
        !screen.unified_flags().get_flag("EVENT_IN_SAFARI_ZONE"),
        "ending the run must clear EVENT_IN_SAFARI_ZONE"
    );
}

/// Walking toward a sighted trainer during its approach must not let the two
/// sprites overlap: the original freezes player movement while the trainer
/// walks up (TrainerEngage → MoveSprite). The audit observed a Channeler on
/// Tower 6F standing ON the player's tile (tower-trainer-player-overlap).
#[test]
fn trainer_approach_freezes_player_no_overlap() {
    use super::screen::OverworldScreen;
    use pokered_data::impl_traits::PokemonRedData;
    let mut screen = OverworldScreen::new(MapId::PokemonTower6F, None, PokemonRedData);
    // Channeler 1 at (12,10) faces Right, range 2: standing at (14,10) is the
    // sight edge. Step LEFT toward the trainer while it approaches.
    screen.state.player.x = 14;
    screen.state.player.y = 10;
    screen.run_on_load();
    let hold_left = super::OverworldInput::new(false, false, true, false, false, false, false, false);
    for _ in 0..100 {
        screen.update_frame(hold_left);
    }
    let player = (screen.state.player.x, screen.state.player.y);
    for (i, npc) in screen.npc_states.iter().enumerate() {
        assert!(
            !(npc.visible && (npc.x, npc.y) == player),
            "NPC {} overlaps the player after the approach (player at {:?})",
            i + 1,
            player
        );
    }
    assert!(
        screen.pending_trainer_battle.is_some() || screen.script_awaiting_battle,
        "the sight approach still hands off to the battle"
    );
}
