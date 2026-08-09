//! Game Corner slot-machine minigame screen.
//!
//! A pure-logic state machine that wraps [`crate::slots::SlotMachineState`]
//! (the faithful reel/flag/payout engine) and drives it through a playable
//! loop: choose a bet (1-3 coins), spin, stop each reel with the A button,
//! resolve the payout, and credit/debit the player's coin balance.
//!
//! Like the other screen state machines (`options_menu`, `party_screen`),
//! this crate is deterministic and I/O-free: rendering lives in the app
//! layer and the coin balance is owned by the caller (persisted to
//! `game_data.player_coins`).

use crate::slots::SlotMachineState;
use pokered_data::slot_machine::{SlotSymbol, WHEEL_OFFSET_MAX};

/// Hard cap on the coin balance, matching the original's 4-digit BCD field.
pub const MAX_COINS: u16 = 9999;

/// Per-frame input for the slots screen (edge-triggered by the caller).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SlotsInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
}

impl SlotsInput {
    pub fn none() -> Self {
        Self::default()
    }
}

/// Result of a single frame update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotsAction {
    /// Stay on the slots screen.
    Continue,
    /// Leave the slots screen; the caller should return to the overworld and
    /// persist [`SlotsScreen::coins`] back to the save.
    Exit,
}

/// High-level phase of the minigame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotsPhase {
    /// Choosing a bet; waiting for the player to insert coins and spin.
    BetSelect,
    /// The reels are spinning; the player stops them one at a time.
    Spinning,
    /// A spin resolved; showing the win/lose message. A/B returns to bet
    /// selection (or exits if the player is out of coins).
    Result,
}

/// Full slot-machine screen state.
#[derive(Debug, Clone)]
pub struct SlotsScreen {
    pub machine: SlotMachineState,
    /// Live coin balance (mirrors the save's `player_coins` while playing).
    pub coins: u16,
    pub phase: SlotsPhase,
    /// Current bet (1-3), chosen during [`SlotsPhase::BetSelect`].
    pub bet: u8,
    /// Which reels have stopped (index 0 = left).
    pub reels_stopped: [bool; 3],
    /// Index of the reel that the next A-press will stop.
    pub current_reel: usize,
    /// A stop was requested for `current_reel`; retried each frame until the
    /// reel is aligned and the slip logic allows it.
    pub stop_pending: bool,
    /// Payout of the most recent resolved spin (0 on a loss).
    pub last_payout: u16,
    /// Winning symbol of the most recent spin, if any.
    pub last_symbol: Option<SlotSymbol>,
    /// Human-readable status line for rendering.
    pub message: String,
    /// Frame counter (drives reel animation cadence).
    pub frame: u64,
    /// Small deterministic RNG so tests are reproducible.
    rng: u32,
}

impl SlotsScreen {
    /// Create a fresh slots screen. `lucky` selects the higher-odds machine,
    /// `coins` is the player's current balance, `seed` seeds the RNG.
    pub fn new(lucky: bool, coins: u16, seed: u32) -> Self {
        Self {
            machine: SlotMachineState::new(lucky),
            coins: coins.min(MAX_COINS),
            phase: SlotsPhase::BetSelect,
            bet: 1,
            reels_stopped: [false; 3],
            current_reel: 0,
            stop_pending: false,
            last_payout: 0,
            last_symbol: None,
            message: String::from("BET 1-3 COINS"),
            frame: 0,
            rng: seed | 1,
        }
    }

    fn next_rng(&mut self) -> u8 {
        // Classic 32-bit LCG (glibc constants); take a high byte for spread.
        self.rng = self.rng.wrapping_mul(1103515245).wrapping_add(12345);
        (self.rng >> 16) as u8
    }

    /// Add coins, respecting the 9999 cap.
    fn credit(&mut self, amount: u16) {
        self.coins = self.coins.saturating_add(amount).min(MAX_COINS);
    }

    /// Advance one frame.
    pub fn update_frame(&mut self, input: SlotsInput) -> SlotsAction {
        self.frame = self.frame.wrapping_add(1);
        match self.phase {
            SlotsPhase::BetSelect => self.update_bet_select(input),
            SlotsPhase::Spinning => self.update_spinning(input),
            SlotsPhase::Result => self.update_result(input),
        }
    }

    fn update_bet_select(&mut self, input: SlotsInput) -> SlotsAction {
        if input.b {
            return SlotsAction::Exit;
        }
        // Raising/lowering the bet with the d-pad.
        if input.up || input.right {
            self.bet = (self.bet + 1).min(3);
        }
        if input.down || input.left {
            self.bet = self.bet.saturating_sub(1).max(1);
        }
        // Never let the bet exceed the coins on hand.
        if self.coins == 0 {
            self.message = String::from("OUT OF COINS!");
            if input.a {
                return SlotsAction::Exit;
            }
            return SlotsAction::Continue;
        }
        self.bet = self.bet.clamp(1, 3).min(self.coins as u8).max(1);

        if input.a {
            // Deduct the bet up front and start the spin.
            let bet = self.bet.min(self.coins.min(3) as u8).max(1);
            self.bet = bet;
            self.coins = self.coins.saturating_sub(bet as u16);
            self.machine.place_bet(bet);
            let flag_byte = self.next_rng();
            self.machine.set_flags(flag_byte);
            self.reels_stopped = [false; 3];
            self.current_reel = 0;
            self.stop_pending = false;
            self.last_payout = 0;
            self.last_symbol = None;
            self.phase = SlotsPhase::Spinning;
            self.message = String::from("STOP THE REELS!");
        } else {
            self.message = format!("BET {} COIN{}", self.bet, if self.bet == 1 { "" } else { "S" });
        }
        SlotsAction::Continue
    }

    fn update_spinning(&mut self, input: SlotsInput) -> SlotsAction {
        // Animate every reel that is still moving.
        for idx in 0..3 {
            if !self.reels_stopped[idx] {
                self.machine.advance_wheel(idx);
            }
        }

        // Pressing A requests a stop for the current reel.
        if input.a && self.current_reel < 3 {
            self.stop_pending = true;
        }

        if self.stop_pending && self.current_reel < 3 {
            let idx = self.current_reel;
            if self.machine.try_stop_wheel(idx) {
                self.reels_stopped[idx] = true;
                self.current_reel += 1;
                self.stop_pending = false;
            }
        }

        if self.reels_stopped.iter().all(|&s| s) {
            self.resolve();
        }
        SlotsAction::Continue
    }

    fn resolve(&mut self) {
        let outcome = self.machine.resolve_spin();
        if let Some((symbol, payout)) = outcome {
            // Give the winning symbol its post-reward RNG side effects.
            let rng = self.next_rng();
            self.machine.post_reward_effects_with_rng(symbol, rng);
            self.last_symbol = Some(symbol);
            self.last_payout = payout;
            self.credit(payout);
            self.message = format!("WIN! {} COINS", payout);
        } else {
            self.last_symbol = None;
            self.last_payout = 0;
            self.message = String::from("NO MATCH...");
        }
        self.phase = SlotsPhase::Result;
    }

    fn update_result(&mut self, input: SlotsInput) -> SlotsAction {
        if input.a || input.b {
            if self.coins == 0 {
                return SlotsAction::Exit;
            }
            self.phase = SlotsPhase::BetSelect;
            self.message = format!("BET {} COIN{}", self.bet, if self.bet == 1 { "" } else { "S" });
        }
        SlotsAction::Continue
    }

    /// Reel animation progress 0.0..1.0 for a moving reel (for rendering).
    pub fn reel_progress(&self, idx: usize) -> f32 {
        if idx >= 3 {
            return 0.0;
        }
        self.machine.wheel_offsets[idx] as f32 / WHEEL_OFFSET_MAX as f32
    }
}

/// Short, fixed-width display label for a slot symbol.
pub fn symbol_label(sym: SlotSymbol) -> &'static str {
    match sym {
        SlotSymbol::Seven => "  7 ",
        SlotSymbol::Bar => "BAR ",
        SlotSymbol::Cherry => "CHER",
        SlotSymbol::Fish => "FISH",
        SlotSymbol::Bird => "BIRD",
        SlotSymbol::Mouse => "MOUS",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press_a() -> SlotsInput {
        SlotsInput { a: true, ..SlotsInput::none() }
    }

    #[test]
    fn bet_selection_is_clamped_1_to_3() {
        let mut s = SlotsScreen::new(false, 100, 1);
        // Raise past the cap.
        for _ in 0..5 {
            s.update_frame(SlotsInput { up: true, ..SlotsInput::none() });
        }
        assert_eq!(s.bet, 3);
        // Lower past the floor.
        for _ in 0..5 {
            s.update_frame(SlotsInput { down: true, ..SlotsInput::none() });
        }
        assert_eq!(s.bet, 1);
    }

    #[test]
    fn pressing_a_deducts_bet_and_starts_spin() {
        let mut s = SlotsScreen::new(false, 50, 7);
        s.update_frame(SlotsInput { up: true, ..SlotsInput::none() }); // bet -> 2
        assert_eq!(s.bet, 2);
        s.update_frame(press_a());
        assert_eq!(s.phase, SlotsPhase::Spinning);
        assert_eq!(s.coins, 48, "bet of 2 should be deducted");
    }

    #[test]
    fn pressing_b_exits_from_bet_select() {
        let mut s = SlotsScreen::new(false, 50, 3);
        assert_eq!(
            s.update_frame(SlotsInput { b: true, ..SlotsInput::none() }),
            SlotsAction::Exit
        );
    }

    #[test]
    fn out_of_coins_exits_on_confirm() {
        let mut s = SlotsScreen::new(false, 0, 3);
        assert_eq!(s.update_frame(press_a()), SlotsAction::Exit);
    }

    #[test]
    fn full_spin_stops_all_reels_and_resolves() {
        let mut s = SlotsScreen::new(false, 100, 42);
        s.update_frame(press_a());
        assert_eq!(s.phase, SlotsPhase::Spinning);
        // Drive frames, pressing A repeatedly to stop each reel. The slip
        // logic may need several aligned frames, so give it plenty.
        for _ in 0..2000 {
            if s.phase != SlotsPhase::Spinning {
                break;
            }
            s.update_frame(press_a());
        }
        assert_eq!(s.phase, SlotsPhase::Result, "all reels should stop and resolve");
        assert!(s.reels_stopped.iter().all(|&x| x));
        // Returning from the result screen goes back to bet selection.
        s.update_frame(press_a());
        assert_eq!(s.phase, SlotsPhase::BetSelect);
    }

    #[test]
    fn credit_respects_9999_cap() {
        let mut s = SlotsScreen::new(false, 9990, 1);
        s.credit(100);
        assert_eq!(s.coins, MAX_COINS);
    }

    #[test]
    fn bet_never_exceeds_coins() {
        let mut s = SlotsScreen::new(false, 2, 9);
        // Try to bet 3 with only 2 coins.
        s.update_frame(SlotsInput { up: true, ..SlotsInput::none() });
        s.update_frame(SlotsInput { up: true, ..SlotsInput::none() });
        s.update_frame(press_a());
        // Bet is capped to the coins on hand, and the deduction can't underflow.
        assert!(s.bet <= 2);
        assert_eq!(s.coins, 2 - s.bet as u16);
    }
}
