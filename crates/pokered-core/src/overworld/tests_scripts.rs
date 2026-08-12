use super::event_flags::EventFlags;
use super::trainer_engine::*;
use super::Direction;
use pokered_data::event_flags::{EventFlag, EVENT_FLAGS_SIZE};
use pokered_data::maps::MapId;
use pokered_data::trainer_headers::get_trainer_headers;

// ══════════════════════════════════════════════════════════════════════
// EventFlags tests
// ══════════════════════════════════════════════════════════════════════

#[test]
fn event_flags_new_all_cleared() {
    let flags = EventFlags::new();
    assert_eq!(flags.count_set(), 0);
}

#[test]
fn event_flags_default_same_as_new() {
    let a = EventFlags::new();
    let b = EventFlags::default();
    assert_eq!(a.as_bytes(), b.as_bytes());
}

#[test]
fn event_flags_set_and_check() {
    let mut flags = EventFlags::new();
    let flag = EventFlag::EVENT_GOT_STARTER;
    assert!(!flags.check(flag));
    flags.set(flag);
    assert!(flags.check(flag));
}

#[test]
fn event_flags_reset() {
    let mut flags = EventFlags::new();
    let flag = EventFlag::EVENT_GOT_STARTER;
    flags.set(flag);
    assert!(flags.check(flag));
    flags.reset(flag);
    assert!(!flags.check(flag));
}

#[test]
fn event_flags_multiple_independent() {
    let mut flags = EventFlags::new();
    let f1 = EventFlag::EVENT_GOT_STARTER;
    let f2 = EventFlag::EVENT_BEAT_BROCK;
    flags.set(f1);
    assert!(flags.check(f1));
    assert!(!flags.check(f2));
    flags.set(f2);
    assert!(flags.check(f1));
    assert!(flags.check(f2));
    flags.reset(f1);
    assert!(!flags.check(f1));
    assert!(flags.check(f2));
}

#[test]
fn event_flags_count_set() {
    let mut flags = EventFlags::new();
    assert_eq!(flags.count_set(), 0);
    flags.set(EventFlag::EVENT_GOT_STARTER);
    assert_eq!(flags.count_set(), 1);
    flags.set(EventFlag::EVENT_BEAT_BROCK);
    assert_eq!(flags.count_set(), 2);
    flags.set(EventFlag::EVENT_BEAT_MISTY);
    assert_eq!(flags.count_set(), 3);
}

#[test]
fn event_flags_clear_all() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_BEAT_BROCK);
    flags.set(EventFlag::EVENT_BEAT_MISTY);
    assert_eq!(flags.count_set(), 3);
    flags.clear_all();
    assert_eq!(flags.count_set(), 0);
    assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));
}

#[test]
fn event_flags_raw_operations() {
    let mut flags = EventFlags::new();
    // Bit 0
    assert!(!flags.check_raw(0));
    flags.set_raw(0);
    assert!(flags.check_raw(0));
    flags.reset_raw(0);
    assert!(!flags.check_raw(0));
    // High bit
    flags.set_raw(100);
    assert!(flags.check_raw(100));
    assert!(!flags.check_raw(99));
    assert!(!flags.check_raw(101));
}

#[test]
fn event_flags_raw_out_of_bounds_are_noops() {
    let mut flags = EventFlags::new();
    flags.set_raw(0xFFFF);
    assert!(!flags.check_raw(0xFFFF));
    flags.reset_raw(0xFFFF);
    assert!(!flags.check_raw(0xFFFF));
}

#[test]
fn event_flags_save_load_roundtrip() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_BEAT_BROCK);
    flags.set(EventFlag::EVENT_BEAT_MISTY);
    let bytes = flags.as_bytes();
    let loaded = EventFlags::from_bytes(bytes);
    assert!(loaded.check(EventFlag::EVENT_GOT_STARTER));
    assert!(loaded.check(EventFlag::EVENT_BEAT_BROCK));
    assert!(loaded.check(EventFlag::EVENT_BEAT_MISTY));
    assert!(!loaded.check(EventFlag::EVENT_BEAT_LT_SURGE));
    assert_eq!(loaded.count_set(), 3);
}

#[test]
fn event_flags_size_matches_original_wram() {
    // The original wEventFlags array is flag_array NUM_EVENTS with
    // NUM_EVENTS = $A00 bits → 320 bytes; SRAM holds the full region.
    assert_eq!(EVENT_FLAGS_SIZE, 320);
    let flags = EventFlags::new();
    assert_eq!(flags.as_bytes().len(), 320);
}

// ══════════════════════════════════════════════════════════════════════
// TrainerEngine tests
// ══════════════════════════════════════════════════════════════════════

fn make_trainer_header(flag: EventFlag, sight_range: u8) -> TrainerHeader {
    TrainerHeader {
        event_flag: flag,
        sight_range,
        before_battle_text_id: 1,
        end_battle_text_id: 2,
        after_battle_text_id: 3,
    }
}

#[test]
fn trainer_not_defeated_initially() {
    let flags = EventFlags::new();
    let header = make_trainer_header(EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0, 5);
    assert!(!is_trainer_defeated(&flags, &header));
}

#[test]
fn trainer_defeated_after_marking() {
    let mut flags = EventFlags::new();
    let header = make_trainer_header(EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0, 5);
    mark_trainer_defeated(&mut flags, &header);
    assert!(is_trainer_defeated(&flags, &header));
}

#[test]
fn trainer_see_player_facing_right() {
    // Trainer at (5,5) facing right, player at (8,5), range 5
    assert!(can_trainer_see_player(5, 5, 1, 0, 8, 5, 5));
    // Player at (11,5) — out of range
    assert!(!can_trainer_see_player(5, 5, 1, 0, 11, 5, 5));
    // Player at (5,5) — same tile (distance 0, not > 0)
    assert!(!can_trainer_see_player(5, 5, 1, 0, 5, 5, 5));
}

#[test]
fn trainer_see_player_facing_left() {
    // Trainer at (10,5) facing left, player at (7,5), range 5
    assert!(can_trainer_see_player(10, 5, -1, 0, 7, 5, 5));
    // Player at (4,5) — out of range
    assert!(!can_trainer_see_player(10, 5, -1, 0, 4, 5, 5));
}

#[test]
fn trainer_see_player_facing_down() {
    // Trainer at (5,3) facing down, player at (5,7), range 5
    assert!(can_trainer_see_player(5, 3, 0, 1, 5, 7, 5));
    // Player at (5,9) — out of range
    assert!(!can_trainer_see_player(5, 3, 0, 1, 5, 9, 5));
}

#[test]
fn trainer_see_player_facing_up() {
    // Trainer at (5,10) facing up, player at (5,6), range 5
    assert!(can_trainer_see_player(5, 10, 0, -1, 5, 6, 5));
    // Player at (5,4) — out of range
    assert!(!can_trainer_see_player(5, 10, 0, -1, 5, 4, 5));
}

#[test]
fn trainer_cannot_see_off_axis() {
    // Trainer facing right, but player is not on same row
    assert!(!can_trainer_see_player(5, 5, 1, 0, 8, 6, 5));
    // Trainer facing down, but player is not on same column
    assert!(!can_trainer_see_player(5, 5, 0, 1, 6, 8, 5));
}

#[test]
fn trainer_cannot_see_behind() {
    // Trainer facing right, player is to the left
    assert!(!can_trainer_see_player(5, 5, 1, 0, 3, 5, 5));
    // Trainer facing up, player is below
    assert!(!can_trainer_see_player(5, 5, 0, -1, 5, 8, 5));
}

#[test]
fn trainer_see_at_exact_range() {
    // Range 3, facing right. Player exactly 3 tiles away
    assert!(can_trainer_see_player(5, 5, 1, 0, 8, 5, 3));
    // Player at 4 tiles — out of range
    assert!(!can_trainer_see_player(5, 5, 1, 0, 9, 5, 3));
}

#[test]
fn trainer_zero_facing_cannot_see() {
    // Both dx=0 and dy=0 — no facing direction
    assert!(!can_trainer_see_player(5, 5, 0, 0, 6, 5, 5));
}

#[test]
fn check_all_trainers_finds_visible() {
    let headers = vec![make_trainer_header(
        EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0,
        5,
    )];
    let flags = EventFlags::new();
    let positions = vec![TrainerPosition {
        x: 5,
        y: 5,
        facing_dx: 1,
        facing_dy: 0,
    }];
    // Player at (8,5) — in range
    assert_eq!(
        check_all_trainers(&headers, &flags, &positions, 8, 5),
        Some(0)
    );
}

#[test]
fn check_all_trainers_skips_defeated() {
    let mut flags = EventFlags::new();
    let header = make_trainer_header(EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0, 5);
    flags.set(header.event_flag);
    let headers = vec![header];
    let positions = vec![TrainerPosition {
        x: 5,
        y: 5,
        facing_dx: 1,
        facing_dy: 0,
    }];
    assert_eq!(check_all_trainers(&headers, &flags, &positions, 8, 5), None);
}

#[test]
fn check_all_trainers_finds_second_when_first_out_of_range() {
    let headers = vec![
        make_trainer_header(EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0, 2),
        make_trainer_header(EventFlag::EVENT_BEAT_CERULEAN_GYM_TRAINER_0, 5),
    ];
    let flags = EventFlags::new();
    let positions = vec![
        TrainerPosition {
            x: 0,
            y: 0,
            facing_dx: 1,
            facing_dy: 0,
        },
        TrainerPosition {
            x: 5,
            y: 10,
            facing_dx: 0,
            facing_dy: -1,
        },
    ];
    // Player at (5,6) — first trainer can't see (wrong row+range), second can (same col, 4 tiles up)
    assert_eq!(
        check_all_trainers(&headers, &flags, &positions, 5, 6),
        Some(1)
    );
}

#[test]
fn check_all_trainers_no_match_returns_none() {
    let headers = vec![make_trainer_header(
        EventFlag::EVENT_BEAT_PEWTER_GYM_TRAINER_0,
        2,
    )];
    let flags = EventFlags::new();
    let positions = vec![TrainerPosition {
        x: 0,
        y: 0,
        facing_dx: 1,
        facing_dy: 0,
    }];
    // Player way out of range
    assert_eq!(
        check_all_trainers(&headers, &flags, &positions, 50, 50),
        None
    );
}

#[test]
fn trainer_battle_state_machine_full_cycle() {
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

#[test]
fn trainer_encounter_new_defaults() {
    let enc = TrainerEncounter::new(MapId::CeruleanGym, 2, 5);
    assert_eq!(enc.map, MapId::CeruleanGym);
    assert_eq!(enc.trainer_index, 2);
    assert_eq!(enc.npc_index, 5);
    assert_eq!(enc.state, TrainerBattleState::NotEngaged);
}

// ── Trainer header data tests ─────────────────────────────────────

#[test]
fn pewter_gym_has_one_trainer_header() {
    let headers = get_trainer_headers(MapId::PewterGym);
    assert_eq!(headers.len(), 1, "PewterGym has 1 trainer");
}

#[test]
fn celadon_gym_has_seven_trainer_headers() {
    let headers = get_trainer_headers(MapId::CeladonGym);
    assert_eq!(headers.len(), 7, "CeladonGym has 7 trainers");
}

#[test]
fn viridian_gym_has_eight_trainer_headers() {
    let headers = get_trainer_headers(MapId::ViridianGym);
    assert_eq!(headers.len(), 8, "ViridianGym has 8 trainers");
}

#[test]
fn route24_has_six_trainer_headers() {
    let headers = get_trainer_headers(MapId::Route24);
    assert_eq!(headers.len(), 6, "Route24 has 6 trainers");
}

#[test]
fn total_trainer_headers_322() {
    let mut total = 0usize;
    for i in 0..248u8 {
        if let Some(map) = MapId::from_u8(i) {
            total += get_trainer_headers(map).len();
        }
    }
    assert_eq!(
        total, 322,
        "Total trainer headers across all maps should be 322"
    );
}

#[test]
fn trainer_header_count_matches() {
    assert_eq!(get_trainer_headers(MapId::PewterGym).len(), 1);
    assert_eq!(get_trainer_headers(MapId::CeladonGym).len(), 7);
    assert_eq!(get_trainer_headers(MapId::PalletTown).len(), 0);
}

#[test]
fn trainer_header_valid_index() {
    let headers = get_trainer_headers(MapId::PewterGym);
    assert!(!headers.is_empty());
    assert_eq!(headers[0].sight_range, 5);
}

#[test]
fn trainer_header_invalid_index() {
    let headers = get_trainer_headers(MapId::PewterGym);
    assert!(headers.get(99).is_none());
}

#[test]
fn map_with_no_trainers_returns_empty() {
    let headers = get_trainer_headers(MapId::PalletTown);
    assert!(headers.is_empty());
}

#[test]
fn pewter_gym_trainer_sight_range_5() {
    let headers = get_trainer_headers(MapId::PewterGym);
    assert_eq!(headers[0].sight_range, 5);
}

// ══════════════════════════════════════════════════════════════════════
// Toggleable object tests (OaksLab starter ball persistence)
// ══════════════════════════════════════════════════════════════════════

#[test]
fn toggleable_objects_starter_ball_indices_correct() {
    use pokered_data::toggleable_objects::{toggle_id_to_bit_index, ToggleableObject};
    // Verify OaksLab starter balls map to correct bit indices
    assert_eq!(
        toggle_id_to_bit_index("OAKS_LAB_OBJ_2"),
        Some(0x2B),
        "Charmander ball"
    );
    assert_eq!(
        toggle_id_to_bit_index("OAKS_LAB_OBJ_3"),
        Some(0x2C),
        "Squirtle ball"
    );
    assert_eq!(
        toggle_id_to_bit_index("OAKS_LAB_OBJ_4"),
        Some(0x2D),
        "Bulbasaur ball"
    );
    // Also verify constants match
    assert_eq!(ToggleableObject::STARTER_BALL_1.bit_index(), 0x2B);
    assert_eq!(ToggleableObject::STARTER_BALL_2.bit_index(), 0x2C);
    assert_eq!(ToggleableObject::STARTER_BALL_3.bit_index(), 0x2D);
}

#[test]
fn toggleable_object_flags_hide_starter_ball() {
    use pokered_data::toggleable_objects::{is_object_hidden, set_object_hidden, ToggleableObject};
    let mut flags = [0u8; 32];
    // Hide Charmander ball (OAKS_LAB_OBJ_2, bit 0x2B)
    set_object_hidden(&mut flags, ToggleableObject::STARTER_BALL_1.bit_index());
    assert!(is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_1.bit_index()
    ));
    // Other balls still visible
    assert!(!is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_2.bit_index()
    ));
    assert!(!is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_3.bit_index()
    ));
}

#[test]
fn toggleable_object_flags_show_after_hide() {
    use pokered_data::toggleable_objects::{
        is_object_hidden, set_object_hidden, set_object_shown, ToggleableObject,
    };
    let mut flags = [0u8; 32];
    // Hide then show
    set_object_hidden(&mut flags, ToggleableObject::STARTER_BALL_1.bit_index());
    assert!(is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_1.bit_index()
    ));
    set_object_shown(&mut flags, ToggleableObject::STARTER_BALL_1.bit_index());
    assert!(!is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_1.bit_index()
    ));
}

#[test]
fn toggleable_object_flags_multiple_balls_hidden() {
    use pokered_data::toggleable_objects::{is_object_hidden, set_object_hidden, ToggleableObject};
    let mut flags = [0u8; 32];
    // Hide two balls (player picked Charmander, rival took Squirtle)
    set_object_hidden(&mut flags, ToggleableObject::STARTER_BALL_1.bit_index()); // Charmander
    set_object_hidden(&mut flags, ToggleableObject::STARTER_BALL_2.bit_index()); // Squirtle
    assert!(is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_1.bit_index()
    ));
    assert!(is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_2.bit_index()
    ));
    // Bulbasaur ball still visible
    assert!(!is_object_hidden(
        &flags,
        ToggleableObject::STARTER_BALL_3.bit_index()
    ));
}
