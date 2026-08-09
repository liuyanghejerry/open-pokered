//! Safari Zone battle mechanics — Ball / Bait / Rock / Run, faithful to Gen-1.
//!
//! Ported directly from the disassembly:
//! - `engine/items/item_effects.asm` — `ItemUseBait` / `ItemUseRock` / `BaitRockCommon`
//! - `engine/battle/safari_zone.asm` — `PrintSafariZoneBattleText` (per-turn upkeep)
//! - `engine/battle/core.asm` — the Safari flee check (`.notOutOfSafariBalls` …)
//!
//! A Safari encounter replaces the FIGHT menu with BALL/BAIT/ROCK/RUN (no attacking).
//! The wild mon has a live `catch_rate` (init = species base) mutated by bait (halve) /
//! rock (double, restored to base when the anger wears off), plus an "eating" (bait) and
//! "angry" (rock) counter that bias the per-turn flee roll.

use crate::battle::capture::{try_capture, CaptureContext, CaptureResult, CaptureRandoms};
use crate::battle::state::StatusCondition;
use pokered_data::items::ItemId;

/// Per-battle Safari state (Gen-1 `wEnemyMonActualCatchRate` + `wSafariBaitFactor` +
/// `wSafariEscapeFactor` + `wNumSafariBalls`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafariState {
    /// The species base catch rate — restored onto `catch_rate` when anger wears off.
    pub base_catch_rate: u8,
    /// The live catch rate fed to the capture formula (bait halves, rock doubles).
    pub catch_rate: u8,
    /// "Eating" counter — while > 0 the mon is less likely to run (bait).
    pub bait_factor: u8,
    /// "Angry" counter — while > 0 the mon is more likely to run (rock).
    pub escape_factor: u8,
    /// Remaining Safari Balls.
    pub balls: u8,
}

/// Which per-turn upkeep text (Gen-1 `PrintSafariZoneBattleText`) to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafariUpkeep {
    /// The mon is eating (bait active); nothing else happens.
    Eating,
    /// The mon is angry (rock active).
    Angry,
    /// Neither — no upkeep text.
    None,
}

impl SafariState {
    /// Start a Safari battle with the wild mon's base catch rate and the party's ball count.
    pub fn new(base_catch_rate: u8, balls: u8) -> Self {
        Self {
            base_catch_rate,
            catch_rate: base_catch_rate,
            bait_factor: 0,
            escape_factor: 0,
            balls,
        }
    }

    /// Throw **BAIT** (`ItemUseBait`): halve the catch rate, clear anger, and raise the
    /// eating counter by `amount` (1..=5, capped at 255).
    pub fn apply_bait(&mut self, amount: u8) {
        self.catch_rate >>= 1; // srl [hl]
        self.escape_factor = 0; // ld [de], a  (de = escape factor)
        self.bait_factor = self.bait_factor.saturating_add(amount);
    }

    /// Throw a **ROCK** (`ItemUseRock`): double the catch rate (capped 255), clear eating,
    /// and raise the anger counter by `amount` (1..=5, capped at 255).
    pub fn apply_rock(&mut self, amount: u8) {
        self.catch_rate = self.catch_rate.saturating_mul(2); // add a; cap $ff
        self.bait_factor = 0; // ld [de], a  (de = bait factor)
        self.escape_factor = self.escape_factor.saturating_add(amount);
    }

    /// Throw a **SAFARI BALL**: consume one ball and attempt the capture with the current
    /// (bait/rock-modified) catch rate.
    pub fn throw_ball(
        &mut self,
        wild_max_hp: u16,
        wild_current_hp: u16,
        wild_status: StatusCondition,
        randoms: CaptureRandoms,
    ) -> CaptureResult {
        self.balls = self.balls.saturating_sub(1);
        let ctx = CaptureContext {
            ball: ItemId::SafariBall,
            wild_max_hp,
            wild_current_hp,
            wild_catch_rate: self.catch_rate,
            wild_status,
        };
        try_capture(&ctx, &randoms)
    }

    /// Per-turn upkeep (`PrintSafariZoneBattleText`): decrement the active counter (bait
    /// first, then anger), restoring the catch rate to base when the anger wears off.
    /// Returns which status text to show.
    pub fn upkeep(&mut self) -> SafariUpkeep {
        if self.bait_factor != 0 {
            self.bait_factor -= 1;
            SafariUpkeep::Eating
        } else if self.escape_factor != 0 {
            self.escape_factor -= 1;
            if self.escape_factor == 0 {
                // Anger wore off → undo the rock's catch-rate boost (back to base).
                self.catch_rate = self.base_catch_rate;
            }
            SafariUpkeep::Angry
        } else {
            SafariUpkeep::None
        }
    }

    /// The per-turn flee roll (`engine/battle/core.asm`). `enemy_speed` is the wild mon's
    /// speed stat; `random_byte` is a fresh 0..=255 draw. Returns `true` if the mon flees.
    ///
    /// Gen-1: `b = (speed & 0xFF) * 2`. If the low byte of speed exceeds 127 the doubling
    /// overflows a byte and the mon flees outright. Bait (`bait_factor != 0`) divides `b`
    /// by 4 (less likely); anger (`escape_factor != 0`) doubles it (capped 255, more
    /// likely). The mon flees if `random_byte < b`.
    pub fn flee_roll(&self, enemy_speed: u16, random_byte: u8) -> bool {
        let speed_low = (enemy_speed & 0xFF) as u8;
        if speed_low > 127 {
            return true; // `add a` carries → EnemyRan
        }
        let mut b: u16 = (speed_low as u16) * 2; // 0..=254
        if self.bait_factor != 0 {
            b >>= 2;
        }
        if self.escape_factor != 0 {
            b = (b << 1).min(255);
        }
        (random_byte as u16) < b
    }
}

/// A uniform 1..=5 (Gen-1 `BaitRockCommon`'s `.randomLoop`): draw `Random & 7`, reject
/// values >= 5, then `+1`. `next` yields fresh random bytes.
pub fn roll_bait_rock_amount(next: &mut dyn FnMut() -> u8) -> u8 {
    loop {
        let r = next() & 7;
        if r < 5 {
            return r + 1;
        }
    }
}

#[cfg(test)]
#[path = "safari_tests.rs"]
mod safari_tests;
