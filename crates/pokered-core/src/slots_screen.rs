//! Game Corner slot-machine minigame screen.
//!
//! A pure-logic state machine that wraps [`crate::slots::SlotMachineState`]
//! (the faithful reel/flag/payout engine) and drives it through the original's
//! playable loop (`engine/slots/slot_machine.asm`): choose a bet (1-3 coins),
//! mandatory warm-up spin, stop each reel with the A button, then — on a win —
//! flash the screen, wait for a button press, and tick the payout into the
//! coin balance one coin at a time.
//!
//! Like the other screen state machines (`options_menu`, `party_screen`),
//! this crate is deterministic and I/O-free: rendering lives in the app
//! layer and the coin balance is owned by the caller (persisted to
//! `game_data.player_coins`).

use crate::slots::SlotMachineState;
use pokered_data::slot_machine::{SlotSymbol, WHEEL_OFFSET_MAX};

/// Hard cap on the coin balance, matching the original's 4-digit BCD field.
pub const MAX_COINS: u16 = 9999;

/// Mandatory warm-up before stop input is accepted: the ASM spins all three
/// wheels for 20 iterations of a 2-frame delay (`SlotMachine_SpinWheels`
/// .loop1, slot_machine.asm:206-217).
const SPIN_WARMUP_FRAMES: u16 = 40;

/// Frames between payout coin ticks for small wins; halved for 7/bar wins
/// (`SlotMachine_PayCoinsToPlayer`, slot_machine.asm:705-711).
const PAYOUT_TICK_FRAMES: u16 = 8;

/// Each screen flash in the win fanfare lasts 5 frames (flashScreenLoop,
/// slot_machine.asm:457-464).
const FLASH_FRAME_INTERVAL: u16 = 5;

/// After a spin that leaves the player at 0 coins the machine prints the
/// out-of-coins text and exits by itself after a 60-frame delay
/// (MainSlotMachineLoop .skip2, slot_machine.asm:126-133).
const OUT_OF_COINS_DELAY_FRAMES: u16 = 60;

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

/// Slot-machine sound cues (engine/slots/slot_machine.asm): the spin start
/// (:120), each reel's stop (:842), the per-coin payout tick (:694), and the
/// bar/seven reward stingers (:588, :599). Pure events — the frontends map
/// them to SfxId::SlotsNewSpin / SlotsStopWheel / SlotsReward / GetKeyItem /
/// GetItem2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotsSfx {
    NewSpin,
    StopWheel,
    Reward,
    /// SFX_GET_KEY_ITEM, played when a BAR win is accepted (SlotReward100Func).
    GetKeyItem,
    /// SFX_GET_ITEM_2, played when a 7 win is accepted (SlotReward300Func).
    GetItem2,
}

/// High-level phase of the minigame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotsPhase {
    /// Choosing a bet; waiting for the player to insert coins and spin.
    BetSelect,
    /// The reels are spinning; the player stops them one at a time.
    Spinning,
    /// A win was rolled: flash the screen, wait for a button press, then tick
    /// the payout into the balance one coin at a time (the ASM's
    /// flashScreenLoop → "lined up" text → SlotMachine_PayCoinsToPlayer).
    Payout,
    /// A spin fully resolved; "One more go?" — A returns to bet selection,
    /// B exits. Exits by itself after a short delay when out of coins.
    Result,
}

/// Sub-stage of the [`SlotsPhase::Payout`] fanfare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayoutStage {
    /// Screen flash: `flash_count` blocks of [`FLASH_FRAME_INTERVAL`] frames.
    Flash,
    /// " lined up! Scored N coins!" — waits for A/B (the ASM's
    /// WaitForTextScrollButtonPress).
    WaitPress,
    /// Crediting 1 coin every few frames with a Reward SFX per coin.
    Tick,
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
    /// Remaining warm-up frames of the current spin (input is ignored).
    pub spin_warmup: u16,
    /// Payout of the most recent resolved spin (0 on a loss).
    pub last_payout: u16,
    /// Coins of the last payout not yet ticked into [`Self::coins`].
    pub payout_remaining: u16,
    /// Frames until the next payout coin tick.
    pub payout_cooldown: u16,
    /// Current sub-stage of the payout fanfare.
    pub payout_stage: PayoutStage,
    /// Frames left in the flash stage (0 outside it).
    pub flash_frames_remaining: u16,
    /// Flash polarity for the renderer while `payout_stage` is [`PayoutStage::Flash`].
    pub flash_on: bool,
    /// Frames elapsed in the out-of-coins auto-exit countdown.
    pub out_of_coins_frames: u16,
    /// Winning symbol of the most recent spin, if any.
    pub last_symbol: Option<SlotSymbol>,
    /// Pending sound cues (drained by the frontend each frame).
    pending_sfx: std::collections::VecDeque<SlotsSfx>,
    /// Human-readable status line for rendering.
    pub message: String,
    /// Frame counter (drives reel animation cadence).
    pub frame: u64,
    /// Small deterministic RNG so tests are reproducible.
    rng: u32,
}

/// Full symbol name used in the "lined up" win message.
fn symbol_display_name(sym: SlotSymbol) -> &'static str {
    match sym {
        SlotSymbol::Seven => "7",
        SlotSymbol::Bar => "BAR",
        SlotSymbol::Cherry => "CHERRY",
        SlotSymbol::Fish => "FISH",
        SlotSymbol::Bird => "BIRD",
        SlotSymbol::Mouse => "MOUSE",
    }
}

impl SlotsScreen {
    /// Create a fresh slots screen. `lucky` selects the higher-odds machine,
    /// `coins` is the player's current balance, `seed` seeds the RNG.
    pub fn new(lucky: bool, coins: u16, seed: u32) -> Self {
        // LoadSlotMachineTiles draws each wheel once at $1c, leaving the
        // offset at $1d (the next byte). The first screen is fully aligned.
        let mut machine = SlotMachineState::new(lucky);
        for idx in 0..3 {
            machine.advance_wheel(idx);
        }
        Self {
            machine,
            coins: coins.min(MAX_COINS),
            phase: SlotsPhase::BetSelect,
            pending_sfx: std::collections::VecDeque::new(),
            bet: 1,
            reels_stopped: [false; 3],
            current_reel: 0,
            stop_pending: false,
            spin_warmup: 0,
            last_payout: 0,
            payout_remaining: 0,
            payout_cooldown: 0,
            payout_stage: PayoutStage::Flash,
            flash_frames_remaining: 0,
            flash_on: false,
            out_of_coins_frames: 0,
            last_symbol: None,
            message: String::from("BET HOW MANY COINS?"),
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

    /// Frames between payout coin ticks: 8, halved to 4 for a 7/bar win
    /// (`srl c` in SlotMachine_PayCoinsToPlayer).
    fn payout_tick_interval(&self) -> u16 {
        match self.last_symbol {
            Some(s) if SlotMachineState::is_seven_or_bar(s) => PAYOUT_TICK_FRAMES / 2,
            _ => PAYOUT_TICK_FRAMES,
        }
    }

    /// Advance one frame.
    pub fn update_frame(&mut self, input: SlotsInput) -> SlotsAction {
        self.frame = self.frame.wrapping_add(1);
        match self.phase {
            SlotsPhase::BetSelect => self.update_bet_select(input),
            SlotsPhase::Spinning => self.update_spinning(input),
            SlotsPhase::Payout => self.update_payout(input),
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
        // Entry guard: the original refuses to even start without coins
        // (AbleToPlaySlotsCheck).
        if self.coins == 0 {
            self.message = String::from("OUT OF COINS!");
            if input.a {
                return SlotsAction::Exit;
            }
            return SlotsAction::Continue;
        }
        self.message = String::from("BET HOW MANY COINS?");
        if input.a {
            if self.bet as u16 > self.coins {
                // Stay in the bet menu with the refusal message
                // (NotEnoughCoinsSlotMachineText) — the bet is NOT clamped.
                self.message = String::from("NOT ENOUGH COINS!");
                return SlotsAction::Continue;
            }
            // Deduct the bet up front and start the spin.
            self.coins -= self.bet as u16;
            self.machine.place_bet(self.bet);
            let flag_byte = self.next_rng();
            self.machine.set_flags(flag_byte);
            self.reels_stopped = [false; 3];
            self.current_reel = 0;
            self.stop_pending = false;
            self.last_payout = 0;
            self.last_symbol = None;
            self.spin_warmup = SPIN_WARMUP_FRAMES;
            self.phase = SlotsPhase::Spinning;
            self.message = String::from("START!");
            self.pending_sfx.push_back(SlotsSfx::NewSpin);
        }
        SlotsAction::Continue
    }

    fn update_spinning(&mut self, input: SlotsInput) -> SlotsAction {
        if self.spin_warmup > 0 {
            // Mandatory warm-up: all wheels animate every other frame and
            // stop input is ignored (SlotMachine_SpinWheels .loop1).
            self.spin_warmup -= 1;
            if self.spin_warmup % 2 == 0 {
                for idx in 0..3 {
                    if !self.reels_stopped[idx] {
                        self.machine.advance_wheel(idx);
                    }
                }
            }
            return SlotsAction::Continue;
        }

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
                self.pending_sfx.push_back(SlotsSfx::StopWheel);
            }
        }

        if self.reels_stopped.iter().all(|&s| s) {
            self.resolve();
        }
        SlotsAction::Continue
    }

    fn resolve(&mut self) {
        match self.machine.resolve_spin() {
            Some((symbol, payout)) => {
                // Give the winning symbol its post-reward RNG side effects —
                // exactly once (the ASM reward funcs run them inline).
                let rng = self.next_rng();
                self.machine.post_reward_effects_with_rng(symbol, rng);
                self.last_symbol = Some(symbol);
                self.last_payout = payout;
                self.payout_remaining = payout;
                // Bar/seven play their reward stinger before the flash
                // (SlotReward100Func / SlotReward300Func); a 7 also shows
                // "Yeah!" (YeahText).
                match symbol {
                    SlotSymbol::Bar => self.pending_sfx.push_back(SlotsSfx::GetKeyItem),
                    SlotSymbol::Seven => {
                        self.pending_sfx.push_back(SlotsSfx::GetItem2);
                        self.message = String::from("YEAH!");
                    }
                    _ => {}
                }
                self.payout_stage = PayoutStage::Flash;
                self.flash_frames_remaining =
                    SlotMachineState::flash_count_for_symbol(symbol) as u16 * FLASH_FRAME_INTERVAL;
                self.flash_on = true;
                self.phase = SlotsPhase::Payout;
            }
            None => {
                self.last_symbol = None;
                self.last_payout = 0;
                self.message = String::from("NOT THIS TIME!");
                self.phase = SlotsPhase::Result;
            }
        }
    }

    fn update_payout(&mut self, input: SlotsInput) -> SlotsAction {
        match self.payout_stage {
            PayoutStage::Flash => {
                if self.flash_frames_remaining > 0 {
                    self.flash_frames_remaining -= 1;
                    // Toggle the flash polarity once per 5-frame block (the
                    // ASM XORs rBGP at the top of each flashScreenLoop pass).
                    if self.flash_frames_remaining % FLASH_FRAME_INTERVAL == 0 {
                        self.flash_on = !self.flash_on;
                    }
                }
                if self.flash_frames_remaining == 0 {
                    self.flash_on = false;
                    self.payout_stage = PayoutStage::WaitPress;
                    let sym = self
                        .last_symbol
                        .map(symbol_display_name)
                        .unwrap_or("?");
                    self.message = format!(
                        "{} lined up! Scored {} coins!",
                        sym, self.last_payout
                    );
                }
            }
            PayoutStage::WaitPress => {
                // WaitForTextScrollButtonPress.
                if input.a || input.b {
                    self.payout_stage = PayoutStage::Tick;
                    self.payout_cooldown = 0;
                }
            }
            PayoutStage::Tick => {
                if self.payout_remaining == 0 || self.coins >= MAX_COINS {
                    self.finish_payout();
                    return SlotsAction::Continue;
                }
                if self.payout_cooldown > 0 {
                    self.payout_cooldown -= 1;
                } else {
                    // One coin per tick, each with its own Reward SFX
                    // (SlotMachine_PayCoinsToPlayer .loop).
                    self.payout_remaining -= 1;
                    self.credit(1);
                    self.pending_sfx.push_back(SlotsSfx::Reward);
                    self.payout_cooldown = self.payout_tick_interval() - 1;
                }
            }
        }
        SlotsAction::Continue
    }

    fn finish_payout(&mut self) {
        self.flash_on = false;
        self.payout_remaining = 0;
        self.phase = SlotsPhase::Result;
        if self.coins == 0 {
            self.message = String::from("OUT OF COINS!");
        } else {
            self.message = String::from("ONE MORE GO?");
        }
    }

    fn update_result(&mut self, input: SlotsInput) -> SlotsAction {
        if self.coins == 0 {
            // "Darn! Ran out of coins!" — the machine exits on its own after
            // a 60-frame delay; input is not consulted (MainSlotMachineLoop
            // prints the text then `jp DelayFrames` returns from the loop).
            self.message = String::from("OUT OF COINS!");
            self.out_of_coins_frames += 1;
            if self.out_of_coins_frames >= OUT_OF_COINS_DELAY_FRAMES {
                return SlotsAction::Exit;
            }
            return SlotsAction::Continue;
        }
        self.message = String::from("ONE MORE GO?");
        if input.a {
            self.phase = SlotsPhase::BetSelect;
            self.message = String::from("BET HOW MANY COINS?");
        } else if input.b {
            // "One more go?" → NO: leave the machine.
            return SlotsAction::Exit;
        }
        SlotsAction::Continue
    }

    /// Drain the pending sound cues (frontends play them this frame).
    pub fn take_sfx(&mut self) -> Vec<SlotsSfx> {
        self.pending_sfx.drain(..).collect()
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
    use pokered_data::slot_machine::{SLOTS_CAN_WIN, SLOTS_CAN_WIN_WITH_7_OR_BAR};

    fn press_a() -> SlotsInput {
        SlotsInput { a: true, ..SlotsInput::none() }
    }

    /// Drive a spin from the BetSelect phase all the way to Result by holding
    /// A (warm-up, reel stops, and a possible payout fanfare included).
    fn drive_to_result(s: &mut SlotsScreen) {
        s.update_frame(press_a());
        for _ in 0..20000 {
            if s.phase == SlotsPhase::Result {
                return;
            }
            s.update_frame(press_a());
        }
        panic!("spin never resolved; phase = {:?}", s.phase);
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
        assert_eq!(s.spin_warmup, SPIN_WARMUP_FRAMES);
    }

    #[test]
    fn warmup_ignores_stop_input_and_animates_every_other_frame() {
        let mut s = SlotsScreen::new(false, 50, 7);
        s.update_frame(press_a());
        let start = s.machine.wheel_offsets;
        // Hold A through the whole warm-up: no reel may stop.
        for _ in 0..SPIN_WARMUP_FRAMES {
            s.update_frame(press_a());
        }
        assert_eq!(s.phase, SlotsPhase::Spinning);
        assert!(!s.reels_stopped.iter().any(|&x| x));
        // 40 warm-up frames animate 20 times (every other frame): each wheel
        // advanced exactly 20 offsets from its start (mod 30).
        for i in 0..3 {
            let moved = (s.machine.wheel_offsets[i] + 30 - start[i]) % 30;
            assert_eq!(moved as u16 % 30, 20, "wheel {i} advanced {moved} in warm-up");
        }
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
    fn insufficient_bet_is_refused_not_clamped() {
        let mut s = SlotsScreen::new(false, 2, 9);
        // Try to bet 3 with only 2 coins.
        s.update_frame(SlotsInput { up: true, ..SlotsInput::none() });
        s.update_frame(SlotsInput { up: true, ..SlotsInput::none() });
        assert_eq!(s.bet, 3);
        assert_eq!(s.update_frame(press_a()), SlotsAction::Continue);
        assert_eq!(s.phase, SlotsPhase::BetSelect, "spin must not start");
        assert_eq!(s.coins, 2, "nothing deducted");
        assert_eq!(s.message, "NOT ENOUGH COINS!");
    }

    #[test]
    fn full_spin_stops_all_reels_and_resolves() {
        let mut s = SlotsScreen::new(false, 100, 42);
        drive_to_result(&mut s);
        assert!(s.reels_stopped.iter().all(|&x| x));
        assert_eq!(s.payout_remaining, 0);
        // A on the result screen goes back to bet selection.
        s.update_frame(press_a());
        assert_eq!(s.phase, SlotsPhase::BetSelect);
    }

    #[test]
    fn out_of_coins_after_spin_auto_exits_after_60_frames() {
        let mut s = SlotsScreen::new(false, 1, 42);
        s.coins = 0;
        // The machine enters the Result phase with an empty balance after a
        // spin; the out-of-coins countdown then exits by itself without
        // waiting for input (the ASM prints the text and `jp DelayFrames`
        // returns from MainSlotMachineLoop).
        s.phase = SlotsPhase::Result;
        for _ in 0..OUT_OF_COINS_DELAY_FRAMES - 1 {
            assert_eq!(s.update_frame(SlotsInput::none()), SlotsAction::Continue);
        }
        assert_eq!(s.message, "OUT OF COINS!");
        assert_eq!(s.update_frame(SlotsInput::none()), SlotsAction::Exit);
    }

    #[test]
    fn one_more_go_menu_a_restarts_b_exits() {
        let mut s = SlotsScreen::new(false, 500, 42);
        drive_to_result(&mut s);
        // One Result-phase frame publishes the "One more go?" prompt (a loss
        // resolves with "NOT THIS TIME!" on the very same transition).
        s.update_frame(SlotsInput::none());
        assert_eq!(s.message, "ONE MORE GO?");
        // B = NO → leave the machine.
        assert_eq!(
            s.update_frame(SlotsInput { b: true, ..SlotsInput::none() }),
            SlotsAction::Exit
        );
    }

    #[test]
    fn payout_fanfare_ticks_one_coin_at_a_time() {
        let mut s = SlotsScreen::new(false, 100, 42);
        // Start a spin, then force a known cherry win on the middle payline.
        s.update_frame(press_a());
        s.machine.flags = SLOTS_CAN_WIN;
        s.machine.wheel_offsets = [6, 10, 12]; // middle row: cherry × 3
        s.reels_stopped = [true; 3];
        while s.spin_warmup > 0 {
            s.update_frame(SlotsInput::none());
        }
        s.update_frame(SlotsInput::none()); // all reels stopped → resolve()

        assert_eq!(s.phase, SlotsPhase::Payout);
        assert_eq!(s.last_payout, 8);
        assert_eq!(s.payout_remaining, 8);
        // Cherry flashes 2 blocks of 5 frames.
        for _ in 0..10 {
            s.update_frame(SlotsInput::none());
        }
        assert_eq!(s.payout_stage, PayoutStage::WaitPress);
        assert_eq!(s.message, "CHERRY lined up! Scored 8 coins!");
        assert_eq!(s.coins, 99, "no coins credited before the tick stage");
        s.update_frame(press_a()); // confirm the "lined up" text
        assert_eq!(s.payout_stage, PayoutStage::Tick);

        // 8 coins × 8 frames; count the per-coin Reward cues.
        let mut reward_cues = 0;
        for _ in 0..200 {
            reward_cues += s
                .take_sfx()
                .into_iter()
                .filter(|c| *c == SlotsSfx::Reward)
                .count();
            if s.phase == SlotsPhase::Result {
                break;
            }
            s.update_frame(SlotsInput::none());
        }
        assert_eq!(s.phase, SlotsPhase::Result);
        assert_eq!(s.payout_remaining, 0);
        assert_eq!(s.coins, 99 + 8);
        assert_eq!(reward_cues, 8, "one Reward SFX per coin");
        assert_eq!(s.message, "ONE MORE GO?");
    }

    #[test]
    fn seven_win_keeps_flags_when_rng_below_half() {
        // Engine-level regression: a 7 win with rng < 0x80 must preserve the
        // 7/bar flags (the "lucky streak" the old double-application killed);
        // the allow-matches counter is always cleared (SlotReward300Func).
        use crate::slots::SlotMachineState;
        let mut m = SlotMachineState::new(true);
        m.flags = SLOTS_CAN_WIN_WITH_7_OR_BAR;
        m.allow_matches_counter = 5;
        m.post_reward_effects_with_rng(SlotSymbol::Seven, 0x7F);
        assert_eq!(m.flags, SLOTS_CAN_WIN_WITH_7_OR_BAR, "flags kept");
        assert_eq!(m.allow_matches_counter, 0);
        m.flags = SLOTS_CAN_WIN_WITH_7_OR_BAR;
        m.post_reward_effects_with_rng(SlotSymbol::Seven, 0x80);
        assert_eq!(m.flags, 0, "flags cleared on rng >= 0x80");
    }

    #[test]
    fn credit_respects_9999_cap() {
        let mut s = SlotsScreen::new(false, 9990, 1);
        s.credit(100);
        assert_eq!(s.coins, MAX_COINS);
    }

    /// With the wheel-2 stop condition fixed, CAN_WIN spins must actually pay
    /// out at roughly the original rate (~17%): wheels 1+2 align, wheel 3
    /// rerolls onto the match. The old inverted condition made wins
    /// structurally impossible on normal machines. A 3-coin bet keeps every
    /// payline active, matching how the alignment mechanic pays out.
    #[test]
    fn can_win_spins_pay_out_like_the_original() {
        let mut wins = 0u32;
        const SPINS: u32 = 400;
        let mut seed = 0xC0FFEEu32;
        for _ in 0..SPINS {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let mut s = SlotsScreen::new(false, 1000, seed);
            // Raise the bet to 3, then spin and hold A through the fanfare.
            s.update_frame(SlotsInput { up: true, ..SlotsInput::none() });
            s.update_frame(SlotsInput { up: true, ..SlotsInput::none() });
            drive_to_result(&mut s);
            if s.last_payout > 0 {
                wins += 1;
            }
        }
        assert!(
            wins >= 30,
            "expected the original ~17% win rate, got {wins}/{SPINS}"
        );
    }
}
