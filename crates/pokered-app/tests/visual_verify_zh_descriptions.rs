//! Matched baseline/candidate captures through production renderers.
use dotzuki_app::InputState;
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::{render, PokemonGame};
use pokered_core::{
    evolution_screen::{EvolutionScreenState, PendingEvolution},
    game_state::{GameScreen, Lang},
    oak_speech::{OakSpeechPhase, OakSpeechState},
};
use pokered_data::{items::ItemId, species::Species, wild_data::GameVersion};
use pokered_renderer::{
    input::GbButton,
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};
use pokered_ui::{backends::FrameBufferPainter, menus, Ui};

fn opening_after_six_presses() -> PokemonGame {
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
    game.state.screen = GameScreen::OakSpeech;
    game.state.config.language = Lang::Zh;
    game.oak_speech.phase_frame = 100;
    for frame in 0..=43 {
        let mut input = InputState::new();
        if [0, 2, 4, 6, 8, 10].contains(&frame) {
            input.press(GbButton::A);
        }
        game.update(&input);
    }
    game
}

#[test]
fn chinese_opening_uses_localized_pages_in_the_real_game_loop() {
    let game = opening_after_six_presses();
    assert!(matches!(
        game.oak_speech.phase,
        OakSpeechPhase::ShowNidorino { .. }
    ));
    assert!(game.oak_speech.is_waiting_for_input());
    let page = game.oak_speech.current_text_page().unwrap();
    assert_eq!(page.line2, "一种叫做宝可梦的神奇生物！");
}

fn pc_step(
    pc: &mut pokered_core::pc_screen::PcScreen,
    save: &mut pokered_core::save::SaveData,
    input: pokered_core::main_menu::MenuInput,
) {
    pc.update_frame(
        input,
        &mut pokered_core::pc_screen::PcContext {
            party: &mut save.party,
            pc_storage: &mut save.pc_storage,
            bag: &mut save.game_data.bag,
            pc_items: &mut save.game_data.pc_items,
            pokedex: &save.game_data.pokedex,
        },
    );
}

fn oak_pc_rating() -> PokemonGame {
    use pokered_core::{main_menu::MenuInput, pc_screen::PcPhase};
    let mut game = opening_after_six_presses();
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
    let a = MenuInput {
        a: true,
        ..MenuInput::none()
    };
    while pc.phase() == PcPhase::Message {
        pc_step(pc, &mut game.save_data, a);
    }
    for _ in 0..2 {
        pc_step(
            pc,
            &mut game.save_data,
            MenuInput {
                down: true,
                ..MenuInput::none()
            },
        );
    }
    pc_step(pc, &mut game.save_data, a);
    while pc.phase() == PcPhase::Message {
        pc_step(pc, &mut game.save_data, a);
    }
    assert_eq!(pc.phase(), PcPhase::OaksConfirm);
    pc_step(
        pc,
        &mut game.save_data,
        MenuInput {
            up: true,
            ..MenuInput::none()
        },
    );
    pc_step(pc, &mut game.save_data, a);
    let pages = pc.message_page_count();
    for _ in 0..pages {
        pc_step(pc, &mut game.save_data, a);
    }
    game
}

#[test]
fn chinese_pc_is_localized_before_pagination_in_the_real_game_loop() {
    let game = oak_pc_rating();
    let pc = game.pc_screen.unwrap();
    assert_eq!(pc.message_page_count(), 1);
    assert!(pc.message_lines().concat().contains("学习装置"));
    assert!(pc
        .message_lines()
        .iter()
        .all(|s| pokered_data::dialogue_layout::measure_text(s) <= 144));
}

fn hof_stats() -> pokered_core::hof_ceremony::HofCeremonyState {
    use pokered_core::hof_ceremony::{HofCeremonyState, HofEntry, HofPhase, HofPlayerStats};
    let mut hof = HofCeremonyState::new(
        vec![HofEntry {
            species: Species::Bulbasaur,
            level: 50,
            nickname: "妙蛙种子".into(),
        }],
        HofPlayerStats {
            name: "张三丰".into(),
            play_time_hours: 25,
            play_time_minutes: 30,
            money: 99999,
            dex_seen: 75,
            dex_owned: 50,
            rating: pokered_core::pc_screen::dex_rating_text(50),
        },
    );
    for _ in 0..10_000 {
        if hof.phase() == HofPhase::PlayerStats {
            return hof;
        }
        hof.update_frame();
    }
    panic!("Hall of Fame did not reach player stats");
}

#[test]
fn chinese_hall_of_fame_keeps_the_learning_device_and_every_rating_pixel() {
    let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut actual = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    render::draw_hof_ceremony(&hof_stats(), &mut resources, &mut actual, Lang::Zh);
    let mut expected = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    for (i, line) in [
        "你终于凑满至少50只了！",
        "记得去拿我助手那里的",
        "学习装置！",
    ]
    .iter()
    .enumerate()
    {
        pokered_renderer::embedded_font::draw_text(
            line,
            8,
            108 + i as u32 * 12,
            Rgba::BLACK,
            &mut expected,
        );
    }
    for y in 108..144 {
        for x in 8..152 {
            assert_eq!(
                actual.get_pixel(x, y),
                expected.get_pixel(x, y),
                "complete rating pixel {x},{y}"
            );
        }
    }
}

#[test]
#[ignore = "writes matched frames to PR_SCREENSHOTS"]
fn capture_zh_descriptions() {
    let output = std::path::PathBuf::from(std::env::var("PR_SCREENSHOTS").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let save =
        |fb: &FrameBuffer, name: &str| fb.save_png(&output.join(format!("{name}.png"))).unwrap();
    let mut resources = Some(ResourceManager::new(AssetRoot::auto_detect().unwrap()));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);

    let game = opening_after_six_presses();
    render::draw_oak_speech(&game.oak_speech, &mut resources, &mut fb, Lang::Zh);
    save(&fb, "oak-input-frame-43");

    let mut oak = OakSpeechState::new();
    oak.phase_frame = 100;
    oak.phase = OakSpeechPhase::Greeting {
        page_index: 0,
        char_index: u16::MAX,
        waiting_for_input: true,
    };
    render::draw_oak_speech(&oak, &mut resources, &mut fb, Lang::Zh);
    save(&fb, "oak-greeting-page-0");
    oak.phase = OakSpeechPhase::PlayerNameChoice { cursor: 0 };
    render::draw_oak_speech(&oak, &mut resources, &mut fb, Lang::Zh);
    save(&fb, "oak-name-choice");

    for (species, name) in [
        (Species::Bulbasaur, "dex-bulbasaur-page-0"),
        (Species::Caterpie, "dex-caterpie-page-0"),
    ] {
        let mut dex = pokered_core::pokemon::pokedex::Pokedex::new();
        dex.set_seen(species);
        dex.set_owned(species);
        let state = pokered_core::pokedex_screen::PokedexScreenState::new_entry(
            dex,
            species,
            GameVersion::Red,
        );
        render::draw_pokedex_screen(
            &state,
            pokered_data::maps::MapId::PalletTown,
            true,
            &mut resources,
            &mut fb,
        );
        save(&fb, name);
    }

    fb.clear(Rgba::WHITE);
    {
        let mut painter = FrameBufferPainter::new(&mut fb).with_lang(Lang::Zh);
        let mut ui = Ui::new(&mut painter);
        menus::bag::draw_machine_prompt(ItemId::Tm01, Some(0), &mut ui, Lang::Zh);
    }
    save(&fb, "tm01-teach-prompt");

    let mut battle = pokered_core::battle::BattleScreen::new(true);
    battle.phase = pokered_core::battle::BattlePhase::ShowingText {
        messages: vec!["对方的皮卡丘使用了\n十万伏特！".into()],
        current: 0,
        wait_frames: 0,
        next_phase: Box::new(pokered_core::battle::BattlePhase::PlayerMenu),
    };
    battle.current_message = Some("对方的皮卡丘使用了\n十万伏特！".into());
    render::draw_battle(
        &battle,
        &mut resources,
        &mut fb,
        &mut render::BattleVisualEffects::default(),
        Lang::Zh,
    );
    save(&fb, "battle-text");

    let evolution = EvolutionScreenState::new(
        vec![PendingEvolution {
            party_index: 0,
            from: Species::Bulbasaur,
            to: Species::Ivysaur,
            name: "阿尔法贝塔".into(),
            force: false,
        }],
        None,
        true,
    );
    render::draw_evolution(&evolution, &mut resources, &mut fb);
    save(&fb, "evolution-text");

    let mut trade = pokered_core::trade::TradeAnim::new(
        Species::Bulbasaur,
        Species::MrMime,
        "张三丰".into(),
        true,
    );
    while trade.phase() != pokered_core::trade::TradeAnimPhase::TextForSends {
        trade.tick();
    }
    render::draw_trade(&trade, &mut resources, &mut fb);
    save(&fb, "trade-text");

    let mut flow = pokered_app::link::cable_club::CableClubFlow::new();
    flow.on_session_error("The link was\ncanceled.".into());
    fb.clear(Rgba::WHITE);
    render::draw_link_flow(&flow, &mut fb, true, resources.as_mut());
    save(&fb, "link-text");

    let pc_game = oak_pc_rating();
    render::draw_pc(
        pc_game.pc_screen.as_ref().unwrap(),
        &pc_game.save_data,
        &mut resources,
        &mut fb,
        Lang::Zh,
    );
    save(&fb, "pc-oak-rating-50");

    let mut overlay_game = opening_after_six_presses();
    overlay_game.overworld.pending_pokedex_entry =
        Some(pokered_core::overworld::PokedexEntryState {
            species: "Bulbasaur".into(),
            page: 0,
            total_pages: 0,
        });
    render::draw_overworld(
        &mut overlay_game.overworld,
        &mut resources,
        &mut fb,
        Lang::Zh,
    );
    save(&fb, "starter-dex-overlay");

    render::draw_hof_ceremony(&hof_stats(), &mut resources, &mut fb, Lang::Zh);
    save(&fb, "hof-rating-50");
}
