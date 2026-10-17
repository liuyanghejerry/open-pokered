use super::*;
use pokered_core::game_state::MainMenuChoice;
use pokered_core::overworld::Direction;
use pokered_core::save::game_data::REDS_HOUSE_2F_SCRIPT_INDEX;

struct SaveDirectory(PathBuf);

impl Drop for SaveDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn continue_from_file(path: &Path) -> PokemonGame {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        Some(path.to_owned()),
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    assert_eq!(
        game.save_data.game_data.position.map_id,
        MapId::RedsHouse2F as u8
    );
    assert_eq!(game.save_data.game_data.player_id, 1234);
    game.state.screen = GameScreen::MainMenu;
    game.main_menu.last_choice = Some(MainMenuChoice::Continue);
    // Use the host's real Continue path: no injected map, script flags, or @load.
    game.handle_transition(GameScreen::Overworld);
    assert_eq!(game.overworld.state.current_map, MapId::RedsHouse2F);
    assert_eq!(
        (game.overworld.state.player.x, game.overworld.state.player.y),
        (3, 6)
    );
    game
}

fn finish_initial_load(game: &mut PokemonGame) {
    let idle = InputState::new();
    for _ in 0..64 {
        game.update(&idle);
    }
    assert_eq!(game.state.screen, GameScreen::Overworld);
    assert!(game.overworld.script_engine_idle());
    assert!(game.overworld.active_script_effect_label().is_none());
    assert_eq!(
        (game.overworld.state.player.x, game.overworld.state.player.y),
        (3, 6)
    );
}

#[test]
fn actual_bedroom_continue_persists_original_script_byte_without_companion() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = SaveDirectory(std::env::temp_dir().join(format!(
        "pokered-bedroom-save-{}-{unique}",
        std::process::id()
    )));
    std::fs::create_dir_all(&directory.0).unwrap();

    for original_script in [0, 1] {
        let original_path = directory.0.join(format!("original-{original_script}.sav"));
        let saved_path = directory.0.join(format!("saved-{original_script}.sav"));
        let mut save = SaveData::new();
        save.player_name = pokered_data::charmap::encode_string("RED").unwrap();
        save.game_data.rival_name = pokered_data::charmap::encode_string("BLUE").unwrap();
        save.game_data.player_id = 1234;
        save.game_data.position.map_id = MapId::RedsHouse2F as u8;
        save.game_data.position.x = 3;
        save.game_data.position.y = 6;
        save.game_data.player_direction = 8;
        save.game_data.game_progress_flags[REDS_HOUSE_2F_SCRIPT_INDEX] = original_script;
        let original_bytes = export_sram(&save);
        std::fs::write(&original_path, &original_bytes).unwrap();

        let mut first = continue_from_file(&original_path);
        assert_eq!(
            first.save_data.game_data.game_progress_flags[REDS_HOUSE_2F_SCRIPT_INDEX],
            original_script
        );
        if original_script == 0 {
            assert_eq!(
                first.overworld.active_script_effect_value(),
                Some(serde_json::json!({
                    "effect": "FacePlayer", "direction": "Up",
                }))
            );
        } else {
            assert_ne!(
                first.overworld.active_script_effect_label().as_deref(),
                Some("FacePlayer")
            );
        }
        finish_initial_load(&mut first);
        assert_eq!(
            first.overworld.state.player.facing,
            if original_script == 0 {
                Direction::Up
            } else {
                Direction::Down
            }
        );
        first.save_to_path(&saved_path).unwrap();
        drop(first);

        let saved_bytes = std::fs::read(&saved_path).unwrap();
        assert_eq!(saved_bytes.len(), 32768);
        let saved = import_sram(&saved_bytes).unwrap();
        assert_eq!(
            saved.game_data.game_progress_flags[REDS_HOUSE_2F_SCRIPT_INDEX],
            1
        );
        assert!(!companion_path_for_save(&saved_path).exists());

        // A second cold app instance must recover NOOP from SRAM alone.
        // Original Continue resets facing Down even if the saved byte was Up.
        let mut second = continue_from_file(&saved_path);
        assert_ne!(
            second.overworld.active_script_effect_label().as_deref(),
            Some("FacePlayer")
        );
        finish_initial_load(&mut second);
        assert_eq!(second.overworld.state.player.facing, Direction::Down);
        assert_eq!(std::fs::read(&original_path).unwrap(), original_bytes);
        assert_eq!(std::fs::read(&saved_path).unwrap(), saved_bytes);
    }
}

#[test]
fn new_game_entry_clears_a_reused_bedroom_script_before_running_it() {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.handle_transition(GameScreen::OakSpeech);
    game.overworld = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
    game.state.screen = GameScreen::Overworld;
    game.overworld.state.player.x = 3;
    game.overworld.state.player.y = 6;
    game.run_initial_new_game_map_script();
    finish_initial_load(&mut game);
    assert!(game
        .overworld
        .unified_flags()
        .get_flag("SCRIPT_REDSHOUSE2F_NOOP"));

    // GBA retains this screen across soft reset. Oak's NEW GAME entry clears
    // SaveData, but the old screen still has its completed script alias.
    game.handle_transition(GameScreen::OakSpeech);
    assert_eq!(
        game.save_data.game_data.game_progress_flags[REDS_HOUSE_2F_SCRIPT_INDEX],
        0
    );
    assert!(game
        .overworld
        .unified_flags()
        .get_flag("SCRIPT_REDSHOUSE2F_NOOP"));
    game.overworld.state.player.facing = Direction::Down;
    game.state.screen = GameScreen::Overworld;

    // Exercise the production entry phase shared by host and bare-metal,
    // without the host's replacement Overworld masking the stale alias.
    game.run_initial_new_game_map_script();
    finish_initial_load(&mut game);
    assert_eq!(game.overworld.state.player.facing, Direction::Up);
    assert!(game
        .overworld
        .unified_flags()
        .get_flag("SCRIPT_REDSHOUSE2F_NOOP"));
}
