//! Capture the TUI's actual framebuffer without starting a terminal.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::PokemonGame;
use pokered_core::{
    game_state::{GameScreen, Lang},
    oak_speech::OakSpeechPhase,
    pokedex_screen::PokedexScreenState,
    pokemon::pokedex::Pokedex,
};
use pokered_data::{species::Species, wild_data::GameVersion};
use pokered_renderer::input::InputState;
use pokered_renderer::{input::GbButton, FrameBuffer, Rgba};

#[test]
#[ignore = "writes TUI frames to PR_SCREENSHOTS"]
fn capture_tui_zh_descriptions() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let save =
        |fb: &FrameBuffer, name: &str| fb.save_png(&output.join(format!("{name}.png"))).unwrap();
    let mut game = PokemonGame::new(GameVersion::Red);
    game.state.config.language = Lang::Zh;
    game.overworld.set_script_lang("zh");
    game.state.screen = GameScreen::OakSpeech;
    game.oak_speech.phase_frame = 100;
    for frame in 0..=43 {
        let mut input = InputState::new();
        if [0, 2, 4, 6, 8, 10].contains(&frame) {
            input.press(GbButton::A);
        }
        game.update(&input);
    }
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    game.draw(&mut fb);
    save(&fb, "tui-oak-input-frame-43");
    game.oak_speech.phase = OakSpeechPhase::PlayerNameChoice { cursor: 0 };
    game.draw(&mut fb);
    save(&fb, "tui-oak-name-choice");

    let mut dex = Pokedex::new();
    dex.set_seen(Species::Caterpie);
    dex.set_owned(Species::Caterpie);
    game.pokedex_screen = PokedexScreenState::new_entry(dex, Species::Caterpie, GameVersion::Red);
    game.state.screen = GameScreen::Pokedex;
    game.draw(&mut fb);
    save(&fb, "tui-dex-caterpie-page-0");

    game.state.screen = GameScreen::Overworld;
    let mut flags = game.overworld.script_flags();
    flags.insert("EVENT_GOT_POKEDEX".into(), true);
    game.overworld.set_script_flags(flags);
    game.overworld.pending_pc = Some("pokemon".into());
    game.update(&InputState::new());
    assert_eq!(game.state.screen, GameScreen::PC);
    for n in 1..=50 {
        game.save_data
            .game_data
            .pokedex
            .set_owned(Species::from_index_id(n));
    }
    let pc = game.pc_screen.as_mut().unwrap();
    use pokered_core::{
        main_menu::MenuInput,
        pc_screen::{PcContext, PcPhase},
    };
    let mut ctx = PcContext {
        party: &mut game.save_data.party,
        pc_storage: &mut game.save_data.pc_storage,
        bag: &mut game.save_data.game_data.bag,
        pc_items: &mut game.save_data.game_data.pc_items,
        pokedex: &game.save_data.game_data.pokedex,
    };
    let a = MenuInput {
        a: true,
        ..MenuInput::none()
    };
    while pc.phase() == PcPhase::Message {
        pc.update_frame(a, &mut ctx);
    }
    for _ in 0..2 {
        pc.update_frame(
            MenuInput {
                down: true,
                ..MenuInput::none()
            },
            &mut ctx,
        );
    }
    pc.update_frame(a, &mut ctx);
    while pc.phase() == PcPhase::Message {
        pc.update_frame(a, &mut ctx);
    }
    pc.update_frame(
        MenuInput {
            up: true,
            ..MenuInput::none()
        },
        &mut ctx,
    );
    pc.update_frame(a, &mut ctx);
    let pages = pc.message_page_count();
    for _ in 0..pages {
        pc.update_frame(a, &mut ctx);
    }
    game.draw(&mut fb);
    save(&fb, "tui-pc-oak-rating-50");

    game.state.screen = GameScreen::Overworld;
    game.overworld.pending_pokedex_entry = Some(pokered_core::overworld::PokedexEntryState {
        species: "Bulbasaur".into(),
        page: 0,
        total_pages: 0,
    });
    game.draw(&mut fb);
    save(&fb, "tui-starter-dex-overlay");

    game.hof_ceremony = Some(hof_stats_with_values(25, 30, 99999));
    game.draw(&mut fb);
    save(&fb, "tui-hof-rating-50");
}

fn hof_stats_with_values(
    hours: u16,
    minutes: u8,
    money: u32,
) -> pokered_core::hof_ceremony::HofCeremonyState {
    use pokered_core::hof_ceremony::{HofCeremonyState, HofEntry, HofPhase, HofPlayerStats};
    let mut hof = HofCeremonyState::new(
        vec![HofEntry {
            species: Species::Bulbasaur,
            level: 50,
            nickname: "妙蛙种子".into(),
        }],
        HofPlayerStats {
            name: "张三丰".into(),
            play_time_hours: hours,
            play_time_minutes: minutes,
            money,
            dex_seen: 75,
            dex_owned: 50,
            rating: pokered_core::pc_screen::dex_rating_text(50),
        },
    );
    for _ in 0..10_000 {
        if hof.phase() == HofPhase::PlayerStats {
            break;
        }
        hof.update_frame();
    }
    assert_eq!(hof.phase(), HofPhase::PlayerStats);
    hof
}

#[test]
#[ignore = "writes TUI Hall of Fame value limits to PR_SCREENSHOTS"]
fn capture_tui_zh_hof_stats_limits() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let mut game = PokemonGame::new(GameVersion::Red);
    game.state.config.language = Lang::Zh;
    game.state.screen = GameScreen::Overworld;
    for (name, hours, minutes, money) in [
        ("tui-hof-stats-zero", 0, 0, 0),
        ("tui-hof-stats-max", 255, 59, 999999),
    ] {
        game.hof_ceremony = Some(hof_stats_with_values(hours, minutes, money));
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut fb);
        fb.save_png(&output.join(format!("{name}.png"))).unwrap();
    }
}

/// Same state/frame on master and the shared-runtime branch.
#[test]
#[ignore = "writes matched TUI frames to PR_SCREENSHOTS"]
fn capture_tui_shared_runtime_screens() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for (lang, suffix) in [(Lang::En, "en"), (Lang::Zh, "zh")] {
        for (screen, name) in [
            (GameScreen::CopyrightSplash, "copyright"),
            (GameScreen::GameFreakSplash, "splash"),
            (GameScreen::LanguageSelect, "language"),
            (GameScreen::IntroScene, "intro"),
            (GameScreen::TitleScreen, "title"),
            (GameScreen::MainMenu, "main-menu"),
            (GameScreen::Overworld, "overworld"),
            (GameScreen::StartMenu, "start-menu"),
            (GameScreen::OptionsMenu, "options"),
            (GameScreen::SaveMenu, "save"),
            (GameScreen::PartyScreen, "party"),
            (GameScreen::Bag, "bag"),
            (GameScreen::TownMap, "town-map"),
            (GameScreen::TrainerCard, "trainer-card"),
            (GameScreen::Battle, "battle"),
        ] {
            let mut game = PokemonGame::new(GameVersion::Red);
            game.audio = None;
            game.state.config.language = lang;
            game.overworld.set_script_lang(suffix);
            game.battle.is_zh = lang == Lang::Zh;
            game.battle.phase = pokered_core::battle::BattlePhase::PlayerMenu;
            game.frame_count = 180;
            game.state.screen = screen;
            let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            game.draw(&mut fb);
            fb.save_png(&output.join(format!("tui-{name}-{suffix}.png")))
                .unwrap();
        }
    }
}

#[test]
#[ignore = "writes matched FLY departure frames to PR_SCREENSHOTS"]
fn capture_tui_fly_departure() {
    use pokered_core::town_map_screen::TownMapScreenState;
    use pokered_data::maps::MapId;
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for (lang, suffix) in [(Lang::En, "en"), (Lang::Zh, "zh")] {
        let mut game = PokemonGame::new(GameVersion::Red);
        game.audio = None;
        game.state.config.language = lang;
        game.overworld.set_script_lang(suffix);
        game.overworld.state.player.x = 5;
        game.overworld.state.player.y = 6;
        game.town_map_screen =
            TownMapScreenState::new_fly(MapId::PalletTown, vec![MapId::ViridianCity]);
        game.state.screen = GameScreen::TownMap;
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut fb);
        for frame in 0..=8 {
            let mut input = InputState::new();
            if frame == 0 {
                input.press(GbButton::A);
            }
            game.update(&input);
            game.draw(&mut fb);
            if [0, 7, 8].contains(&frame) {
                fb.save_png(&output.join(format!("tui-fly-departure-{frame}-{suffix}.png")))
                    .unwrap();
            }
        }
    }
}

#[test]
#[ignore = "writes matched font-source frames to PR_SCREENSHOTS"]
fn capture_tui_font_sources() {
    use pokered_core::title_screen::TitlePhase;
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for version in [GameVersion::Red, GameVersion::Blue] {
        let mut game = PokemonGame::new(version);
        game.audio = None;
        game.state.screen = GameScreen::TitleScreen;
        game.title_screen.phase = TitlePhase::WaitingForInput;
        game.title_screen.version_text_visible = true;
        game.title_screen.logo_visible = true;
        game.title_screen.scroll_y = 0;
        let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
        game.draw(&mut fb);
        fb.save_png(&output.join(format!("tui-font-title-{version:?}.png"))).unwrap();
    }
    let mut splash = PokemonGame::new(GameVersion::Red);
    splash.audio = None;
    splash.state.screen = GameScreen::GameFreakSplash;
    splash.gamefreak_splash.phase = pokered_core::gamefreak_splash::SplashPhase::PostDelay;
    let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
    splash.draw(&mut fb);
    fb.save_png(&output.join("tui-font-gamefreak.png")).unwrap();
    for (lang, label) in [(Lang::En, "en"), (Lang::Zh, "zh")] {
        let mut game = PokemonGame::new(GameVersion::Red);
        game.audio = None;
        game.state.config.language = lang;
        game.battle.is_zh = lang == Lang::Zh;
        game.battle.phase = pokered_core::battle::BattlePhase::PlayerMenu;
        game.battle.player_level = 100;
        game.battle.player_hp = 703;
        game.battle.player_max_hp = 703;
        game.battle.player_status = pokered_core::battle::state::StatusCondition::Poison;
        game.state.screen = GameScreen::Battle;
        let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
        game.draw(&mut fb);
        fb.save_png(&output.join(format!("tui-font-battle-status-{label}.png"))).unwrap();
        game.state.screen = GameScreen::Overworld;
        game.save_data.party.add(pokered_core::pokemon::stats::create_pokemon(Species::Chansey, 100, [255,255]).unwrap()).unwrap();
        game.party_screen = pokered_core::party_screen::PartyScreenState::new(game.save_data.party.to_vec());
        game.state.screen = GameScreen::PartyScreen;
        game.draw(&mut fb);
        fb.save_png(&output.join(format!("tui-font-party-{label}.png"))).unwrap();
    }
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = None;
    game.state.screen = GameScreen::OakSpeech;
    game.oak_speech.phase = OakSpeechPhase::PlayerNameChoice { cursor: 3 };
    let mut input = InputState::new(); input.press(GbButton::A);
    game.update(&input);
    for _ in 0..10 { game.update(&InputState::new()); }
    let mut fb = FrameBuffer::new(RenderConfig::new(160,144), Rgba::WHITE);
    game.draw(&mut fb);
    fb.save_png(&output.join("tui-font-naming.png")).unwrap();
    game.state.screen = GameScreen::Overworld;
    game.overworld.pending_pc = Some("pokemon".into());
    game.update(&InputState::new());
    use pokered_core::pc_screen::{PcContext, PcPhase};
    let pc = game.pc_screen.as_mut().unwrap();
    let mut ctx = PcContext { party: &mut game.save_data.party, pc_storage: &mut game.save_data.pc_storage,
        bag: &mut game.save_data.game_data.bag, pc_items: &mut game.save_data.game_data.pc_items,
        pokedex: &game.save_data.game_data.pokedex };
    for _ in 0..30 {
        if pc.phase() != PcPhase::Message { break; }
        pc.update_frame(pokered_core::main_menu::MenuInput { a:true, ..pokered_core::main_menu::MenuInput::none() }, &mut ctx);
    }
    pc.update_frame(pokered_core::main_menu::MenuInput { a:true, ..pokered_core::main_menu::MenuInput::none() }, &mut ctx);
    for _ in 0..30 {
        if pc.phase() != PcPhase::Message { break; }
        pc.update_frame(pokered_core::main_menu::MenuInput { a:true, ..pokered_core::main_menu::MenuInput::none() }, &mut ctx);
    }
    assert_eq!(pc.phase(), PcPhase::BillsMenu);
    game.draw(&mut fb);
    fb.save_png(&output.join("tui-font-pc.png")).unwrap();
}

#[test]
#[ignore = "writes matched Safari action-menu frames to PR_SCREENSHOTS"]
fn capture_tui_safari_menu_padding() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for (lang, label) in [(Lang::En, "en"), (Lang::Zh, "zh")] {
        let mut game = PokemonGame::new(GameVersion::Red);
        game.audio = None;
        game.state.config.language = lang;
        game.battle.is_zh = lang == Lang::Zh;
        game.battle.is_safari = true;
        game.battle.safari_menu = pokered_core::battle::menu::SafariBattleMenuState::new(30);
        game.battle.phase = pokered_core::battle::BattlePhase::PlayerMenu;
        game.state.screen = GameScreen::Battle;
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        game.draw(&mut fb);
        fb.save_png(&output.join(format!("tui-safari-menu-{label}.png"))).unwrap();
    }
}
