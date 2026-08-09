//! Slot machine mini-game logic.
//!
//! Faithfully reimplements the Game Corner slot machine from
//! `engine/slots/slot_machine.asm`.

#[cfg(test)]
mod tests;

use pokered_data::slot_machine::*;

/// The three wheels available in the slot machine.
pub const WHEELS: [&[SlotSymbol; WHEEL_SIZE]; 3] = [
    &SLOT_MACHINE_WHEEL1,
    &SLOT_MACHINE_WHEEL2,
    &SLOT_MACHINE_WHEEL3,
];

/// Represents the visible symbols on a single wheel (bottom, middle, top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelView {
    pub bottom: SlotSymbol,
    pub middle: SlotSymbol,
    pub top: SlotSymbol,
}

/// Full state of the slot machine.
#[derive(Debug, Clone)]
pub struct SlotMachineState {
    pub bet: u8,
    pub wheel_offsets: [u8; 3],
    pub wheel_slip_counters: [u8; 3],
    pub stopping_wheel: u8,
    pub flags: u8,
    pub allow_matches_counter: u8,
    pub reroll_counter: u8,
    pub payout_coins: u16,
    pub seven_and_bar_mode_chance: u8,
}

impl SlotMachineState {
    /// Create a new slot machine. `lucky` = true for the lucky machine
    /// (seven_and_bar_mode_chance = 250 vs 253).
    pub fn new(lucky: bool) -> Self {
        Self {
            bet: 0,
            wheel_offsets: [INITIAL_WHEEL_OFFSET; 3],
            wheel_slip_counters: [INITIAL_SLIP_COUNTER; 3],
            stopping_wheel: 0,
            flags: 0,
            allow_matches_counter: 0,
            reroll_counter: 0,
            payout_coins: 0,
            seven_and_bar_mode_chance: if lucky {
                SEVEN_AND_BAR_MODE_LUCKY
            } else {
                SEVEN_AND_BAR_MODE_NORMAL
            },
        }
    }

    /// Place a bet (1-3 coins). Returns false if bet is invalid.
    pub fn place_bet(&mut self, bet: u8) -> bool {
        if bet >= 1 && bet <= 3 {
            self.bet = bet;
            self.payout_coins = 0;
            self.reroll_counter = 0;
            self.wheel_slip_counters = [INITIAL_SLIP_COUNTER; 3];
            self.stopping_wheel = 0;
            true
        } else {
            false
        }
    }

    /// Set flags for this spin based on RNG.
    /// `random_byte` is a random u8 (0-255).
    ///
    /// Matches `SlotMachine_SetFlags` from the ASM:
    /// - If BIT_SLOTS_CAN_WIN_WITH_7_OR_BAR already set, keep it
    /// - If allow_matches_counter > 0, set CAN_WIN
    /// - random == 0 → set allow_matches_counter = 60 (1/256 chance)
    /// - random >= seven_and_bar_mode_chance → set CAN_WIN_WITH_7_OR_BAR
    /// - random >= 210 → set CAN_WIN (~21.5% chance)
    /// - otherwise → clear flags
    pub fn set_flags(&mut self, random_byte: u8) {
        // If 7/bar mode already active, keep it
        if self.flags & SLOTS_CAN_WIN_WITH_7_OR_BAR != 0 {
            return;
        }

        // If allow_matches_counter is active, always allow matches
        if self.allow_matches_counter > 0 {
            self.flags |= SLOTS_CAN_WIN;
            return;
        }

        // 1/256 chance to start allow_matches_counter
        if random_byte == 0 {
            self.allow_matches_counter = ALLOW_MATCHES_DURATION;
            return;
        }

        // Check seven_and_bar_mode_chance:
        // ASM: `ld a, [wSlotMachineSevenAndBarModeChance]; cp b; jr c, .allowSevenAndBarMatches`
        // cp b sets carry if a < b, i.e. chance < random_byte
        if self.seven_and_bar_mode_chance < random_byte {
            self.flags |= SLOTS_CAN_WIN_WITH_7_OR_BAR;
            return;
        }

        // ~21.5% chance: ASM `ld a, 210; cp b; jr c, .allowMatches`
        // carry if 210 < random_byte
        if 210u8 < random_byte {
            self.flags |= SLOTS_CAN_WIN;
            return;
        }

        // No win
        self.flags = 0;
    }

    /// Advance a wheel's offset by 1 (animation tick). Wraps at WHEEL_OFFSET_MAX (30).
    pub fn advance_wheel(&mut self, wheel_idx: usize) {
        if wheel_idx < 3 {
            self.wheel_offsets[wheel_idx] += 1;
            if self.wheel_offsets[wheel_idx] >= WHEEL_OFFSET_MAX {
                self.wheel_offsets[wheel_idx] = 0;
            }
        }
    }

    /// Get the 3 visible symbols for a wheel.
    ///
    /// The ASM `SlotMachine_GetWheelTiles` reads high bytes at
    /// offset, offset+2, offset+4 from the wheel data. Since each
    /// symbol is a `dw` (2 bytes), this reads 3 consecutive symbols
    /// starting at `offset / 2` (with wrapping).
    ///
    /// Returns [bottom, middle, top] as a `WheelView`.
    pub fn get_wheel_view(&self, wheel_idx: usize) -> WheelView {
        let wheel = WHEELS[wheel_idx];
        let offset = self.wheel_offsets[wheel_idx] as usize;
        // In ASM, offset is a byte offset into dw array. Each symbol = 2 bytes.
        // GetWheelTiles does: add hl,bc (base+offset); then reads [hli],[skip],[hli],[skip],[hli]
        // So it reads bytes at offset, offset+2, offset+4 → symbols at offset/2, offset/2+1, offset/2+2
        // But offset increments by 1 each frame (AnimWheel), wrapping at 30.
        // When offset is even, symbols are aligned. Odd = between symbols.
        //
        // For match checking, wheels are only stopped on odd offsets (after the inc
        // in AnimWheel, rra checks bit 0). So at stop time, offset is odd.
        // GetWheelTiles adds offset to base pointer, reads high bytes at +0, +2, +4.
        // With odd offset into a dw array, byte 0 is the high byte of symbol offset/2,
        // byte +2 is high byte of symbol offset/2+1, byte +4 is high byte of offset/2+2.
        //
        // We model this as: symbol_index = offset / 2, reading 3 consecutive symbols.
        let sym_idx = offset / 2;
        WheelView {
            bottom: wheel[sym_idx % WHEEL_SIZE],
            middle: wheel[(sym_idx + 1) % WHEEL_SIZE],
            top: wheel[(sym_idx + 2) % WHEEL_SIZE],
        }
    }

    /// Check if a wheel can stop (symbol must be centered = odd offset after inc).
    /// In ASM: after `inc` in AnimWheel, `rra; jr nc` means stop only when bit 0
    /// is set (odd offset).
    pub fn can_wheel_stop(&self, wheel_idx: usize) -> bool {
        self.wheel_offsets[wheel_idx] & 1 != 0
    }

    /// Try to stop a wheel. Returns true if the wheel was stopped.
    /// Wheel 3 stops immediately when aligned. Wheels 1/2 use slip counters
    /// and early-stop logic.
    pub fn try_stop_wheel(&mut self, wheel_idx: usize) -> bool {
        if !self.can_wheel_stop(wheel_idx) {
            return false;
        }
        match wheel_idx {
            0 => self.try_stop_wheel1(),
            1 => self.try_stop_wheel2(),
            2 => true, // wheel 3 stops immediately
            _ => false,
        }
    }

    /// Wheel 1 early stop logic (SlotMachine_StopWheel1Early).
    fn try_stop_wheel1(&mut self) -> bool {
        if self.wheel_slip_counters[0] == 0 {
            return true;
        }
        self.wheel_slip_counters[0] -= 1;

        let view = self.get_wheel_view(0);
        if self.flags & SLOTS_CAN_WIN_WITH_7_OR_BAR != 0 {
            // BUG: In sevenAndBarMode, `cp HIGH(SLOTS7); jr c` is never true
            // because Seven has the smallest high byte (0x02). No symbol has
            // high_byte < 0x02, so the carry is never set.
            // This means the wheel never stops early in 7/bar mode.
            let dominated = [view.bottom, view.middle, view.top]
                .iter()
                .any(|s| s.high_byte() < SlotSymbol::Seven.high_byte());
            if dominated {
                self.wheel_slip_counters[0] = 0;
                return true;
            }
            false
        } else {
            // Stop early if middle symbol is NOT cherry
            if view.middle != SlotSymbol::Cherry {
                self.wheel_slip_counters[0] = 0;
                return true;
            }
            false
        }
    }

    /// Wheel 2 early stop logic (SlotMachine_StopWheel2Early).
    fn try_stop_wheel2(&mut self) -> bool {
        if self.wheel_slip_counters[1] == 0 {
            return true;
        }
        self.wheel_slip_counters[1] -= 1;

        let _view2 = self.get_wheel_view(1);
        if self.flags & SLOTS_CAN_WIN_WITH_7_OR_BAR != 0 {
            // In 7/bar mode: stop if wheels 1&2 match OR if bottom of wheel2
            // is 7 or bar (high_byte <= HIGH(SLOTSBAR))
            let has_match = self.find_wheel1_wheel2_match();
            let view2 = self.get_wheel_view(1);
            if has_match || view2.bottom.high_byte() <= SlotSymbol::Bar.high_byte() {
                self.wheel_slip_counters[1] = 0;
                return true;
            }
            false
        } else {
            // Normal mode: stop if NO symbols line up between wheels 1 and 2
            if !self.find_wheel1_wheel2_match() {
                self.wheel_slip_counters[1] = 0;
                return true;
            }
            false
        }
    }

    /// Check if any line between wheel 1 and wheel 2 could form a match.
    /// Returns true if any potential match exists.
    ///
    /// Checks all pairs that could complete a 3-line match:
    /// (w1_bottom, w2_bottom), (w1_bottom, w2_middle),
    /// (w1_middle, w2_middle), (w1_top, w2_middle),
    /// (w1_top, w2_top)
    fn find_wheel1_wheel2_match(&self) -> bool {
        let v1 = self.get_wheel_view(0);
        let v2 = self.get_wheel_view(1);
        v1.bottom == v2.bottom
            || v1.bottom == v2.middle
            || v1.middle == v2.middle
            || v1.top == v2.middle
            || v1.top == v2.top
    }

    /// Check all paylines for matches based on bet amount.
    /// Returns the matching symbol if found, or None.
    ///
    /// Matches `SlotMachine_CheckForMatches`:
    /// - 1 coin: middle row only
    /// - 2 coins: + top row + bottom row
    /// - 3 coins: + two diagonals
    pub fn check_for_matches(&self) -> Option<SlotSymbol> {
        let v1 = self.get_wheel_view(0);
        let v2 = self.get_wheel_view(1);
        let v3 = self.get_wheel_view(2);

        // 3-coin diagonals (checked first in ASM)
        if self.bet >= 3 {
            // bottom-middle-top diagonal
            if v1.bottom == v2.middle && v2.middle == v3.top {
                return Some(v1.bottom);
            }
            // top-middle-bottom diagonal
            if v1.top == v2.middle && v2.middle == v3.bottom {
                return Some(v1.top);
            }
        }

        // 2-coin top/bottom rows
        if self.bet >= 2 {
            if v1.top == v2.top && v2.top == v3.top {
                return Some(v1.top);
            }
            if v1.bottom == v2.bottom && v2.bottom == v3.bottom {
                return Some(v1.bottom);
            }
        }

        // 1-coin middle row (always checked)
        if v1.middle == v2.middle && v2.middle == v3.middle {
            return Some(v1.middle);
        }

        None
    }

    /// Returns true if the symbol is Seven or Bar.
    pub fn is_seven_or_bar(symbol: SlotSymbol) -> bool {
        symbol.high_byte() <= SlotSymbol::Bar.high_byte()
    }

    /// Perform the full match-check-and-reroll cycle.
    ///
    /// Matches `SlotMachine_CheckForMatches` from the ASM:
    /// 1. Check for matches on active paylines
    /// 2. If match found but player can't win (no flags), reroll wheel 3
    /// 3. If match found and can win but it's 7/bar and 7/bar mode isn't on, reroll
    /// 4. Otherwise accept the match and calculate payout
    ///
    /// Returns the winning symbol and payout, or None.
    pub fn resolve_spin(&mut self) -> Option<(SlotSymbol, u16)> {
        let max_rerolls = WHEEL_SIZE as u8 * 2;
        self.reroll_counter = max_rerolls;

        loop {
            if let Some(symbol) = self.check_for_matches() {
                let can_win = self.flags & (SLOTS_CAN_WIN | SLOTS_CAN_WIN_WITH_7_OR_BAR) != 0;
                if !can_win {
                    // Player can't win — try reroll
                    if self.reroll_counter == 0 {
                        return None;
                    }
                    self.reroll_counter -= 1;
                    self.advance_wheel(2);
                    self.advance_wheel(2);
                    continue;
                }
                // Can win, but check 7/bar restriction
                let has_7bar_mode = self.flags & SLOTS_CAN_WIN_WITH_7_OR_BAR != 0;
                if !has_7bar_mode && Self::is_seven_or_bar(symbol) {
                    // 7/bar match but not in 7/bar mode — reroll
                    if self.reroll_counter == 0 {
                        return None;
                    }
                    self.reroll_counter -= 1;
                    self.advance_wheel(2);
                    self.advance_wheel(2);
                    continue;
                }
                // Accept match
                let payout = self.calculate_payout(symbol);
                self.payout_coins = payout;
                self.post_reward_effects(symbol);
                return Some((symbol, payout));
            } else {
                // No match found — check if we should reroll
                let can_win = self.flags & (SLOTS_CAN_WIN | SLOTS_CAN_WIN_WITH_7_OR_BAR) != 0;
                if !can_win || self.reroll_counter == 0 {
                    return None;
                }
                self.reroll_counter -= 1;
                self.advance_wheel(2);
                self.advance_wheel(2);
            }
        }
    }

    /// Calculate payout for a winning symbol.
    pub fn calculate_payout(&self, symbol: SlotSymbol) -> u16 {
        reward_for_symbol(symbol).payout
    }

    /// Apply post-reward effects matching the ASM reward functions.
    ///
    /// - Cherry/Fish/Bird/Mouse: decrement allow_matches_counter
    /// - Bar: clear all flags
    /// - Seven: 50% chance to clear flags, always clear allow_matches_counter
    fn post_reward_effects(&mut self, symbol: SlotSymbol) {
        match symbol {
            SlotSymbol::Cherry | SlotSymbol::Fish | SlotSymbol::Bird | SlotSymbol::Mouse => {
                if self.allow_matches_counter > 0 {
                    self.allow_matches_counter -= 1;
                }
            }
            SlotSymbol::Bar => {
                self.flags = 0;
            }
            SlotSymbol::Seven => {
                // In ASM: `Random; cp $80; jr c, .skip; ld [wSlotMachineFlags], a`
                // 50% chance to clear flags. We model this deterministically here;
                // caller should handle RNG. For now, always clear on Seven.
                self.flags = 0;
                self.allow_matches_counter = 0;
            }
        }
    }

    /// Post-reward effects with explicit RNG for Seven (50% chance to keep flags).
    pub fn post_reward_effects_with_rng(&mut self, symbol: SlotSymbol, random_byte: u8) {
        match symbol {
            SlotSymbol::Cherry | SlotSymbol::Fish | SlotSymbol::Bird | SlotSymbol::Mouse => {
                if self.allow_matches_counter > 0 {
                    self.allow_matches_counter -= 1;
                }
            }
            SlotSymbol::Bar => {
                self.flags = 0;
            }
            SlotSymbol::Seven => {
                // ASM: `cp $80; jr c, .skip` → if random < 0x80, skip clearing flags
                if random_byte >= 0x80 {
                    self.flags = 0;
                }
                self.allow_matches_counter = 0;
            }
        }
    }

    /// Get the flash count for a winning symbol (for visual feedback).
    pub fn flash_count_for_symbol(symbol: SlotSymbol) -> u8 {
        reward_for_symbol(symbol).flash_count
    }
}
