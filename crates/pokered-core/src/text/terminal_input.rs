//! The real returning PrintText owners after a trainer battle.
//!
//! The caller lends one foreground ledger. Only Printing/Prompt poll it;
//! preparation, protected waits and sound waits merely let raw input change.
//! Intermediate battle/morph/forget-menu timing is outside this capability.

use crate::alloc_prelude::*;
use crate::foreground_input::{ForegroundJoypad, A, B};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalTextKind {
    Prize,
    PayDay,
    EvolvedInto,
    Stopped,
    Learned,
    DidNotLearn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalTextEnd {
    Done,
    /// The PROMPT character calls ProtectedDelay3 before ManualTextScroll.
    Prompt,
    /// sound_GET_ITEM_1 waits for sound, then PROMPT_BUTTON has no Delay3.
    LearnedMove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalTextStage {
    Preparation { remaining: u8 },
    Printing,
    ProtectedPrompt { remaining: u8 },
    LearnedSound,
    WaitLearnedSound,
    Prompt,
    Complete,
    Blocked(TerminalInputError),
}

// Compatibility name for the older terminal API; the canonical input
// contract is shared with ordinary field consumers, not owned by battles.
pub use crate::foreground_input::ForegroundInputError as TerminalInputError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum LetterWait {
    None,
    Polling {
        remaining: u8,
    },
    /// PrintLetterDelay's held A/B branch returns after one DelayFrame;
    /// it does not execute a final poll after that frame.
    HeldVBlank,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalTextStep {
    pub complete: bool,
    pub press_ab: bool,
    pub learned_sound: bool,
    pub input_error: Option<TerminalInputError>,
}

/// This work is the display cursor and the input owner, not a hidden observer
/// running alongside an already fully displayed sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalTextWork {
    pub kind: TerminalTextKind,
    text: String,
    visible_chars: usize,
    stage: TerminalTextStage,
    letter_wait: LetterWait,
    end: TerminalTextEnd,
    /// The verified normal caller has NO_TEXT_DELAY clear and TEXT_DELAY set.
    /// Options 1/3/5 control the real wait. Even 0 still executes Joypad.
    options_delay: u8,
}

impl TerminalTextWork {
    pub fn new(
        kind: TerminalTextKind,
        text: String,
        end: TerminalTextEnd,
        options_delay: u8,
        preparation: u8,
    ) -> Self {
        assert!(
            text.chars().any(|c| c != '\n'),
            "terminal owner needs real nonempty text"
        );
        Self {
            kind,
            text,
            visible_chars: 0,
            stage: TerminalTextStage::Preparation {
                remaining: preparation,
            },
            letter_wait: LetterWait::None,
            end,
            options_delay,
        }
    }

    pub fn stage(&self) -> TerminalTextStage {
        self.stage
    }
    pub fn options_delay(&self) -> u8 {
        self.options_delay
    }
    pub fn is_complete(&self) -> bool {
        self.stage == TerminalTextStage::Complete
    }
    pub fn waiting_for_input(&self) -> bool {
        self.stage == TerminalTextStage::Prompt
    }

    /// _ContText has already scrolled an existing row to the top. That row
    /// is displayed, not reprinted or repolled; only the new bottom row owns
    /// subsequent PrintLetterDelay calls.
    pub fn after_cont(
        kind: TerminalTextKind,
        retained_line: &str,
        new_line: &str,
        end: TerminalTextEnd,
        options_delay: u8,
    ) -> Self {
        assert!(!new_line.is_empty(), "CONT must append actual new text");
        let mut work = Self::new(
            kind,
            format!("{retained_line}\n{new_line}"),
            end,
            options_delay,
            0,
        );
        work.visible_chars = retained_line.chars().count();
        work
    }

    pub fn visible_text(&self) -> String {
        let mut left = self.visible_chars;
        self.text
            .chars()
            .take_while(|c| {
                if *c == '\n' {
                    return left != 0;
                }
                if left == 0 {
                    return false;
                }
                left -= 1;
                true
            })
            .collect()
    }

    fn glyphs(&self) -> usize {
        self.text.chars().filter(|c| *c != '\n').count()
    }

    /// One VBlank resume of this real text call. Creation itself does not
    /// consume a wait. The caller refreshes raw once before the canonical
    /// update and supplies the actual sequencer's sound completion.
    pub fn tick(&mut self, owner: &mut ForegroundJoypad, sound_playing: bool) -> TerminalTextStep {
        let mut result = TerminalTextStep::default();
        loop {
            match self.stage {
                TerminalTextStage::Preparation { remaining } => {
                    if remaining > 1 {
                        self.stage = TerminalTextStage::Preparation {
                            remaining: remaining - 1,
                        };
                        return result;
                    }
                    self.stage = TerminalTextStage::Printing;
                }
                TerminalTextStage::Printing => {
                    match self.letter_wait {
                        LetterWait::HeldVBlank => self.letter_wait = LetterWait::None,
                        LetterWait::Polling { remaining } => {
                            let poll = owner.poll();
                            if poll.held & (A | B) != 0 {
                                self.letter_wait = LetterWait::HeldVBlank;
                                return result;
                            }
                            if remaining > 1 {
                                self.letter_wait = LetterWait::Polling {
                                    remaining: remaining - 1,
                                };
                                return result;
                            }
                            self.letter_wait = LetterWait::None;
                        }
                        LetterWait::None => {}
                    }
                    if self.visible_chars == self.glyphs() {
                        self.stage = match self.end {
                            TerminalTextEnd::Done => TerminalTextStage::Complete,
                            TerminalTextEnd::Prompt => {
                                TerminalTextStage::ProtectedPrompt { remaining: 3 }
                            }
                            TerminalTextEnd::LearnedMove => TerminalTextStage::LearnedSound,
                        };
                        // The call just reached a wait; do not consume its
                        // first VBlank on the glyph's finishing update.
                        if matches!(self.stage, TerminalTextStage::ProtectedPrompt { .. }) {
                            return result;
                        }
                        continue;
                    }
                    self.visible_chars += 1;
                    let poll = owner.poll();
                    self.letter_wait = if poll.held & (A | B) != 0 {
                        LetterWait::HeldVBlank
                    } else if self.options_delay == 0 {
                        LetterWait::None
                    } else {
                        LetterWait::Polling {
                            remaining: self.options_delay,
                        }
                    };
                    if self.options_delay != 0 || poll.held & (A | B) != 0 {
                        return result;
                    }
                }
                TerminalTextStage::ProtectedPrompt { remaining } => {
                    if remaining > 1 {
                        self.stage = TerminalTextStage::ProtectedPrompt {
                            remaining: remaining - 1,
                        };
                        return result;
                    }
                    self.stage = TerminalTextStage::Prompt;
                }
                TerminalTextStage::LearnedSound => {
                    result.learned_sound = true;
                    self.stage = TerminalTextStage::WaitLearnedSound;
                    return result;
                }
                TerminalTextStage::WaitLearnedSound => {
                    if sound_playing {
                        return result;
                    }
                    self.stage = TerminalTextStage::Prompt;
                }
                TerminalTextStage::Prompt => {
                    let poll = owner.poll();
                    let Some(pressed) = poll.pressed else {
                        self.stage =
                            TerminalTextStage::Blocked(TerminalInputError::UnknownBeforePrompt);
                        result.input_error = Some(TerminalInputError::UnknownBeforePrompt);
                        return result;
                    };
                    if pressed & (A | B) == 0 {
                        return result;
                    }
                    result.press_ab = true;
                    self.stage = TerminalTextStage::Complete;
                }
                TerminalTextStage::Complete => {
                    result.complete = true;
                    return result;
                }
                TerminalTextStage::Blocked(error) => {
                    result.input_error = Some(error);
                    return result;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foreground_input::{SELECT, START, UP};

    fn frame(
        work: &mut TerminalTextWork,
        owner: &mut ForegroundJoypad,
        raw: u8,
        sound: bool,
    ) -> TerminalTextStep {
        owner.read_raw(raw);
        work.tick(owner, sound)
    }

    #[test]
    fn final_glyph_held_wait_has_no_poll_but_neutral_expiry_really_polls() {
        for delay in [0, 1, 3, 5] {
            let mut owner = ForegroundJoypad::default();
            let mut work = TerminalTextWork::new(
                TerminalTextKind::EvolvedInto,
                "X".into(),
                TerminalTextEnd::Done,
                delay,
                0,
            );
            frame(&mut work, &mut owner, A | SELECT, false);
            assert_eq!(owner.last(), Some(A | SELECT));
            assert!(frame(&mut work, &mut owner, START | UP, false).complete);
            assert_eq!(
                owner.last(),
                Some(A | SELECT),
                "held DelayFrame is not a poll"
            );
            let mut owner = ForegroundJoypad::default();
            let mut work = TerminalTextWork::new(
                TerminalTextKind::EvolvedInto,
                "X".into(),
                TerminalTextEnd::Done,
                delay,
                0,
            );
            let first = frame(&mut work, &mut owner, SELECT, false);
            if delay == 0 {
                assert!(first.complete);
                assert_eq!(
                    owner.last(),
                    Some(SELECT),
                    "zero delay still executes the glyph's poll"
                );
            } else {
                for _ in 0..delay - 1 {
                    assert!(!frame(&mut work, &mut owner, UP, false).complete);
                }
                assert!(frame(&mut work, &mut owner, START, false).complete);
                assert_eq!(
                    owner.last(),
                    Some(START),
                    "normal expiry polls before checking the counter"
                );
            }
        }
    }

    #[test]
    fn prompt_waits_keep_last_then_poll_all_keys_and_ack_once_after_snapshot() {
        let mut owner = ForegroundJoypad::default();
        let mut work = TerminalTextWork::new(
            TerminalTextKind::Prize,
            "X".into(),
            TerminalTextEnd::Prompt,
            1,
            3,
        );
        for _ in 0..2 {
            frame(&mut work, &mut owner, A, false);
            assert_eq!(owner.last(), None);
        }
        frame(&mut work, &mut owner, A | SELECT, false);
        frame(&mut work, &mut owner, START, false); // Held glyph's VBlank; enter ProtectedDelay3.
        assert_eq!(owner.last(), Some(A | SELECT));
        for _ in 0..2 {
            frame(&mut work, &mut owner, START, false);
        }
        assert_eq!(owner.last(), Some(A | SELECT));
        let (mut work, mut owner): (TerminalTextWork, ForegroundJoypad) =
            serde_json::from_str(&serde_json::to_string(&(work, owner)).unwrap()).unwrap();
        let ack = frame(&mut work, &mut owner, B | START | UP, false);
        assert!(ack.complete && ack.press_ab);
        assert_eq!(owner.last(), Some(B | START | UP));
        assert_eq!(owner.pressed(), Some(B | START | UP));
        assert_eq!(owner.released(), Some(A | SELECT));
        assert!(!frame(&mut work, &mut owner, B | START | UP, false).press_ab);
    }

    #[test]
    fn learned_sound_wait_is_not_a_poll_or_a_second_protected_delay() {
        let mut owner = ForegroundJoypad::default();
        let mut work = TerminalTextWork::new(
            TerminalTextKind::Learned,
            "X".into(),
            TerminalTextEnd::LearnedMove,
            1,
            0,
        );
        frame(&mut work, &mut owner, B, false);
        let cue = frame(&mut work, &mut owner, START, false); // DONE, then its actual sound command.
        assert!(cue.learned_sound);
        for _ in 0..20 {
            assert!(!frame(&mut work, &mut owner, START, true).complete);
        }
        assert_eq!(owner.last(), Some(B));
        let ack = frame(&mut work, &mut owner, A | SELECT, false);
        assert!(ack.complete && ack.press_ab && !ack.learned_sound);
        assert_eq!(owner.last(), Some(A | SELECT));
    }

    #[test]
    fn lost_ledger_is_an_explicit_blocked_contract_not_a_synthetic_edge() {
        let mut owner = ForegroundJoypad::default();
        let mut work = TerminalTextWork::new(
            TerminalTextKind::Prize,
            "X".into(),
            TerminalTextEnd::Prompt,
            1,
            0,
        );
        frame(&mut work, &mut owner, B, false);
        frame(&mut work, &mut owner, 0, false);
        owner.retire_history(); // An invalid caller/snapshot handoff, not normal input.
        frame(&mut work, &mut owner, A, false);
        frame(&mut work, &mut owner, A, false);
        let failed = frame(&mut work, &mut owner, A, false);
        assert_eq!(
            failed.input_error,
            Some(TerminalInputError::UnknownBeforePrompt)
        );
        assert!(!failed.complete && !failed.press_ab);
        let last = owner.last();
        let still_blocked = frame(&mut work, &mut owner, START, false);
        assert_eq!(still_blocked.input_error, failed.input_error);
        assert_eq!(
            owner.last(),
            last,
            "error cannot silently resume using the exit raw sample"
        );
    }
}
