use std::path::PathBuf;

use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::{GameScreen, Lang};
use pokered_data::maps::MapId;
use dotzuki_app::InputState;
use pokered_renderer::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;

use crate::cli::{screen_name, screen_target_to_game_screen, ScreenTarget, ALL_SCREENS};
use crate::game::PokemonGame;

/// Apply a capture language to a freshly constructed game: set the config
/// language (menus / screen text) and keep the overworld script engine in
/// sync, so `@t("english", "中文")` dialogue resolves to the right side.
fn apply_lang(game: &mut PokemonGame, lang: Lang) {
    game.state.config.language = lang;
    game.overworld.set_script_lang(match lang {
        Lang::En => "en",
        Lang::Zh => "zh",
    });
}

/// Overworld captures run on a fresh game whose player position was never
/// set (engine default (0,0), the map's top-left corner). Put the player on
/// the doorstep of their Pallet Town house — the walk-out tile just below
/// the door mat at (5,5) — so the shot opens on home ground.
fn seed_overworld_spawn(game: &mut PokemonGame) {
    let (px, py) = game
        .overworld
        .resolve_editor_warp_position(MapId::PalletTown, Some((5, 6)));
    game.overworld.state.player.x = px as u16;
    game.overworld.state.player.y = py as u16;
}

pub fn capture_screen(game: &mut PokemonGame, target: GameScreen, frames: u32) -> FrameBuffer {
    game.handle_transition(target);
    let input = InputState::new();
    for _ in 0..frames {
        game.update(&input);
    }
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    game.draw(&mut fb);
    fb
}

pub fn cmd_screenshot(target: &ScreenTarget, output: &PathBuf, frames: u32, lang: Lang) {
    let version = GameVersion::Red;
    let mut game = PokemonGame::new(version);
    apply_lang(&mut game, lang);
    if matches!(target, ScreenTarget::Overworld) {
        seed_overworld_spawn(&mut game);
    }
    let screen = screen_target_to_game_screen(target);
    let label = if matches!(target, ScreenTarget::Naming) { "naming" } else { screen_name(&screen) };
    println!(
        "Capturing screen: {} ({} frames, lang={:?})...",
        label,
        frames,
        lang
    );
    if matches!(target, ScreenTarget::Pc) {
        // The PC screen only exists once a script opens it; seed a demo
        // party/box/item state and open the Pokémon Center PC directly, then
        // advance past the boot message into the main menu.
        use pokered_core::pc_screen::{PcEntry, PcOpenContext, PcScreen};
        use pokered_core::pokemon::stats::create_pokemon;
        use pokered_data::items::ItemId;
        use pokered_data::species::Species;
        let _ = game
            .save_data
            .party
            .add(create_pokemon(Species::Pikachu, 12, [0x9A, 0x78]).unwrap());
        let _ = game
            .save_data
            .party
            .add(create_pokemon(Species::Bulbasaur, 7, [0x9A, 0x78]).unwrap());
        let _ = game
            .save_data
            .pc_storage
            .current_box_mut()
            .deposit(create_pokemon(Species::Charmander, 9, [0x9A, 0x78]).unwrap());
        let _ = game.save_data.game_data.bag.add_item(ItemId::Potion, 5);
        let _ = game.save_data.game_data.pc_items.add_item(ItemId::PokeBall, 3);
        game.player_name = "RED".to_string();
        game.pc_screen = Some(PcScreen::new(
            PcEntry::PokemonCenter,
            &PcOpenContext {
                has_pokedex: true,
                met_bill: true,
                beaten_league: false,
                player_name: "RED".to_string(),
                hof_teams: Vec::new(),
            },
        ));
        game.handle_transition(GameScreen::PC);
        // A-press to dismiss the "RED turned on the PC." boot message, then a
        // few idle frames so the main menu is what's captured.
        let mut input = InputState::new();
        input.press(dotzuki_renderer::input::GbButton::A);
        game.update(&input);
        input.release(dotzuki_renderer::input::GbButton::A);
        for _ in 0..frames {
            game.update(&input);
        }
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut fb);
        fb.save_png(output).expect("Failed to save PNG");
        println!("Saved: {}", output.display());
        return;
    }
    if matches!(target, ScreenTarget::Naming) {
        // The pinyin naming screen only exists mid-oak-speech with the
        // Chinese language selected. Seed the config, drop into the
        // player-naming phase, and drive the IME into a representative
        // state. This branch runs its own transition→seed→update→draw loop
        // because capture_screen() would handle_transition() — which resets
        // the oak speech — after the seeding.
        use pokered_core::oak_speech::OakSpeechPhase;

        game.handle_transition(GameScreen::OakSpeech);
        game.state.config.language = Lang::Zh;
        game.oak_speech.phase = OakSpeechPhase::PlayerNaming;
        game.oak_speech.naming_screen = Some(seed_naming_screen());
        let input = InputState::new();
        for _ in 0..frames {
            game.update(&input);
        }
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut fb);
        fb.save_png(output).expect("Failed to save PNG");
        println!("Saved: {}", output.display());
        return;
    }
    let fb = capture_screen(&mut game, screen, frames);
    fb.save_png(output).expect("Failed to save PNG");
    println!("Saved: {}", output.display());
}

/// Seed the naming screen state machine into a representative pinyin-IME
/// frame: name "阿阿" (two committed candidates), buffer "zh" with its
/// candidate strip, cursor resting on the H tile.
fn seed_naming_screen() -> pokered_core::naming_screen::NamingScreenState {
    use pokered_core::naming_screen::{NamingInput, NamingScreenState, NamingScreenType};

    let a = NamingInput { a: true, ..NamingInput::none() };
    let select = NamingInput { select: true, ..NamingInput::none() };
    let down = NamingInput { down: true, ..NamingInput::none() };
    let up = NamingInput { up: true, ..NamingInput::none() };
    let right = NamingInput { right: true, ..NamingInput::none() };

    let mut ns = NamingScreenState::new(NamingScreenType::Player);
    ns.update_frame(select, true); // → pinyin mode
    for _ in 0..2 {
        ns.update_frame(a, true); // type 'a' at (0,0)
        for _ in 0..3 {
            ns.update_frame(down, true); // rows 0→1→2→candidate strip
        }
        ns.update_frame(a, true); // commit 阿
    }
    // Type "zh": after each commit the cursor returns to (0,0).
    ns.update_frame(down, true); // (1,0)
    ns.update_frame(down, true); // (2,0)
    for _ in 0..7 {
        ns.update_frame(right, true); // (2,7) = Z
    }
    ns.update_frame(a, true); // 'z'
    ns.update_frame(up, true); // (1,7)
    ns.update_frame(up, true); // (0,7) = H
    ns.update_frame(a, true); // 'h'
    ns
}

pub fn cmd_screenshot_all(output_dir: &PathBuf, frames: u32, lang: Lang) {
    std::fs::create_dir_all(output_dir).expect("Failed to create output directory");
    let version = GameVersion::Red;
    let mut game = PokemonGame::new(version);
    apply_lang(&mut game, lang);
    for screen in ALL_SCREENS {
        let name = screen_name(screen);
        let path = output_dir.join(format!("{}.png", name));
        println!("Capturing: {}...", name);
        if matches!(screen, GameScreen::Overworld) {
            seed_overworld_spawn(&mut game);
        }
        let fb = capture_screen(&mut game, screen.clone(), frames);
        fb.save_png(&path).expect("Failed to save PNG");
        println!("  -> {}", path.display());
    }
    println!(
        "Done. {} screenshots saved to {}",
        ALL_SCREENS.len(),
        output_dir.display()
    );
}

pub fn cmd_dump_state(target: &ScreenTarget, frames: u32) {
    let version = GameVersion::Red;
    let mut game = PokemonGame::new(version);
    let screen = screen_target_to_game_screen(target);
    game.handle_transition(screen);
    let input = InputState::new();
    for _ in 0..frames {
        game.update(&input);
    }

    let map_id = game.overworld.state.current_map as u8;
    let map_name = format!("{:?}", game.overworld.state.current_map);
    let player_x = game.overworld.state.player.x;
    let player_y = game.overworld.state.player.y;
    let screen_name_str = format!("{:?}", game.state.screen);
    let in_battle = matches!(game.state.screen, GameScreen::Battle);
    let battle_phase = format!("{:?}", game.battle.phase);
    let is_wild_battle = game.battle.is_wild;

    let state = serde_json::json!({
        "screen": screen_name_str,
        "map_id": map_id,
        "map_name": map_name,
        "player_x": player_x,
        "player_y": player_y,
        "in_battle": in_battle,
        "battle_phase": battle_phase,
        "is_wild_battle": is_wild_battle,
        "player_name": game.player_name,
        "rival_name": game.rival_name,
        "frame_count": game.frame_count,
    });
    println!("{}", serde_json::to_string_pretty(&state).unwrap());
}
