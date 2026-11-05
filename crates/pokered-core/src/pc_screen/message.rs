//! PrintText continuations shared by the ordinary PC messages and warnings.
//! The renderer keeps the PC's existing font/layout; this owns letter waits,
//! authored paragraph/CONT acknowledgements, sound waits and DONE returns.
use super::*;
use crate::foreground_input::{A, B};
use crate::overworld::{BedroomDialogue, DialoguePage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MessageEnd { Prompt, Done }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MessageWait {
    Printing,
    Prompt,
    PageDelay(u8),
    BeforeSound,
    AfterSound,
    MainMenuDelay(u8),
}

impl PcScreen {
    /// Carry the actual preceding field poll into PrintText. A VBlank/raw
    /// sample alone must not manufacture a released button before the prompt.
    pub fn configure_field_text(&mut self, frames: u16, incoming_no_delay: bool,
        input: Option<crate::foreground_input::ForegroundJoypad>) {
        self.set_quantity_text_delay_frames(frames);
        self.text_no_delay = self.field_text_delay_change.unwrap_or(incoming_no_delay);
        if let Some(input) = input { self.rating_input = input; }
        if let Some(text) = self.message_text.as_mut() { text.set_text_delay_frames(frames); }
    }

    pub fn take_field_text_delay_change(&mut self) -> Option<bool> {
        self.field_text_delay_change.take()
    }

    pub(super) fn write_field_text_delay(&mut self, disabled: bool) {
        self.text_no_delay = disabled;
        self.item_text_no_delay = disabled;
        self.field_text_delay_change = Some(disabled);
    }

    pub fn message_page_lines(&self) -> &[String] {
        let (start, end) = self.message_pages.get(self.msg_page).copied().unwrap_or((0, 0));
        &self.msg_lines[start..end]
    }

    pub fn message_visible_chars(&self) -> Option<usize> {
        if self.rating_phase.is_some() { return self.rating_visible_chars(); }
        self.message_text.as_ref().map(|t| usize::from(t.char_index()))
    }

    /// Which ready menu remains underneath a warning PrintText call.
    pub fn message_underlay(&self) -> Option<PcPhase> {
        match self.msg_next {
            AfterMessage::ChangeBoxConfirm => Some(PcPhase::BillsMenu),
            AfterMessage::ReleaseConfirm => Some(PcPhase::MonList),
            AfterMessage::TossConfirm => Some(PcPhase::ItemList),
            _ => None,
        }
    }

    pub(super) fn prepare_message(&mut self, end: MessageEnd) {
        self.message_pages.clear();
        let mut start = 0;
        for (i, line) in self.msg_lines.iter().enumerate() {
            if line.is_empty() {
                if start < i { self.message_pages.push((start, i)); }
                start = i + 1;
            } else if i + 1 - start == MSG_LINES_PER_PAGE {
                self.message_pages.push((start, i + 1));
                start = i + 1;
            }
        }
        if start < self.msg_lines.len() { self.message_pages.push((start, self.msg_lines.len())); }
        if self.message_pages.is_empty() { self.message_pages.push((0, 0)); }
        self.message_end = end;
        self.message_wait_after_ack = false;
        self.start_message_page(true, false);
    }

    pub(super) fn start_message_page(&mut self, prepare: bool, continued: bool) {
        let lines = self.message_page_lines();
        let first = if continued { lines.first().cloned().unwrap_or_default() } else { String::new() };
        let rest = lines.iter().skip(usize::from(continued)).cloned().collect::<String>();
        let mut text = BedroomDialogue::from_pages(vec![DialoguePage {
            line1: first.into_boxed_str(), line2: rest.into_boxed_str(),
        }]);
        text.set_text_delay_frames(self.quantity_text_delay_frames);
        if prepare { text.begin_field_display(false); }
        if continued { text.retain_first_row_for_continuation(); }
        self.message_text = Some(Box::new(text));
        self.message_wait = Some(MessageWait::Printing);
    }

    pub(super) fn set_warning(&mut self, lines: Vec<String>, next: AfterMessage,
        end: MessageEnd, pages: &[(usize, usize)]) {
        self.set_message(lines, next);
        self.message_end = end;
        if self.language != crate::game_state::Lang::Zh && !pages.is_empty() {
            self.message_pages = pages.to_vec();
            self.start_message_page(true, false);
        }
    }

    pub(super) fn continue_message_lines(&mut self) {
        // These strings use LINE followed by CONT, not PARA. Retain the old
        // bottom line and reveal only the new lower line after the two scrolls.
        if self.language != crate::game_state::Lang::Zh && self.msg_lines.len() > 2 {
            self.message_pages = core::iter::once((0, 2))
                .chain((2..self.msg_lines.len()).map(|i| (i - 1, i + 1))).collect();
            self.start_message_page(true, false);
        }
    }

    pub(super) fn tick_message(&mut self, sound_playing: bool) -> PcScreenAction {
        let wait = self.message_wait.expect("ordinary PC text continuation");
        match wait {
            MessageWait::BeforeSound => {
                if sound_playing { return PcScreenAction::Continue; }
                self.message_wait = Some(MessageWait::Printing);
                return self.tick_message(false);
            }
            MessageWait::AfterSound => {
                if sound_playing { return PcScreenAction::Continue; }
                // ActivatePC reloads its saved background, then Delay3.
                self.message_wait = Some(MessageWait::MainMenuDelay(3));
            }
            MessageWait::MainMenuDelay(n) => {
                if n > 1 { self.message_wait = Some(MessageWait::MainMenuDelay(n - 1)); }
                else { self.advance_message(); }
            }
            MessageWait::PageDelay(n) => {
                if n > 1 { self.message_wait = Some(MessageWait::PageDelay(n - 1)); }
                else {
                    self.message_wait = Some(MessageWait::Printing);
                    return self.tick_message(sound_playing);
                }
            }
            MessageWait::Printing => {
                let text = self.message_text.as_mut().expect("PC text cursor");
                text.reveal_next_char_with_source(self.text_no_delay, &mut self.rating_input);
                if text.waiting_for_input() {
                    if self.msg_page + 1 == self.message_pages.len() && self.message_end == MessageEnd::Done {
                        self.advance_message();
                    } else {
                        // ManualTextScroll's initial Delay3 does not poll.
                        text.source_prompt_ready(true);
                        self.message_wait = Some(MessageWait::Prompt);
                    }
                }
            }
            MessageWait::Prompt => {
                if !self.message_text.as_mut().unwrap().source_prompt_ready(true) {
                    return PcScreenAction::Continue;
                }
                if self.rating_input.poll().pressed.is_some_and(|p| p & (A | B) != 0) {
                    self.sfx.push(PcSfx::TextAdvance);
                    if self.message_wait_after_ack && self.msg_page + 1 == self.message_pages.len() {
                        self.message_wait_after_ack = false;
                        self.message_wait = Some(MessageWait::AfterSound);
                    } else {
                        let exit = self.msg_next == AfterMessage::Exit
                            && self.msg_page + 1 == self.message_pages.len();
                        self.advance_message();
                        if exit { self.sfx.push(PcSfx::TurnOff); return PcScreenAction::Exit; }
                    }
                }
            }
        }
        PcScreenAction::Continue
    }
}
