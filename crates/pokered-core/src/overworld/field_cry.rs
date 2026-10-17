//! DisplayTextID far@ -> PlayCry/WaitForSoundToFinish -> silent outer ACK.
//! Far-entry producers: VermilionPidgeyHouse npc2 and the audited plain pets.
//! Inner PrintText callers, multi-part text and battle entrances keep their
//! separate continuations; sharing a cry species is not enough to use this flow.
use super::screen::OverworldAudioRequest;
use super::{BedroomDialogue, OverworldScreen};
use crate::alloc_prelude::*;
use crate::foreground_input::{A, B};
use dotzuki_engine::GameData;
use pokered_data::tilesets::TilesetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Start,
    Printing,
    NativePagePrompt,
    SoundWait,
    OuterAck,
    CloseWait,
    Done,
}

/// Only the caller's continuation and payload live here. Raw/last/edges and
/// the completed display cursor remain in the existing Overworld owner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldCryState {
    text: String,
    species: String,
    phase: Phase,
}

impl FieldCryState {
    pub fn new(text: String, species: String) -> Self {
        Self {
            text,
            species,
            phase: Phase::Start,
        }
    }
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    pub(super) fn tick_field_cry(&mut self, state: &mut FieldCryState) -> bool {
        match state.phase {
            Phase::Start => {
                // Ordinary npc talk already executed JoypadOverworld. Borrow
                // that ledger; unsupported snapshots keep unknown history.
                self.begin_source_field();
                let names: Vec<&str> = core::iter::once(self.player_name.as_str())
                    .chain(core::iter::once(self.rival_name.as_str()))
                    .chain(self.script_dialogue_names.iter().map(String::as_str))
                    .collect();
                let mut dialogue =
                    super::script_bridge::text_to_dialogue_with_names(&state.text, &names);
                dialogue.set_text_delay_frames(self.text_delay_frames);
                dialogue.begin_far_field_display();
                self.pending_dialogue = Some(dialogue);
                self.last_script_dialogue = None;
                self.retain_field_text = true;
                state.phase = Phase::Printing;
            }
            Phase::Printing => {
                if self.source_foreground().is_none() {
                    self.block_source_foreground(
                        crate::foreground_input::ForegroundInputError::MissingFieldOwner,
                    );
                    return false;
                }
                let owner = self.source_foreground.as_mut().unwrap();
                let Some(dialogue) = self.pending_dialogue.as_mut() else {
                    return false;
                };
                if !dialogue.waiting_for_input() {
                    dialogue.reveal_next_char_with_source(self.no_text_delay, owner);
                }
                if !dialogue.waiting_for_input() {
                    return false;
                }
                if dialogue.has_more_pages() {
                    // Extra native layout pages retain their explicit ACK.
                    state.phase = Phase::NativePagePrompt;
                } else {
                    // Literal @ returns without an inner ACK. Dispatch once,
                    // then yield so App can start its real SFX sequencer.
                    self.audio_requests.push(OverworldAudioRequest::PlayCry {
                        species: state.species.clone(),
                    });
                    state.phase = Phase::SoundWait;
                }
            }
            Phase::NativePagePrompt => {
                let Some(dialogue) = self.pending_dialogue.as_mut() else {
                    return false;
                };
                if !dialogue.source_prompt_ready(true) {
                    return false;
                }
                let Some(owner) = self.source_foreground.as_mut() else {
                    return false;
                };
                let Some(pressed) = owner.poll().pressed else {
                    self.block_source_foreground(
                        crate::foreground_input::ForegroundInputError::UnknownBeforePrompt,
                    );
                    return false;
                };
                if pressed & (A | B) != 0 {
                    self.sfx_event = super::screen::OverworldSfxEvent::TextAdvance;
                    dialogue.advance();
                    state.phase = Phase::Printing;
                }
            }
            Phase::SoundWait => {
                // Same actual App sequencer sample consumed by waitSound().
                // PlayCry's internal wait and the caller's explicit wait both
                // return on completion; neither polls Joypad or uses a timer.
                if !super::script_bridge::ScriptEffect::sound_wait_complete(self.script_sfx_playing)
                {
                    return false;
                }
                state.phase = Phase::OuterAck;
                return self.tick_field_cry(state);
            }
            Phase::OuterAck => {
                if self.tick_retained_field_outer_ack() {
                    self.clear_retained_field_text();
                    // CloseTextDisplay moves the window offscreen, then its
                    // DelayFrame returns before normal field input resumes.
                    state.phase = Phase::CloseWait;
                }
            }
            Phase::CloseWait => state.phase = Phase::Done,
            Phase::Done => {}
        }
        state.phase == Phase::Done
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod tests;
