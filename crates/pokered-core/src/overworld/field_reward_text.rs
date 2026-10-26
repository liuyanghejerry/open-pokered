//! Inner gift text keeps one field cursor through PARA, fanfare and the
//! caller's flag write. DisplayTextID's outer ACK is a separate continuation.
use super::screen::{OverworldAudioRequest, OverworldSfxEvent};
use super::{BedroomDialogue, DialoguePage, OverworldScreen};
use crate::alloc_prelude::*;
use crate::foreground_input::{A, B};
use dotzuki_engine::GameData;
use pokered_data::{script_command::FieldTextPrompt, tilesets::TilesetId};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum Ending {
    Inner(FieldTextPrompt),
    Sound(String),
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Prompt {
    NativePage,
    Paragraph,
    FirstParagraph,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Start,
    Printing,
    Protected { remaining: u8, next: Prompt },
    Prompt(Prompt),
    ParagraphClear { remaining: u8 },
    FirstParagraphClear { remaining: u8 },
    SoundWait,
    OuterAck,
    CloseWait,
    Done,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldRewardTextState {
    parts: Vec<String>,
    part: usize,
    ending: Ending,
    phase: Phase,
    #[serde(default)]
    continues_text: bool,
}

impl FieldRewardTextState {
    pub fn parts(parts: Vec<String>, prompt: FieldTextPrompt) -> Self {
        assert!(!parts.is_empty(), "validated explicit paragraph payload");
        Self {
            parts,
            part: 0,
            ending: Ending::Inner(prompt),
            phase: Phase::Start,
            continues_text: false,
        }
    }

    pub fn continuation(parts: Vec<String>, prompt: FieldTextPrompt) -> Self {
        let mut state = Self::parts(parts, prompt);
        state.continues_text = true;
        state
    }

    pub fn receipt(text: String, sound: String) -> Self {
        Self {
            parts: vec![text],
            part: 0,
            ending: Ending::Sound(sound),
            phase: Phase::Start,
            continues_text: false,
        }
    }

    pub fn outer_ack() -> Self {
        Self {
            parts: Vec::new(),
            part: 0,
            ending: Ending::Outer,
            phase: Phase::Start,
            continues_text: false,
        }
    }

    pub fn label(&self) -> &'static str {
        match self.ending {
            Ending::Inner(_) if self.continues_text => "ContinueFieldTextParts",
            Ending::Inner(_) => "PrintFieldTextParts",
            Ending::Sound(_) => "PrintItemFieldText",
            Ending::Outer => "AcknowledgeFieldText",
        }
    }

    pub fn debug_phase(&self) -> String {
        format!("{:?}", self.phase)
    }

    pub(super) fn needs_button(&self) -> bool {
        matches!(
            self.phase,
            Phase::Protected { .. } | Phase::Prompt(_) | Phase::OuterAck
        )
    }

    pub(super) fn shows_arrow(&self) -> bool {
        matches!(self.phase, Phase::Protected { .. } | Phase::Prompt(_))
    }

    pub(super) fn resolve_texts(&mut self, mut resolve: impl FnMut(&str) -> String) {
        for text in &mut self.parts {
            *text = resolve(text);
        }
    }
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    fn reward_text_part(&mut self, state: &FieldRewardTextState, paragraph: bool) {
        let names: Vec<&str> = core::iter::once(self.player_name.as_str())
            .chain(core::iter::once(self.rival_name.as_str()))
            .chain(self.script_dialogue_names.iter().map(String::as_str))
            .collect();
        let mut text =
            super::script_bridge::text_to_dialogue_with_names(&state.parts[state.part], &names);
        text.set_text_delay_frames(self.text_delay_frames);
        if !paragraph {
            text.begin_field_display(self.last_script_dialogue.is_none());
        }
        self.pending_dialogue = Some(text);
        self.last_script_dialogue = None;
        self.retain_field_text = true;
    }

    fn reward_text_return(&mut self, state: &mut FieldRewardTextState) -> bool {
        self.last_script_dialogue = self.pending_dialogue.take();
        state.phase = Phase::Done;
        true
    }

    pub(super) fn tick_field_reward_text(&mut self, state: &mut FieldRewardTextState) -> bool {
        match state.phase {
            Phase::Start => {
                self.begin_source_field();
                if matches!(state.ending, Ending::Outer) {
                    if self.pending_dialogue.is_none() {
                        self.pending_dialogue = self.last_script_dialogue.take();
                    }
                    self.retain_field_text = true;
                    state.phase = Phase::OuterAck;
                    return self.tick_field_reward_text(state);
                }
                if state.continues_text {
                    // A leading PARA keeps the receipt visible until its own
                    // protected manual scroll; it must not open a new box.
                    if self.pending_dialogue.is_none() {
                        self.pending_dialogue = self.last_script_dialogue.take();
                    }
                    self.retain_field_text = true;
                    state.phase = Phase::Protected {
                        remaining: 3,
                        next: Prompt::FirstParagraph,
                    };
                    return false;
                }
                self.reward_text_part(state, false);
                state.phase = Phase::Printing;
            }
            Phase::Printing => {
                let Some(owner) = self.source_foreground.as_mut() else {
                    self.block_source_foreground(
                        crate::foreground_input::ForegroundInputError::MissingFieldOwner,
                    );
                    return false;
                };
                let Some(text) = self.pending_dialogue.as_mut() else {
                    return false;
                };
                if !text.waiting_for_input() {
                    text.reveal_next_char_with_source(self.no_text_delay, owner);
                }
                if !text.waiting_for_input() {
                    return false;
                }
                if text.has_more_pages() {
                    state.phase = Phase::Protected {
                        remaining: 3,
                        next: Prompt::NativePage,
                    };
                } else if state.part + 1 < state.parts.len() {
                    state.phase = Phase::Protected {
                        remaining: 3,
                        next: Prompt::Paragraph,
                    };
                } else {
                    match &state.ending {
                        Ending::Inner(FieldTextPrompt::Done) => {
                            return self.reward_text_return(state)
                        }
                        Ending::Inner(FieldTextPrompt::Protected) => {
                            state.phase = Phase::Protected {
                                remaining: 3,
                                next: Prompt::Return,
                            };
                        }
                        Ending::Inner(FieldTextPrompt::Button) => {
                            state.phase = Phase::Prompt(Prompt::Return);
                            return self.tick_field_reward_text(state);
                        }
                        Ending::Sound(sound_id) => {
                            self.audio_requests.push(OverworldAudioRequest::PlaySound {
                                sound_id: sound_id.clone(),
                            });
                            // Yield once so App starts the actual sequencer before
                            // this caller consumes its completion sample.
                            state.phase = Phase::SoundWait;
                        }
                        Ending::Outer => unreachable!("outer ACK never prints again"),
                    }
                }
            }
            Phase::Protected { remaining, next } => {
                if remaining > 1 {
                    state.phase = Phase::Protected {
                        remaining: remaining - 1,
                        next,
                    };
                } else {
                    state.phase = Phase::Prompt(next);
                    return self.tick_field_reward_text(state);
                }
            }
            Phase::Prompt(next) => {
                let Some(owner) = self.source_foreground_mut() else {
                    self.block_source_foreground(
                        crate::foreground_input::ForegroundInputError::MissingFieldOwner,
                    );
                    return false;
                };
                let Some(pressed) = owner.poll().pressed else {
                    self.block_source_foreground(
                        crate::foreground_input::ForegroundInputError::UnknownBeforePrompt,
                    );
                    return false;
                };
                if pressed & (A | B) == 0 {
                    return false;
                }
                self.sfx_event = OverworldSfxEvent::TextAdvance;
                match next {
                    Prompt::NativePage => {
                        self.pending_dialogue.as_mut().unwrap().advance();
                        state.phase = Phase::Printing;
                    }
                    Prompt::Paragraph | Prompt::FirstParagraph => {
                        self.pending_dialogue = None;
                        self.last_script_dialogue =
                            Some(BedroomDialogue::from_pages(vec![DialoguePage {
                                line1: "".into(),
                                line2: "".into(),
                            }]));
                        state.phase = if next == Prompt::FirstParagraph {
                            Phase::FirstParagraphClear { remaining: 20 }
                        } else {
                            Phase::ParagraphClear { remaining: 20 }
                        };
                    }
                    Prompt::Return => return self.reward_text_return(state),
                }
            }
            Phase::ParagraphClear { remaining } | Phase::FirstParagraphClear { remaining } => {
                let first = matches!(state.phase, Phase::FirstParagraphClear { .. });
                if remaining > 1 {
                    state.phase = if first {
                        Phase::FirstParagraphClear {
                            remaining: remaining - 1,
                        }
                    } else {
                        Phase::ParagraphClear {
                            remaining: remaining - 1,
                        }
                    };
                } else {
                    if !first {
                        state.part += 1;
                    }
                    self.reward_text_part(state, true);
                    state.phase = Phase::Printing;
                    return self.tick_field_reward_text(state);
                }
            }
            Phase::SoundWait => {
                // WaitForSoundToFinish does not poll Joypad. Short taps made
                // entirely inside the fanfare cannot become an outer ACK.
                if !super::script_bridge::ScriptEffect::sound_wait_complete(self.script_sfx_playing)
                {
                    return false;
                }
                return self.reward_text_return(state);
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
