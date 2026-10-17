//! Oak's inner rating PrintText/DONE, shared fanfare, silent ACK and outer close.
use super::OverworldScreen;
use super::screen::OverworldAudioRequest;
use crate::alloc_prelude::*;
use crate::foreground_input::{A, B};
use crate::pokedex_rating::{DexRatingAudio, RatingAudioStep};
use dotzuki_engine::GameData;
use pokered_data::tilesets::TilesetId;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum Phase { Start, Printing, PagePrompt, Audio(DexRatingAudio), Ack, ReleaseA, CloseWait, Done }
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PokedexRatingState { text: String, owned: u32, phase: Phase }
impl PokedexRatingState {
    pub fn new(text: String, owned: u32) -> Self { Self { text, owned, phase: Phase::Start } }
}
impl<G: GameData<Tileset=TilesetId>> OverworldScreen<G> {
    pub(super) fn tick_pokedex_rating(&mut self, state: &mut PokedexRatingState) -> bool {
        match &mut state.phase {
            Phase::Start => {
                self.begin_source_field();
                let mut text = super::script_bridge::text_to_dialogue(&state.text);
                text.set_text_delay_frames(self.text_delay_frames);
                // Greeting/counts already performed the outer transfers.
                text.begin_field_display(false);
                self.pending_dialogue = Some(text); self.last_script_dialogue = None;
                self.retain_field_text = true; state.phase = Phase::Printing;
            }
            Phase::Printing => {
                let Some(owner) = self.source_foreground.as_mut() else {
                    self.block_source_foreground(crate::foreground_input::ForegroundInputError::MissingFieldOwner);
                    return false;
                };
                let Some(text) = self.pending_dialogue.as_mut() else { return false; };
                if !text.waiting_for_input() { text.reveal_next_char_with_source(self.no_text_delay, owner); }
                if text.waiting_for_input() {
                    state.phase = if text.has_more_pages() { Phase::PagePrompt }
                        else { Phase::Audio(DexRatingAudio::new(state.owned)) };
                    if matches!(state.phase, Phase::Audio(_)) { return self.tick_pokedex_rating(state); }
                }
            }
            Phase::PagePrompt => {
                let Some(text) = self.pending_dialogue.as_mut() else { return false; };
                if !text.source_prompt_ready(true) { return false; }
                let Some(owner) = self.source_foreground.as_mut() else {
                    self.block_source_foreground(crate::foreground_input::ForegroundInputError::MissingFieldOwner);
                    return false;
                };
                let Some(pressed) = owner.poll().pressed else {
                    self.block_source_foreground(crate::foreground_input::ForegroundInputError::UnknownBeforePrompt);
                    return false;
                };
                if pressed & (A|B) != 0 {
                    self.sfx_event = super::screen::OverworldSfxEvent::TextAdvance;
                    text.advance(); state.phase = Phase::Printing;
                }
            }
            Phase::Audio(audio) => match audio.tick(self.script_sfx_playing) {
                RatingAudioStep::Play(sound) => {
                    self.audio_requests.push(OverworldAudioRequest::PlaySound { sound_id: "SFX_STOP_ALL_MUSIC".into() });
                    self.audio_requests.push(OverworldAudioRequest::PlaySound { sound_id: sound.script_id().into() });
                }
                RatingAudioStep::RestartMap => {
                    self.audio_requests.push(OverworldAudioRequest::RestartMapMusic { map: self.state.current_map });
                    state.phase = Phase::Ack;
                    return self.tick_pokedex_rating(state);
                },
                RatingAudioStep::Ready => { state.phase = Phase::Ack; return self.tick_pokedex_rating(state); },
                RatingAudioStep::Waiting => {},
            },
            Phase::Ack => {
                let Some(owner) = self.source_foreground.as_mut() else {
                    self.block_source_foreground(crate::foreground_input::ForegroundInputError::MissingFieldOwner);
                    return false;
                };
                let Some(pressed) = owner.poll().pressed else {
                    self.block_source_foreground(crate::foreground_input::ForegroundInputError::UnknownBeforePrompt);
                    return false;
                };
                if pressed & (A|B) != 0 {
                    if let Some(text) = self.pending_dialogue.as_mut() { text.start_holding_open(); }
                    state.phase = Phase::ReleaseA;
                    return self.tick_pokedex_rating(state);
                }
            }
            Phase::ReleaseA => {
                let Some(owner) = self.source_foreground.as_mut() else {
                    self.block_source_foreground(crate::foreground_input::ForegroundInputError::MissingFieldOwner);
                    return false;
                };
                if owner.poll().held & A == 0 {
                    self.last_script_dialogue = self.pending_dialogue.take();
                    self.clear_retained_field_text(); state.phase = Phase::CloseWait;
                }
            }
            Phase::CloseWait => state.phase = Phase::Done,
            Phase::Done => {},
        }
        matches!(state.phase, Phase::Done)
    }
}
