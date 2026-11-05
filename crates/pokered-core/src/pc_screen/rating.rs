//! PC-specific text/ACK ownership around the shared rating sound gate.
use super::*;
use crate::foreground_input::{A, B};
use crate::overworld::{BedroomDialogue, DialoguePage};
use crate::pokedex_rating::{DexRatingAudio, RatingAudioStep};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RatingPhase { Printing { closed: bool }, PagePrompt { closed: bool }, Scroll { frames: u8, closed: bool }, Audio(DexRatingAudio), Ack, ClosedPrompt }
impl PcScreen {
    pub(super) fn observe_rating_input(&mut self, buttons: u8) {
        self.rating_input.read_raw(buttons);
        // Outside this continuation keep the existing PC host's observed
        // history. Neither sound waits nor protected prompt delays poll.
        if self.rating_phase.is_none() && self.message_wait.is_none() && !self.closing_pc {
            self.rating_input.poll();
        }
    }
    pub(super) fn start_rating_text(&mut self, closed: bool, prepare: bool) {
        self.message_wait = None;
        self.message_text = None;
        let continued = !prepare && self.msg_page > 0
            && self.message_pages[self.msg_page].0 < self.message_pages[self.msg_page - 1].1;
        let lines = self.message_page_lines();
        let first = if continued { lines.first().cloned().unwrap_or_default() } else { String::new() };
        let flat = lines.iter().skip(usize::from(continued)).cloned().collect::<String>();
        // Only reuse the established PrintLetterDelay cursor. PC keeps its
        // existing four-row layout; line boundaries stay in msg_lines.
        let mut text = BedroomDialogue::from_pages(vec![DialoguePage {
            line1: first.into_boxed_str(), line2: flat.into_boxed_str(),
        }]);
        text.set_text_delay_frames(self.quantity_text_delay_frames);
        if prepare { text.begin_field_display(false); }
        if continued { text.retain_first_row_for_continuation(); }
        self.rating_text = Some(Box::new(text));
        self.rating_phase = Some(RatingPhase::Printing { closed });
    }
    pub(super) fn close_oaks_link(&mut self) {
        self.set_message(vec!["Closed link to".into(), "PROF.OAK's PC.".into()], AfterMessage::MainMenu);
        self.start_rating_text(true, true);
    }
    pub fn rating_visible_chars(&self) -> Option<usize> {
        self.rating_text.as_ref().map(|t| usize::from(t.char_index()))
    }
    pub fn rating_waiting_for_audio(&self) -> bool { matches!(self.rating_phase, Some(RatingPhase::Audio(_))) }
    pub fn rating_waiting_for_page(&self) -> bool { matches!(self.rating_phase, Some(RatingPhase::PagePrompt { .. })) }
    pub(super) fn tick_rating(&mut self, sound_playing: bool) {
        let phase = self.rating_phase.expect("owned rating continuation");
        match phase {
            RatingPhase::Printing { closed } => {
                let text = self.rating_text.as_mut().unwrap();
                text.reveal_next_char_with_source(false, &mut self.rating_input);
                if text.waiting_for_input() {
                    self.rating_phase = Some(if self.msg_page + 1 < self.message_page_count() {
                        RatingPhase::PagePrompt { closed }
                    } else if closed { RatingPhase::ClosedPrompt }
                    else { RatingPhase::Audio(DexRatingAudio::new(self.dex_owned)) });
                    if self.rating_waiting_for_audio() { self.tick_rating(sound_playing); }
                }
            }
            RatingPhase::PagePrompt { closed } => {
                if !self.rating_text.as_mut().unwrap().source_prompt_ready(true) { return; }
                if self.rating_input.poll().pressed.is_some_and(|p| p & (A|B) != 0) {
                    self.sfx.push(PcSfx::TextAdvance); self.msg_page += 1;
                    self.start_rating_text(closed, false);
                    self.rating_phase = Some(RatingPhase::Scroll { frames: 10, closed });
                }
            }
            RatingPhase::Scroll { frames, closed } => {
                if frames > 1 { self.rating_phase = Some(RatingPhase::Scroll { frames: frames - 1, closed }); }
                else { self.rating_phase = Some(RatingPhase::Printing { closed }); self.tick_rating(sound_playing); }
            }
            RatingPhase::Audio(mut audio) => {
                match audio.tick(sound_playing) {
                    RatingAudioStep::Play(sound) => self.sfx.push(PcSfx::PokedexRating(sound)),
                    RatingAudioStep::RestartMap => { self.sfx.push(PcSfx::RestartMapMusic); self.rating_phase = Some(RatingPhase::Ack); self.tick_rating(sound_playing); return; },
                    RatingAudioStep::Ready => { self.rating_phase = Some(RatingPhase::Ack); self.tick_rating(sound_playing); return; }
                    RatingAudioStep::Waiting => {},
                }
                self.rating_phase = Some(RatingPhase::Audio(audio));
            }
            RatingPhase::Ack => {
                if self.rating_input.poll().pressed.is_some_and(|p| p & (A|B) != 0) {
                    // DisplayDexRating's own WaitForTextScrollButtonPress is
                    // silent and does not wait for A release in this caller.
                    self.close_oaks_link();
                }
            }
            RatingPhase::ClosedPrompt => {
                if !self.rating_text.as_mut().unwrap().source_prompt_ready(true) { return; }
                if self.rating_input.poll().pressed.is_some_and(|p| p & (A|B) != 0) {
                    // ClosedOaksPCText has its own text_waitbutton.
                    self.sfx.push(PcSfx::TextAdvance);
                    self.rating_phase = None; self.rating_text = None;
                    self.enter_main_menu();
                }
            }
        }
    }
}
