//! Regression: the LanguageSelect screen must keep the overworld script
//! engine language in sync with `config.language`, so NPC dialogue (`@t`
//! literals in map scenes) renders in the selected language.
//!
//! Before the fix the engine kept its construction-time language forever
//! (and the desktop constructor never set it at all), so picking Chinese
//! still produced English NPC speech.

use pokered_app::game::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, Lang};
use pokered_renderer::input::{GbButton, InputState};

#[test]
fn language_select_syncs_script_lang() {
    let mut game =
        PokemonGame::new_with_options(GameVersion::Red, None, None, None, false, None, false);
    game.state.screen = GameScreen::LanguageSelect;

    // Boot default: English everywhere.
    assert_eq!(game.state.config.language, Lang::En);
    assert_eq!(game.overworld.script_lang(), Some("en"));

    // One Down press toggles to Chinese and must re-sync the script engine.
    let mut down = InputState::new();
    down.press(GbButton::Down);
    game.update(&down);
    assert_eq!(game.state.config.language, Lang::Zh);
    assert_eq!(game.overworld.script_lang(), Some("zh"));

    // Toggling back to English re-syncs again.
    let mut up = InputState::new();
    up.press(GbButton::Up);
    game.update(&up);
    assert_eq!(game.state.config.language, Lang::En);
    assert_eq!(game.overworld.script_lang(), Some("en"));

    // Pick Chinese and confirm with A (which leaves the LanguageSelect
    // screen) — the sync must persist for the rest of the game.
    let mut down = InputState::new();
    down.press(GbButton::Down);
    game.update(&down);
    let mut a = InputState::new();
    a.press(GbButton::A);
    game.update(&a);
    assert_eq!(game.state.config.language, Lang::Zh);
    assert_eq!(game.overworld.script_lang(), Some("zh"));
}
