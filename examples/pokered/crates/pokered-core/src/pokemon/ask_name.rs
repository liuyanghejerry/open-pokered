use crate::naming_screen::{NamingInput, NamingScreenResult, NamingScreenState, NamingScreenType};
use pokered_data::species::Species;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskNamePhase {
    AskYesNo,
    Naming,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskNameResult {
    Waiting,
    ShowYesNoDialog,
    ShowNamingScreen,
    Finished(String),
}

#[derive(Debug, Clone)]
pub struct AskNameState {
    species: Species,
    phase: AskNamePhase,
    naming_screen: NamingScreenState,
}

impl AskNameState {
    pub fn new(species: Species) -> Self {
        Self {
            species,
            phase: AskNamePhase::AskYesNo,
            naming_screen: NamingScreenState::new(NamingScreenType::Pokemon),
        }
    }

    pub fn species(&self) -> Species {
        self.species
    }

    pub fn phase(&self) -> &AskNamePhase {
        &self.phase
    }

    pub fn naming_screen(&self) -> &NamingScreenState {
        &self.naming_screen
    }

    pub fn species_name(&self) -> String {
        format!("{:?}", self.species).to_uppercase()
    }

    pub fn update_yesno(&mut self, yes_selected: bool, no_selected: bool) -> AskNameResult {
        if self.phase != AskNamePhase::AskYesNo {
            return AskNameResult::Waiting;
        }

        if yes_selected {
            self.phase = AskNamePhase::Naming;
            return AskNameResult::ShowNamingScreen;
        }

        if no_selected {
            self.phase = AskNamePhase::Done;
            return AskNameResult::Finished(self.species_name());
        }

        AskNameResult::ShowYesNoDialog
    }

    pub fn update_naming(&mut self, input: NamingInput, is_zh: bool) -> AskNameResult {
        if self.phase != AskNamePhase::Naming {
            return AskNameResult::Waiting;
        }

        match self.naming_screen.update_frame(input, is_zh) {
            NamingScreenResult::Editing => AskNameResult::ShowNamingScreen,
            NamingScreenResult::Submitted(name) => {
                self.phase = AskNamePhase::Done;
                AskNameResult::Finished(name)
            }
            NamingScreenResult::Cancelled => {
                self.phase = AskNamePhase::Done;
                AskNameResult::Finished(self.species_name())
            }
        }
    }

    pub fn is_done(&self) -> bool {
        self.phase == AskNamePhase::Done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_name_new() {
        let state = AskNameState::new(Species::Pikachu);
        assert_eq!(state.species(), Species::Pikachu);
        assert_eq!(state.phase(), &AskNamePhase::AskYesNo);
    }

    #[test]
    fn ask_name_declined_uses_species_name() {
        let mut state = AskNameState::new(Species::Charmander);
        let result = state.update_yesno(false, true);
        assert_eq!(result, AskNameResult::Finished("CHARMANDER".to_string()));
        assert!(state.is_done());
    }

    #[test]
    fn ask_name_accepted_enters_naming() {
        let mut state = AskNameState::new(Species::Squirtle);
        let result = state.update_yesno(true, false);
        assert_eq!(result, AskNameResult::ShowNamingScreen);
        assert_eq!(state.phase(), &AskNamePhase::Naming);
    }

    #[test]
    fn ask_name_naming_submitted() {
        let mut state = AskNameState::new(Species::Bulbasaur);
        state.phase = AskNamePhase::Naming;

        state.naming_screen.update_frame(NamingInput {
            a: true,
            ..NamingInput::none()
        }, false);

        let result = state.update_naming(NamingInput {
            start: true,
            ..NamingInput::none()
        }, false);

        if let AskNameResult::Finished(name) = result {
            assert_eq!(name, "A");
        } else {
            panic!("Expected Finished result");
        }
        assert!(state.is_done());
    }

    #[test]
    fn ask_name_naming_cancelled_uses_species_name() {
        let mut state = AskNameState::new(Species::Bulbasaur);
        state.phase = AskNamePhase::Naming;

        let result = state.update_naming(NamingInput {
            start: true,
            ..NamingInput::none()
        }, false);

        assert_eq!(result, AskNameResult::Finished("BULBASAUR".to_string()));
        assert!(state.is_done());
    }
}
