use crate::game_state::{GameScreen, MainMenuChoice, SaveFileSummary, ScreenAction};
use crate::main_menu::*;

fn make_save() -> SaveFileSummary {
    SaveFileSummary {
        player_name: vec![0x91, 0x84, 0x83], // "RED" in pokered charmap
        badges: 0b0000_0011,
        pokedex_owned: 42,
        play_time_hours: 12,
        play_time_minutes: 34,
        play_time_seconds: 56,
    }
}

fn skip_init_delay(menu: &mut MainMenuState) {
    for _ in 0..MENU_INIT_DELAY_FRAMES {
        menu.update_frame(MenuInput::none());
    }
}

#[test]
fn no_save_has_two_items() {
    let menu = MainMenuState::new(None);
    assert_eq!(menu.item_count(), 2);
    assert_eq!(menu.items[0], MainMenuItem::NewGame);
    assert_eq!(menu.items[1], MainMenuItem::Option);
    assert!(!menu.has_save);
}

#[test]
fn with_save_has_three_items() {
    let menu = MainMenuState::new(Some(make_save()));
    assert_eq!(menu.item_count(), 3);
    assert_eq!(menu.items[0], MainMenuItem::Continue);
    assert_eq!(menu.items[1], MainMenuItem::NewGame);
    assert_eq!(menu.items[2], MainMenuItem::Option);
    assert!(menu.has_save);
}

#[test]
fn init_delay_blocks_input() {
    let mut menu = MainMenuState::new(None);
    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Continue);
    assert!(menu.last_choice.is_none());
}

#[test]
fn cursor_down_wraps() {
    let mut menu = MainMenuState::new(None);
    skip_init_delay(&mut menu);
    assert_eq!(menu.cursor, 0);

    menu.update_frame(MenuInput {
        down: true,
        ..MenuInput::none()
    });
    assert_eq!(menu.cursor, 1);

    menu.update_frame(MenuInput {
        down: true,
        ..MenuInput::none()
    });
    assert_eq!(menu.cursor, 0);
}

#[test]
fn cursor_up_wraps() {
    let mut menu = MainMenuState::new(None);
    skip_init_delay(&mut menu);
    assert_eq!(menu.cursor, 0);

    menu.update_frame(MenuInput {
        up: true,
        ..MenuInput::none()
    });
    assert_eq!(menu.cursor, 1);
}

#[test]
fn b_returns_to_title() {
    let mut menu = MainMenuState::new(None);
    skip_init_delay(&mut menu);

    let action = menu.update_frame(MenuInput {
        b: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::TitleScreen));
    assert_eq!(menu.last_choice, Some(MainMenuChoice::BackToTitle));
}

#[test]
fn select_new_game_transitions_to_oak_speech() {
    let mut menu = MainMenuState::new(None);
    skip_init_delay(&mut menu);

    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::OakSpeech));
    assert_eq!(menu.last_choice, Some(MainMenuChoice::NewGame));
}

#[test]
fn select_option_transitions_to_options_menu() {
    let mut menu = MainMenuState::new(None);
    skip_init_delay(&mut menu);

    menu.update_frame(MenuInput {
        down: true,
        ..MenuInput::none()
    });
    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::OptionsMenu));
    assert_eq!(menu.last_choice, Some(MainMenuChoice::Option));
}

#[test]
fn select_continue_shows_info_then_loads_on_a() {
    let mut menu = MainMenuState::new(Some(make_save()));
    skip_init_delay(&mut menu);

    // A on CONTINUE shows the save-info panel (DisplayContinueGameInfo),
    // it does NOT load immediately.
    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Continue);
    assert!(menu.is_showing_continue_info());
    assert!(menu.last_choice.is_none());

    // The panel ignores input during its 30-frame display delay
    // (main_menu.asm:378-379).
    for _ in 0..CONTINUE_INFO_DELAY_FRAMES {
        let action = menu.update_frame(MenuInput {
            a: true,
            ..MenuInput::none()
        });
        assert_eq!(action, ScreenAction::Continue, "input during the delay");
        assert!(menu.is_showing_continue_info());
    }

    // A on the panel loads the game.
    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::Overworld));
    assert_eq!(menu.last_choice, Some(MainMenuChoice::Continue));
    assert!(!menu.is_showing_continue_info());
}

#[test]
fn continue_info_b_returns_to_menu() {
    let mut menu = MainMenuState::new(Some(make_save()));
    skip_init_delay(&mut menu);
    menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert!(menu.is_showing_continue_info());
    for _ in 0..CONTINUE_INFO_DELAY_FRAMES {
        menu.update_frame(MenuInput::none());
    }
    let action = menu.update_frame(MenuInput {
        b: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Continue);
    assert!(!menu.is_showing_continue_info());
    assert!(menu.last_choice.is_none(), "B backs out without loading");
}

#[test]
fn return_from_options_clears_choice() {
    let mut menu = MainMenuState::new(None);
    skip_init_delay(&mut menu);
    menu.update_frame(MenuInput {
        down: true,
        ..MenuInput::none()
    });
    menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(menu.last_choice, Some(MainMenuChoice::Option));

    menu.return_from_options();
    assert!(menu.last_choice.is_none());
}

#[test]
fn item_labels_no_save() {
    let menu = MainMenuState::new(None);
    assert_eq!(menu.item_labels(), vec!["NEW GAME", "OPTION"]);
}

#[test]
fn item_labels_with_save() {
    let menu = MainMenuState::new(Some(make_save()));
    assert_eq!(menu.item_labels(), vec!["CONTINUE", "NEW GAME", "OPTION"]);
}

#[test]
fn continue_info_lines_with_save() {
    let mut menu = MainMenuState::new(Some(make_save()));
    skip_init_delay(&mut menu);
    menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });

    let lines = menu.continue_info_lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], ("PLAYER", "RED".to_string()), "charmap-decoded name");
    assert_eq!(lines[1], ("BADGES", "2".to_string()));
    assert_eq!(lines[2], ("#DEX", "42".to_string()));
    assert_eq!(lines[3], ("TIME", "12:34".to_string()));
}

#[test]
fn badge_count_uses_popcount() {
    let save = SaveFileSummary {
        player_name: vec![],
        badges: 0b1111_1111,
        pokedex_owned: 0,
        play_time_hours: 0,
        play_time_minutes: 0,
        play_time_seconds: 0,
    };
    assert_eq!(save.badge_count(), 8);
}
