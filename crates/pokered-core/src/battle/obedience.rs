//! Traded-mon obedience (`CheckForDisobedience`, engine/battle/core.asm:3828-4010).
//!
//! ## The mechanic (asm evidence)
//!
//! A Pokémon whose OT ID differs from the player's (`wPartyMon1OTID` vs
//! `wPlayerID`) may refuse orders once its level exceeds the badge threshold:
//!
//! ```text
//! .monIsTraded
//!     ld hl, wObtainedBadges
//!     bit BIT_EARTHBADGE, [hl]
//!     ld a, 101
//!     jr nz, .next
//!     bit BIT_MARSHBADGE, [hl]
//!     ld a, 70
//!     jr nz, .next
//!     bit BIT_RAINBOWBADGE, [hl]
//!     ld a, 50
//!     jr nz, .next
//!     bit BIT_CASCADEBADGE, [hl]
//!     ld a, 30
//!     jr nz, .next
//!     ld a, 10
//! ```
//!
//! i.e. L10 with no badges / L30 CascadeBadge / L50 RainbowBadge / L70
//! MarshBadge / always obey with EarthBadge (threshold 101 > max level).
//! (Badge bits per constants/ram_constants.asm:56-63; note it is RAINBOW that
//! grants 50 and MARSH that grants 70 — NOT Volcano.)
//!
//! If `level > threshold` the roll sequence is (b = threshold + level, capped
//! at $ff; c = threshold; d = level):
//!   1. `.loop1`: draw (nibble-swapped) until `< b`; if `< c` → obey.
//!   2. `.loop2`: draw until `< b`; if `< c` → use a RANDOM other move
//!      (`.useRandomMove` — see below).
//!   3. else with `diff = level − threshold`, one nibble-swapped draw `r`:
//!      `r < diff` → falls asleep (`.monNaps`: `swap(2×rand) & 7`, reroll on 0
//!      → 1..=7 sleep turns, "began to nap!");
//!      `r − diff >= diff` → does nothing (`.monDoesNothing`: one more draw
//!      `& 3` → 0 "is loafing around." / 1 "won't obey!" / 2 "turned away!" /
//!      3 "ignored orders!");
//!      otherwise → "won't obey!" + `HandleSelfConfusionDamage` (a typeless
//!      40-power self-hit, core.asm:3672).
//!
//! `.useRandomMove` falls back to `.monDoesNothing` when the mon knows only one
//! move, has a disabled move, selected Struggle, or only one move has PP left.
//! Otherwise it picks a slot via `rand & 3`, rerolling while `>= wMaxMenuItem`
//! (the fight menu's `wNumMovesMinusOne + 2`, core.asm:2550-2552 — so all known
//! slots are eligible), while it equals the selected slot, or while the slot
//! has no PP. No special text prints — the mon just uses that move.
//!
//! Link battles skip the check entirely (no link battles exist here).

use pokered_data::moves::MoveId;

/// Badge bit positions (constants/ram_constants.asm:56-63).
const BIT_CASCADEBADGE: u8 = 1;
const BIT_RAINBOWBADGE: u8 = 3;
const BIT_MARSHBADGE: u8 = 5;
const BIT_EARTHBADGE: u8 = 7;

/// The obedience level threshold for a badge set (`.monIsTraded`'s ladder).
pub fn obedience_threshold(badges: u8) -> u8 {
    if badges & (1 << BIT_EARTHBADGE) != 0 {
        101
    } else if badges & (1 << BIT_MARSHBADGE) != 0 {
        70
    } else if badges & (1 << BIT_RAINBOWBADGE) != 0 {
        50
    } else if badges & (1 << BIT_CASCADEBADGE) != 0 {
        30
    } else {
        10
    }
}

/// Is this mon traded (for obedience)? The asm compares the 2-byte OT ID
/// against the player's ID. `ot_id == 0` is treated as OWN, not traded: saves
/// written before OT IDs were tracked (and mons from code paths that never
/// stamp one) carry 0, and flagging them all as traded would make every
/// high-level legacy mon disobey. (Deviation: a genuine OT ID of 0x0000 —
/// 1/65536 — would disobey on original hardware.)
pub fn is_traded_for(ot_id: u16, player_id: u16) -> bool {
    ot_id != 0 && ot_id != player_id
}

/// What a disobedience roll produces. The do-nothing family differs only in
/// the printed line; the caller emits the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisobedienceOutcome {
    /// The mon obeys (level ≤ threshold, or the roll passed).
    Obey,
    /// "began to nap!" — falls asleep for the given turns (1..=7).
    Nap(u8),
    /// "is loafing around."
    LoafAround,
    /// "won't obey!" (do-nothing variant — no self-hit).
    WontObey,
    /// "turned away!"
    TurnedAway,
    /// "ignored orders!"
    IgnoredOrders,
    /// "won't obey!" + `HandleSelfConfusionDamage` (typeless 40-power self-hit).
    WontObeySelfHit,
    /// Uses `moves[slot]` instead of the selected move (no special text).
    UseRandomMove(usize),
}

/// Nibble swap — the asm's `swap a` after `BattleRandom`.
fn swap(a: u8) -> u8 {
    (a << 4) | (a >> 4)
}

/// `.monDoesNothing`: one more `BattleRandom & 3` picks the flavour line.
fn does_nothing(rng: &mut dyn FnMut() -> u8) -> DisobedienceOutcome {
    match rng() & 3 {
        0 => DisobedienceOutcome::LoafAround,
        1 => DisobedienceOutcome::WontObey,
        2 => DisobedienceOutcome::TurnedAway,
        _ => DisobedienceOutcome::IgnoredOrders,
    }
}

/// The full `CheckForDisobedience` roll sequence for a TRADED mon (the caller
/// gates on [`is_traded_for`] and on the pre-move states the original checks
/// first — sleep/freeze/flinch/recharge/charging). `selected_slot` is the
/// menu cursor (`wCurrentMenuItem`); `has_disabled_move` is
/// `wPlayerDisabledMoveNumber != 0`.
///
/// The reroll loops mirror the asm's unbounded rejection sampling, capped at
/// [`MAX_REROLL`] iterations as a safety net: a real rng terminates in a few
/// draws, and the cap is only reachable by a degenerate (scripted/exhausted)
/// rng, where the fallback keeps the game from hanging (the original WOULD
/// hang on such a stream).
pub fn check_disobedience(
    level: u8,
    badges: u8,
    selected_slot: usize,
    moves: &[MoveId; 4],
    pp: &[u8; 4],
    has_disabled_move: bool,
    rng: &mut dyn FnMut() -> u8,
) -> DisobedienceOutcome {
    let c = obedience_threshold(badges) as u16;
    let d = level as u16;
    // b = threshold + level, capped at $ff.
    let b = (c + d).min(0xFF);
    // `cp d / jp nc` — threshold >= level: always obeys, NO rng drawn.
    if c >= d {
        return DisobedienceOutcome::Obey;
    }

    // .loop1: nibble-swapped draws until < b; < c → obey.
    let mut r1 = 0u16;
    for _ in 0..MAX_REROLL {
        r1 = swap(rng()) as u16;
        if r1 < b {
            break;
        }
    }
    if r1 < c {
        return DisobedienceOutcome::Obey;
    }

    // .loop2: draws until < b; < c → use a random other move.
    let mut r2 = 0u16;
    for _ in 0..MAX_REROLL {
        r2 = rng() as u16;
        if r2 < b {
            break;
        }
    }
    if r2 < c {
        return use_random_move(selected_slot, moves, pp, has_disabled_move, rng);
    }

    // .monNaps / .monDoesNothing / won't-obey split on diff = level − threshold.
    let diff = d - c;
    let r = swap(rng()) as u16;
    if r < diff {
        // .monNaps: `call BattleRandom; add a; swap a; and SLP_MASK`, reroll on 0.
        let mut turns = 1u8;
        for _ in 0..MAX_REROLL {
            let draw = rng();
            let t = swap(draw.wrapping_add(draw)) & 0x07;
            if t != 0 {
                turns = t;
                break;
            }
        }
        return DisobedienceOutcome::Nap(turns);
    }
    if r - diff >= diff {
        return does_nothing(rng);
    }
    DisobedienceOutcome::WontObeySelfHit
}

/// Safety cap for the asm's unbounded rejection-sampling loops (see above).
const MAX_REROLL: u32 = 4096;

/// `.useRandomMove` (core.asm:3941-4000): pick another known move with PP.
/// Guards fall back to `.monDoesNothing`.
fn use_random_move(
    selected_slot: usize,
    moves: &[MoveId; 4],
    pp: &[u8; 4],
    has_disabled_move: bool,
    rng: &mut dyn FnMut() -> u8,
) -> DisobedienceOutcome {
    let known: Vec<usize> = (0..4).filter(|&i| moves[i] != MoveId::None).collect();
    // "is the second move slot empty?" — a one-move mon never swaps.
    if known.len() < 2 {
        return does_nothing(rng);
    }
    if has_disabled_move {
        return does_nothing(rng);
    }
    if moves[selected_slot] == MoveId::Struggle {
        return does_nothing(rng);
    }
    // "mon will not use move if only one move has remaining PP": total PP over
    // all slots (PP_MASK — PP-Up bits excluded) == the selected slot's PP.
    let total_pp: u16 = pp.iter().map(|p| (p & 0x3F) as u16).sum();
    let selected_pp = (pp[selected_slot] & 0x3F) as u16;
    if total_pp == selected_pp {
        return does_nothing(rng);
    }
    // .chooseMove: wMaxMenuItem = wNumMovesMinusOne + 2 (the fight-menu init,
    // core.asm:2550-2552), so the count check admits every known slot.
    let max_menu_item = known.len() + 1;
    for _ in 0..MAX_REROLL {
        let slot = (rng() & 3) as usize;
        if slot >= max_menu_item || slot == selected_slot {
            continue;
        }
        if slot >= 4 || pp[slot] & 0x3F == 0 {
            continue;
        }
        return DisobedienceOutcome::UseRandomMove(slot);
    }
    // Degenerate rng safety net (unreachable with a real rng): the original
    // would spin in .chooseMove forever; we give up and do nothing.
    does_nothing(rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::map_flags::{
        BIT_CASCADEBADGE, BIT_EARTHBADGE, BIT_MARSHBADGE, BIT_RAINBOWBADGE,
    };

    const MOVES2: [MoveId; 4] = [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None];
    const MOVES4: [MoveId; 4] = [
        MoveId::Tackle,
        MoveId::Growl,
        MoveId::ThunderWave,
        MoveId::QuickAttack,
    ];

    fn scripted(bytes: Vec<u8>) -> impl FnMut() -> u8 {
        let mut it = bytes.into_iter();
        move || it.next().unwrap_or(0)
    }

    #[test]
    fn thresholds_per_badge_tier() {
        assert_eq!(obedience_threshold(0), 10);
        assert_eq!(obedience_threshold(1 << BIT_CASCADEBADGE), 30);
        assert_eq!(obedience_threshold(1 << BIT_RAINBOWBADGE), 50);
        assert_eq!(obedience_threshold(1 << BIT_MARSHBADGE), 70);
        assert_eq!(obedience_threshold(1 << BIT_EARTHBADGE), 101);
        // Higher badges subsume lower ones regardless of combination.
        assert_eq!(
            obedience_threshold((1 << BIT_CASCADEBADGE) | (1 << BIT_MARSHBADGE)),
            70
        );
        assert_eq!(obedience_threshold(0xFF), 101);
    }

    #[test]
    fn traded_detection() {
        assert!(!is_traded_for(0, 1234), "ot_id 0 = unknown → own (legacy saves)");
        assert!(!is_traded_for(1234, 1234), "matching IDs = own");
        assert!(is_traded_for(9999, 1234), "mismatched IDs = traded");
    }

    #[test]
    fn obeys_at_or_below_threshold_no_rng() {
        // Level == threshold always obeys (and would draw nothing in the asm).
        let mut rng = scripted(vec![]);
        let o = check_disobedience(10, 0, 0, &MOVES2, &[35, 35, 0, 0], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::Obey);
        let mut rng = scripted(vec![]);
        let o = check_disobedience(101, 0xFF, 0, &MOVES2, &[35, 35, 0, 0], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::Obey, "EarthBadge: always obey");
    }

    #[test]
    fn loop1_low_byte_obeys() {
        // L20, no badges: c=10, d=20, b=30. loop1 draw 0x05 (swap→0x50=80)? —
        // use 0x50: swap(0x50)=0x05=5 < 30, and 5 < 10 → obey.
        let mut rng = scripted(vec![0x50]);
        let o = check_disobedience(20, 0, 0, &MOVES2, &[35, 35, 0, 0], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::Obey);
    }

    #[test]
    fn loop1_rerolls_until_below_b() {
        // b=30: swap(0xFF)=0xFF and swap(0xE1)=0x1E=30 are both >= 30 → reroll;
        // swap(0x50)=5 < 30 and < c=10 → obey.
        let mut rng = scripted(vec![0xFF, 0xE1, 0x50]);
        let o = check_disobedience(20, 0, 0, &MOVES2, &[35, 35, 0, 0], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::Obey);
    }

    #[test]
    fn loop2_low_byte_uses_random_move() {
        // L20 no badges (c=10, b=30): loop1 swap(0xC1)=0x1C=28 → <30, >=10.
        // loop2 draw 0x05 <30 and <10 → random move. Pick: 0x01 & 3 = 1 (< 3,
        // != selected 0, pp ok) → slot 1.
        let mut rng = scripted(vec![0xC1, 0x05, 0x01]);
        let o = check_disobedience(20, 0, 0, &MOVES2, &[35, 35, 0, 0], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::UseRandomMove(1));
    }

    #[test]
    fn random_move_rerolls_selected_and_empty_slots() {
        // Same entry as above; the pick loop sees selected slot 0, then an
        // out-of-range 3 (only 2 moves, max_menu_item=3 → 3 >= 3 reroll),
        // then empty slot 2 (pp 0), before landing on slot 1.
        let mut rng = scripted(vec![0xC1, 0x05, 0x00, 0x03, 0x02, 0x01]);
        let o = check_disobedience(20, 0, 0, &MOVES2, &[35, 35, 0, 0], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::UseRandomMove(1));
    }

    #[test]
    fn random_move_guards_fall_back_to_nothing() {
        let entry = || vec![0xC1, 0x05];
        // One-move mon → does nothing (0x00 & 3 → loafing).
        let one_move = [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None];
        let mut rng = scripted([entry(), vec![0x00]].concat());
        assert_eq!(
            check_disobedience(20, 0, 0, &one_move, &[35, 0, 0, 0], false, &mut rng),
            DisobedienceOutcome::LoafAround
        );
        // Disabled move → does nothing (0x01 → won't obey).
        let mut rng = scripted([entry(), vec![0x01]].concat());
        assert_eq!(
            check_disobedience(20, 0, 0, &MOVES2, &[35, 35, 0, 0], true, &mut rng),
            DisobedienceOutcome::WontObey
        );
        // Selected Struggle → does nothing (0x02 → turned away).
        let struggle = [MoveId::Struggle, MoveId::Growl, MoveId::None, MoveId::None];
        let mut rng = scripted([entry(), vec![0x02]].concat());
        assert_eq!(
            check_disobedience(20, 0, 0, &struggle, &[35, 35, 0, 0], false, &mut rng),
            DisobedienceOutcome::TurnedAway
        );
        // Only the selected move has PP → does nothing (0x03 → ignored orders).
        let mut rng = scripted([entry(), vec![0x03]].concat());
        assert_eq!(
            check_disobedience(20, 0, 0, &MOVES2, &[35, 0, 0, 0], false, &mut rng),
            DisobedienceOutcome::IgnoredOrders
        );
    }

    #[test]
    fn nap_outcome_sets_sleep_turns() {
        // L20 no badges (c=10, b=30, diff=10): loop1 swap(0xC1)=0x1C=28 (>= c);
        // loop2 raw 0x1C=28 (>= c) → branch roll. Draw 0x90 → swap = 0x09 = 9
        // < diff → nap. Nap turns: 2×0x40=0x80 → swap=0x08 & 7 = 0 → reroll;
        // 2×0x02=0x04 → swap=0x40 & 7 = 0 → reroll; 2×0x19=0x32 → swap=0x23 &
        // 7 = 3 → 3 turns.
        let mut rng = scripted(vec![0xC1, 0x1C, 0x90, 0x40, 0x02, 0x19]);
        let o = check_disobedience(20, 0, 0, &MOVES4, &[35, 35, 35, 35], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::Nap(3));
    }

    #[test]
    fn loaf_family_outcomes() {
        // r − diff >= diff → does nothing. diff=10: need swap(r) >= 20.
        // swap(0x02)=0x20=32 ≥ 20 → does nothing; the &3 draw picks the line.
        for (byte, want) in [
            (0x00u8, DisobedienceOutcome::LoafAround),
            (0x01, DisobedienceOutcome::WontObey),
            (0x02, DisobedienceOutcome::TurnedAway),
            (0x03, DisobedienceOutcome::IgnoredOrders),
        ] {
            let mut rng = scripted(vec![0xC1, 0x1C, 0x02, byte]);
            let o = check_disobedience(20, 0, 0, &MOVES4, &[35, 35, 35, 35], false, &mut rng);
            assert_eq!(o, want, "flavour byte {byte}");
        }
    }

    #[test]
    fn wont_obey_self_hit_outcome() {
        // diff=10: need 10 <= swap(r) < 20. 0x01 → swap = 0x10 = 16: 16 >= 10
        // (not nap), 16−10=6 < 10 (not nothing) → "won't obey!" + self-hit.
        let mut rng = scripted(vec![0xC1, 0x1C, 0x01]);
        let o = check_disobedience(20, 0, 0, &MOVES4, &[35, 35, 35, 35], false, &mut rng);
        assert_eq!(o, DisobedienceOutcome::WontObeySelfHit);
    }

    #[test]
    fn high_level_traded_mon_almost_always_disobeys() {
        // L100, no badges (c=10, b=110): obey ONLY when a nibble-swapped loop1
        // draw lands < c=10 — ~10/256 ≈ 4% of the time. Drive 1000 independent
        // rolls from a deterministic LCG.
        let mut state = 0x1234_5678u32;
        let mut rng = move || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (state >> 16) as u8
        };
        let mut obey = 0;
        for _ in 0..1000 {
            if check_disobedience(100, 0, 0, &MOVES4, &[35, 35, 35, 35], false, &mut rng)
                == DisobedienceOutcome::Obey
            {
                obey += 1;
            }
        }
        assert!(obey > 0, "some rolls still obey (got 0/1000)");
        assert!(obey < 150, "a L100 mon disobeys ~96% of the time, got {obey}/1000 obeys");
    }
}
