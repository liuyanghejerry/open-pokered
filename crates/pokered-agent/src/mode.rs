//! [`AgentMode`] classification from the screen state machine plus
//! overworld sub-state.

use pokered_core::battle::BattlePhase;
use pokered_core::game_state::GameScreen;
use serde::{Deserialize, Serialize};

/// The high-level interaction mode an agent currently faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Player has control in the overworld (can walk and interact).
    Overworld,
    /// A dialogue box or choice prompt owns the overworld; advance with A.
    Dialogue,
    /// A menu screen owns input (start menu, bag, party, shop, ...).
    Menu,
    /// A battle is in progress.
    Battle,
    /// Map transition or script-driven cutscene: the game is busy and
    /// player input is frozen.
    Transition,
    /// Anything else (boot splashes, title, Oak's intro, ...).
    Unknown,
}

/// Overworld sub-state relevant to mode classification.
#[derive(Debug, Clone, Copy, Default)]
pub struct OverworldObs {
    /// A dialogue box is on screen (`pending_dialogue`).
    pub dialogue_open: bool,
    /// A script choice prompt is on screen (`pending_choice`).
    pub choice_open: bool,
    /// A storyline script or script effect owns the frame.
    pub script_running: bool,
    /// A warp/map transition is in flight (fade active or warp pending).
    pub warp_transition: bool,
}

/// Inputs to [`classify_mode`]. `battle_phase` rides along for future
/// finer battle sub-classification; v1 treats every battle phase alike.
pub struct ModeInput<'a> {
    pub screen: &'a GameScreen,
    pub overworld: OverworldObs,
    pub battle_phase: Option<&'a BattlePhase>,
}

/// Classify the current interaction mode. Battle wins outright; on the
/// overworld, an open dialogue/choice outranks transitions and scripts,
/// which outrank free control.
pub fn classify_mode(input: &ModeInput) -> AgentMode {
    let _ = input.battle_phase;
    match input.screen {
        GameScreen::Battle => AgentMode::Battle,
        GameScreen::Overworld => {
            if input.overworld.dialogue_open || input.overworld.choice_open {
                AgentMode::Dialogue
            } else if input.overworld.warp_transition || input.overworld.script_running {
                AgentMode::Transition
            } else {
                AgentMode::Overworld
            }
        }
        GameScreen::MainMenu
        | GameScreen::StartMenu
        | GameScreen::Bag
        | GameScreen::PartyScreen
        | GameScreen::PokemonStatsScreen(_)
        | GameScreen::TownMap
        | GameScreen::Pokedex
        | GameScreen::TrainerCard
        | GameScreen::PC
        | GameScreen::Shop(_)
        | GameScreen::Elevator
        | GameScreen::FilterBag
        | GameScreen::OptionsMenu
        | GameScreen::SaveMenu
        | GameScreen::Slots
        | GameScreen::Diploma => AgentMode::Menu,
        _ => AgentMode::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::items::{MartState, ShopInventory};

    fn classify(screen: &GameScreen, overworld: OverworldObs) -> AgentMode {
        classify_mode(&ModeInput {
            screen,
            overworld,
            battle_phase: None,
        })
    }

    fn idle() -> OverworldObs {
        OverworldObs::default()
    }

    #[test]
    fn battle_screen_is_battle() {
        assert_eq!(classify(&GameScreen::Battle, idle()), AgentMode::Battle);
    }

    #[test]
    fn plain_overworld_is_overworld() {
        assert_eq!(classify(&GameScreen::Overworld, idle()), AgentMode::Overworld);
    }

    #[test]
    fn dialogue_and_choice_are_dialogue() {
        let dialogue = OverworldObs {
            dialogue_open: true,
            ..idle()
        };
        assert_eq!(classify(&GameScreen::Overworld, dialogue), AgentMode::Dialogue);
        let choice = OverworldObs {
            choice_open: true,
            ..idle()
        };
        assert_eq!(classify(&GameScreen::Overworld, choice), AgentMode::Dialogue);
        // Dialogue outranks a running script / warp.
        let busy = OverworldObs {
            dialogue_open: true,
            script_running: true,
            warp_transition: true,
            ..idle()
        };
        assert_eq!(classify(&GameScreen::Overworld, busy), AgentMode::Dialogue);
    }

    #[test]
    fn script_and_warp_are_transition() {
        let script = OverworldObs {
            script_running: true,
            ..idle()
        };
        assert_eq!(classify(&GameScreen::Overworld, script), AgentMode::Transition);
        let warp = OverworldObs {
            warp_transition: true,
            ..idle()
        };
        assert_eq!(classify(&GameScreen::Overworld, warp), AgentMode::Transition);
    }

    #[test]
    fn menu_screens_are_menu() {
        for screen in [
            GameScreen::MainMenu,
            GameScreen::StartMenu,
            GameScreen::Bag,
            GameScreen::PartyScreen,
            GameScreen::PokemonStatsScreen(0),
            GameScreen::TownMap,
            GameScreen::Pokedex,
            GameScreen::TrainerCard,
            GameScreen::PC,
            GameScreen::OptionsMenu,
            GameScreen::SaveMenu,
            GameScreen::Slots,
            GameScreen::Elevator,
            GameScreen::FilterBag,
            GameScreen::Diploma,
        ] {
            assert_eq!(classify(&screen, idle()), AgentMode::Menu, "{screen:?}");
        }
        let shop = GameScreen::Shop(MartState::new(ShopInventory::new(vec![])));
        assert_eq!(classify(&shop, idle()), AgentMode::Menu);
    }

    #[test]
    fn boot_and_title_are_unknown() {
        for screen in [
            GameScreen::GameFreakSplash,
            GameScreen::CopyrightSplash,
            GameScreen::LanguageSelect,
            GameScreen::IntroScene,
            GameScreen::TitleScreen,
            GameScreen::OakSpeech,
        ] {
            assert_eq!(classify(&screen, idle()), AgentMode::Unknown, "{screen:?}");
        }
    }

    #[test]
    fn mode_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&AgentMode::Overworld).unwrap(), "\"overworld\"");
        assert_eq!(serde_json::to_string(&AgentMode::Transition).unwrap(), "\"transition\"");
    }
}
