use crate::save_menu::*;

fn make_info() -> SaveScreenInfo {
    SaveScreenInfo {
        player_name: "RED".to_string(),
        num_badges: 3,
        pokedex_owned: 42,
        play_time_hours: 12,
        play_time_minutes: 34,
    }
}

fn press_a() -> YesNoInput {
    YesNoInput {
        a: true,
        ..YesNoInput::none()
    }
}

fn press_b() -> YesNoInput {
    YesNoInput {
        b: true,
        ..YesNoInput::none()
    }
}

fn press_down() -> YesNoInput {
    YesNoInput {
        down: true,
        ..YesNoInput::none()
    }
}

fn press_up() -> YesNoInput {
    YesNoInput {
        up: true,
        ..YesNoInput::none()
    }
}

#[test]
fn save_menu_initial_state() {
    let state = SaveMenuState::new(make_info(), false, false);
    assert_eq!(state.phase, SavePhase::AskSave);
    assert_eq!(state.cursor, YesNoChoice::Yes);
    assert_eq!(state.info.player_name, "RED");
    assert_eq!(state.info.num_badges, 3);
    assert_eq!(state.info.pokedex_owned, 42);
}

#[test]
fn save_menu_cancel_immediately() {
    let mut state = SaveMenuState::new(make_info(), false, false);
    let result = state.tick(press_b());
    assert_eq!(result, SaveMenuResult::Cancelled);
}

#[test]
fn save_menu_select_no() {
    let mut state = SaveMenuState::new(make_info(), false, false);
    state.tick(press_down());
    assert_eq!(state.cursor, YesNoChoice::No);
    let result = state.tick(press_a());
    assert_eq!(result, SaveMenuResult::Cancelled);
}

#[test]
fn save_menu_simple_save_flow() {
    let mut state = SaveMenuState::new(make_info(), false, false);
    let result = state.tick(press_a());
    assert_eq!(result, SaveMenuResult::Active);
    assert!(matches!(
        state.phase,
        SavePhase::Saving {
            frames_remaining: 120
        }
    ));

    for _ in 0..119 {
        assert_eq!(state.tick(YesNoInput::none()), SaveMenuResult::Active);
    }
    assert!(matches!(state.phase, SavePhase::Saving { .. }));

    assert_eq!(state.tick(YesNoInput::none()), SaveMenuResult::Active);
    assert_eq!(state.phase, SavePhase::SaveComplete);
    assert_eq!(state.sfx_event, SaveSfxEvent::Save);

    assert_eq!(state.tick(YesNoInput::none()), SaveMenuResult::Active);
    assert_eq!(state.phase, SavePhase::SaveComplete);
    assert_eq!(state.sfx_event, SaveSfxEvent::None);

    state.notify_sfx_done();
    assert!(matches!(
        state.phase,
        SavePhase::WaitAfterSave {
            frames_remaining: 30
        }
    ));

    for _ in 0..29 {
        assert_eq!(state.tick(YesNoInput::none()), SaveMenuResult::Active);
    }
    assert_eq!(state.tick(YesNoInput::none()), SaveMenuResult::Saved);
}

#[test]
fn save_menu_with_previous_save_same_player() {
    let mut state = SaveMenuState::new(make_info(), true, false);
    let result = state.tick(press_a());
    assert_eq!(result, SaveMenuResult::Active);
    assert!(matches!(state.phase, SavePhase::Saving { .. }));
}

#[test]
fn save_menu_with_previous_save_different_player_confirm() {
    let mut state = SaveMenuState::new(make_info(), true, true);
    let result = state.tick(press_a());
    assert_eq!(result, SaveMenuResult::Active);
    assert_eq!(state.phase, SavePhase::ConfirmOverwrite);
    assert_eq!(state.cursor, YesNoChoice::Yes);

    let result = state.tick(press_a());
    assert_eq!(result, SaveMenuResult::Active);
    assert!(matches!(state.phase, SavePhase::Saving { .. }));
}

#[test]
fn save_menu_with_previous_save_different_player_cancel() {
    let mut state = SaveMenuState::new(make_info(), true, true);
    state.tick(press_a());
    assert_eq!(state.phase, SavePhase::ConfirmOverwrite);

    let result = state.tick(press_b());
    assert_eq!(result, SaveMenuResult::Cancelled);
}

#[test]
fn save_menu_overwrite_select_no() {
    let mut state = SaveMenuState::new(make_info(), true, true);
    state.tick(press_a());
    state.tick(press_down());
    assert_eq!(state.cursor, YesNoChoice::No);
    let result = state.tick(press_a());
    assert_eq!(result, SaveMenuResult::Cancelled);
}

#[test]
fn save_menu_cursor_navigation() {
    let mut state = SaveMenuState::new(make_info(), false, false);
    assert_eq!(state.cursor, YesNoChoice::Yes);
    state.tick(press_down());
    assert_eq!(state.cursor, YesNoChoice::No);
    state.tick(press_up());
    assert_eq!(state.cursor, YesNoChoice::Yes);
    state.tick(press_up());
    assert_eq!(state.cursor, YesNoChoice::Yes);
}

#[test]
fn change_box_initial_state() {
    let counts = [0u8; NUM_BOXES];
    let state = ChangeBoxMenuState::new(3, counts, false);
    assert!(!state.confirmed);
    assert_eq!(state.cursor, 3);
    assert_eq!(state.current_box, 3);
}

#[test]
fn change_box_clamps_to_max() {
    let counts = [0u8; NUM_BOXES];
    let state = ChangeBoxMenuState::new(99, counts, false);
    assert_eq!(state.cursor, NUM_BOXES - 1);
    assert_eq!(state.current_box, NUM_BOXES - 1);
}

#[test]
fn change_box_cancel_before_confirm() {
    let mut state = ChangeBoxMenuState::new(0, [0; NUM_BOXES], false);
    let result = state.tick(press_b());
    assert_eq!(result, ChangeBoxResult::Cancelled);
}

#[test]
fn change_box_deny_confirm() {
    let mut state = ChangeBoxMenuState::new(0, [0; NUM_BOXES], false);
    state.tick(press_down());
    assert_eq!(state.confirm_cursor, YesNoChoice::No);
    let result = state.tick(press_a());
    assert_eq!(result, ChangeBoxResult::Cancelled);
}

#[test]
fn change_box_confirm_then_select() {
    let mut state = ChangeBoxMenuState::new(0, [0; NUM_BOXES], false);
    let result = state.tick(press_a());
    assert_eq!(result, ChangeBoxResult::Active);
    assert!(state.confirmed);

    state.tick(press_down());
    assert_eq!(state.cursor, 1);
    state.tick(press_down());
    assert_eq!(state.cursor, 2);

    let result = state.tick(press_a());
    assert_eq!(result, ChangeBoxResult::Selected(2));
}

#[test]
fn change_box_cursor_wraps_down() {
    let mut state = ChangeBoxMenuState::new(NUM_BOXES - 1, [0; NUM_BOXES], false);
    state.tick(press_a());
    assert_eq!(state.cursor, NUM_BOXES - 1);
    state.tick(press_down());
    assert_eq!(state.cursor, 0);
}

#[test]
fn change_box_cursor_wraps_up() {
    let mut state = ChangeBoxMenuState::new(0, [0; NUM_BOXES], false);
    state.tick(press_a());
    state.tick(press_up());
    assert_eq!(state.cursor, NUM_BOXES - 1);
}

#[test]
fn change_box_cancel_after_confirm() {
    let mut state = ChangeBoxMenuState::new(0, [0; NUM_BOXES], false);
    state.tick(press_a());
    let result = state.tick(press_b());
    assert_eq!(result, ChangeBoxResult::Cancelled);
}

#[test]
fn change_box_has_pokemon() {
    let mut counts = [0u8; NUM_BOXES];
    counts[0] = 5;
    counts[5] = 1;
    counts[11] = 20;
    let state = ChangeBoxMenuState::new(0, counts, false);
    assert!(state.box_has_pokemon(0));
    assert!(!state.box_has_pokemon(1));
    assert!(state.box_has_pokemon(5));
    assert!(!state.box_has_pokemon(6));
    assert!(state.box_has_pokemon(11));
    assert!(!state.box_has_pokemon(99));
}

#[test]
fn change_box_names() {
    assert_eq!(BOX_NAMES[0], "BOX 1");
    assert_eq!(BOX_NAMES[9], "BOX10");
    assert_eq!(BOX_NAMES[11], "BOX12");
    assert_eq!(BOX_NAMES.len(), NUM_BOXES);
}

#[test]
fn clear_save_defaults_to_no() {
    let state = ClearSaveState::new();
    assert_eq!(state.cursor, YesNoChoice::No);
}

#[test]
fn clear_save_cancel_with_b() {
    let mut state = ClearSaveState::new();
    let result = state.tick(press_b());
    assert_eq!(result, ClearSaveResult::Cancelled);
}

#[test]
fn clear_save_confirm_no() {
    let mut state = ClearSaveState::new();
    let result = state.tick(press_a());
    assert_eq!(result, ClearSaveResult::Cancelled);
}

#[test]
fn clear_save_confirm_yes() {
    let mut state = ClearSaveState::new();
    state.tick(press_down());
    assert_eq!(state.cursor, YesNoChoice::Yes);
    let result = state.tick(press_a());
    assert_eq!(result, ClearSaveResult::Cleared);
}

#[test]
fn clear_save_cursor_navigation() {
    let mut state = ClearSaveState::new();
    assert_eq!(state.cursor, YesNoChoice::No);
    state.tick(press_down());
    assert_eq!(state.cursor, YesNoChoice::Yes);
    state.tick(press_up());
    assert_eq!(state.cursor, YesNoChoice::No);
}

#[test]
fn clear_save_default_trait() {
    let state = ClearSaveState::default();
    assert_eq!(state.cursor, YesNoChoice::No);
}

#[test]
fn checksum_empty_data() {
    assert_eq!(calc_checksum(&[]), !0u8);
}

#[test]
fn checksum_single_byte() {
    assert_eq!(calc_checksum(&[0x42]), !0x42u8);
}

#[test]
fn checksum_wrapping() {
    assert_eq!(calc_checksum(&[0xFF, 0x01]), !0x00u8);
    assert_eq!(calc_checksum(&[0xFF, 0x02]), !0x01u8);
}

#[test]
fn checksum_known_data() {
    let data = [0x10, 0x20, 0x30, 0x40];
    let sum: u8 = data.iter().fold(0u8, |a, &b| a.wrapping_add(b));
    assert_eq!(calc_checksum(&data), !sum);
}

#[test]
fn save_file_status_roundtrip() {
    assert_eq!(SaveFileStatus::from_byte(0), SaveFileStatus::None);
    assert_eq!(SaveFileStatus::from_byte(1), SaveFileStatus::BadChecksum);
    assert_eq!(SaveFileStatus::from_byte(2), SaveFileStatus::Good);
    assert_eq!(SaveFileStatus::from_byte(255), SaveFileStatus::None);

    assert_eq!(SaveFileStatus::None.to_byte(), 0);
    assert_eq!(SaveFileStatus::BadChecksum.to_byte(), 1);
    assert_eq!(SaveFileStatus::Good.to_byte(), 2);
}

#[test]
fn save_file_status_has_save() {
    assert!(!SaveFileStatus::None.has_save());
    assert!(!SaveFileStatus::BadChecksum.has_save());
    assert!(SaveFileStatus::Good.has_save());
}
