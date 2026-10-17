//! Plain pet text -> PlayCry/WaitForSoundToFinish -> silent outer ACK.
//! Far and inner PrintText entries retain their distinct preparation. Both
//! return from the final text without an inner ACK, then share the cry/close.
//! Multi-part text and battle entrances need their own continuations.
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Entry {
    #[default]
    Far,
    InnerPrintText,
}

/// Only the caller's continuation and payload live here. Raw/last/edges and
/// the completed display cursor remain in the existing Overworld owner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldCryState {
    text: String,
    species: String,
    // Already-published Far snapshots predate the explicit entry selector.
    #[serde(default)]
    entry: Entry,
    phase: Phase,
}

impl FieldCryState {
    pub fn new(text: String, species: String) -> Self {
        Self {
            text,
            species,
            entry: Entry::Far,
            phase: Phase::Start,
        }
    }

    pub fn inner_print_text(text: String, species: String) -> Self {
        Self {
            entry: Entry::InnerPrintText,
            ..Self::new(text, species)
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
                match state.entry {
                    Entry::Far => dialogue.begin_far_field_display(),
                    // These text_asm callers perform PrintText's own Delay3
                    // after DisplayTextIDInit's background/font transfers.
                    Entry::InnerPrintText => dialogue.begin_field_display(true),
                }
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
                    // Literal @ / inner DONE returns without an inner ACK.
                    // Dispatch once, then yield so App starts its real SFX.
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
