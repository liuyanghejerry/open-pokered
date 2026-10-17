//! VermilionCityMachopText: far literal @, cry/wait, then a second far
//! text_start payload with authored PARA and CONT, DONE and one outer ACK.
//! Parts are explicit source continuations, never inferred from text/layout.
use super::screen::OverworldAudioRequest;
use super::script_bridge::ScriptEffect;
use super::{BedroomDialogue, DialoguePage, OverworldScreen};
use crate::alloc_prelude::*;
use crate::foreground_input::{A, B};
use dotzuki_engine::GameData;
use pokered_data::tilesets::TilesetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Part {
    Initial,
    Paragraph,
    Tail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Prompt {
    Paragraph,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Start,
    Printing(Part),
    NativePagePrompt(Part),
    SoundWait,
    Protected { remaining: u8, next: Prompt },
    Prompt(Prompt),
    ParagraphClear { remaining: u8 },
    ScrollFirst { remaining: u8 },
    ScrollSecond { remaining: u8 },
    OuterAck,
    CloseWait,
    Done,
}

/// Boxed by ScriptEffect: three owned text parts do not grow its stack enum.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldCryContinuationState {
    texts: Vec<String>,
    species: String,
    phase: Phase,
}

impl FieldCryContinuationState {
    pub fn new(texts: Vec<String>, species: String) -> Self {
        assert_eq!(texts.len(), 3, "validated far/PARA/CONT payload");
        Self {
            texts,
            species,
            phase: Phase::Start,
        }
    }

    pub(super) fn needs_button(&self) -> bool {
        matches!(
            self.phase,
            Phase::NativePagePrompt(_)
                | Phase::Protected { .. }
                | Phase::Prompt(_)
                | Phase::OuterAck
        )
    }

    pub(super) fn shows_prompt_arrow(&self) -> bool {
        // PARA and CONT explicitly write ▼ before ProtectedDelay3. DONE
        // returns to the silent DisplayTextID ACK without drawing a new ▼.
        // Input readiness and authored cursor graphics are separate states.
        matches!(
            self.phase,
            Phase::NativePagePrompt(_) | Phase::Protected { .. } | Phase::Prompt(_)
        )
    }
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    /// The first ScrollTextUpOneLine holds its intermediate image for five
    /// frames. The second has already retained the old bottom row at the top.
    pub fn field_text_half_scroll(&self) -> bool {
        matches!(&self.active_script_effect,
            Some(ScriptEffect::FieldCryContinuation { state })
                if matches!(state.phase, Phase::ScrollFirst { .. }))
    }

    fn cry_continuation_text(&mut self, state: &mut FieldCryContinuationState, part: Part) {
        let index = match part {
            Part::Initial => 0,
            Part::Paragraph => 1,
            Part::Tail => 2,
        };
        let previous_row = if part == Part::Tail {
            self.pending_dialogue
                .as_ref()
                .and_then(|d| d.current())
                .map(|page| page.line2.to_string())
        } else {
            None
        };
        let text = if let Some(row) = previous_row {
            format!("{row}\n{}", state.texts[index])
        } else {
            state.texts[index].clone()
        };
        let names: Vec<&str> = core::iter::once(self.player_name.as_str())
            .chain(core::iter::once(self.rival_name.as_str()))
            .chain(self.script_dialogue_names.iter().map(String::as_str))
            .collect();
        let mut dialogue = super::script_bridge::text_to_dialogue_with_names(&text, &names);
        dialogue.set_text_delay_frames(self.text_delay_frames);
        match part {
            Part::Initial => dialogue.begin_far_field_display(),
            // PARA jumps to NextChar after Delay20, not PrintText/DisplayTextID.
            Part::Paragraph => {}
            // CONT moves the existing lower row up; only the new lower row
            // prints after both Scroll5 calls. No font/BG/PrintText prologue.
            Part::Tail => dialogue.retain_first_row_for_continuation(),
        }
        self.pending_dialogue = Some(dialogue);
        self.last_script_dialogue = None;
        self.retain_field_text = true;
    }

    fn cry_continuation_pressed(&mut self) -> Option<u8> {
        let Some(owner) = self.source_foreground_mut() else {
            self.block_source_foreground(
                crate::foreground_input::ForegroundInputError::MissingFieldOwner,
            );
            return None;
        };
        let pressed = owner.poll().pressed;
        if pressed.is_none() {
            self.block_source_foreground(
                crate::foreground_input::ForegroundInputError::UnknownBeforePrompt,
            );
        }
        pressed
    }

    pub(super) fn tick_field_cry_continuation(
        &mut self,
        state: &mut FieldCryContinuationState,
    ) -> bool {
        match state.phase {
            Phase::Start => {
                self.begin_source_field();
                self.cry_continuation_text(state, Part::Initial);
                state.phase = Phase::Printing(Part::Initial);
            }
            Phase::Printing(part) => {
                let Some(owner) = self.source_foreground.as_mut() else {
                    self.block_source_foreground(
                        crate::foreground_input::ForegroundInputError::MissingFieldOwner,
                    );
                    return false;
                };
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
                    // Native layout page ACKs are separate from authored
                    // PARA/CONT, whose continuations are explicit below.
                    state.phase = Phase::NativePagePrompt(part);
                } else {
                    match part {
                        Part::Initial => {
                            self.audio_requests.push(OverworldAudioRequest::PlayCry {
                                species: state.species.clone(),
                            });
                            // Yield before consuming the pre-dispatch false
                            // sample. Actual App playback owns this wait.
                            state.phase = Phase::SoundWait;
                        }
                        Part::Paragraph => {
                            state.phase = Phase::Protected {
                                remaining: 3,
                                next: Prompt::Continue,
                            }
                        }
                        Part::Tail => {
                            // Authored DONE returns to the same DisplayTextID;
                            // there is no inner prompt after the final glyph.
                            state.phase = Phase::OuterAck;
                            return self.tick_field_cry_continuation(state);
                        }
                    }
                }
            }
            Phase::NativePagePrompt(part) => {
                let Some(dialogue) = self.pending_dialogue.as_mut() else {
                    return false;
                };
                if !dialogue.source_prompt_ready(true) {
                    return false;
                }
                let Some(pressed) = self.cry_continuation_pressed() else {
                    return false;
                };
                if pressed & (A | B) != 0 {
                    self.sfx_event = super::screen::OverworldSfxEvent::TextAdvance;
                    self.pending_dialogue.as_mut().unwrap().advance();
                    state.phase = Phase::Printing(part);
                }
            }
            Phase::SoundWait => {
                if !ScriptEffect::sound_wait_complete(self.script_sfx_playing) {
                    return false;
                }
                // text_start itself neither prints nor polls. Its leading
                // PARA keeps the initial message visible through Protected3.
                state.phase = Phase::Protected {
                    remaining: 3,
                    next: Prompt::Paragraph,
                };
            }
            Phase::Protected { remaining, next } => {
                if remaining > 1 {
                    state.phase = Phase::Protected {
                        remaining: remaining - 1,
                        next,
                    };
                } else {
                    state.phase = Phase::Prompt(next);
                    return self.tick_field_cry_continuation(state);
                }
            }
            Phase::Prompt(next) => {
                let Some(pressed) = self.cry_continuation_pressed() else {
                    return false;
                };
                if pressed & (A | B) == 0 {
                    return false;
                }
                self.sfx_event = super::screen::OverworldSfxEvent::TextAdvance;
                state.phase = match next {
                    Prompt::Paragraph => {
                        self.pending_dialogue = None;
                        self.last_script_dialogue =
                            Some(BedroomDialogue::from_pages(vec![DialoguePage {
                                line1: "".into(),
                                line2: "".into(),
                            }]));
                        Phase::ParagraphClear { remaining: 20 }
                    }
                    Prompt::Continue => Phase::ScrollFirst { remaining: 5 },
                };
            }
            Phase::ParagraphClear { remaining } => {
                if remaining > 1 {
                    state.phase = Phase::ParagraphClear {
                        remaining: remaining - 1,
                    };
                } else {
                    self.cry_continuation_text(state, Part::Paragraph);
                    state.phase = Phase::Printing(Part::Paragraph);
                    return self.tick_field_cry_continuation(state);
                }
            }
            Phase::ScrollFirst { remaining } => {
                if remaining > 1 {
                    state.phase = Phase::ScrollFirst {
                        remaining: remaining - 1,
                    };
                } else {
                    self.cry_continuation_text(state, Part::Tail);
                    state.phase = Phase::ScrollSecond { remaining: 5 };
                }
            }
            Phase::ScrollSecond { remaining } => {
                if remaining > 1 {
                    state.phase = Phase::ScrollSecond {
                        remaining: remaining - 1,
                    };
                } else {
                    state.phase = Phase::Printing(Part::Tail);
                    return self.tick_field_cry_continuation(state);
                }
            }
            Phase::OuterAck => {
                if self.tick_retained_field_outer_ack() {
                    self.clear_retained_field_text();
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
