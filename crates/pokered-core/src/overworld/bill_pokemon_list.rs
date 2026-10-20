//! BillsHousePokemonList's explicit PrintText/HandleMenuInput/DisplayPokedex loop.
//! The field Joypad ledger survives text, direct-menu delays, cry and map reload.
use super::{
    screen::{
        OverworldAudioRequest, OverworldSfxEvent, PokedexEntryPresentation, PokedexEntryState,
    },
    script_bridge::{PendingChoice, ScriptEffect},
    OverworldScreen,
};
use crate::{
    alloc_prelude::*,
    foreground_input::{ForegroundInputError, A, B, DOWN, LEFT, RIGHT, SELECT, START, UP},
};
use dotzuki_engine::GameData;
use pokered_data::{script_command::FieldTextPrompt, tilesets::TilesetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Favorite,
    Question,
    CursorDelay(u8),
    Poll,
    White(u8),
    Cry,
    DataDelay(u8),
    PageClear(u8),
    Data,
    ReturnMap(u8),
    CancelHold,
    CloseWait,
    Done,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BillPokemonListState {
    texts: Vec<String>,
    text: Box<ScriptEffect>,
    phase: Phase,
    selected: u32,
    started: bool,
    saved_question: Option<super::BedroomDialogue>,
}

fn text_effect(text: String, prompt: FieldTextPrompt) -> Box<ScriptEffect> {
    Box::new(ScriptEffect::PrintFieldText {
        text,
        retain_window: true,
        prompt,
        prompt_frames_remaining: 0,
        prompt_last_buttons: None,
        leading_sound: None,
        leading_sound_started: false,
    })
}

impl BillPokemonListState {
    pub fn new(texts: Vec<String>) -> Self {
        Self {
            text: text_effect(texts[0].clone(), FieldTextPrompt::Protected),
            texts,
            phase: Phase::Favorite,
            selected: 0,
            started: false,
            saved_question: None,
        }
    }
    pub fn accepts_input(&self) -> bool {
        self.phase == Phase::Poll
    }
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    fn bill_question(&mut self, state: &mut BillPokemonListState) {
        self.set_no_text_delay(true);
        state.text = text_effect(state.texts[1].clone(), FieldTextPrompt::Done);
        let mut choice = PendingChoice::new(state.texts[2..].to_vec());
        choice.selected = state.selected;
        choice.cursor_visible = false;
        choice.input_enabled = false;
        self.pending_choice = Some(choice);
        state.phase = Phase::Question;
    }

    pub(super) fn tick_bill_pokemon_list(&mut self, state: &mut BillPokemonListState) -> bool {
        match state.phase {
            Phase::Favorite => {
                if !state.started {
                    self.begin_source_field();
                    state.started = true;
                }
                if self.tick_owned_field_text(&mut state.text) {
                    self.bill_question(state);
                }
                return false;
            }
            Phase::Question => {
                // DONE returns directly to HandleMenuInput, with no second ACK.
                if self.tick_owned_field_text(&mut state.text) {
                    self.pending_choice.as_mut().unwrap().cursor_visible = true;
                    state.phase = Phase::CursorDelay(3);
                }
                return false;
            }
            Phase::CursorDelay(n) if n > 1 => {
                state.phase = Phase::CursorDelay(n - 1);
                return false;
            }
            Phase::CursorDelay(_) => {
                self.pending_choice.as_mut().unwrap().input_enabled = true;
                state.phase = Phase::Poll;
            }
            Phase::White(n) if n > 1 => {
                // External ShowPokedexData Delay3, followed by the internal
                // white palette's Delay3. Neither samples Joypad.
                if n == 4 {
                    self.audio_requests
                        .push(OverworldAudioRequest::SetMasterVolume { volume: 3 });
                }
                state.phase = Phase::White(n - 1);
                return false;
            }
            Phase::White(_) => {
                let entry = self.pending_pokedex_entry.as_mut().unwrap();
                entry.presentation = PokedexEntryPresentation::CryHeader;
                // Audio1 temporarily restores full volume for the cry, then
                // restores the quiet data-screen music when its SFX ends.
                self.audio_requests
                    .push(OverworldAudioRequest::SetMasterVolume { volume: 7 });
                self.audio_requests.push(OverworldAudioRequest::PlayCry {
                    species: entry.species.clone(),
                });
                state.phase = Phase::Cry;
                // Yield once for the host to dispatch the real cry.
                return false;
            }
            Phase::Cry => {
                if self.script_sfx_playing {
                    return false;
                }
                self.audio_requests
                    .push(OverworldAudioRequest::SetMasterVolume { volume: 3 });
                let entry = self.pending_pokedex_entry.as_mut().unwrap();
                entry.presentation = PokedexEntryPresentation::Data;
                if entry.owned {
                    state.phase = Phase::DataDelay(3);
                    return false;
                }
                state.phase = Phase::Data;
            }
            Phase::DataDelay(n) if n > 1 => {
                state.phase = Phase::DataDelay(n - 1);
                return false;
            }
            Phase::DataDelay(_) => state.phase = Phase::Data,
            Phase::PageClear(n) if n > 1 => {
                state.phase = Phase::PageClear(n - 1);
                return false;
            }
            Phase::PageClear(_) => {
                let entry = self.pending_pokedex_entry.as_mut().unwrap();
                entry.page += 1;
                entry.presentation = PokedexEntryPresentation::Data;
                // PAGE clears the body and waits twenty VBlanks before the
                // next no-delay paragraph. Only another PAGE needs Delay3;
                // the final DEXEND returns directly to the silent final wait.
                if entry.page + 1 < entry.total_pages {
                    state.phase = Phase::DataDelay(3);
                    return false;
                }
                state.phase = Phase::Data;
            }
            Phase::ReturnMap(n) if n > 1 => {
                state.phase = Phase::ReturnMap(n - 1);
                return false;
            }
            Phase::ReturnMap(_) => {
                // The host marks seen only when this pending entry disappears,
                // after DisplayPokedex's ten ReloadMapData VBlanks.
                self.pending_pokedex_entry = None;
                self.last_script_dialogue = state.saved_question.take();
                self.retain_field_text = true;
                self.bill_question(state);
                return false;
            }
            Phase::CancelHold => {
                let Some(owner) = self.source_foreground.as_mut() else {
                    self.block_source_foreground(ForegroundInputError::MissingFieldOwner);
                    return false;
                };
                if owner.poll().held & A != 0 {
                    return false;
                }
                self.pending_choice = None;
                self.pending_dialogue = None;
                self.clear_retained_field_text();
                state.phase = Phase::CloseWait;
                return false;
            }
            Phase::CloseWait => {
                state.phase = Phase::Done;
                return true;
            }
            Phase::Done => return true,
            Phase::Poll | Phase::Data => {}
        }
        let Some(owner) = self.source_foreground.as_mut() else {
            self.block_source_foreground(ForegroundInputError::MissingFieldOwner);
            return false;
        };
        let Some(pressed) = owner.poll().pressed else {
            self.block_source_foreground(ForegroundInputError::UnknownBeforePrompt);
            return false;
        };
        if state.phase == Phase::Data {
            if pressed & (A | B) == 0 {
                return false;
            }
            let entry = self.pending_pokedex_entry.as_mut().unwrap();
            if entry.page + 1 < entry.total_pages {
                entry.presentation = PokedexEntryPresentation::PageClear;
                self.sfx_event = OverworldSfxEvent::TextAdvance;
                state.phase = Phase::PageClear(20);
            } else {
                // Final WaitForTextScrollButtonPress is silent; it does not
                // wait for A release or poll during ReloadMapData/DelayFrames.
                entry.presentation = PokedexEntryPresentation::ReturnMap;
                self.pending_choice = None;
                self.pending_dialogue = None;
                self.retain_field_text = false;
                self.set_no_text_delay(false);
                self.audio_requests
                    .push(OverworldAudioRequest::SetMasterVolume { volume: 7 });
                state.phase = Phase::ReturnMap(10);
            }
            return false;
        }
        if pressed & UP != 0 {
            state.selected = state.selected.saturating_sub(1);
        } else if pressed & DOWN != 0 {
            state.selected = (state.selected + 1).min(4);
        }
        if pressed & (A | B) != 0 {
            self.sfx_event = OverworldSfxEvent::TextAdvance;
            self.pending_choice.as_mut().unwrap().input_enabled = false;
            if pressed & B != 0 || state.selected == 4 {
                self.set_no_text_delay(false);
                state.phase = Phase::CancelHold;
                // HoldTextDisplayOpen samples Joypad in the same continuation.
                return self.tick_bill_pokemon_list(state);
            }
            let species = ["Eevee", "Flareon", "Jolteon", "Vaporeon"][state.selected as usize];
            // Buffer2 is restored only after DisplayPokedex returns. Do not
            // expose a hidden field choice/text owner during the full-screen UI.
            state.saved_question = self.last_script_dialogue.take();
            self.pending_choice = None;
            self.retain_field_text = false;
            self.pending_pokedex_entry = Some(PokedexEntryState {
                species: species.into(),
                page: 0,
                owned: false,
                total_pages: 0,
                presentation: PokedexEntryPresentation::White,
            });
            state.phase = Phase::White(6);
        } else if pressed & (UP | DOWN | LEFT | RIGHT | START | SELECT) != 0 {
            let choice = self.pending_choice.as_mut().unwrap();
            choice.selected = state.selected;
            choice.input_enabled = false;
            state.phase = Phase::CursorDelay(3);
        }
        false
    }
}

#[cfg(all(test, not(target_os = "none"), not(feature = "script-boa")))]
mod tests;
