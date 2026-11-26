//! LearnMove's two real returning text calls (learn_move.asm:76-95,186-190).
//! Trying/forget/abandon-confirm/HM-retry menus are earlier owners, not modeled
//! here. A caller starts this only after its actual learned/abandoned outcome.

use crate::alloc_prelude::*;
use crate::foreground_input::ForegroundJoypad;
use crate::text::terminal_input::{
    TerminalTextEnd, TerminalTextKind, TerminalTextStep, TerminalTextWork,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum LearnTerminalStage {
    Learned(TerminalTextWork),
    AbandonedIntro(TerminalTextWork),
    /// _ContText calls ScrollTextUpOneLine twice, each with five DelayFrames.
    AbandonedScroll {
        remaining: u8,
        previous: String,
    },
    AbandonedFinal(TerminalTextWork),
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnMoveTerminal {
    stage: LearnTerminalStage,
    final_line: String,
    options_delay: u8,
    final_text: String,
}

impl LearnMoveTerminal {
    /// LearnedMove1Text: resolved far text, sound_GET_ITEM_1 + WaitSound,
    /// then PROMPT_BUTTON (without ProtectedDelay3).
    pub fn learned(mon_name: &str, move_name: &str, options_delay: u8) -> Self {
        Self::learned_text(format!("{mon_name} learned\n{move_name}!"), options_delay)
    }

    /// The caller resolves its existing localization before creating work.
    pub fn learned_text(text: String, options_delay: u8) -> Self {
        Self {
            stage: LearnTerminalStage::Learned(TerminalTextWork::new(
                TerminalTextKind::Learned,
                text,
                TerminalTextEnd::LearnedMove,
                options_delay,
                3,
            )),
            final_line: String::new(),
            options_delay,
            final_text: String::new(),
        }
    }

    /// DidNotLearn is not a direct field return: its CONT prompt, two
    /// five-frame scrolls and final PROMPT all belong to this real text call.
    pub fn abandoned(mon_name: &str, move_name: &str, options_delay: u8) -> Self {
        Self::abandoned_parts(
            format!("{mon_name}\ndid not learn"),
            "did not learn".into(),
            format!("{move_name}!"),
            options_delay,
        )
    }

    /// Explicit resolved CONT parts; this does not infer an input command
    /// from labels, message contents or the number of generic UI options.
    pub fn abandoned_parts(
        intro: String,
        retained_line: String,
        final_line: String,
        options_delay: u8,
    ) -> Self {
        Self {
            stage: LearnTerminalStage::AbandonedIntro(TerminalTextWork::new(
                TerminalTextKind::DidNotLearn,
                intro,
                TerminalTextEnd::Prompt,
                options_delay,
                3,
            )),
            final_line,
            options_delay,
            final_text: retained_line,
        }
    }

    pub fn visible_text(&self) -> String {
        match &self.stage {
            LearnTerminalStage::Learned(work)
            | LearnTerminalStage::AbandonedIntro(work)
            | LearnTerminalStage::AbandonedFinal(work) => work.visible_text(),
            LearnTerminalStage::AbandonedScroll { previous, .. } => previous.clone(),
            LearnTerminalStage::Complete => self.final_text.clone(),
        }
    }

    pub fn waiting_for_input(&self) -> bool {
        match &self.stage {
            LearnTerminalStage::Learned(work)
            | LearnTerminalStage::AbandonedIntro(work)
            | LearnTerminalStage::AbandonedFinal(work) => work.waiting_for_input(),
            _ => false,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.stage == LearnTerminalStage::Complete
    }

    pub fn tick_with_input(
        &mut self,
        held_buttons: u8,
        sound_playing: bool,
        owner: &mut ForegroundJoypad,
    ) -> TerminalTextStep {
        owner.read_raw(held_buttons);
        let mut result = TerminalTextStep::default();
        match &mut self.stage {
            LearnTerminalStage::Learned(work) | LearnTerminalStage::AbandonedFinal(work) => {
                result = work.tick(owner, sound_playing);
                if result.complete {
                    self.final_text = work.visible_text();
                    self.stage = LearnTerminalStage::Complete;
                }
            }
            LearnTerminalStage::AbandonedIntro(work) => {
                result = work.tick(owner, sound_playing);
                if result.complete {
                    self.stage = LearnTerminalStage::AbandonedScroll {
                        remaining: 10,
                        previous: work.visible_text(),
                    };
                    result.complete = false;
                }
            }
            LearnTerminalStage::AbandonedScroll { remaining, .. } => {
                *remaining -= 1;
                if *remaining == 0 {
                    let mut work = TerminalTextWork::after_cont(
                        TerminalTextKind::DidNotLearn,
                        &self.final_text,
                        &self.final_line,
                        TerminalTextEnd::Prompt,
                        self.options_delay,
                    );
                    // The second ScrollTextUpOneLine returned on this VBlank;
                    // resume NextChar immediately, with no invented extra wait.
                    result = work.tick(owner, sound_playing);
                    self.stage = LearnTerminalStage::AbandonedFinal(work);
                }
            }
            LearnTerminalStage::Complete => result.complete = true,
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foreground_input::{A, B, SELECT, START};

    #[test]
    fn learned_result_waits_for_its_sound_then_real_prompt_button_updates_all_keys_once() {
        let mut text = LearnMoveTerminal::learned("BULBASAUR", "GROWL", 1);
        let mut owner = ForegroundJoypad::default();
        let mut cues = 0;
        for _ in 0..200 {
            let step = text.tick_with_input(B | SELECT, false, &mut owner);
            cues += u8::from(step.learned_sound);
            if step.learned_sound {
                break;
            }
        }
        assert_eq!(cues, 1);
        for _ in 0..10 {
            assert!(!text.tick_with_input(START, true, &mut owner).complete);
        }
        assert_eq!(owner.last(), Some(B | SELECT));
        let step = text.tick_with_input(A | START, false, &mut owner);
        assert!(step.complete && step.press_ab);
        assert_eq!(owner.last(), Some(A | START));
        assert!(!text.tick_with_input(A | START, false, &mut owner).press_ab);
    }

    #[test]
    fn abandoned_result_has_real_cont_and_final_prompt_with_no_polls_during_scroll() {
        let mut text = LearnMoveTerminal::abandoned("BULBASAUR", "GROWL", 1);
        let mut owner = ForegroundJoypad::default();
        for _ in 0..200 {
            text.tick_with_input(B, false, &mut owner);
            if text.waiting_for_input() {
                break;
            }
        }
        assert!(text.waiting_for_input());
        text.tick_with_input(0, false, &mut owner);
        let first = text.tick_with_input(A | SELECT, false, &mut owner);
        assert!(first.press_ab && !first.complete);
        let json = serde_json::to_string(&(text, owner)).unwrap();
        let (mut text, mut owner): (LearnMoveTerminal, ForegroundJoypad) =
            serde_json::from_str(&json).unwrap();
        for _ in 0..9 {
            text.tick_with_input(START, false, &mut owner);
            assert_eq!(owner.last(), Some(A | SELECT));
        }
        text.tick_with_input(START, false, &mut owner);
        assert_eq!(
            owner.last(),
            Some(START),
            "NextChar really polls after the second scroll"
        );
        for _ in 0..200 {
            text.tick_with_input(B, false, &mut owner);
            if text.waiting_for_input() {
                break;
            }
        }
        text.tick_with_input(0, false, &mut owner);
        let last = text.tick_with_input(A | START, false, &mut owner);
        assert!(last.complete && last.press_ab);
        assert_eq!(owner.last(), Some(A | START));
        assert_eq!(text.visible_text(), "did not learn\nGROWL!");
    }
}
