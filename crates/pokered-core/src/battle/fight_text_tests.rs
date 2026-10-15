//! Explicit source-order model checks, not an original-ROM wall-frame oracle.
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

const FAST: BattleTextSettings = BattleTextSettings {
    options_delay: 1,
    letter_flags_before_processor: 1,
    no_text_delay: false,
};

fn a() -> BattleInput {
    BattleInput {
        a: true,
        ..BattleInput::none()
    }
}
fn step(
    s: &mut BattleScreen,
    edge: BattleInput,
    held: u8,
    settings: Option<BattleTextSettings>,
) -> FightTextReceipt {
    let mut receipt = FightTextReceipt::default();
    s.update_frame_with_text_input(
        edge,
        BattleTextFrame {
            held_buttons: held,
            settings,
        },
        &mut receipt,
    );
    receipt
}
fn scene(no_moves: bool) -> BattleScreen {
    let mut player = create_pokemon_with_moves(
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
    if no_moves {
        player.pp = [0; 4];
    } else {
        player.pp[0] = 0;
    }
    let enemy = create_pokemon_with_moves(
        Species::Magikarp,
        50,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut s = BattleScreen::from_parties(true, &[player], &[enemy], None);
    s.phase = BattlePhase::PlayerMenu;
    s.rng = pokered_rules::runtime::StdBattleRng::from_seed(73);
    s
}
fn notice(settings: BattleTextSettings, link: bool) -> BattleScreen {
    let mut s = scene(false);
    s.link_mode = link;
    step(&mut s, a(), 1, Some(settings));
    assert!(matches!(s.phase, BattlePhase::MoveSelect));
    assert!(step(&mut s, a(), 1, Some(settings)).press_ab);
    assert_eq!(s.fight_text_work().unwrap().stage, FightTextStage::BoxSync);
    s
}
fn wait(s: &BattleScreen) -> u16 {
    match s.phase {
        BattlePhase::ShowingText { wait_frames, .. } => wait_frames,
        _ => panic!("{:?}", s.phase),
    }
}

#[test]
fn fight_text_receipt_does_not_repeat_selection_sound_during_waits_or_presentation() {
    let mut s = scene(false);
    let text = BattleTextFrame {
        held_buttons: 1,
        settings: Some(FAST),
    };
    let mut receipt = FightTextReceipt::default();
    s.update_frame_with_text_input(a(), text, &mut receipt);
    s.update_frame_with_text_input(a(), text, &mut receipt);
    assert!(receipt.press_ab);
    assert_eq!(wait(&s), 3);

    s.update_frame_with_text_input(a(), text, &mut receipt);
    assert!(
        !receipt.press_ab,
        "a protected wait must clear the previous selection sound"
    );
    assert_eq!(wait(&s), 2);

    s.presentation.enabled = true;
    s.presentation.waiting = true;
    receipt.press_ab = true;
    let phase = s.phase.clone();
    s.update_frame_with_text_input(a(), text, &mut receipt);
    assert!(
        !receipt.press_ab,
        "a frozen update must also clear its output receipt"
    );
    assert_eq!(s.phase, phase);
}

#[test]
fn fight_processor_reenables_letters_and_retains_the_final_glyph_wait() {
    for delay in [1, 3, 5] {
        let mut s = notice(BattleTextSettings::fight_menu(delay, false), false);
        for frame in 1..=2 {
            step(&mut s, a(), 1, None);
            assert_eq!(s.fight_text_visible_message().as_deref(), Some(""));
            assert_eq!(wait(&s), 3 - frame);
        }
        // Release before PrintLetterDelay starts: first letter follows box3.
        step(&mut s, BattleInput::none(), 0, None);
        assert_eq!(s.fight_text_visible_message().as_deref(), Some("N"));
        assert_eq!(wait(&s), u16::from(delay));
        for frame in 1..=24 * u16::from(delay) {
            step(&mut s, BattleInput::none(), 0, None);
            if frame < 24 * u16::from(delay) {
                assert_eq!(s.fight_text_work().unwrap().stage, FightTextStage::Printing);
            }
        }
        assert_eq!(
            s.fight_text_work().unwrap().stage,
            FightTextStage::PromptSync
        );
        assert_eq!(wait(&s), 3);
        assert!(s.fight_text_work().unwrap().arrow_visible());
    }
    let flags = BattleTextSettings::fight_menu(5, true);
    assert_eq!(
        flags.letter_flags_before_processor, 0,
        "LinkMenu clears FAST"
    );
    assert!(
        !flags.no_text_delay,
        "temporary menus returned before FIGHT"
    );
    assert_eq!(
        flags.effective_letter_flags(),
        2,
        "InitBattleCommon's clear is not a permanent bypass"
    );
    let mut s = notice(flags, false);
    for _ in 0..3 {
        step(&mut s, BattleInput::none(), 0, None);
    }
    assert_eq!(
        wait(&s),
        1,
        "without FAST flag, PrintLetterDelay uses one frame"
    );
}

#[test]
fn fight_no_delay_still_has_box_and_prompt_sync_and_never_reuses_entry_a() {
    let mut s = notice(
        BattleTextSettings {
            no_text_delay: true,
            ..FAST
        },
        false,
    );
    for frame in 1..=5 {
        let receipt = step(&mut s, a(), 1, None);
        assert!(!receipt.press_ab);
        if frame < 3 {
            assert_eq!(s.fight_text_visible_message().as_deref(), Some(""));
        } else {
            assert_eq!(
                s.fight_text_visible_message().as_deref(),
                Some("No PP left for\nthis move!")
            );
        }
    }
    step(&mut s, a(), 1, None);
    assert_eq!(
        s.fight_text_work().unwrap().stage,
        FightTextStage::ManualScroll
    );
    assert_eq!(wait(&s), 0);
    // NO_TEXT_DELAY skipped Joypad while printing. Held entry A is not a new
    // sample, even if a host kept sending an A edge during the protected waits.
    assert!(!step(&mut s, a(), 1, None).press_ab);
    step(&mut s, BattleInput::none(), 0, None);
    assert!(step(&mut s, a(), 1, None).press_ab);
    assert_eq!(
        s.fight_text_work().unwrap().stage,
        FightTextStage::MenuInfoSync
    );
}

#[test]
fn fight_final_glyph_expiry_a_schedules_one_real_frame_and_is_not_an_ack() {
    let mut s = notice(FAST, false);
    for _ in 0..26 {
        step(&mut s, BattleInput::none(), 0, None);
    }
    assert_eq!(s.fight_text_work().unwrap().visible_chars, 24);
    assert_eq!(wait(&s), 1);
    assert!(!step(&mut s, a(), 1, None).press_ab);
    let work = s.fight_text_work().unwrap();
    assert_eq!(work.stage, FightTextStage::Printing);
    assert!(work.short_exit_scheduled);
    assert_eq!(wait(&s), 1);
    assert!(!step(&mut s, BattleInput::none(), 1, None).press_ab);
    assert_eq!(
        s.fight_text_work().unwrap().stage,
        FightTextStage::PromptSync
    );
    assert_eq!(wait(&s), 3);
}

#[test]
fn fight_short_wait_return_polls_only_when_a_following_glyph_calls_joypad() {
    let mut final_glyph = notice(FAST, false);
    for _ in 0..26 {
        step(&mut final_glyph, BattleInput::none(), 0, None);
    }
    assert_eq!(final_glyph.fight_text_work().unwrap().visible_chars, 24);
    // A causes the final PrintLetterDelay to take .endWait -> DelayFrame.
    assert!(!step(&mut final_glyph, a(), 1, None).press_ab);
    assert!(final_glyph.fight_text_work().unwrap().short_exit_scheduled);
    // Releasing on .done's return frame must not change hJoyLast's analogue.
    assert!(!step(&mut final_glyph, BattleInput::none(), 0, None).press_ab);
    let work = final_glyph.fight_text_work().unwrap();
    assert_eq!(work.stage, FightTextStage::PromptSync);
    assert_eq!(work.last_polled_buttons, 1);
    for held in [0, 1, 1] {
        assert!(!step(&mut final_glyph, BattleInput::none(), held, None).press_ab);
    }
    assert_eq!(
        final_glyph.fight_text_work().unwrap().stage,
        FightTextStage::ManualScroll
    );
    // Only an actual ManualTextScroll release poll and subsequent press ACK.
    assert!(!step(&mut final_glyph, BattleInput::none(), 0, None).press_ab);
    assert!(step(&mut final_glyph, a(), 1, None).press_ab);
    assert!(final_glyph.fight_text_work().unwrap().returning_menu());

    let mut next_glyph = notice(FAST, false);
    for _ in 0..3 {
        step(&mut next_glyph, BattleInput::none(), 1, None);
    }
    let work = next_glyph.fight_text_work().unwrap();
    assert_eq!(work.visible_chars, 1);
    assert!(work.short_exit_scheduled);
    assert_eq!(work.last_polled_buttons, 1);
    // The same .done return has another glyph after it. That glyph really
    // enters PrintLetterDelay and observes the release in this update.
    assert!(!step(&mut next_glyph, BattleInput::none(), 0, None).press_ab);
    let work = next_glyph.fight_text_work().unwrap();
    assert_eq!(work.stage, FightTextStage::Printing);
    assert_eq!(work.visible_chars, 2);
    assert_eq!(work.last_polled_buttons, 0);
}

#[test]
fn fight_disabled_prompt_returns_to_its_original_row_after_the_two_menu_syncs() {
    let settings = BattleTextSettings {
        no_text_delay: true,
        ..FAST
    };
    let mut s = scene(false);
    s.battle_state.as_mut().unwrap().player.disabled_move = 3;
    step(&mut s, a(), 1, Some(settings));
    for _ in 0..2 {
        step(
            &mut s,
            BattleInput {
                down: true,
                ..BattleInput::none()
            },
            128,
            Some(settings),
        );
    }
    let before = serde_json::to_value(&s.battle_state).unwrap();
    let rng = serde_json::to_value(&s.rng).unwrap();
    assert!(step(&mut s, a(), 1, Some(settings)).press_ab);
    assert_eq!(s.move_menu.as_ref().unwrap().cursor(), 2);
    for _ in 0..6 {
        step(&mut s, BattleInput::none(), 0, None);
    }
    assert_eq!(
        s.fight_text_visible_message().as_deref(),
        Some("The move is\ndisabled!")
    );
    assert!(
        step(
            &mut s,
            BattleInput {
                b: true,
                ..BattleInput::none()
            },
            2,
            None
        )
        .press_ab
    );
    for frame in 1..=6 {
        step(&mut s, BattleInput::none(), 0, None);
        assert_eq!(s.move_menu.as_ref().unwrap().cursor(), 2);
        if frame < 6 {
            assert!(s.fight_text_work().unwrap().returning_menu());
        }
    }
    assert!(matches!(s.phase, BattlePhase::MoveSelect));
    assert_eq!(s.current_message, None);
    assert_eq!(serde_json::to_value(&s.battle_state).unwrap(), before);
    assert_eq!(serde_json::to_value(&s.rng).unwrap(), rng);
}

#[test]
fn fight_ack_and_return_menu_use_program_samples_across_both_delay3_calls() {
    let mut s = notice(FAST, false);
    let state = serde_json::to_value(&s.battle_state).unwrap();
    let rng = serde_json::to_value(&s.rng).unwrap();
    for frame in 1..=29 {
        // A starts in the final protected prompt frame, not at the poll.
        let held = if frame == 29 { 1 } else { 0 };
        assert!(!step(&mut s, BattleInput::none(), held, None).press_ab);
    }
    assert_eq!(
        s.fight_text_work().unwrap().stage,
        FightTextStage::PromptSync
    );
    assert!(
        step(&mut s, BattleInput::none(), 1, None).press_ab,
        "actual Joypad sees new held A although the host has no fresh edge now"
    );
    assert_eq!(
        s.fight_text_work().unwrap().stage,
        FightTextStage::MenuInfoSync
    );
    for frame in 1..=5 {
        let held = if frame == 5 { 2 } else { 0 };
        assert!(!step(&mut s, BattleInput::none(), held, None).press_ab);
        assert!(s.fight_text_work().unwrap().returning_menu());
    }
    assert!(step(&mut s, BattleInput::none(), 2, None).press_ab);
    assert!(
        matches!(s.phase, BattlePhase::PlayerMenu),
        "first resumed menu poll accepts B, not earlier"
    );
    assert_eq!(serde_json::to_value(&s.battle_state).unwrap(), state);
    assert_eq!(serde_json::to_value(&s.rng).unwrap(), rng);
}

#[test]
fn fight_link_65_starts_after_print_and_prompt_and_has_no_manual_ack_sound() {
    let mut s = notice(
        BattleTextSettings {
            no_text_delay: true,
            ..FAST
        },
        true,
    );
    let rng = serde_json::to_value(&s.rng).unwrap();
    for frame in 1..=6 {
        assert!(!step(&mut s, a(), 1, None).press_ab);
        if frame == 6 {
            assert_eq!(wait(&s), 65);
        }
    }
    assert_eq!(
        s.fight_text_work().unwrap().stage,
        FightTextStage::LinkScroll
    );
    assert!(!s.fight_text_work().unwrap().arrow_visible());
    for frame in 1..=64 {
        assert!(!step(&mut s, a(), if frame % 2 == 0 { 1 } else { 2 }, None).press_ab);
        assert_eq!(wait(&s), 65 - frame);
    }
    assert!(!step(&mut s, a(), 1, None).press_ab);
    assert_eq!(
        s.fight_text_work().unwrap().stage,
        FightTextStage::MenuInfoSync
    );
    for _ in 0..6 {
        assert!(!step(&mut s, BattleInput::none(), 0, None).press_ab);
    }
    assert!(matches!(s.phase, BattlePhase::MoveSelect));
    assert!(s.link_pending_local_action.is_none());
    assert_eq!(serde_json::to_value(&s.rng).unwrap(), rng);
}

#[test]
fn fight_no_moves_done_starts_caller60_after_printing_without_a_prompt() {
    let mut s = scene(true);
    assert!(
        step(
            &mut s,
            a(),
            1,
            Some(BattleTextSettings {
                no_text_delay: true,
                ..FAST
            })
        )
        .press_ab
    );
    let rng = serde_json::to_value(&s.rng).unwrap();
    for _ in 0..2 {
        step(&mut s, a(), 1, None);
        assert_eq!(s.fight_text_work().unwrap().stage, FightTextStage::BoxSync);
    }
    step(&mut s, a(), 1, None);
    assert!(matches!(
        s.phase,
        BattlePhase::ForcedStruggle { wait_frames: 60 }
    ));
    assert_eq!(
        s.current_message.as_deref(),
        Some("SNORLAX has no\nmoves left!")
    );
    for _ in 0..59 {
        assert!(!step(&mut s, a(), 1, None).press_ab);
    }
    assert_eq!(serde_json::to_value(&s.rng).unwrap(), rng);
    step(&mut s, a(), 1, None);
    assert_ne!(serde_json::to_value(&s.rng).unwrap(), rng);
    assert_eq!(
        s.battle_state.as_ref().unwrap().player.active_mon().pp,
        [0; 4]
    );
}

#[test]
fn fight_phase_json_restores_glyph_wait_settings_and_actual_joypad_history() {
    use crate::snapshot::BattleSnapshot;
    for checkpoint in [2, 5, 26, 28, 31, 34] {
        let mut original = notice(FAST, false);
        for frame in 1..=checkpoint {
            step(
                &mut original,
                BattleInput::none(),
                if frame >= 30 { 1 } else { 0 },
                None,
            );
        }
        let json = serde_json::to_string(&BattleSnapshot::capture(&original)).unwrap();
        let snap: BattleSnapshot = serde_json::from_str(&json).unwrap();
        let mut restored = BattleScreen::new(true);
        snap.restore_into(&mut restored);
        for frame in 0..12 {
            let held = if frame % 4 < 2 { 2 } else { 0 };
            let r1 = step(&mut original, BattleInput::none(), held, None);
            let r2 = step(&mut restored, BattleInput::none(), held, None);
            assert_eq!(r1, r2);
            assert_eq!(
                serde_json::to_value(BattleSnapshot::capture(&original)).unwrap(),
                serde_json::to_value(BattleSnapshot::capture(&restored)).unwrap()
            );
        }
    }
}
