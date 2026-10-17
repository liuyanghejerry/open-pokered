//! Ordinary field text consumes the same ledger retained by the Bike caller.
//! Legacy None/observed callers keep their old path; no text substring chooses
//! an inner terminator or substitutes a physical previous-frame key sample.
use super::screen::OverworldSfxEvent;
use super::{script_bridge, OverworldScreen};
use crate::alloc_prelude::*;
use crate::foreground_input::{A, B};
use dotzuki_engine::GameData;
use pokered_data::script_command::FieldTextPrompt;
use pokered_data::tilesets::TilesetId;

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    pub(super) fn tick_owned_field_text(
        &mut self,
        effect: &mut script_bridge::ScriptEffect,
    ) -> bool {
        let (text, prompt, outer) = match effect {
            script_bridge::ScriptEffect::ShowDialogue { text } => (text.as_str(), None, true),
            script_bridge::ScriptEffect::PrintFieldText { text, prompt, .. } => {
                (text.as_str(), Some(*prompt), false)
            }
            _ => unreachable!("typed field text caller"),
        };
        if self.pending_dialogue.is_none() {
            let names: Vec<&str> = core::iter::once(self.player_name.as_str())
                .chain(core::iter::once(self.rival_name.as_str()))
                .chain(self.script_dialogue_names.iter().map(String::as_str))
                .collect();
            let mut dialogue = script_bridge::text_to_dialogue_with_names(text, &names);
            dialogue.set_text_delay_frames(self.text_delay_frames);
            dialogue.begin_field_display(outer || self.last_script_dialogue.is_none());
            self.pending_dialogue = Some(dialogue);
            self.last_script_dialogue = None;
            self.retain_field_text = match effect {
                script_bridge::ScriptEffect::PrintFieldText { retain_window, .. } => *retain_window,
                _ => false,
            };
            return false;
        }
        if let script_bridge::ScriptEffect::PrintFieldText {
            leading_sound,
            leading_sound_started,
            ..
        } = effect
        {
            if let Some(sound_id) = leading_sound.as_ref() {
                let dialogue = self.pending_dialogue.as_mut().unwrap();
                if !*leading_sound_started {
                    if dialogue.tick_field_preparation() {
                        return false;
                    }
                    self.audio_requests
                        .push(super::screen::OverworldAudioRequest::PlaySound {
                            sound_id: sound_id.clone(),
                        });
                    *leading_sound_started = true;
                    return false;
                }
                if self.script_sfx_playing {
                    return false;
                }
                *leading_sound = None;
            }
        }
        let Some(owner) = self.source_foreground.as_mut() else {
            self.block_source_foreground(
                crate::foreground_input::ForegroundInputError::MissingFieldOwner,
            );
            return false;
        };
        let dialogue = self.pending_dialogue.as_mut().unwrap();
        if dialogue.holding_open() {
            // This outer loop really executes Joypad, including A release.
            if owner.poll().held & A != 0 {
                return false;
            }
            self.last_script_dialogue = self.pending_dialogue.take();
            return true;
        }
        if !dialogue.waiting_for_input() {
            dialogue.reveal_next_char_with_source(self.no_text_delay, owner);
            if !dialogue.waiting_for_input() {
                return false;
            }
        }
        if dialogue.is_last_page() && prompt == Some(FieldTextPrompt::Done) {
            self.last_script_dialogue = self.pending_dialogue.take();
            return true;
        }
        // Native layout page breaks keep their existing ACK boundary. The
        // explicit Bike PARA and its20 waits belong to BikeShopOfferState;
        // no original CONT/PARA provenance is inferred from wrapped pixels.
        let protected = dialogue.has_more_pages() || prompt == Some(FieldTextPrompt::Protected);
        if !dialogue.source_prompt_ready(protected) {
            return false;
        }
        let Some(pressed) = owner.poll().pressed else {
            self.block_source_foreground(
                crate::foreground_input::ForegroundInputError::UnknownBeforePrompt,
            );
            return false;
        };
        if pressed & (A | B) == 0 {
            return false;
        }
        if dialogue.has_more_pages() {
            self.sfx_event = OverworldSfxEvent::TextAdvance;
            dialogue.advance();
            return false;
        }
        if prompt.is_some() {
            self.sfx_event = OverworldSfxEvent::TextAdvance;
            self.last_script_dialogue = self.pending_dialogue.take();
            return true;
        }
        // DisplayTextID's final WaitForTextScrollButtonPress is silent; its
        // following HoldTextDisplayOpen is a separate real foreground poll.
        if owner.poll().held & A != 0 {
            dialogue.start_holding_open();
            false
        } else {
            self.last_script_dialogue = self.pending_dialogue.take();
            true
        }
    }
}
