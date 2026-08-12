use crate::naming_screen::NamingInput;
use crate::pokemon::ask_name::{AskNameResult, AskNameState};
use crate::pokemon::party::Party;
use pokered_data::species::Species;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OaksLabStarterPhase {
    WaitingForChoice,
    PlayerChoseStarter { species: Species },
    AskNicknameYesNo,
    NamingPokemon,
    SetNickname { species: Species, nickname: String },
    Done,
}

#[derive(Debug, Clone)]
pub struct OaksLabStarterEvent {
    pub phase: OaksLabStarterPhase,
    pub player_party: Party,
    pub ask_name_state: Option<AskNameState>,
}

impl OaksLabStarterEvent {
    pub fn new() -> Self {
        Self {
            phase: OaksLabStarterPhase::WaitingForChoice,
            player_party: Party::new(),
            ask_name_state: None,
        }
    }

    pub fn select_starter(&mut self, species: Species) {
        if self.phase != OaksLabStarterPhase::WaitingForChoice {
            return;
        }

        match self.player_party.add_with_naming(species, 5) {
            Ok((_, ask_name)) => {
                self.phase = OaksLabStarterPhase::AskNicknameYesNo;
                self.ask_name_state = Some(ask_name);
            }
            Err(_) => {
                self.phase = OaksLabStarterPhase::Done;
            }
        }
    }

    pub fn update_yesno(&mut self, yes_selected: bool, no_selected: bool) {
        if self.phase != OaksLabStarterPhase::AskNicknameYesNo {
            return;
        }

        if let Some(state) = &mut self.ask_name_state {
            match state.update_yesno(yes_selected, no_selected) {
                AskNameResult::ShowNamingScreen => {
                    self.phase = OaksLabStarterPhase::NamingPokemon;
                }
                AskNameResult::Finished(nickname) => {
                    self.player_party.set_nickname(0, &nickname).ok();
                    self.phase = OaksLabStarterPhase::Done;
                    self.ask_name_state = None;
                }
                _ => {}
            }
        }
    }

    pub fn update_naming(&mut self, input: NamingInput, is_zh: bool) {
        if self.phase != OaksLabStarterPhase::NamingPokemon {
            return;
        }

        if let Some(state) = &mut self.ask_name_state {
            match state.update_naming(input, is_zh) {
                AskNameResult::Finished(nickname) => {
                    self.player_party.set_nickname(0, &nickname).ok();
                    self.phase = OaksLabStarterPhase::Done;
                    self.ask_name_state = None;
                }
                _ => {}
            }
        }
    }

    pub fn is_done(&self) -> bool {
        self.phase == OaksLabStarterPhase::Done
    }

    pub fn starter_species(&self) -> Option<Species> {
        self.player_party.leader().map(|p| p.species)
    }
}

impl Default for OaksLabStarterEvent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oaks_lab_event_new() {
        let event = OaksLabStarterEvent::new();
        assert_eq!(event.phase, OaksLabStarterPhase::WaitingForChoice);
        assert!(event.player_party.is_empty());
    }

    #[test]
    fn select_charmander_starter() {
        let mut event = OaksLabStarterEvent::new();
        event.select_starter(Species::Charmander);

        assert_eq!(event.phase, OaksLabStarterPhase::AskNicknameYesNo);
        assert_eq!(event.starter_species(), Some(Species::Charmander));
        assert_eq!(event.player_party.count(), 1);
    }

    #[test]
    fn decline_nickname_uses_species_name() {
        let mut event = OaksLabStarterEvent::new();
        event.select_starter(Species::Bulbasaur);
        event.update_yesno(false, true);

        assert_eq!(event.phase, OaksLabStarterPhase::Done);
        assert!(event.is_done());

        let pokemon = event.player_party.get(0).unwrap();
        let mut buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
        assert_eq!(pokemon.display_name(&mut buf), "BULBASAUR");
    }

    #[test]
    fn accept_nickname_enters_naming_screen() {
        let mut event = OaksLabStarterEvent::new();
        event.select_starter(Species::Squirtle);
        event.update_yesno(true, false);

        assert_eq!(event.phase, OaksLabStarterPhase::NamingPokemon);
    }

    #[test]
    fn submit_nickname_sets_custom_name() {
        let mut event = OaksLabStarterEvent::new();
        event.select_starter(Species::Pikachu);
        event.update_yesno(true, false);

        let input = NamingInput {
            a: true,
            ..NamingInput::none()
        };
        event.update_naming(input, false);

        let submit_input = NamingInput {
            start: true,
            ..NamingInput::none()
        };
        event.update_naming(submit_input, false);

        assert_eq!(event.phase, OaksLabStarterPhase::Done);

        let pokemon = event.player_party.get(0).unwrap();
        let mut buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
        assert_eq!(pokemon.display_name(&mut buf), "A");
    }
}
