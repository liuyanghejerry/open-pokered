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

/// Extra held-only frames kept after a terminal action button press/repeat
/// (`A`/`B`/`Start`/`Select`). `OverworldLoop` polls every 2 VBlanks, so a
/// 1-frame terminal pulse otherwise drops ~50% of overworld presses. Holding
/// for 2 extra frames (3 total: 1 edge + 2 held-only) guarantees the field
/// loop observes every tap without double-triggering edge-driven menus.
const ACTION_HOLD_LATCH_FRAMES: u8 = 2;

/// Extra held-only frames kept after a terminal D-pad press/repeat
/// (`Up`/`Down`/`Left`/`Right`). Bridges typical 30–50ms OS key-repeat gaps
/// (~2–3 frames at 60 Hz) so walking, ledge jumps, and boulder pushes stay
/// continuous like desktop.
const DPAD_HOLD_LATCH_FRAMES: u8 = 4;

fn button_index(button: GbButton) -> usize {
    match button {
        GbButton::A => 0,
        GbButton::B => 1,
        GbButton::Select => 2,
        GbButton::Start => 3,
        GbButton::Right => 4,
        GbButton::Left => 5,
        GbButton::Up => 6,
        GbButton::Down => 7,
    }
}

fn is_dpad(button: GbButton) -> bool {
    matches!(
        button,
        GbButton::Right | GbButton::Left | GbButton::Up | GbButton::Down
    )
}

pub(crate) struct TerminalGame {
    game: PokemonGame,
    quit: Arc<AtomicBool>,
    hold_latch: [u8; 8],
}

impl TerminalGame {
    #[allow(dead_code)]
    pub(crate) fn new(version: GameVersion, quit: Arc<AtomicBool>) -> Self {
        Self::with_game(PokemonGame::new(version), quit)
    }

    pub(crate) fn with_game(game: PokemonGame, quit: Arc<AtomicBool>) -> Self {
        Self {
            game,
            quit,
            hold_latch: [0; 8],
        }
    }

    /// Translate per-frame terminal input into native `InputState` with short
    /// held-only latches so standard terminals (which emit no key-release
    /// events and repeat at 20–30 Hz) behave like desktop in the 2-VBlank
    /// overworld loop without double-firing edge-triggered menus.
    fn translate_input(&mut self, input: &dotzuki_tui::InputState<GbButton>) -> InputState {
        // Clear stale D-pad latches immediately when a new direction arrives so
        // rapid turns stay crisp.
        let any_new_dpad = [GbButton::Right, GbButton::Left, GbButton::Up, GbButton::Down]
            .into_iter()
            .any(|b| input.is_just_pressed(b));
        if any_new_dpad {
            for b in [GbButton::Right, GbButton::Left, GbButton::Up, GbButton::Down] {
                if !input.is_just_pressed(b) {
                    self.hold_latch[button_index(b)] = 0;
                }
            }
        }

        let mut current = 0u8;
        let mut previous = 0u8;
        for button in GbButton::ALL {
            let idx = button_index(button);
            if input.is_held(button) {
                current |= button.bit_mask();
                if !input.is_just_pressed(button) {
                    previous |= button.bit_mask();
                }
                self.hold_latch[idx] = if is_dpad(button) {
                    DPAD_HOLD_LATCH_FRAMES
                } else {
                    ACTION_HOLD_LATCH_FRAMES
                };
            } else if self.hold_latch[idx] > 0 {
                self.hold_latch[idx] -= 1;
                current |= button.bit_mask();
                previous |= button.bit_mask();
            }
        }
        let mut translated = InputState::new();
        translated.set_from_bitmask(previous);
        translated.begin_frame();
        translated.set_from_bitmask(current);
        translated
    }
}

/// Pure single-frame translation without hold latching (kept for direct edge
/// semantics verification).
#[cfg(test)]
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
        let translated = self.translate_input(input);
        self.game.update(&translated);
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

    #[test]
    fn dpad_hold_latch_bridges_repeat_gaps_and_clears_on_turn() {
        let mut host = host();
        let mut press_right = dotzuki_tui::InputState::new();
        press_right.press(GbButton::Right);
        let f0 = host.translate_input(&press_right);
        assert!(f0.is_just_pressed(GbButton::Right) && f0.is_held(GbButton::Right));

        // Intermediate frames with no terminal event keep Right held (not just_pressed).
        let empty = dotzuki_tui::InputState::new();
        for _ in 0..DPAD_HOLD_LATCH_FRAMES {
            let f = host.translate_input(&empty);
            assert!(f.is_held(GbButton::Right));
            assert!(!f.is_just_pressed(GbButton::Right));
        }
        // After the latch expires, Right releases cleanly.
        let expired = host.translate_input(&empty);
        assert!(!expired.is_held(GbButton::Right));

        // Pressing a new direction immediately clears the previous D-pad latch.
        let _ = host.translate_input(&press_right);
        let mut press_down = dotzuki_tui::InputState::new();
        press_down.press(GbButton::Down);
        let turn = host.translate_input(&press_down);
        assert!(!turn.is_held(GbButton::Right));
        assert!(turn.is_just_pressed(GbButton::Down) && turn.is_held(GbButton::Down));
    }

    #[test]
    fn action_hold_latch_crosses_two_vblank_overworld_wait_without_menu_double_fire() {
        // 1. Menu: a single Down press + idle frames moves the cursor only once.
        let mut menu_host = host();
        menu_host.game.state.screen = GameScreen::LanguageSelect;
        menu_host.game.state.config.language = Lang::En;
        let mut press_down = dotzuki_tui::InputState::new();
        press_down.press(GbButton::Down);
        menu_host.update(&press_down);
        assert_eq!(menu_host.game.state.config.language, Lang::Zh);
        let empty = dotzuki_tui::InputState::new();
        for _ in 0..6 {
            menu_host.update(&empty);
        }
        assert_eq!(menu_host.game.state.config.language, Lang::Zh);

        // 2. Overworld: a single 1-frame Start tap opens the pause menu regardless
        // of which phase of OverworldLoop's 2-VBlank cadence it lands on.
        for idle_offset in [0, 1] {
            let game = PokemonGame::new_with_options(
                GameVersion::Red,
                None,
                None,
                None,
                true,
                Some("PalletTown,5,6".to_string()),
                false,
                true,
            );
            let mut ow_host = TerminalGame::with_game(game, Arc::new(AtomicBool::new(false)));
            for _ in 0..(40 + idle_offset) {
                ow_host.update(&empty);
            }
            assert_eq!(ow_host.game.state.screen, GameScreen::Overworld);
            let mut press_start = dotzuki_tui::InputState::new();
            press_start.press(GbButton::Start);
            ow_host.update(&press_start);
            for _ in 0..10 {
                ow_host.update(&empty);
            }
            assert_eq!(
                ow_host.game.state.screen,
                GameScreen::StartMenu,
                "1-frame Start tap dropped on overworld phase offset {idle_offset}"
            );
        }
    }
}
