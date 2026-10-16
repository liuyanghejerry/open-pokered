//! Evolution cutscene — port of `EvolveMon` (engine/movie/evolution.asm) and
//! its driver in `EvolutionAfterBattle`/`TryEvolvingMon`
//! (engine/pokemon/evos_moves.asm:2-260).
//!
//! Per-evolution sequence (all timings in 60 fps frames, from the asm):
//! 1. [`EvolutionPhase::IsEvolving`] — "_IsEvolvingText_" ("What? <MON>
//!    is evolving!", data/text/text_3.asm:54-59), then `DelayFrames(50)`
//!    (evos_moves.asm:122-123).
//! 2. [`EvolutionPhase::Tink`] — all music stops, `SFX_TINK` plays
//!    (evolution.asm:9-18), `Delay3` (evolution.asm:19).
//! 3. [`EvolutionPhase::OldCry`] — the old species' cry (`PlayCry`,
//!    evolution.asm:41-43). The original waits for the cry to finish
//!    (`WaitForSoundToFinish`), using the frontend sequencer status.
//! 4. [`EvolutionPhase::MorphMusic`] — `MUSIC_SAFARI_ZONE` starts
//!    (evolution.asm:44-46 — yes, the evolution "jingle" IS the Safari Zone
//!    theme) and the old pic sits in its own palette for `DelayFrames(80)`
//!    (evolution.asm:47-48).
//! 5. [`EvolutionPhase::Morph`] — the screen palette goes black
//!    (`EvolutionSetWholeScreenPalette` with `c=1`, evolution.asm:49-50) and
//!    the pic flickers between the old and new species: outer loop
//!    `lb bc, $1, $10` (evolution.asm:51) runs 8 iterations with
//!    `b = 1..8` flickers and `c = 16, 14, …, 2` cancel-check frames. Each
//!    cancel frame polls the B button (`Evolution_CheckForCancel`,
//!    evolution.asm:142-160); each flicker swaps the pic for `Delay3` frames
//!    per side (`Evolution_BackAndForthAnim` / `Evolution_ChangeMonPic`,
//!    evolution.asm:105-140). B during a cancel window aborts the evolution —
//!    unless `wForceEvolution` is set (evolution.asm:155-158), which the
//!    original sets for evolution stones (`ItemUseEvoStone`,
//!    engine/items/item_effects.asm:776-778) and clears for level-ups
//!    (`EndOfBattle`, engine/battle/end_of_battle.asm:43-44) and Rare Candy
//!    (item_effects.asm:1409-1410).
//! 6a. Success — the new species' pic stays up, music stops, the NEW cry
//!     plays (evolution.asm:62-76), then "_EvolvedText_/_IntoText_"
//!     ("<MON> evolved into <SPECIES>!") with `SFX_GET_ITEM_2`
//!     (evos_moves.asm:136-155).
//! 6b. Cancelled — the old pic stays, the OLD cry plays
//!     (evolution.asm:89-94), then "_StoppedEvolvingText_" ("Huh? <MON>
//!     stopped evolving!", a `prompt` text — it waits for a button press,
//!     text_3.asm:47-52).
//!
//! The state machine owns NO game data: when an evolution resolves it emits
//! an [`EvolutionOutcome`] the frontend applies (species swap / stat recalc /
//! Pokédex) via [`crate::pokemon::evolution::finalize_evolution`]. A B-cancel
//! therefore leaves the mon untouched, and — because the level-up flag is
//! re-set on every later level-up (`experience.asm` sets the
//! `wCanEvolveFlags` bit each level; `EvolutionAfterBattle` never clears a
//! cancelled mon's eligibility permanently) — the evolution is re-attempted
//! on the next level-up, exactly like the original.

use crate::alloc_prelude::*;
use pokered_data::species::Species;
use crate::foreground_input::ForegroundJoypad;
use crate::text::terminal_input::{TerminalTextEnd, TerminalTextKind, TerminalTextStep, TerminalTextWork};

/// `DelayFrames(50)` after "What? X is evolving!" (evos_moves.asm:122-123).
pub const IS_EVOLVING_FRAMES: u16 = 50;
/// `Delay3` after SFX_TINK (evolution.asm:19).
pub const TINK_FRAMES: u16 = 3;
/// `DelayFrames(80)` with the morph music playing (evolution.asm:47-48).
pub const MORPH_MUSIC_FRAMES: u16 = 80;
/// `Delay3` per pic swap within a flicker (`Evolution_ChangeMonPic`,
/// evolution.asm:138).
pub const FLICKER_HALF_FRAMES: u16 = 3;
/// Outer morph-loop iterations (`lb bc, $1, $10` with `dec c, dec c`,
/// evolution.asm:51-61): 16/14/…/2 cancel frames, 1..8 flickers.
pub const MORPH_ITERATIONS: u8 = 8;
/// Delay after the success sound finishes (evos_moves.asm:151-155).
pub const EVOLVED_TEXT_FRAMES: u16 = 40;

/// One queued evolution (party member `party_index` evolving `from` → `to`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEvolution {
    pub party_index: usize,
    pub from: Species,
    pub to: Species,
    /// Display name (nickname) at queue time, for the "What? X is evolving!"
    /// texts — the original prints the party nickname (`GetPartyMonName`,
    /// evos_moves.asm:116-118).
    pub name: String,
    /// `wForceEvolution`: set for evolution stones (uncancellable), clear for
    /// level-up / Rare Candy evolutions (B cancels).
    pub force: bool,
}

/// Sounds the cutscene asks the frontend to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionSfx {
    /// `SFX_STOP_ALL_MUSIC` (evolution.asm:12-14 and again at :70-72).
    StopMusic,
    /// `SFX_TINK` before the morph (evolution.asm:17-18).
    Tink,
    /// `PlayCry` of the given species (old pre-morph, final post-morph —
    /// evolution.asm:41-42 / :73-74).
    Cry(Species),
    /// `MUSIC_SAFARI_ZONE` during the morph (evolution.asm:44-46).
    MorphMusic,
    /// `SFX_GET_ITEM_2` with "X evolved into Y!" (evos_moves.asm:151-152).
    GetItem2,
}

/// How a single queued evolution resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionOutcomeKind {
    /// The morph completed: apply the species swap, stat recalc, move
    /// learning and Pokédex updates.
    Evolved,
    /// B was pressed during a cancel window: the mon is unchanged and no
    /// Pokédex update happens (`CancelledEvolution`, evos_moves.asm:293-299).
    Cancelled,
}

/// Emitted when one queued evolution resolves; drained by the frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolutionOutcome {
    pub party_index: usize,
    pub from: Species,
    pub to: Species,
    pub kind: EvolutionOutcomeKind,
}

/// Per-frame input for the cutscene.
#[derive(Debug, Clone, Copy, Default)]
pub struct EvolutionInput {
    pub a: bool,
    pub b: bool,
}

impl EvolutionInput {
    pub fn none() -> Self {
        Self::default()
    }
}

/// Phases of one evolution, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionPhase {
    /// Optional pre-message (e.g. Rare Candy's "X grew to level Y!", which
    /// the original prints before `TryEvolvingMon`, item_effects.asm). Waits
    /// for A/B.
    IntroText,
    /// "What? X is evolving!" + `DelayFrames(50)`.
    IsEvolving,
    /// Music stops, `SFX_TINK`, `Delay3`.
    Tink,
    /// Old species' cry.
    OldCry,
    /// `MUSIC_SAFARI_ZONE` playing, old pic in its palette, 80 frames.
    MorphMusic,
    /// Black-palette flicker between old and new; B-cancel windows open
    /// (unless `force`).
    Morph,
    /// "X evolved into Y!" + `SFX_GET_ITEM_2` (auto-advances; the original
    /// does not wait for a button here).
    EvolvedText,
    /// "Huh? X stopped evolving!" — a `prompt` text, waits for A/B.
    StoppedText,
    /// Queue exhausted.
    Done,
}

#[derive(Debug, Clone, Copy)]
enum SuccessSoundStage {
    Cry,
    Text,
    Jingle,
    Delay,
}

/// Frame-stepped evolution cutscene. Frontends call [`Self::tick`] once per
/// frame, drain [`Self::pending_sfx`] and [`Self::take_outcome`], and render
/// from the accessors. Multiple pending evolutions play back-to-back (the
/// original's `Evolution_PartyMonLoop`, evos_moves.asm:26).
#[derive(Debug, Clone)]
pub struct EvolutionScreenState {
    queue: VecDeque<PendingEvolution>,
    /// Optional one-shot message shown before the first evolution (Rare
    /// Candy's level-up text; `None` for battle / stone triggers).
    pre_text: Option<String>,
    phase: EvolutionPhase,
    frame: u16,
    /// Morph outer-loop iteration, 0..MORPH_ITERATIONS.
    morph_iter: u8,
    /// Frame within the current morph iteration (cancel window + flickers).
    morph_frame: u16,
    cancelled: bool,
    success_sound_stage: SuccessSoundStage,
    /// Chinese text when true, English otherwise.
    pub is_zh: bool,
    /// SFX queued since the last tick; drained by the frontend.
    pub pending_sfx: Vec<EvolutionSfx>,
    outcomes: VecDeque<EvolutionOutcome>,
    /// Borrowed-ledger API owns only the returning text. Legacy wrappers
    /// leave this disabled; no second input history lives in the cutscene.
    source_terminal_enabled: bool,
    terminal_text: Option<TerminalTextWork>,
}

impl EvolutionScreenState {
    pub fn new(
        queue: Vec<PendingEvolution>,
        pre_text: Option<String>,
        is_zh: bool,
    ) -> Self {
        let phase = if pre_text.is_some() {
            EvolutionPhase::IntroText
        } else {
            EvolutionPhase::IsEvolving
        };
        Self {
            queue: queue.into(),
            pre_text,
            phase,
            frame: 0,
            morph_iter: 0,
            morph_frame: 0,
            cancelled: false,
            success_sound_stage: SuccessSoundStage::Cry,
            is_zh,
            pending_sfx: Vec::new(),
            outcomes: VecDeque::new(),
            source_terminal_enabled: false,
            terminal_text: None,
        }
    }

    pub fn phase(&self) -> EvolutionPhase {
        self.phase
    }

    pub fn is_done(&self) -> bool {
        self.phase == EvolutionPhase::Done
    }

    /// The evolution currently on screen, if any.
    pub fn current(&self) -> Option<&PendingEvolution> {
        self.queue.front()
    }

    /// Cancel-check frames of morph iteration `k` (`c = 16, 14, …, 2`,
    /// evolution.asm:51 + :59-60).
    fn cancel_window_frames(k: u8) -> u16 {
        (16 - 2 * k) as u16
    }

    /// Flickers of morph iteration `k` (`b = 1..8`, evolution.asm:51 + :58).
    fn flickers(k: u8) -> u16 {
        (k + 1) as u16
    }

    /// Total frames of morph iteration `k` (cancel window + flickers at
    /// 2 × [`FLICKER_HALF_FRAMES`] each).
    fn iteration_frames(k: u8) -> u16 {
        Self::cancel_window_frames(k) + Self::flickers(k) * 2 * FLICKER_HALF_FRAMES
    }

    /// True while the B button is being polled (the morph cancel windows,
    /// `Evolution_CheckForCancel`) and this evolution allows cancelling.
    pub fn cancel_window_open(&self) -> bool {
        if self.phase != EvolutionPhase::Morph {
            return false;
        }
        let force = self.current().map(|c| c.force).unwrap_or(true);
        !force && self.morph_frame < Self::cancel_window_frames(self.morph_iter)
    }

    /// Ordinary GB never executes the SGB whole-screen palette command.
    pub fn black_palette(&self) -> bool {
        false
    }

    /// SGB-only request (`RunPaletteCommand` returns immediately on plain GB).
    pub fn sgb_black_palette_requested(&self) -> bool {
        self.phase == EvolutionPhase::Morph
    }

    /// Species whose pic is currently on screen. During a morph flicker the
    /// pic alternates old ↔ new every [`FLICKER_HALF_FRAMES`] frames (the
    /// cancel window shows the old pic — `Evolution_ChangeMonPic` only runs
    /// inside `Evolution_BackAndForthAnim`).
    pub fn visible_species(&self) -> Option<Species> {
        let cur = self.current()?;
        match self.phase {
            EvolutionPhase::IntroText | EvolutionPhase::Done => None,
            EvolutionPhase::Morph => {
                let window = Self::cancel_window_frames(self.morph_iter);
                if self.morph_frame < window {
                    return Some(cur.from);
                }
                let flicker_frame = (self.morph_frame - window) % (2 * FLICKER_HALF_FRAMES);
                Some(if flicker_frame < FLICKER_HALF_FRAMES {
                    cur.to
                } else {
                    cur.from
                })
            }
            EvolutionPhase::EvolvedText => Some(cur.to),
            EvolutionPhase::StoppedText => Some(cur.from),
            _ => Some(cur.from),
        }
    }

    /// The current two text-box lines, if a text phase is active.
    pub fn text_lines(&self) -> Option<(String, String)> {
        if self.source_terminal_enabled
            && matches!(
                self.phase,
                EvolutionPhase::EvolvedText | EvolutionPhase::StoppedText
            )
        {
            let visible = self.terminal_text.as_ref()?.visible_text();
            let mut lines = visible.splitn(2, '\n');
            return Some((
                lines.next().unwrap_or("").into(),
                lines.next().unwrap_or("").into(),
            ));
        }
        match self.phase {
            EvolutionPhase::IntroText => {
                let text = self.pre_text.clone()?;
                let mut lines = text.splitn(2, '\n');
                let l1 = lines.next().unwrap_or("").to_string();
                let l2 = lines.next().unwrap_or("").to_string();
                Some((l1, l2))
            }
            EvolutionPhase::IsEvolving => {
                let cur = self.current()?;
                // _IsEvolvingText (text_3.asm:54-59).
                if self.is_zh {
                    Some(("什么？".to_string(), format!("{} 正在进化！", cur.name))
                    )
                } else {
                    Some((format!("What? {}", cur.name), "is evolving!".to_string()))
                }
            }
            EvolutionPhase::EvolvedText => {
                let cur = self.current()?;
                let to = pokered_data::lang_data::species_name(cur.to, self.is_zh);
                // _EvolvedText + _IntoText (text_3.asm:35-45).
                if self.is_zh {
                    Some((format!("{} 进化成了", cur.name), format!("{}！", to)))
                } else {
                    Some((format!("{} evolved", cur.name), format!("into {}!", to)))
                }
            }
            EvolutionPhase::StoppedText => {
                let cur = self.current()?;
                // _StoppedEvolvingText (text_3.asm:47-52).
                if self.is_zh {
                    Some(("啊？".to_string(), format!("{} 停止了进化！", cur.name)))
                } else {
                    Some((format!("Huh? {}", cur.name), "stopped evolving!".to_string()))
                }
            }
            _ => None,
        }
    }

    /// Take the next resolved evolution, if any.
    pub fn take_outcome(&mut self) -> Option<EvolutionOutcome> {
        self.outcomes.pop_front()
    }

    fn enter(&mut self, phase: EvolutionPhase) {
        self.phase = phase;
        self.frame = 0;
        self.terminal_text = None;
        let cur = self.queue.front().cloned();
        match phase {
            EvolutionPhase::Tink => {
                self.pending_sfx.push(EvolutionSfx::StopMusic);
                self.pending_sfx.push(EvolutionSfx::Tink);
            }
            EvolutionPhase::OldCry => {
                if let Some(cur) = cur {
                    self.pending_sfx.push(EvolutionSfx::Cry(cur.from));
                }
            }
            EvolutionPhase::MorphMusic => {
                self.pending_sfx.push(EvolutionSfx::MorphMusic);
            }
            EvolutionPhase::Morph => {
                self.morph_iter = 0;
                self.morph_frame = 0;
            }
            EvolutionPhase::EvolvedText => {
                if let Some(cur) = cur {
                    self.pending_sfx.push(EvolutionSfx::StopMusic);
                    self.pending_sfx.push(EvolutionSfx::Cry(cur.to));
                    self.success_sound_stage = SuccessSoundStage::Cry;
                }
            }
            EvolutionPhase::StoppedText => {
                if let Some(cur) = cur {
                    self.pending_sfx.push(EvolutionSfx::StopMusic);
                    self.pending_sfx.push(EvolutionSfx::Cry(cur.from));
                }
            }
            _ => {}
        }
    }

    /// Resolve the current evolution and advance the queue (the original's
    /// `Evolution_PartyMonLoop` continues with the next party member).
    fn finish_current(&mut self, kind: EvolutionOutcomeKind) {
        if let Some(cur) = self.queue.pop_front() {
            self.outcomes.push_back(EvolutionOutcome {
                party_index: cur.party_index,
                from: cur.from,
                to: cur.to,
                kind,
            });
        }
        self.cancelled = false;
        if self.queue.is_empty() {
            self.enter(EvolutionPhase::Done);
        } else {
            self.enter(EvolutionPhase::IsEvolving);
        }
    }

    /// Advance a synthetic silent frame. Frontends use `tick_with_sound`
    /// with the real sequencer status, even when device output is muted.
    pub fn tick(&mut self, input: EvolutionInput) -> bool {
        self.tick_with_sound(input, false)
    }

    /// Advance using the sequencer status, including muted/headless playback.
    pub fn tick_with_sound(&mut self, input: EvolutionInput, sound_playing: bool) -> bool {
        self.tick_inner(input, sound_playing, None).complete
    }

    /// Real returning Evolved/Into DONE and Stopped PROMPT owner. Earlier
    /// morph/forget-menu parity is not inferred from this terminal contract.
    /// The frontend lends the same ledger used for money/PayDay/learning.
    pub fn tick_with_terminal_input(
        &mut self,
        input: EvolutionInput,
        sound_playing: bool,
        held_buttons: u8,
        options_delay: u8,
        owner: &mut ForegroundJoypad,
    ) -> TerminalTextStep {
        owner.read_raw(held_buttons);
        self.source_terminal_enabled = true;
        self.tick_inner(input, sound_playing, Some((owner, options_delay)))
    }

    fn tick_inner(
        &mut self,
        input: EvolutionInput,
        sound_playing: bool,
        mut foreground: Option<(&mut ForegroundJoypad, u8)>,
    ) -> TerminalTextStep {
        let mut result = TerminalTextStep::default();
        match self.phase {
            EvolutionPhase::IntroText => {
                if input.a || input.b {
                    self.pre_text = None;
                    self.enter(EvolutionPhase::IsEvolving);
                }
            }
            EvolutionPhase::IsEvolving => {
                self.frame += 1;
                if self.frame >= IS_EVOLVING_FRAMES {
                    self.enter(EvolutionPhase::Tink);
                }
            }
            EvolutionPhase::Tink => {
                self.frame += 1;
                if self.frame >= TINK_FRAMES {
                    self.enter(EvolutionPhase::OldCry);
                }
            }
            EvolutionPhase::OldCry => {
                if !sound_playing {
                    self.enter(EvolutionPhase::MorphMusic);
                }
            }
            EvolutionPhase::MorphMusic => {
                self.frame += 1;
                if self.frame >= MORPH_MUSIC_FRAMES {
                    self.enter(EvolutionPhase::Morph);
                }
            }
            EvolutionPhase::Morph => {
                // Evolution_CheckForCancel (evolution.asm:142-160): B during a
                // cancel window aborts unless wForceEvolution is set.
                if input.b && self.cancel_window_open() {
                    self.cancelled = true;
                }
                if self.cancelled {
                    self.enter(EvolutionPhase::StoppedText);
                    result.complete = self.is_done();
                    return result;
                }
                self.morph_frame += 1;
                if self.morph_frame >= Self::iteration_frames(self.morph_iter) {
                    self.morph_iter += 1;
                    self.morph_frame = 0;
                    if self.morph_iter >= MORPH_ITERATIONS {
                        self.enter(EvolutionPhase::EvolvedText);
                    }
                }
            }
            EvolutionPhase::EvolvedText if self.source_terminal_enabled => {
                match self.success_sound_stage {
                    SuccessSoundStage::Cry if !sound_playing => {
                        if let Some((_, delay)) = foreground.as_ref() {
                            let (first, second) = self.full_success_lines();
                            // PrintText(Evolved) has Delay3; Into immediately
                            // uses PrintText_NoCreatingTextBox, with no extra3.
                            self.terminal_text = Some(TerminalTextWork::new(
                                TerminalTextKind::EvolvedInto,
                                format!("{first}\n{second}"),
                                TerminalTextEnd::Done,
                                *delay,
                                3,
                            ));
                            self.success_sound_stage = SuccessSoundStage::Text;
                        }
                    }
                    SuccessSoundStage::Text => {
                        if let Some((owner, _)) = foreground.as_mut() {
                            let step = self
                                .terminal_text
                                .as_mut()
                                .unwrap()
                                .tick(owner, sound_playing);
                            result.input_error = step.input_error;
                            if step.complete {
                                self.pending_sfx.push(EvolutionSfx::GetItem2);
                                self.success_sound_stage = SuccessSoundStage::Jingle;
                            }
                        }
                    }
                    SuccessSoundStage::Jingle if !sound_playing => {
                        self.success_sound_stage = SuccessSoundStage::Delay;
                        self.frame = 0;
                    }
                    SuccessSoundStage::Delay => {
                        self.frame += 1;
                        if self.frame >= EVOLVED_TEXT_FRAMES {
                            self.finish_current(EvolutionOutcomeKind::Evolved);
                        }
                    }
                    _ => {}
                }
            }
            EvolutionPhase::EvolvedText => match self.success_sound_stage {
                SuccessSoundStage::Cry if !sound_playing => {
                    self.pending_sfx.push(EvolutionSfx::GetItem2);
                    self.success_sound_stage = SuccessSoundStage::Jingle;
                }
                SuccessSoundStage::Jingle if !sound_playing => {
                    self.success_sound_stage = SuccessSoundStage::Delay;
                    self.frame = 0;
                }
                SuccessSoundStage::Delay => {
                    self.frame += 1;
                    if self.frame >= EVOLVED_TEXT_FRAMES {
                        self.finish_current(EvolutionOutcomeKind::Evolved);
                    }
                }
                _ => {}
            },
            EvolutionPhase::StoppedText if self.source_terminal_enabled => {
                if let Some((owner, delay)) = foreground.as_mut() {
                    if self.terminal_text.is_none() {
                        if !sound_playing {
                            let (first, second) = self.full_stopped_lines();
                            self.terminal_text = Some(TerminalTextWork::new(
                                TerminalTextKind::Stopped,
                                format!("{first}\n{second}"),
                                TerminalTextEnd::Prompt,
                                *delay,
                                3,
                            ));
                        }
                    } else {
                        let step = self
                            .terminal_text
                            .as_mut()
                            .unwrap()
                            .tick(owner, sound_playing);
                        result.press_ab |= step.press_ab;
                        result.input_error = step.input_error;
                        if step.complete {
                            self.finish_current(EvolutionOutcomeKind::Cancelled);
                        }
                    }
                }
            }
            EvolutionPhase::StoppedText => {
                // PlayCry finishes before the prompt can be dismissed.
                if !sound_playing && (input.a || input.b) {
                    self.finish_current(EvolutionOutcomeKind::Cancelled);
                }
            }
            EvolutionPhase::Done => {}
        }
        result.complete = self.is_done();
        result
    }

    fn full_success_lines(&self) -> (String, String) {
        let cur = self
            .current()
            .expect("success text retains its queue member");
        let to = pokered_data::lang_data::species_name(cur.to, self.is_zh);
        if self.is_zh {
            (format!("{} 进化成了", cur.name), format!("{}！", to))
        } else {
            (format!("{} evolved", cur.name), format!("into {}!", to))
        }
    }

    fn full_stopped_lines(&self) -> (String, String) {
        let cur = self
            .current()
            .expect("stopped text retains its queue member");
        if self.is_zh {
            ("啊？".into(), format!("{} 停止了进化！", cur.name))
        } else {
            (format!("Huh? {}", cur.name), "stopped evolving!".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_success_keeps_last_real_into_letter_poll_through_jingle_and_forty_frames() {
        use crate::foreground_input::{A, SELECT, START, UP};
        let mut anim = screen(false);
        let mut owner = ForegroundJoypad::default();
        let mut saw_terminal_printing = false;
        let mut last_letter = None;
        let mut delay_frames = 0;
        for _ in 0..2000 {
            let printing = anim.phase == EvolutionPhase::EvolvedText
                && matches!(anim.success_sound_stage, SuccessSoundStage::Text);
            let tail = anim.phase == EvolutionPhase::EvolvedText
                && matches!(
                    anim.success_sound_stage,
                    SuccessSoundStage::Jingle | SuccessSoundStage::Delay
                );
            let counted_delay = anim.phase == EvolutionPhase::EvolvedText
                && matches!(anim.success_sound_stage, SuccessSoundStage::Delay);
            let held = if printing { A | SELECT } else { START | UP };
            let step =
                anim.tick_with_terminal_input(EvolutionInput::none(), false, held, 1, &mut owner);
            if printing {
                saw_terminal_printing = true;
                last_letter = owner.last();
            }
            if tail {
                assert_eq!(owner.last(), last_letter, "sound/Delay40 do not poll");
                delay_frames += u16::from(counted_delay);
            }
            if step.complete {
                break;
            }
        }
        assert!(anim.is_done() && saw_terminal_printing);
        assert_eq!(last_letter, Some(A | SELECT));
        assert_eq!(delay_frames, EVOLVED_TEXT_FRAMES);
        assert_eq!(owner.last(), last_letter);
        assert_eq!(
            anim.take_outcome().unwrap().kind,
            EvolutionOutcomeKind::Evolved
        );
    }

    #[test]
    fn source_cancel_uses_stopped_real_prompt_instead_of_morph_or_exit_raw() {
        use crate::foreground_input::{A, B, SELECT, START};
        let mut anim = screen(false);
        let mut owner = ForegroundJoypad::default();
        for _ in 0..2000 {
            let cancel = anim.cancel_window_open();
            anim.tick_with_terminal_input(
                EvolutionInput {
                    a: false,
                    b: cancel,
                },
                false,
                B | SELECT,
                1,
                &mut owner,
            );
            if anim
                .terminal_text
                .as_ref()
                .is_some_and(|work| work.waiting_for_input())
            {
                break;
            }
        }
        assert_eq!(anim.phase(), EvolutionPhase::StoppedText);
        assert_eq!(owner.last(), Some(B | SELECT));
        let held =
            anim.tick_with_terminal_input(EvolutionInput::none(), false, B | SELECT, 1, &mut owner);
        assert!(
            !held.complete && !held.press_ab,
            "held printing B is already consumed"
        );
        anim.tick_with_terminal_input(EvolutionInput::none(), false, 0, 1, &mut owner);
        let ack =
            anim.tick_with_terminal_input(EvolutionInput::none(), false, A | START, 1, &mut owner);
        assert!(ack.complete && ack.press_ab);
        assert_eq!(owner.last(), Some(A | START));
        assert_eq!(
            anim.take_outcome().unwrap().kind,
            EvolutionOutcomeKind::Cancelled
        );
    }

    #[test]
    fn chinese_evolution_rows_fit_all_species_and_valid_nicknames() {
        use pokered_data::dialogue_layout::measure_text;
        for entry in pokered_data::pokedex::POKEDEX_ENTRIES {
            for name in ["ABCDEFGHIJ", "阿尔法贝塔", "AB张三丰"] {
                let mut s = EvolutionScreenState::new(vec![PendingEvolution {
                    party_index: 0, from: Species::Bulbasaur, to: entry.species,
                    name: name.into(), force: false,
                }], None, true);
                for phase in [EvolutionPhase::IsEvolving, EvolutionPhase::EvolvedText, EvolutionPhase::StoppedText] {
                    s.phase = phase;
                    let (a,b) = s.text_lines().unwrap();
                    assert!(measure_text(&a) <= 144 && measure_text(&b) <= 144,
                        "{:?}/{phase:?}/{name}: {a} / {b}", entry.species);
                }
            }
        }
    }

    fn evo(force: bool) -> PendingEvolution {
        PendingEvolution {
            party_index: 0,
            from: Species::Bulbasaur,
            to: Species::Ivysaur,
            name: "BULBASAUR".to_string(),
            force,
        }
    }

    fn screen(force: bool) -> EvolutionScreenState {
        EvolutionScreenState::new(vec![evo(force)], None, false)
    }

    fn tick_n(s: &mut EvolutionScreenState, n: u16) {
        for _ in 0..n {
            s.tick(EvolutionInput::none());
        }
    }

    /// The full phase order of an uninterrupted evolution, with the asm frame
    /// counts.
    #[test]
    fn phase_order_and_frame_counts() {
        let mut s = screen(false);
        assert_eq!(s.phase(), EvolutionPhase::IsEvolving);
        assert_eq!(
            s.text_lines(),
            Some(("What? BULBASAUR".to_string(), "is evolving!".to_string()))
        );
        tick_n(&mut s, IS_EVOLVING_FRAMES);
        assert_eq!(s.phase(), EvolutionPhase::Tink);
        // Music stops + SFX_TINK at the morph setup (evolution.asm:12-18).
        let sfx: Vec<_> = s.pending_sfx.drain(..).collect();
        assert_eq!(sfx, vec![EvolutionSfx::StopMusic, EvolutionSfx::Tink]);
        tick_n(&mut s, TINK_FRAMES);
        assert_eq!(s.phase(), EvolutionPhase::OldCry);
        assert_eq!(
            s.pending_sfx.drain(..).collect::<Vec<_>>(),
            vec![EvolutionSfx::Cry(Species::Bulbasaur)]
        );
        for _ in 0..123 {
            s.tick_with_sound(EvolutionInput::none(), true);
            assert_eq!(s.phase(), EvolutionPhase::OldCry);
        }
        tick_n(&mut s, 1);
        assert_eq!(s.phase(), EvolutionPhase::MorphMusic);
        assert_eq!(
            s.pending_sfx.drain(..).collect::<Vec<_>>(),
            vec![EvolutionSfx::MorphMusic],
            "MUSIC_SAFARI_ZONE during the morph (evolution.asm:44-46)"
        );
        assert!(!s.black_palette());
        tick_n(&mut s, MORPH_MUSIC_FRAMES);
        assert_eq!(s.phase(), EvolutionPhase::Morph);
        assert!(!s.black_palette(), "ordinary GB palette stays unchanged");
        assert!(s.sgb_black_palette_requested());
        // Run the whole morph: sum over k of (16-2k) + 6(k+1) frames.
        let morph_frames: u16 = (0..MORPH_ITERATIONS)
            .map(EvolutionScreenState::iteration_frames)
            .sum();
        assert_eq!(morph_frames, 72 + 216);
        tick_n(&mut s, morph_frames);
        assert_eq!(s.phase(), EvolutionPhase::EvolvedText);
        assert_eq!(
            s.text_lines(),
            Some(("BULBASAUR evolved".to_string(), "into IVYSAUR!".to_string()))
        );
        let sfx: Vec<_> = s.pending_sfx.drain(..).collect();
        assert_eq!(
            sfx,
            vec![EvolutionSfx::StopMusic, EvolutionSfx::Cry(Species::Ivysaur)]
        );
        for _ in 0..137 {
            s.tick_with_sound(EvolutionInput::none(), true);
            assert!(
                s.pending_sfx.is_empty(),
                "success jingle must wait for new cry"
            );
        }
        tick_n(&mut s, 1);
        assert_eq!(
            s.pending_sfx.drain(..).collect::<Vec<_>>(),
            vec![EvolutionSfx::GetItem2]
        );
        for _ in 0..151 {
            s.tick_with_sound(EvolutionInput::none(), true);
            assert!(s.take_outcome().is_none());
        }
        tick_n(&mut s, 1);
        tick_n(&mut s, EVOLVED_TEXT_FRAMES - 1);
        assert!(!s.is_done());
        tick_n(&mut s, 1);
        assert!(s.is_done());
        assert_eq!(
            s.take_outcome(),
            Some(EvolutionOutcome {
                party_index: 0,
                from: Species::Bulbasaur,
                to: Species::Ivysaur,
                kind: EvolutionOutcomeKind::Evolved,
            })
        );
        assert!(s.take_outcome().is_none());
    }

    /// The flicker alternates old/new species pics; the cancel window shows
    /// the old pic.
    #[test]
    fn morph_flicker_alternates_species() {
        let mut s = screen(false);
        s.enter(EvolutionPhase::Morph);
        assert_eq!(s.visible_species(), Some(Species::Bulbasaur));
        assert!(s.cancel_window_open());
        // 16-frame cancel window of iteration 0, then 1 flicker.
        tick_n(&mut s, 16);
        assert!(!s.cancel_window_open(), "window closed after 16 frames");
        assert_eq!(s.visible_species(), Some(Species::Ivysaur));
        tick_n(&mut s, FLICKER_HALF_FRAMES);
        assert_eq!(s.visible_species(), Some(Species::Bulbasaur));
        tick_n(&mut s, FLICKER_HALF_FRAMES);
        // Iteration 1: 14-frame cancel window, old pic again.
        assert_eq!(s.visible_species(), Some(Species::Bulbasaur));
        assert!(s.cancel_window_open());
    }

    /// B during a cancel window aborts a level-up evolution: the old cry
    /// plays, "Huh? X stopped evolving!" waits for a button, and the outcome
    /// is Cancelled.
    #[test]
    fn b_cancel_during_window() {
        let mut s = screen(false);
        s.enter(EvolutionPhase::Morph);
        s.tick(EvolutionInput {
            a: false,
            b: true,
        });
        assert_eq!(s.phase(), EvolutionPhase::StoppedText);
        assert_eq!(
            s.pending_sfx.drain(..).collect::<Vec<_>>(),
            vec![
                EvolutionSfx::StopMusic,
                EvolutionSfx::Cry(Species::Bulbasaur)
            ]
        );
        assert_eq!(
            s.text_lines(),
            Some(("Huh? BULBASAUR".to_string(), "stopped evolving!".to_string()))
        );
        s.tick_with_sound(EvolutionInput { a: true, b: false }, true);
        assert_eq!(
            s.phase(),
            EvolutionPhase::StoppedText,
            "cancel cry must finish first"
        );
        // Prompt text: no auto-advance.
        tick_n(&mut s, 500);
        assert_eq!(s.phase(), EvolutionPhase::StoppedText);
        s.tick(EvolutionInput {
            a: true,
            b: false,
        });
        assert!(s.is_done());
        assert_eq!(
            s.take_outcome().map(|o| o.kind),
            Some(EvolutionOutcomeKind::Cancelled)
        );
    }

    /// B outside the cancel windows (during a flicker) does NOT cancel — the
    /// original only polls B inside Evolution_CheckForCancel.
    #[test]
    fn b_outside_window_does_not_cancel() {
        let mut s = screen(false);
        s.enter(EvolutionPhase::Morph);
        tick_n(&mut s, 16); // window of iteration 0 closes
        assert!(!s.cancel_window_open());
        s.tick(EvolutionInput {
            a: false,
            b: true,
        });
        assert_eq!(s.phase(), EvolutionPhase::Morph, "flicker B is ignored");
    }

    /// Forced (stone) evolutions ignore B entirely (wForceEvolution,
    /// evolution.asm:155-158).
    #[test]
    fn forced_evolution_ignores_b() {
        let mut s = screen(true);
        s.enter(EvolutionPhase::Morph);
        assert!(!s.cancel_window_open());
        let morph_frames: u16 = (0..MORPH_ITERATIONS)
            .map(EvolutionScreenState::iteration_frames)
            .sum();
        for _ in 0..morph_frames - 1 {
            s.tick(EvolutionInput {
                a: false,
                b: true,
            });
            assert_eq!(s.phase(), EvolutionPhase::Morph);
        }
        s.tick(EvolutionInput {
            a: false,
            b: true,
        });
        assert_eq!(s.phase(), EvolutionPhase::EvolvedText);
    }

    /// Multiple queued evolutions play back-to-back (Evolution_PartyMonLoop).
    #[test]
    fn queue_plays_back_to_back() {
        let mut second = evo(false);
        second.party_index = 1;
        second.name = "BULBASAUR2".to_string();
        let mut s = EvolutionScreenState::new(vec![evo(false), second], None, false);
        // Cancel the first, let the second complete.
        s.enter(EvolutionPhase::Morph);
        s.tick(EvolutionInput {
            a: false,
            b: true,
        });
        s.tick(EvolutionInput {
            a: true,
            b: false,
        });
        assert_eq!(s.phase(), EvolutionPhase::IsEvolving, "next queued evolution");
        assert_eq!(
            s.take_outcome().map(|o| o.kind),
            Some(EvolutionOutcomeKind::Cancelled)
        );
        // Run the second to completion.
        let mut guard = 0;
        while !s.is_done() {
            s.tick(EvolutionInput::none());
            guard += 1;
            assert!(guard < 5000, "must terminate");
        }
        assert_eq!(
            s.take_outcome(),
            Some(EvolutionOutcome {
                party_index: 1,
                from: Species::Bulbasaur,
                to: Species::Ivysaur,
                kind: EvolutionOutcomeKind::Evolved,
            })
        );
    }

    /// The Rare Candy pre-message plays before "What?" and needs a button.
    #[test]
    fn intro_pre_text() {
        let mut s = EvolutionScreenState::new(
            vec![evo(false)],
            Some("BULBASAUR grew to\nlevel 16!".to_string()),
            false,
        );
        assert_eq!(s.phase(), EvolutionPhase::IntroText);
        assert_eq!(
            s.text_lines(),
            Some(("BULBASAUR grew to".to_string(), "level 16!".to_string()))
        );
        tick_n(&mut s, 100);
        assert_eq!(s.phase(), EvolutionPhase::IntroText, "waits for a button");
        s.tick(EvolutionInput {
            a: true,
            b: false,
        });
        assert_eq!(s.phase(), EvolutionPhase::IsEvolving);
    }
}
