//! Terminal host for the same game runtime used by the other frontends.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use pokered_app::PokemonGame;
use pokered_data::wild_data::GameVersion;
use pokered_renderer::{
    input::{GbButton, InputState},
    FrameBuffer,
};

pub(crate) struct TerminalGame {
    game: PokemonGame,
    quit: Arc<AtomicBool>,
}

impl TerminalGame {
    pub(crate) fn new(version: GameVersion, quit: Arc<AtomicBool>) -> Self {
        Self {
            game: PokemonGame::new(version),
            quit,
        }
    }
}

/// Preserve the terminal host's held/edge semantics, including repeat events.
/// Its loop clears both states each frame because most terminals do not send
/// key releases. Do not retain a native input state between terminal frames:
/// that would turn consecutive repeat events into held-only input.
fn runtime_input(input: &dotzuki_tui::InputState<GbButton>) -> InputState {
    let mut current = 0;
    let mut previous = 0;
    for button in GbButton::ALL {
        if input.is_held(button) {
            current |= button.bit_mask();
            if !input.is_just_pressed(button) {
                previous |= button.bit_mask();
            }
        }
    }
    let mut translated = InputState::new();
    translated.set_from_bitmask(previous);
    translated.begin_frame();
    translated.set_from_bitmask(current);
    translated
}

impl dotzuki_tui::TuiGame for TerminalGame {
    type Button = GbButton;
    type Fb = FrameBuffer;

    fn update(&mut self, input: &dotzuki_tui::InputState<GbButton>) {
        self.game.update(&runtime_input(input));
    }

    fn draw(&mut self, fb: &mut FrameBuffer) {
        self.game.draw(fb);
    }

    fn exit_requested(&self) -> bool {
        self.quit.load(Ordering::Relaxed) || self.game.should_exit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_tui::TuiGame;
    use pokered_core::game_state::{GameScreen, Lang};

    fn host() -> TerminalGame {
        let mut host = TerminalGame::new(GameVersion::Red, Arc::new(AtomicBool::new(false)));
        host.game.audio = None;
        host
    }

    #[test]
    fn all_buttons_preserve_held_and_press_edges() {
        let mut input = dotzuki_tui::InputState::new();
        for button in GbButton::ALL {
            input.press(button);
        }
        for held in [false, true] {
            if held {
                input.begin_frame();
            }
            let native = runtime_input(&input);
            for button in GbButton::ALL {
                assert_eq!(native.is_held(button), input.is_held(button));
                assert_eq!(
                    native.is_just_pressed(button),
                    input.is_just_pressed(button)
                );
            }
        }
        input.clear();
        let native = runtime_input(&input);
        assert!(!native.any_held());
        assert!(!native.any_just_pressed());
    }

    #[test]
    fn terminal_repeat_events_advance_menu_each_frame() {
        let mut host = host();
        host.game.state.screen = GameScreen::LanguageSelect;
        host.game.state.config.language = Lang::En;
        let mut input = dotzuki_tui::InputState::new();
        for expected in [Lang::Zh, Lang::En] {
            input.begin_frame();
            input.clear();
            input.press(GbButton::Down);
            host.update(&input);
            assert_eq!(host.game.state.config.language, expected);
        }
        input.clear();
        host.update(&input);
        assert_eq!(host.game.state.config.language, Lang::En);
        input.press(GbButton::A);
        host.update(&input);
        assert_eq!(host.game.state.screen, GameScreen::IntroScene);
    }

    #[test]
    fn boot_reaches_language_selection_through_terminal_adapter() {
        let mut host = host();
        for _ in 0..600 {
            host.update(&dotzuki_tui::InputState::new());
            if host.game.state.screen == GameScreen::LanguageSelect {
                return;
            }
        }
        panic!("boot did not reach language selection");
    }

    #[test]
    fn terminal_and_game_exit_requests_are_honored() {
        let mut host = host();
        assert!(!host.exit_requested());
        host.game.exit_requested = true;
        assert!(host.exit_requested());
        host.game.exit_requested = false;
        host.quit.store(true, Ordering::Relaxed);
        assert!(host.exit_requested());
    }
}
