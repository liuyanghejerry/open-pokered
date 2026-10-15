//! Source caller contracts ported onto current master. This source has not yet been tested.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn input_a() -> BattleInput {
    BattleInput {
        a: true,
        ..BattleInput::none()
    }
}
fn input_b() -> BattleInput {
    BattleInput {
        b: true,
        ..BattleInput::none()
    }
}
fn input_down() -> BattleInput {
    BattleInput {
        down: true,
        ..BattleInput::none()
    }
}

fn battle() -> BattleScreen {
    let player = create_pokemon_with_moves(
        Species::Snorlax,
        50,
        [0x9a, 0x78],
        [
            MoveId::Headbutt,
            MoveId::Amnesia,
            MoveId::Rest,
            MoveId::BodySlam,
        ],
    )
    .unwrap();
    let enemy = create_pokemon_with_moves(
        Species::Magikarp,
        50,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.phase = BattlePhase::PlayerMenu;
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(73);
    screen
}

fn assert_manual_notice(screen: &mut BattleScreen, selected: usize) {
    assert!(
        matches!(screen.phase, BattlePhase::ShowingText { .. }),
        "MoveSelectionMenu must show a real confirmation page, got {:?}",
        screen.phase
    );
    let rng = serde_json::to_value(&screen.rng).unwrap();
    let state = screen.battle_state.clone();
    for _ in 0..120 {
        screen.update_frame(input_down());
        assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));
        assert_eq!(screen.move_menu.as_ref().unwrap().cursor(), selected);
    }
    assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng);
    assert_eq!(
        serde_json::to_value(&screen.battle_state).unwrap(),
        serde_json::to_value(&state).unwrap()
    );
    screen.update_frame(input_b());
    assert!(
        matches!(screen.phase, BattlePhase::MoveSelect),
        "B confirms the notice, and must not also cancel the rebuilt menu"
    );
    assert_eq!(screen.move_menu.as_ref().unwrap().cursor(), selected);
    assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng);
    screen.update_frame(input_b());
    assert!(matches!(screen.phase, BattlePhase::PlayerMenu));
}

#[test]
fn canonical_fight_no_pp_notice_waits_for_ack() {
    let mut screen = battle();
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .active_mon_mut()
        .pp[0] = 0;
    screen.update_frame(input_a());
    assert!(matches!(screen.phase, BattlePhase::MoveSelect));
    let before = serde_json::to_value(&screen.battle_state).unwrap();
    let rng = serde_json::to_value(&screen.rng).unwrap();
    screen.update_frame(input_a());
    assert_eq!(
        screen.current_message.as_deref(),
        Some("No PP left for\nthis move!")
    );
    assert_eq!(serde_json::to_value(&screen.battle_state).unwrap(), before);
    assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng);
    assert_manual_notice(&mut screen, 0);
}

#[test]
fn canonical_fight_disabled_notice_waits_for_ack() {
    let mut screen = battle();
    screen.battle_state.as_mut().unwrap().player.disabled_move = 3;
    screen.update_frame(input_a());
    screen.update_frame(input_down());
    screen.update_frame(input_down());
    let before = serde_json::to_value(&screen.battle_state).unwrap();
    let rng = serde_json::to_value(&screen.rng).unwrap();
    screen.update_frame(input_a());
    assert_eq!(
        screen.current_message.as_deref(),
        Some("The move is\ndisabled!")
    );
    assert_eq!(serde_json::to_value(&screen.battle_state).unwrap(), before);
    assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng);
    assert_manual_notice(&mut screen, 2);
}

#[test]
fn canonical_fight_no_moves_notice_continues_without_ack() {
    let mut screen = battle();
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .active_mon_mut()
        .pp = [0; 4];
    screen.update_frame(input_a());
    assert!(
        matches!(
            screen.phase,
            BattlePhase::ForcedStruggle { wait_frames: 60 }
        ),
        "NoMovesLeftText ends in done: its automatic delay must start without ACK"
    );
    let rng = serde_json::to_value(&screen.rng).unwrap();
    for _ in 0..59 {
        screen.update_frame(BattleInput::none());
        assert!(matches!(screen.phase, BattlePhase::ForcedStruggle { .. }));
        assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng);
    }
    screen.update_frame(BattleInput::none());
    assert!(!matches!(screen.phase, BattlePhase::ForcedStruggle { .. }));
    assert_ne!(serde_json::to_value(&screen.rng).unwrap(), rng);
    let after_turn = serde_json::to_value(&screen.rng).unwrap();
    screen.update_frame(BattleInput::none());
    assert_eq!(serde_json::to_value(&screen.rng).unwrap(), after_turn);
    assert_eq!(
        screen.battle_state.as_ref().unwrap().player.active_mon().pp,
        [0; 4]
    );
}

#[test]
fn fight_notice_checks_pp_before_disable_and_reformats_only_after_ack() {
    let mut screen = battle();
    let player = &mut screen.battle_state.as_mut().unwrap().player;
    player.active_mon_mut().pp[2] = 0;
    player.disabled_move = 3;
    screen.update_frame(input_a());
    screen.update_frame(input_down());
    screen.update_frame(input_down());
    let menu = serde_json::to_value(&screen.move_menu).unwrap();
    screen.update_frame(input_a());
    assert_eq!(
        screen.current_message.as_deref(),
        Some("No PP left for\nthis move!")
    );
    assert_eq!(serde_json::to_value(&screen.move_menu).unwrap(), menu);
    // A real page owns input: early A/B cannot acknowledge or choose a move.
    for input in [input_a(), input_b()] {
        screen.update_frame(input);
        assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));
    }
    // The return must re-run AnyMoveToSelect, not merely set MoveSelect.
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .active_mon_mut()
        .pp = [0; 4];
    for _ in 0..BATTLE_TEXT_PAGE_WAIT_FRAMES {
        screen.update_frame(BattleInput::none());
    }
    screen.update_frame(input_a());
    assert!(matches!(
        screen.phase,
        BattlePhase::ForcedStruggle { wait_frames: 60 }
    ));
}

#[test]
fn fight_notice_return_preserves_the_row_and_rebuilds_only_after_ack() {
    let mut screen = battle();
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .active_mon_mut()
        .pp[2] = 0;
    screen.update_frame(input_a());
    screen.update_frame(input_down());
    screen.update_frame(input_down());
    screen.update_frame(input_a());
    let retained = serde_json::to_value(&screen.move_menu).unwrap();
    // Isolate the return continuation: the retained menu remains untouched
    // until ACK, then the existing formatter reads current battle data.
    screen
        .battle_state
        .as_mut()
        .unwrap()
        .player
        .active_mon_mut()
        .pp[2] = 4;
    for _ in 0..BATTLE_TEXT_PAGE_WAIT_FRAMES {
        screen.update_frame(BattleInput::none());
    }
    assert_eq!(serde_json::to_value(&screen.move_menu).unwrap(), retained);
    screen.update_frame(input_b());
    assert!(matches!(screen.phase, BattlePhase::MoveSelect));
    let menu = screen.move_menu.as_ref().unwrap();
    assert_eq!(menu.cursor(), 2);
    assert_eq!(menu.current_move().unwrap().current_pp, 4);
    screen.update_frame(BattleInput::none());
    assert_eq!(screen.move_menu.as_ref().unwrap().cursor(), 2);
}

#[test]
fn link_fight_notices_delay_65_frames_without_accepting_buttons_or_drawing_rng() {
    for no_pp in [true, false] {
        let mut screen = battle();
        screen.link_mode = true;
        if no_pp {
            screen
                .battle_state
                .as_mut()
                .unwrap()
                .player
                .active_mon_mut()
                .pp[0] = 0;
        } else {
            screen.battle_state.as_mut().unwrap().player.disabled_move = 1;
        }
        screen.update_frame(input_a());
        let state = serde_json::to_value(&screen.battle_state).unwrap();
        let rng = serde_json::to_value(&screen.rng).unwrap();
        screen.update_frame(input_a());
        assert!(matches!(
            screen.phase,
            BattlePhase::ShowingText {
                wait_frames: 65,
                backdrop: BattleTextBackdrop::FightMoveNotice {
                    wait: FightNoticeWait::LinkDelay
                },
                ..
            }
        ));
        for i in 0..64 {
            screen.update_frame(if i % 2 == 0 { input_a() } else { input_b() });
            assert!(
                matches!(screen.phase, BattlePhase::ShowingText { .. }),
                "frame {}",
                i + 1
            );
            assert!(screen.link_pending_local_action.is_none());
        }
        screen.update_frame(input_a());
        assert!(matches!(screen.phase, BattlePhase::MoveSelect));
        assert!(
            screen.link_pending_local_action.is_none(),
            "expiry's A must not select a move"
        );
        assert_eq!(serde_json::to_value(&screen.battle_state).unwrap(), state);
        assert_eq!(serde_json::to_value(&screen.rng).unwrap(), rng);
    }
}

#[test]
fn no_moves_keeps_original_identity_name_and_ignores_ack_during_automatic_delay() {
    let mut screen = battle();
    let player = &mut screen.battle_state.as_mut().unwrap().player;
    player.preserve_original_identity();
    player.active_mon_mut().species = Species::Magikarp;
    player.active_mon_mut().pp = [0; 4];
    screen.link_mode = true;
    screen.update_frame(input_a());
    assert_eq!(
        screen.current_message.as_deref(),
        Some("SNORLAX has no\nmoves left!")
    );
    for _ in 0..59 {
        screen.update_frame(input_b());
        assert!(matches!(screen.phase, BattlePhase::ForcedStruggle { .. }));
        assert!(screen.link_pending_local_action.is_none());
    }
    screen.update_frame(input_a());
    assert!(matches!(screen.phase, BattlePhase::LinkWaiting));
    assert_eq!(
        screen.link_pending_local_action,
        Some(crate::link::protocol::LinkAction::Struggle)
    );
}

#[test]
fn fight_notice_snapshot_preserves_context_remaining_wait_and_single_return() {
    use crate::snapshot::BattleSnapshot;
    for link in [false, true] {
        let mut screen = battle();
        screen.link_mode = link;
        screen.set_presentation_enabled(true);
        screen
            .battle_state
            .as_mut()
            .unwrap()
            .player
            .active_mon_mut()
            .pp[0] = 0;
        screen.update_frame(input_a());
        screen.update_frame(input_a());
        for _ in 0..4 {
            screen.update_frame(BattleInput::none());
        }
        let snapshot = BattleSnapshot::capture(&screen);
        let encoded = serde_json::to_string(&snapshot).unwrap();
        let decoded: BattleSnapshot = serde_json::from_str(&encoded).unwrap();
        let mut restored = BattleScreen::new(true);
        decoded.restore_into(&mut restored);
        // Link transport is excluded by the existing snapshot API. The typed
        // saved page policy itself still determines this notice's return.
        assert!(!restored.link_mode);
        assert_eq!(
            serde_json::to_value(BattleSnapshot::capture(&restored)).unwrap(),
            serde_json::to_value(&snapshot).unwrap()
        );
        for _ in 0..if link { 60 } else { 6 } {
            restored.update_frame(input_b());
            assert!(matches!(restored.phase, BattlePhase::ShowingText { .. }));
        }
        restored.update_frame(if link { BattleInput::none() } else { input_b() });
        assert!(matches!(restored.phase, BattlePhase::MoveSelect));
        restored.update_frame(input_b());
        assert!(matches!(restored.phase, BattlePhase::PlayerMenu));
    }
}

#[test]
fn legacy_text_snapshot_without_backdrop_retains_ordinary_text_policy() {
    let mut screen = battle();
    screen.show_text_then(vec!["An ordinary page.".into()], BattlePhase::PlayerMenu);
    let mut encoded = serde_json::to_value(&screen.phase).unwrap();
    encoded["ShowingText"]
        .as_object_mut()
        .unwrap()
        .remove("backdrop");
    let phase: BattlePhase = serde_json::from_value(encoded).unwrap();
    assert!(matches!(
        phase,
        BattlePhase::ShowingText {
            backdrop: BattleTextBackdrop::Battle,
            ..
        }
    ));
    screen.phase = phase;
    for _ in 0..70 {
        screen.update_frame(BattleInput::none());
    }
    assert!(
        matches!(screen.phase, BattlePhase::ShowingText { .. }),
        "ordinary text never gains link auto-return"
    );
    screen.update_frame(input_b());
    assert!(matches!(screen.phase, BattlePhase::PlayerMenu));
}
