use super::*;
use pokered_data::slot_machine::*;

#[test]
fn new_lucky_machine() {
    let sm = SlotMachineState::new(true);
    assert_eq!(sm.seven_and_bar_mode_chance, SEVEN_AND_BAR_MODE_LUCKY);
    assert_eq!(sm.flags, 0);
    assert_eq!(sm.bet, 0);
}

#[test]
fn new_normal_machine() {
    let sm = SlotMachineState::new(false);
    assert_eq!(sm.seven_and_bar_mode_chance, SEVEN_AND_BAR_MODE_NORMAL);
}

#[test]
fn place_bet_valid() {
    let mut sm = SlotMachineState::new(false);
    assert!(sm.place_bet(1));
    assert_eq!(sm.bet, 1);
    assert!(sm.place_bet(2));
    assert_eq!(sm.bet, 2);
    assert!(sm.place_bet(3));
    assert_eq!(sm.bet, 3);
}

#[test]
fn place_bet_invalid() {
    let mut sm = SlotMachineState::new(false);
    assert!(!sm.place_bet(0));
    assert!(!sm.place_bet(4));
}

#[test]
fn place_bet_resets_state() {
    let mut sm = SlotMachineState::new(false);
    sm.payout_coins = 100;
    sm.stopping_wheel = 2;
    sm.place_bet(1);
    assert_eq!(sm.payout_coins, 0);
    assert_eq!(sm.stopping_wheel, 0);
    assert_eq!(sm.wheel_slip_counters, [INITIAL_SLIP_COUNTER; 3]);
}

#[test]
fn set_flags_keeps_existing_7bar_mode() {
    let mut sm = SlotMachineState::new(false);
    sm.flags = SLOTS_CAN_WIN_WITH_7_OR_BAR;
    sm.set_flags(100);
    assert_eq!(sm.flags, SLOTS_CAN_WIN_WITH_7_OR_BAR);
}

#[test]
fn set_flags_allow_matches_counter_active() {
    let mut sm = SlotMachineState::new(false);
    sm.allow_matches_counter = 10;
    sm.set_flags(100);
    assert_ne!(sm.flags & SLOTS_CAN_WIN, 0);
}

#[test]
fn set_flags_random_zero_sets_counter() {
    let mut sm = SlotMachineState::new(false);
    sm.set_flags(0);
    assert_eq!(sm.allow_matches_counter, ALLOW_MATCHES_DURATION);
    assert_eq!(sm.flags, 0); // flags not set, just counter
}

#[test]
fn set_flags_high_random_sets_7bar_mode() {
    // For normal machine (chance=253), random > 253 → 7/bar mode
    let mut sm = SlotMachineState::new(false);
    sm.set_flags(254);
    assert_ne!(sm.flags & SLOTS_CAN_WIN_WITH_7_OR_BAR, 0);
}

#[test]
fn set_flags_medium_random_sets_can_win() {
    // random in (210, seven_and_bar_mode_chance] → CAN_WIN
    let mut sm = SlotMachineState::new(false);
    sm.set_flags(211); // > 210, <= 253
    assert_ne!(sm.flags & SLOTS_CAN_WIN, 0);
}

#[test]
fn set_flags_low_random_clears_flags() {
    let mut sm = SlotMachineState::new(false);
    sm.flags = SLOTS_CAN_WIN; // pre-set
    sm.set_flags(100); // <= 210
    assert_eq!(sm.flags, 0);
}

#[test]
fn advance_wheel_wraps_at_max() {
    let mut sm = SlotMachineState::new(false);
    sm.wheel_offsets[0] = WHEEL_OFFSET_MAX - 1;
    sm.advance_wheel(0);
    assert_eq!(sm.wheel_offsets[0], 0);
}

#[test]
fn advance_wheel_normal_increment() {
    let mut sm = SlotMachineState::new(false);
    sm.wheel_offsets[1] = 10;
    sm.advance_wheel(1);
    assert_eq!(sm.wheel_offsets[1], 11);
}

#[test]
fn get_wheel_view_offset_zero() {
    let mut sm = SlotMachineState::new(false);
    sm.wheel_offsets[0] = 0;
    let view = sm.get_wheel_view(0);
    // offset=0, sym_idx=0 → symbols 0,1,2 of wheel1
    assert_eq!(view.bottom, SLOT_MACHINE_WHEEL1[0]); // Seven
    assert_eq!(view.middle, SLOT_MACHINE_WHEEL1[1]); // Mouse
    assert_eq!(view.top, SLOT_MACHINE_WHEEL1[2]); // Fish
}

#[test]
fn get_wheel_view_wraps() {
    let mut sm = SlotMachineState::new(false);
    // sym_idx = 17 (last symbol), should wrap for middle and top
    sm.wheel_offsets[0] = 34; // 34/2 = 17
                              // But wait, offset wraps at 30. So 34 is impossible during normal play.
                              // Let's use offset=28 → sym_idx=14
    sm.wheel_offsets[0] = 28;
    let view = sm.get_wheel_view(0);
    assert_eq!(view.bottom, SLOT_MACHINE_WHEEL1[14]); // Cherry
    assert_eq!(view.middle, SLOT_MACHINE_WHEEL1[15]); // Seven
    assert_eq!(view.top, SLOT_MACHINE_WHEEL1[16]); // Mouse
}

#[test]
fn can_wheel_stop_odd_offset() {
    let mut sm = SlotMachineState::new(false);
    sm.wheel_offsets[0] = 3;
    assert!(sm.can_wheel_stop(0));
}

#[test]
fn can_wheel_stop_even_offset() {
    let mut sm = SlotMachineState::new(false);
    sm.wheel_offsets[0] = 4;
    assert!(!sm.can_wheel_stop(0));
}

#[test]
fn wheel3_stops_immediately() {
    let mut sm = SlotMachineState::new(false);
    sm.wheel_offsets[2] = 3; // odd
    assert!(sm.try_stop_wheel(2));
}

#[test]
fn wheel3_wont_stop_on_even() {
    let mut sm = SlotMachineState::new(false);
    sm.wheel_offsets[2] = 4; // even
    assert!(!sm.try_stop_wheel(2));
}

#[test]
fn check_for_matches_middle_row() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    // Need all 3 wheels to show same symbol in middle position.
    // Wheel1[0]=Seven, so middle=Wheel1[1]=Mouse
    // Need wheel2 middle = Mouse, wheel3 middle = Mouse
    // Wheel2: find Mouse... Wheel2[4]=Mouse, sym_idx=4, offset=8
    // Wheel2 middle = Wheel2[5] = Bar. No.
    // Wheel2[14]=Mouse, sym_idx=14, offset=28, middle=Wheel2[15]=Seven. No.
    // Let's just directly test with known positions.
    // Actually, let's construct a scenario: all wheels at offset where middle=Cherry.
    // Wheel1: Cherry at indices 4,9,14. Middle is idx+1. So if bottom=Cherry(idx=4), middle=Seven(idx=5). No.
    // Let's find where middle is Cherry for each wheel.
    // Wheel1: middle=sym_idx+1=Cherry → sym_idx+1 ∈ {4,9,14} → sym_idx ∈ {3,8,13} → offset ∈ {6,16,26}
    // Wheel2: middle=sym_idx+1=Cherry → find Cherry in wheel2: indices 2,6,9,13,17
    //   sym_idx+1 ∈ {2,6,9,13,17} → sym_idx ∈ {1,5,8,12,16} → offset ∈ {2,10,16,24,32(→wraps)}
    // Wheel3: middle=sym_idx+1=Cherry → find Cherry in wheel3: indices 3,7,11
    //   sym_idx+1 ∈ {3,7,11} → sym_idx ∈ {2,6,10} → offset ∈ {4,12,20}
    // Common: w1 offset=16 (middle=Cherry), w2 offset=16 (sym_idx=8, middle=wheel2[9]=Cherry), w3 offset=12 (sym_idx=6, middle=wheel3[7]=Cherry)
    sm.wheel_offsets[0] = 6; // sym_idx=3, middle=wheel1[4]=Cherry ✓
    sm.wheel_offsets[1] = 10; // sym_idx=5, middle=wheel2[6]=Cherry ✓
    sm.wheel_offsets[2] = 12; // sym_idx=6, middle=wheel3[7]=Cherry ✓
    assert_eq!(sm.check_for_matches(), Some(SlotSymbol::Cherry));
}

#[test]
fn check_for_matches_no_match() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(3);
    // Pick offsets that give all-different symbols across all paylines.
    // w1 offset=2: sym 1,2,3 = Mouse,Fish,Bar
    // w2 offset=4: sym 2,3,4 = Cherry,Bird,Mouse
    // w3 offset=6: sym 3,4,5 = Cherry,Mouse,Bird
    // Middle row: Fish vs Bird vs Mouse → no match
    // Top row: Bar vs Mouse vs Bird → no match
    // Bottom row: Mouse vs Cherry vs Cherry → no match (Mouse≠Cherry)
    // Diag1 (b,m,t): Mouse vs Bird vs Bird → no match (Mouse≠Bird)
    // Diag2 (t,m,b): Bar vs Bird vs Cherry → no match
    sm.wheel_offsets = [2, 4, 6];
    assert_eq!(sm.check_for_matches(), None);
}

#[test]
fn is_seven_or_bar_check() {
    assert!(SlotMachineState::is_seven_or_bar(SlotSymbol::Seven));
    assert!(SlotMachineState::is_seven_or_bar(SlotSymbol::Bar));
    assert!(!SlotMachineState::is_seven_or_bar(SlotSymbol::Cherry));
    assert!(!SlotMachineState::is_seven_or_bar(SlotSymbol::Fish));
    assert!(!SlotMachineState::is_seven_or_bar(SlotSymbol::Bird));
    assert!(!SlotMachineState::is_seven_or_bar(SlotSymbol::Mouse));
}

#[test]
fn calculate_payout_values() {
    let sm = SlotMachineState::new(false);
    assert_eq!(sm.calculate_payout(SlotSymbol::Seven), 300);
    assert_eq!(sm.calculate_payout(SlotSymbol::Bar), 100);
    assert_eq!(sm.calculate_payout(SlotSymbol::Cherry), 8);
    assert_eq!(sm.calculate_payout(SlotSymbol::Fish), 15);
    assert_eq!(sm.calculate_payout(SlotSymbol::Bird), 15);
    assert_eq!(sm.calculate_payout(SlotSymbol::Mouse), 15);
}

#[test]
fn flash_counts() {
    assert_eq!(
        SlotMachineState::flash_count_for_symbol(SlotSymbol::Seven),
        20
    );
    assert_eq!(SlotMachineState::flash_count_for_symbol(SlotSymbol::Bar), 8);
    assert_eq!(
        SlotMachineState::flash_count_for_symbol(SlotSymbol::Cherry),
        2
    );
    assert_eq!(
        SlotMachineState::flash_count_for_symbol(SlotSymbol::Fish),
        4
    );
}

#[test]
fn post_reward_cherry_decrements_counter() {
    let mut sm = SlotMachineState::new(false);
    sm.allow_matches_counter = 10;
    sm.post_reward_effects_with_rng(SlotSymbol::Cherry, 0);
    assert_eq!(sm.allow_matches_counter, 9);
}

#[test]
fn post_reward_bar_clears_flags() {
    let mut sm = SlotMachineState::new(false);
    sm.flags = SLOTS_CAN_WIN | SLOTS_CAN_WIN_WITH_7_OR_BAR;
    sm.post_reward_effects_with_rng(SlotSymbol::Bar, 0);
    assert_eq!(sm.flags, 0);
}

#[test]
fn post_reward_seven_with_high_rng_clears_flags() {
    let mut sm = SlotMachineState::new(false);
    sm.flags = SLOTS_CAN_WIN_WITH_7_OR_BAR;
    sm.allow_matches_counter = 5;
    sm.post_reward_effects_with_rng(SlotSymbol::Seven, 0x80);
    assert_eq!(sm.flags, 0);
    assert_eq!(sm.allow_matches_counter, 0);
}

#[test]
fn post_reward_seven_with_low_rng_keeps_flags() {
    let mut sm = SlotMachineState::new(false);
    sm.flags = SLOTS_CAN_WIN_WITH_7_OR_BAR;
    sm.allow_matches_counter = 5;
    sm.post_reward_effects_with_rng(SlotSymbol::Seven, 0x7F);
    assert_eq!(sm.flags, SLOTS_CAN_WIN_WITH_7_OR_BAR); // preserved
    assert_eq!(sm.allow_matches_counter, 0);
}

#[test]
fn resolve_spin_no_match_returns_none() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    sm.flags = 0; // can't win
    sm.wheel_offsets = [0, 0, 0]; // no match on middle row
    assert_eq!(sm.resolve_spin(), None);
}

#[test]
fn resolve_spin_match_with_can_win() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    sm.flags = SLOTS_CAN_WIN;
    // Set up Cherry match on middle row
    sm.wheel_offsets[0] = 6; // middle=Cherry
    sm.wheel_offsets[1] = 10; // middle=Cherry
    sm.wheel_offsets[2] = 12; // middle=Cherry
    let result = sm.resolve_spin();
    assert!(result.is_some());
    let (sym, payout) = result.unwrap();
    assert_eq!(sym, SlotSymbol::Cherry);
    assert_eq!(payout, 8);
}

// ── Wheel 2 early-stop direction (SlotMachine_StopWheel2Early) ──────────
// Regression guards for the inverted condition that used to make wins
// structurally impossible: the ASM stops wheel 2 early when the first two
// wheels ARE lined up, and keeps it spinning otherwise.

/// w1 offset 1 → bottom Seven, middle Mouse, top Fish (sym_idx 0).
const W1_ALIGNED: u8 = 1;

#[test]
fn wheel2_stops_when_wheels_are_aligned_in_normal_mode() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    sm.flags = 0;
    sm.wheel_offsets[0] = W1_ALIGNED;
    // w2 offset 7 (odd → stoppable) → sym_idx 3: bottom Bird, middle Mouse,
    // top Bar — the middle pair (Mouse/Mouse) lines up with wheel 1.
    sm.wheel_offsets[1] = 7;
    sm.wheel_slip_counters[1] = 4;
    assert!(sm.try_stop_wheel(1), "aligned wheels must stop wheel 2");
    assert_eq!(sm.wheel_slip_counters[1], 0, "stop zeroes the slip counter");
}

#[test]
fn wheel2_keeps_spinning_when_unaligned_in_normal_mode() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    sm.flags = 0;
    sm.wheel_offsets[0] = W1_ALIGNED;
    // w2 offset 3 (odd → stoppable) → sym_idx 1: bottom Fish, middle Cherry,
    // top Bird — no pair matches wheel 1's Seven/Mouse/Fish.
    sm.wheel_offsets[1] = 3;
    sm.wheel_slip_counters[1] = 4;
    assert!(
        !sm.try_stop_wheel(1),
        "unaligned wheels must not stop wheel 2 yet"
    );
    assert_eq!(sm.wheel_slip_counters[1], 3, "a failed attempt burns one slip");
}

#[test]
fn wheel2_7bar_mode_stops_only_on_bottom_seven_or_bar() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    sm.flags = SLOTS_CAN_WIN_WITH_7_OR_BAR;
    sm.wheel_offsets[0] = W1_ALIGNED;
    // Aligned position (as in the normal-mode test) but wheel 2's bottom is
    // Bird: in 7/bar mode the alignment must NOT stop the wheel — the ASM's
    // match result is clobbered by the bottom-tile comparison.
    sm.wheel_offsets[1] = 7;
    sm.wheel_slip_counters[1] = 4;
    assert!(!sm.try_stop_wheel(1), "alignment alone must not stop in 7/bar mode");
    assert_eq!(sm.wheel_slip_counters[1], 3, "the rejected stop burns one slip");
    // Bottom slot Seven (sym_idx 0 → offset 1): stops even unaligned.
    sm.wheel_offsets[1] = 1;
    assert!(sm.try_stop_wheel(1), "bottom 7 must stop wheel 2 in 7/bar mode");
    // Bottom slot Bar: offset 11 (odd) → sym_idx 5.
    sm.wheel_offsets[1] = 11;
    sm.wheel_slip_counters[1] = 4;
    assert!(sm.try_stop_wheel(1), "bottom bar must stop wheel 2 in 7/bar mode");
}

// ── Post-reward effects / reroll counter bookkeeping ────────────────────

#[test]
fn resolve_spin_does_not_apply_reward_effects() {
    // The ASM reward funcs run the side effects once; the caller owns them
    // (post_reward_effects_with_rng). resolve_spin must leave all of it alone.
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    sm.flags = SLOTS_CAN_WIN;
    sm.allow_matches_counter = 3;
    sm.reroll_counter = 7;
    sm.wheel_offsets = [6, 10, 12]; // cherry middle-row match
    let result = sm.resolve_spin();
    assert_eq!(result, Some((SlotSymbol::Cherry, 8)));
    assert_eq!(sm.flags, SLOTS_CAN_WIN, "flags untouched");
    assert_eq!(sm.allow_matches_counter, 3, "counter untouched");
    assert_eq!(sm.reroll_counter, 7, "no rerolls happened for an instant match");
}

#[test]
fn place_bet_preserves_reroll_counter() {
    let mut sm = SlotMachineState::new(false);
    sm.reroll_counter = 200;
    sm.place_bet(3);
    assert_eq!(sm.reroll_counter, 200, "the ASM only zeroes it per session");
}

#[test]
fn reroll_counter_wraps_from_zero_and_persists() {
    let mut sm = SlotMachineState::new(false);
    sm.place_bet(1);
    sm.flags = SLOTS_CAN_WIN;
    // No completable line (wheel 2 shares no symbol with wheel 1 on any of
    // the five pairs), so every reroll fails: the counter wraps 0 → 255 and
    // runs down to 0 before giving up.
    sm.wheel_offsets = [1, 2, 4];
    assert_eq!(sm.resolve_spin(), None);
    assert_eq!(sm.reroll_counter, 0);
}

#[test]
fn can_win_spins_win_via_wheel3_reroll_when_wheels_align() {
    // The core original mechanic: wheels 1+2 aligned + CAN_WIN → wheel 3
    // rerolls onto the match and the spin pays out. With a 3-coin bet every
    // payline is active, so any alignment can be completed.
    for w1 in [1u8, 3, 5, 7, 9] {
        let mut sm = SlotMachineState::new(false);
        sm.place_bet(3);
        sm.flags = SLOTS_CAN_WIN;
        sm.wheel_offsets[0] = w1;
        // Find a wheel-2 offset where some pair lines up; the wheel-3 offset
        // is irrelevant — the reroll scans it.
        let mut aligned = false;
        for w2 in (0..30).step_by(2) {
            sm.wheel_offsets[1] = w2 + 1;
            if sm.find_wheel1_wheel2_match() {
                aligned = true;
                break;
            }
        }
        if !aligned {
            continue;
        }
        sm.wheel_offsets[2] = 1;
        let result = sm.resolve_spin();
        assert!(
            result.is_some(),
            "aligned + CAN_WIN must resolve to a win (w1={w1})"
        );
    }
}
