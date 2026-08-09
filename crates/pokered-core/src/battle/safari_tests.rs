//! Tests for the Gen-1 Safari mechanics — verified against the disassembly
//! (item_effects.asm ItemUseBait/ItemUseRock, safari_zone.asm, core.asm flee check).

use super::*;
use crate::battle::state::StatusCondition;

fn state(catch_rate: u8, balls: u8) -> SafariState {
    SafariState::new(catch_rate, balls)
}

// ── bait (ItemUseBait) ──

#[test]
fn bait_halves_rate_clears_anger_adds_eating() {
    let mut s = state(100, 30);
    s.escape_factor = 3; // was angry
    s.apply_bait(2);
    assert_eq!(s.catch_rate, 50, "bait halves the catch rate (srl)");
    assert_eq!(s.escape_factor, 0, "bait clears anger");
    assert_eq!(s.bait_factor, 2, "bait raises the eating counter");
}

#[test]
fn bait_eating_counter_caps_at_255() {
    let mut s = state(100, 30);
    s.bait_factor = 254;
    s.apply_bait(5);
    assert_eq!(s.bait_factor, 255, "eating counter saturates at 255");
}

// ── rock (ItemUseRock) ──

#[test]
fn rock_doubles_rate_clears_bait_adds_anger() {
    let mut s = state(100, 30);
    s.bait_factor = 3; // was eating
    s.apply_rock(2);
    assert_eq!(s.catch_rate, 200, "rock doubles the catch rate");
    assert_eq!(s.bait_factor, 0, "rock clears eating");
    assert_eq!(s.escape_factor, 2, "rock raises the anger counter");
}

#[test]
fn rock_caps_catch_rate_at_255() {
    let mut s = state(200, 30);
    s.apply_rock(1);
    assert_eq!(s.catch_rate, 255, "doubling caps at 255 (add a; ld a,$ff)");
}

// ── upkeep (PrintSafariZoneBattleText) ──

#[test]
fn upkeep_decrements_bait_first_and_reports_eating() {
    let mut s = state(100, 30);
    s.bait_factor = 2;
    s.escape_factor = 4; // ignored while eating
    assert_eq!(s.upkeep(), SafariUpkeep::Eating);
    assert_eq!(s.bait_factor, 1);
    assert_eq!(s.escape_factor, 4, "anger untouched while eating");
}

#[test]
fn upkeep_anger_wearing_off_restores_base_catch_rate() {
    let mut s = state(100, 30);
    s.catch_rate = 200; // was doubled by a rock
    s.escape_factor = 1;
    assert_eq!(s.upkeep(), SafariUpkeep::Angry);
    assert_eq!(s.escape_factor, 0);
    assert_eq!(s.catch_rate, 100, "catch rate restored to base when anger wears off");
}

#[test]
fn upkeep_anger_still_active_keeps_boosted_rate() {
    let mut s = state(100, 30);
    s.catch_rate = 200;
    s.escape_factor = 2;
    assert_eq!(s.upkeep(), SafariUpkeep::Angry);
    assert_eq!(s.escape_factor, 1);
    assert_eq!(s.catch_rate, 200, "boost persists while still angry");
}

#[test]
fn upkeep_neutral_is_none() {
    let mut s = state(100, 30);
    assert_eq!(s.upkeep(), SafariUpkeep::None);
}

// ── flee roll (core.asm) ──

#[test]
fn flee_high_speed_low_byte_always_runs() {
    let s = state(100, 30);
    // speed low byte > 127 → the doubling overflows → immediate run.
    assert!(s.flee_roll(200, 0), "speed low byte 200 > 127 always flees");
    assert!(s.flee_roll(128, 255), "128 > 127 flees regardless of the random byte");
    // Only the LOW byte matters: 306 & 0xFF = 50.
    assert!(!s.flee_roll(306, 100), "speed 306 → low byte 50 → b=100, random 100 !< 100");
}

#[test]
fn flee_neutral_threshold() {
    let s = state(100, 30); // no bait/anger
    // b = speed_low * 2 = 100. Runs iff random < 100.
    assert!(s.flee_roll(50, 99), "random 99 < 100 → flees");
    assert!(!s.flee_roll(50, 100), "random 100 !< 100 → stays");
}

#[test]
fn flee_bait_quarters_the_chance() {
    let mut s = state(100, 30);
    s.bait_factor = 1;
    // b = 100 >> 2 = 25. Runs iff random < 25.
    assert!(s.flee_roll(50, 24), "eating: random 24 < 25 → flees");
    assert!(!s.flee_roll(50, 25), "eating: random 25 !< 25 → stays (bait cut the chance)");
}

#[test]
fn flee_rock_doubles_the_chance() {
    let mut s = state(100, 30);
    s.escape_factor = 1;
    // b = 100 << 1 = 200. Runs iff random < 200.
    assert!(s.flee_roll(50, 199), "angry: random 199 < 200 → flees");
    assert!(!s.flee_roll(50, 200), "angry: random 200 !< 200 → stays");
}

// ── ball economy ──

#[test]
fn throw_ball_consumes_a_ball_and_uses_the_live_catch_rate() {
    let mut s = state(3, 5); // low catch rate → very hard
    s.apply_rock(1); // rock doubles it to 6 (and makes angry)
    assert_eq!(s.catch_rate, 6);
    let before = s.balls;
    // A high-HP full mon at a tiny catch rate almost never catches; assert the ball spend.
    let _ = s.throw_ball(200, 200, StatusCondition::None, CaptureRandoms { rand1: 254, rand2: 254 });
    assert_eq!(s.balls, before - 1, "a thrown Safari Ball is consumed");
}

// ── roll 1..=5 (BaitRockCommon.randomLoop) ──

#[test]
fn roll_bait_rock_amount_is_1_to_5_via_rejection() {
    // Bytes whose &7 are: 7 (reject), 5 (reject), 6 (reject), 0 → 1.
    let seq = [7u8, 5, 6, 0];
    let mut i = 0;
    let mut next = || {
        let v = seq[i];
        i += 1;
        v
    };
    assert_eq!(roll_bait_rock_amount(&mut next), 1, "0&7=0 → +1 = 1 after rejecting 7/5/6");
    // &7 == 4 → 5 (the max).
    let mut next2 = || 4u8;
    assert_eq!(roll_bait_rock_amount(&mut next2), 5);
}
