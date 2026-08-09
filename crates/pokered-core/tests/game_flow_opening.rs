mod game_flow_helpers;

use game_flow_helpers::*;
use pokered_core::game_state::*;
use pokered_core::main_menu::{MainMenuState, MenuInput};
use pokered_core::oak_speech::{OakSpeechInput, OakSpeechPhase, OakSpeechResult, OakSpeechState};
use pokered_core::title_screen::{TitlePhase, TitleScreenState};
use pokered_data::wild_data::GameVersion;

const OAK_SPEECH_MAX_STEPS: usize = 10_000;

fn oak_input_for_progress(speech: &OakSpeechState) -> OakSpeechInput {
    if speech.is_waiting_for_input() {
        OakSpeechInput {
            a: true,
            ..OakSpeechInput::none()
        }
    } else {
        OakSpeechInput::none()
    }
}

/// Advance through a SlidePic animation (OakSpeechSlidePicRight/Left,
/// oak_speech2.asm:67-160) if one is currently playing.
fn advance_slide_pic(speech: &mut OakSpeechState) {
    let mut steps = 0;
    while matches!(speech.phase, OakSpeechPhase::SlidePic { .. }) {
        steps += 1;
        assert!(steps <= OAK_SPEECH_MAX_STEPS, "slide pic did not finish");
        speech.update_frame(OakSpeechInput::none());
    }
}

#[test]
fn game_starts_at_gamefreak_splash() {
    let state = new_game_state_red();
    // Boot order: Game Freak shooting-star splash (PlayShootingStar,
    // engine/movie/intro.asm:305-341) — the copyright text is part of the
    // splash's first 180 frames — then LanguageSelect / intro / title.
    assert_eq!(state.screen, GameScreen::GameFreakSplash);
    assert_eq!(state.config.version, GameVersion::Red);
    assert!(state.config.battle_animation);
    assert_eq!(state.config.battle_style, BattleStyle::Shift);
    assert_eq!(state.config.text_speed, TextSpeed::Medium);
}

#[test]
fn title_screen_progresses_through_all_phases() {
    let mut title = TitleScreenState::new(GameVersion::Red);
    assert_eq!(title.phase, TitlePhase::Copyright);

    while title.phase == TitlePhase::Copyright {
        title.update_frame(false);
    }
    assert_eq!(title.phase, TitlePhase::Init);

    title.update_frame(false);
    assert_eq!(title.phase, TitlePhase::LogoBounce);

    while title.phase == TitlePhase::LogoBounce {
        title.update_frame(false);
    }
    assert_eq!(title.phase, TitlePhase::LogoPause);

    while title.phase == TitlePhase::LogoPause {
        title.update_frame(false);
    }
    assert_eq!(title.phase, TitlePhase::VersionScroll);

    while title.phase == TitlePhase::VersionScroll {
        title.update_frame(false);
    }
    assert_eq!(title.phase, TitlePhase::WaitingForInput);
}

#[test]
fn title_screen_button_press_triggers_cry_and_fade() {
    let mut title = TitleScreenState::new(GameVersion::Red);

    while title.phase != TitlePhase::WaitingForInput {
        title.update_frame(false);
    }

    let action = title.update_frame(true);
    assert_eq!(action, ScreenAction::Continue);
    assert_eq!(title.phase, TitlePhase::PlayingCry);

    while title.phase == TitlePhase::PlayingCry {
        title.update_frame(false);
    }
    assert_eq!(title.phase, TitlePhase::FadeOut);

    let mut transition = ScreenAction::Continue;
    while title.phase == TitlePhase::FadeOut {
        transition = title.update_frame(false);
    }
    assert_eq!(transition, ScreenAction::Transition(GameScreen::MainMenu));
    assert_eq!(title.phase, TitlePhase::Done);
}

#[test]
fn title_screen_blue_version_starts_with_squirtle() {
    let title = TitleScreenState::new(GameVersion::Blue);
    assert_eq!(title.current_mon, pokered_data::species::Species::Squirtle);
}

#[test]
fn main_menu_new_game_no_save() {
    let mut menu = MainMenuState::new(None);
    assert_eq!(menu.item_count(), 2);
    assert!(!menu.has_save);

    while !menu.init_delay_done {
        menu.update_frame(MenuInput::none());
    }

    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::OakSpeech));
    assert_eq!(menu.last_choice, Some(MainMenuChoice::NewGame));
}

#[test]
fn main_menu_with_save_file_shows_continue() {
    let summary = SaveFileSummary {
        player_name: vec![0x80, 0x81, 0x82],
        badges: 0b00000011,
        pokedex_owned: 15,
        play_time_hours: 5,
        play_time_minutes: 30,
        play_time_seconds: 0,
    };
    let mut menu = MainMenuState::new(Some(summary));
    assert_eq!(menu.item_count(), 3);
    assert!(menu.has_save);

    while !menu.init_delay_done {
        menu.update_frame(MenuInput::none());
    }

    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    // CONTINUE first shows the save-info panel (DisplayContinueGameInfo,
    // engine/menus/main_menu.asm:357) — A again (after its 30-frame display
    // delay) loads the game.
    assert_eq!(action, ScreenAction::Continue);
    assert!(menu.is_showing_continue_info());
    for _ in 0..pokered_core::main_menu::CONTINUE_INFO_DELAY_FRAMES {
        menu.update_frame(MenuInput::none());
    }
    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::Overworld));
    assert_eq!(menu.last_choice, Some(MainMenuChoice::Continue));
}

#[test]
fn main_menu_b_returns_to_title() {
    let mut menu = MainMenuState::new(None);

    while !menu.init_delay_done {
        menu.update_frame(MenuInput::none());
    }

    let action = menu.update_frame(MenuInput {
        b: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::TitleScreen));
    assert_eq!(menu.last_choice, Some(MainMenuChoice::BackToTitle));
}

#[test]
fn main_menu_navigate_to_option() {
    let mut menu = MainMenuState::new(None);

    while !menu.init_delay_done {
        menu.update_frame(MenuInput::none());
    }

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
fn full_opening_sequence_copyright_to_oak_speech() {
    let mut game = new_game_state_red();
    assert_eq!(game.screen, GameScreen::GameFreakSplash);

    game.transition_to(GameScreen::TitleScreen);
    assert_eq!(game.screen, GameScreen::TitleScreen);

    let mut title = TitleScreenState::new(game.config.version);
    while title.phase != TitlePhase::WaitingForInput {
        title.update_frame(false);
    }
    title.update_frame(true);
    while title.phase != TitlePhase::Done {
        title.update_frame(false);
    }

    game.transition_to(GameScreen::MainMenu);
    assert_eq!(game.screen, GameScreen::MainMenu);

    let mut menu = MainMenuState::new(None);
    while !menu.init_delay_done {
        menu.update_frame(MenuInput::none());
    }
    let action = menu.update_frame(MenuInput {
        a: true,
        ..MenuInput::none()
    });
    assert_eq!(action, ScreenAction::Transition(GameScreen::OakSpeech));

    game.transition_to(GameScreen::OakSpeech);
    assert_eq!(game.screen, GameScreen::OakSpeech);
}

#[test]
fn oak_speech_complete_sequence() {
    let mut speech = OakSpeechState::new();

    // Should start with greeting phase
    assert!(matches!(speech.phase, OakSpeechPhase::Greeting { .. }));
    assert!(speech
        .current_intro_text()
        .unwrap()
        .contains("Hello there!"));

    // Advance through Greeting phase
    let mut steps = 0;
    while matches!(speech.phase, OakSpeechPhase::Greeting { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech greeting did not finish"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // Should now be at ShowNidorino
    assert!(matches!(speech.phase, OakSpeechPhase::ShowNidorino { .. }));
    assert!(speech
        .current_intro_text()
        .unwrap()
        .contains("This world is"));

    // Advance through ShowNidorino phase
    while matches!(speech.phase, OakSpeechPhase::ShowNidorino { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech show nidorino did not finish"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // Should now be at Explanation
    assert!(matches!(speech.phase, OakSpeechPhase::Explanation { .. }));
    assert!(speech
        .current_intro_text()
        .unwrap()
        .contains("For some people"));

    // Advance through Explanation phase
    while matches!(speech.phase, OakSpeechPhase::Explanation { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech explanation did not finish"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // Should now be at IntroducePlayer
    assert!(matches!(
        speech.phase,
        OakSpeechPhase::IntroducePlayer { .. }
    ));
    assert!(speech
        .current_intro_text()
        .unwrap()
        .contains("First, what is"));

    // Advance through IntroducePlayer phase
    while matches!(speech.phase, OakSpeechPhase::IntroducePlayer { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech introduce player did not finish"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // The pic slides right (OakSpeechSlidePicRight) before the menu opens.
    advance_slide_pic(&mut speech);

    // Should now be at PlayerNameChoice
    assert!(matches!(
        speech.phase,
        OakSpeechPhase::PlayerNameChoice { .. }
    ));

    // Select first default name (RED) to advance
    let result = speech.update_frame(OakSpeechInput {
        a: true,
        ..OakSpeechInput::none()
    });
    assert!(matches!(result, OakSpeechResult::PlayerNameSet(ref n) if n == "RED"));
    assert_eq!(speech.player_name.as_deref(), Some("RED"));

    // The pic slides back left (OakSpeechSlidePicLeft) before the rival intro.
    advance_slide_pic(&mut speech);

    // Should now be at IntroduceRival
    assert!(matches!(
        speech.phase,
        OakSpeechPhase::IntroduceRival { .. }
    ));
    assert!(speech
        .current_intro_text()
        .unwrap()
        .contains("This is my grand-"));

    // Advance through IntroduceRival phase
    while matches!(speech.phase, OakSpeechPhase::IntroduceRival { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech introduce rival did not finish"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // The pic slides right before the rival name menu opens.
    advance_slide_pic(&mut speech);

    // Should now be at RivalNameChoice
    assert!(matches!(
        speech.phase,
        OakSpeechPhase::RivalNameChoice { .. }
    ));

    // Select first default name (BLUE) to advance
    let result = speech.update_frame(OakSpeechInput {
        a: true,
        ..OakSpeechInput::none()
    });
    assert!(matches!(result, OakSpeechResult::RivalNameSet(ref n) if n == "BLUE"));
    assert_eq!(speech.rival_name.as_deref(), Some("BLUE"));

    // The pic slides back left before the final speech.
    advance_slide_pic(&mut speech);

    // Should now be at FinalSpeech
    assert!(matches!(speech.phase, OakSpeechPhase::FinalSpeech { .. }));
    assert!(speech.current_intro_text().unwrap().contains("RED!"));

    // Advance through FinalSpeech phase
    while matches!(speech.phase, OakSpeechPhase::FinalSpeech { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech final speech did not finish"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // Should now be at ShrinkPlayer
    assert!(matches!(speech.phase, OakSpeechPhase::ShrinkPlayer { .. }));

    // Advance through ShrinkPlayer phase
    while matches!(speech.phase, OakSpeechPhase::ShrinkPlayer { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech shrink player did not finish"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // Should now be complete
    assert!(matches!(speech.phase, OakSpeechPhase::Done));
}

#[test]
fn oak_speech_naming_flow() {
    let mut speech = OakSpeechState::new();

    // Advance to player naming choice
    let mut steps = 0;
    while !matches!(speech.phase, OakSpeechPhase::PlayerNameChoice { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech did not reach player naming choice"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // Select first default name (RED)
    let result = speech.update_frame(OakSpeechInput {
        a: true,
        ..OakSpeechInput::none()
    });
    assert!(matches!(result, OakSpeechResult::PlayerNameSet(ref n) if n == "RED"));
    assert_eq!(speech.player_name, Some("RED".to_string()));

    // The pic slides back left before the rival intro.
    advance_slide_pic(&mut speech);

    // Should now be at IntroduceRival
    assert!(matches!(
        speech.phase,
        OakSpeechPhase::IntroduceRival { .. }
    ));
    assert!(speech
        .current_intro_text()
        .unwrap()
        .contains("This is my grand-"));

    // Advance to rival naming choice
    while !matches!(speech.phase, OakSpeechPhase::RivalNameChoice { .. }) {
        steps += 1;
        assert!(
            steps <= OAK_SPEECH_MAX_STEPS,
            "oak speech did not reach rival naming choice"
        );
        speech.update_frame(oak_input_for_progress(&speech));
    }

    // Select first default name (BLUE)
    let result = speech.update_frame(OakSpeechInput {
        a: true,
        ..OakSpeechInput::none()
    });
    assert!(matches!(result, OakSpeechResult::RivalNameSet(ref n) if n == "BLUE"));
    assert_eq!(speech.rival_name, Some("BLUE".to_string()));

    // The pic slides back left before the final speech.
    advance_slide_pic(&mut speech);

    // Should now be at FinalSpeech
    assert!(matches!(speech.phase, OakSpeechPhase::FinalSpeech { .. }));
    assert!(speech.current_intro_text().unwrap().contains("RED!"));
}

#[test]
fn save_file_summary_badge_count() {
    let summary = SaveFileSummary {
        player_name: vec![],
        badges: 0b00000000,
        pokedex_owned: 0,
        play_time_hours: 0,
        play_time_minutes: 0,
        play_time_seconds: 0,
    };
    assert_eq!(summary.badge_count(), 0);

    let summary = SaveFileSummary {
        player_name: vec![],
        badges: ALL_BADGES,
        pokedex_owned: 151,
        play_time_hours: 99,
        play_time_minutes: 59,
        play_time_seconds: 59,
    };
    assert_eq!(summary.badge_count(), 8);
}

#[test]
fn title_screen_resets_correctly() {
    let mut title = TitleScreenState::new(GameVersion::Red);

    while title.phase != TitlePhase::WaitingForInput {
        title.update_frame(false);
    }

    title.reset();
    assert_eq!(title.phase, TitlePhase::Copyright);
    assert_eq!(title.frame_counter, 0);
    assert!(!title.logo_visible);
    assert!(!title.version_text_visible);
}

#[test]
fn game_state_transitions_are_bidirectional() {
    let mut game = new_game_state_red();

    game.transition_to(GameScreen::TitleScreen);
    assert_eq!(game.screen, GameScreen::TitleScreen);

    game.transition_to(GameScreen::MainMenu);
    assert_eq!(game.screen, GameScreen::MainMenu);

    game.transition_to(GameScreen::TitleScreen);
    assert_eq!(game.screen, GameScreen::TitleScreen);

    game.transition_to(GameScreen::MainMenu);
    game.transition_to(GameScreen::OakSpeech);
    assert_eq!(game.screen, GameScreen::OakSpeech);

    game.transition_to(GameScreen::Overworld);
    assert_eq!(game.screen, GameScreen::Overworld);
}
