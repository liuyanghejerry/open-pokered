//! PrintText invoked from a retained party screen, before map restoration.
//! The party caller owns the screen and chooses when to execute CloseTextDisplay.
//! Text uses the same display cursor and Joypad ledger as ordinary field text;
//! ticking it never runs the map, actors, encounters or movement.
use super::{script_bridge::ScriptEffect, BedroomDialogue, OverworldScreen};
use crate::alloc_prelude::*;
use dotzuki_engine::GameData;
use pokered_data::{script_command::FieldTextPrompt, tilesets::TilesetId};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartyFieldText {
    effect: ScriptEffect,
    returned: bool,
}

impl PartyFieldText {
    pub fn returned(&self) -> bool {
        self.returned
    }
}

impl<G: GameData<Tileset = TilesetId>> OverworldScreen<G> {
    /// StartMenu_Pokemon already loaded the font and restored buffer1.
    /// PrintText itself performs Delay3, without DisplayTextID's font reload.
    pub fn begin_party_field_text(&mut self, text: &str) -> PartyFieldText {
        self.begin_source_field();
        let text = self.localize_message(text);
        let mut dialogue = BedroomDialogue::from_message(&text);
        dialogue.set_text_delay_frames(self.text_delay_frames);
        dialogue.begin_field_display(false);
        self.pending_dialogue = Some(dialogue);
        self.last_script_dialogue = None;
        self.retain_field_text = true;
        PartyFieldText {
            effect: ScriptEffect::PrintFieldText {
                text,
                retain_window: true,
                prompt: FieldTextPrompt::Protected,
                prompt_frames_remaining: 0,
                prompt_last_buttons: None,
                leading_sound: None,
                leading_sound_started: false,
            },
            returned: false,
        }
    }

    /// ReadJoypad refreshes raw each hardware frame. Only the existing
    /// PrintLetterDelay / prompt code may poll and establish hJoyLast; menu
    /// history is never reconstructed from the physical previous-frame key.
    pub fn tick_party_field_text(&mut self, state: &mut PartyFieldText, raw: u8) -> bool {
        self.sfx_event = super::screen::OverworldSfxEvent::None;
        let Some(owner) = self.source_foreground_mut() else {
            self.block_source_foreground(
                crate::foreground_input::ForegroundInputError::MissingFieldOwner,
            );
            return false;
        };
        owner.read_raw(raw);
        if self.source_foreground_error().is_some() {
            return false;
        }
        if !state.returned {
            state.returned = self.tick_owned_field_text(&mut state.effect);
        }
        state.returned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foreground_input::{A, B, DOWN};
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

    fn begin() -> (OverworldScreen<PokemonRedData>, PartyFieldText) {
        let mut field = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        field.suspend_source_foreground(); // actual START caller retires history
        field.set_no_text_delay(false);
        let text = field.begin_party_field_text("A blinding FLASH\nlights the area!");
        (field, text)
    }

    #[test]
    fn held_confirmation_cannot_skip_prompt_and_map_never_ticks() {
        let (mut field, mut text) = begin();
        let position = (field.state.player.x, field.state.player.y);
        let actors: Vec<_> = field.npc_states.iter().map(|n| (n.x, n.y)).collect();
        let frame = field.frame_counter;
        assert_eq!(field.source_foreground().unwrap().last(), None);
        // Delay3 contains no Joypad instruction and must not fabricate history.
        for _ in 0..2 {
            assert!(!field.tick_party_field_text(&mut text, A | DOWN));
            assert_eq!(field.source_foreground().unwrap().last(), None);
        }
        for _ in 0..600 {
            assert!(!field.tick_party_field_text(&mut text, A | DOWN));
        }
        assert!(field.pending_dialogue.as_ref().unwrap().waiting_for_input());
        assert_eq!(field.source_foreground().unwrap().last(), Some(A | DOWN));
        assert_eq!(field.frame_counter, frame);
        assert_eq!((field.state.player.x, field.state.player.y), position);
        assert_eq!(
            field
                .npc_states
                .iter()
                .map(|n| (n.x, n.y))
                .collect::<Vec<_>>(),
            actors
        );
        assert!(!field.tick_party_field_text(&mut text, 0));
        assert!(field.tick_party_field_text(&mut text, B));
        assert!(text.returned());
        assert!(field.pending_dialogue.is_none());
        assert!(
            field.displayed_field_dialogue().is_some(),
            "caller retains party text until restoration"
        );
        // Returning PrintText does not close the retained window or run the map.
        assert!(field.tick_party_field_text(&mut text, DOWN));
        assert_eq!((field.state.player.x, field.state.player.y), position);
    }

    #[test]
    fn unsupported_no_delay_does_not_forge_menu_history() {
        let (mut field, mut text) = begin();
        field.set_no_text_delay(true);
        for _ in 0..20 {
            assert!(!field.tick_party_field_text(&mut text, A));
        }
        // No PrintLetterDelay poll exists on this branch; an unmodeled menu
        // predecessor is explicitly unknown, never synthesized as neutral.
        assert!(field.source_foreground_error().is_some());
        assert!(!text.returned());
    }
}
