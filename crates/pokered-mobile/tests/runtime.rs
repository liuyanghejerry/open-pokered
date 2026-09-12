use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_core::save_menu::SavePhase;
use pokered_renderer::input::InputState;

fn check_frame_and_audio() {
    assert!(
        !pokered_renderer::embedded::list_embedded_assets().is_empty(),
        "mobile graphics must be embedded without a filesystem fallback"
    );
    let empty_root =
        std::env::temp_dir().join(format!("pokered-mobile-assets-{}", std::process::id()));
    std::fs::create_dir_all(&empty_root).unwrap();
    let mut resources = pokered_renderer::resource::ResourceManager::new(
        pokered_renderer::resource::AssetRoot::new(&empty_root).unwrap(),
    );
    assert!(resources.load_title("pokemon_logo").is_ok());
    assert!(resources.load_pokemon_front("charmander").is_ok());
    std::fs::remove_dir(&empty_root).unwrap();
    let runtime = pokered_mobile::create(b"pokered:red:v1".to_vec(), None).unwrap();
    assert_eq!(
        (runtime.width(), runtime.height(), runtime.frame_len()),
        (160, 144, 92160)
    );
    assert_eq!(runtime.copy_frame(&mut [0; 4]), Err(92160));
    let mut frame = vec![0; runtime.frame_len()];
    let mut audible = false;
    let mut pcm = [0.0; 2048];
    for _ in 0..300 {
        runtime.tick(0);
        let n = runtime.fill_audio(&mut pcm) as usize * 2;
        audible |= pcm[..n].iter().any(|v| v.abs() > 0.0001);
    }
    runtime.copy_frame(&mut frame).unwrap();
    assert!(frame.chunks_exact(4).all(|p| p[3] == 255));
    assert!(frame.chunks_exact(4).any(|p| p[..3] != frame[..3]));
    assert!(audible, "intro music must reach the platform PCM queue");
    assert!(
        runtime.export_save().is_none(),
        "playing must not implicitly save"
    );
    assert!(!runtime.import_save("{}"));
}

fn check_save_and_reset() {
    let mut game = PokemonGame::new_mobile(GameVersion::Red, None).unwrap();
    game.player_name = "RED".into();
    game.overworld
        .set_script_flags(std::collections::HashMap::from([(
            "mobile_test_flag".into(),
            true,
        )]));
    game.state.screen = GameScreen::SaveMenu;
    game.save_menu.phase = SavePhase::WaitAfterSave {
        frames_remaining: 0,
    };
    game.update(&InputState::new());
    let save = game
        .export_mobile_save()
        .expect("menu SAVE commits a host payload");
    let json: serde_json::Value = serde_json::from_str(&save).unwrap();
    assert_eq!(json["flags"]["mobile_test_flag"], true);
    game.save_data.game_data.player_money = 999;
    assert_eq!(
        game.export_mobile_save().unwrap(),
        save,
        "unsaved progress must stay uncommitted"
    );
    assert!(game.import_mobile_save("bad json").is_err());
    assert_eq!(game.export_mobile_save().unwrap(), save);
    let mut restored = PokemonGame::new_mobile(GameVersion::Red, Some(&save)).unwrap();
    assert!(restored.state.has_save_file());
    restored.handle_transition(GameScreen::MainMenu);
    restored.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::Continue);
    restored.handle_transition(GameScreen::Overworld);
    assert_eq!(
        restored.overworld.script_flags().get("mobile_test_flag"),
        Some(&true)
    );
    let mut reset = InputState::new();
    reset.set_from_bitmask(0x0f);
    for _ in 0..16 {
        restored.update(&reset);
        reset.begin_frame();
    }
    assert_eq!(restored.state.screen, GameScreen::TitleScreen);
    assert_eq!(restored.export_mobile_save().unwrap(), save);
}

// PokemonGame's debug constructor has large stack temporaries. Match the
// desktop main-thread budget rather than libtest's 2 MiB worker default.
fn with_game_stack(test: fn()) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn mobile_frame_and_audio_follow_game_commands() {
    with_game_stack(check_frame_and_audio);
}
#[test]
fn committed_save_restores_flags_and_survives_soft_reset() {
    with_game_stack(check_save_and_reset);
}
