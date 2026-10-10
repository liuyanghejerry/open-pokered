//! Pokered's authored mart text and the ownership of its DONE/PROMPT/CONT.
//! Transactions still run through the generic mart, after this text returns.
use crate::alloc_prelude::*;
use crate::main_menu::MenuInput;
use super::{MartPhase, MartTopChoice};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AfterText {
    Menu,
    BuyList,
    AnythingElse,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MartText {
    pub lines: Vec<String>,
    pub underlay: MartPhase,
    pub after: AfterText,
    prompt: bool,
    first_line: usize,
    chars: usize,
    intro: u8,
    letter_wait: u16,
    guard: u8,
    ready: bool,
    scroll_wait: u8,
    sound_wait: bool,
    sound_wait_lines: Vec<String>,
    arrow_visible: bool,
    arrow_wait: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TextUpdate { Printing, Scroll, Finished { acknowledged: bool } }

impl MartText {
    pub fn new(lines: Vec<String>, underlay: MartPhase, prompt: bool, after: AfterText) -> Self {
        Self { lines, underlay, after, prompt, first_line: 0, chars: 0, intro: 3,
            letter_wait: 0, guard: 0, ready: false, scroll_wait: 0, sound_wait: false, sound_wait_lines: Vec::new(),
            arrow_visible: true, arrow_wait: 40 }
    }

    pub fn wait_for_purchase_sound(&mut self, lines: Vec<String>) {
        self.sound_wait = true;
        self.sound_wait_lines = lines;
    }

    pub fn page(&self) -> &[String] {
        &self.lines[self.first_line..(self.first_line + 2).min(self.lines.len())]
    }

    pub fn visible_lines(&self) -> Vec<String> {
        if self.sound_wait { return self.sound_wait_lines.clone(); }
        let mut remaining = self.chars;
        self.page().iter().map(|line| {
            let text = line.chars().take(remaining).collect();
            remaining = remaining.saturating_sub(line.chars().count());
            text
        }).collect()
    }

    pub fn waiting(&self) -> bool { self.ready && self.guard == 0 && self.scroll_wait == 0 }

    pub fn prompt_arrow_visible(&self) -> bool {
        self.ready && self.after != AfterText::Exit && self.scroll_wait == 0 && self.arrow_visible
    }

    fn tick_prompt_arrow(&mut self) {
        // The original busy-polling loop toggles on 39/40-frame boundaries
        // in the matched mart recording. Use a local 40-frame presentation
        // clock, independent of map age; shared CPU/PPU phase is a separate
        // fidelity concern. This timer never advances or selects the menu.
        self.arrow_wait -= 1;
        if self.arrow_wait == 0 {
            self.arrow_visible = !self.arrow_visible;
            self.arrow_wait = 40;
        }
    }

    pub fn tick(&mut self, input: MenuInput, held_ab: bool, sound_playing: bool, delay: u16) -> TextUpdate {
        if self.sound_wait {
            if sound_playing { return TextUpdate::Printing; }
            self.sound_wait = false;
            self.sound_wait_lines.clear();
            return TextUpdate::Printing;
        }
        if self.intro != 0 {
            self.intro -= 1;
            if self.intro != 0 { return TextUpdate::Printing; }
        }
        if self.scroll_wait != 0 {
            self.scroll_wait -= 1;
            if self.scroll_wait != 0 { return TextUpdate::Printing; }
        }
        if self.letter_wait != 0 {
            if held_ab { self.letter_wait = 1; }
            self.letter_wait -= 1;
            if self.letter_wait != 0 { return TextUpdate::Printing; }
        }
        let total = self.page().iter().map(|line| line.chars().count()).sum();
        if self.chars < total {
            self.chars += 1;
            self.letter_wait = if held_ab { 1 } else { delay.max(1) };
            return TextUpdate::Printing;
        }
        let last = self.first_line + 2 >= self.lines.len();
        if !self.ready {
            self.ready = true;
            if last && !self.prompt { return TextUpdate::Finished { acknowledged: false }; }
            // PromptText/_ContText protect the first three frames.
            self.guard = if self.after == AfterText::Exit { 0 } else { 3 };
            return TextUpdate::Printing;
        }
        if self.guard != 0 { self.guard -= 1; return TextUpdate::Printing; }
        if self.after != AfterText::Exit { self.tick_prompt_arrow(); }
        if !(input.a || input.b) { return TextUpdate::Printing; }
        if last { return TextUpdate::Finished { acknowledged: true }; }
        // _CONT retains the lower line, then ScrollTextUpOneLine waits five
        // frames twice before printing the next line. No extra page dismissal.
        self.first_line += 1;
        self.chars = self.lines[self.first_line].chars().count();
        self.ready = false;
        self.arrow_visible = true;
        self.arrow_wait = 40;
        self.scroll_wait = 10;
        TextUpdate::Scroll
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ConfirmationWait {
    pub frames: u8,
    pub cancel: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SaleSoundWait {
    pub underlay: MartPhase,
    pub bag: Vec<(pokered_data::items::ItemId, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FieldFlow {
    pub delay: u16,
    pub text: Option<MartText>,
    pub retained_lines: Vec<String>,
    pub text_advance: bool,
    pub exit_wait_a: bool,
    pub confirmation_wait: Option<ConfirmationWait>,
    pub pending_purchase: bool,
    pub sale_sound_wait: Option<SaleSoundWait>,
}

impl FieldFlow {
    pub fn new(delay: u16) -> Self {
        Self { delay: delay.max(1), text: None, retained_lines: vec!["Hi there!".into(), "May I help you?".into()], text_advance: false, exit_wait_a: false, confirmation_wait: None, pending_purchase: false, sale_sound_wait: None }
    }

    pub fn print(&mut self, lines: &[&str], underlay: MartPhase, prompt: bool, after: AfterText) {
        self.text = Some(MartText::new(lines.iter().map(|line| (*line).into()).collect(), underlay, prompt, after));
        self.retained_lines.clear();
    }

    pub fn anything_else(&mut self, underlay: MartPhase) {
        self.print(&["Is there anything", "else I can do?"], underlay, false, AfterText::Menu);
    }

    pub fn reset_main(phase: &mut MartPhase) {
        *phase = MartPhase::MainMenu { cursor: MartTopChoice::Buy };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mart_prompt_arrow_blinks_without_advancing_and_exit_has_no_arrow() {
        let mut text = MartText::new(vec!["x".into()],
            MartPhase::MainMenu { cursor: MartTopChoice::Buy }, true, AfterText::Menu);
        text.ready = true;
        text.intro = 0;
        text.chars = 1;
        for _ in 0..39 {
            assert_eq!(text.tick(MenuInput::none(), false, false, 1), TextUpdate::Printing);
            assert!(text.prompt_arrow_visible());
        }
        text.tick(MenuInput::none(), false, false, 1);
        assert!(!text.prompt_arrow_visible());
        for _ in 0..39 {
            text.tick(MenuInput::none(), false, false, 1);
            assert!(!text.prompt_arrow_visible());
        }
        text.tick(MenuInput::none(), false, false, 1);
        assert!(text.prompt_arrow_visible());
        text.after = AfterText::Exit;
        assert!(text.waiting());
        assert!(!text.prompt_arrow_visible());
    }
}
