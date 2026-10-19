use super::*;
use pokered_core::foreground_input::START;

fn on_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}
fn field_game() -> PokemonGame {
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = None;
    game.save_data = SaveData::new();
    let mon = pokered_core::pokemon::stats::create_pokemon(
        pokered_data::species::Species::Rattata,
        12,
        [0x99, 0x88],
    )
    .unwrap();
    game.save_data.party = pokered_core::pokemon::party::Party::from(vec![mon]);
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
    game.overworld.state.player.x = 3;
    game.overworld.state.player.y = 6;
    for _ in 0..4 {
        game.update(&InputState::new());
    }
    let mut input = InputState::new();
    for _ in 0..4 {
        input.begin_frame();
        input.press(GbButton::Start);
        game.update(&input);
        if game.state.screen == GameScreen::StartMenu {
            return game;
        }
    }
    panic!("real START did not enter the preparation caller");
}
#[test]
fn start_menu_entry_keeps_short_early_a_out_and_accepts_held_or_ready_a() {
    on_stack(|| {
        for early_hold in [false, true] {
            let mut game = field_game();
            assert_eq!(
                game.overworld.source_foreground().unwrap().last(),
                Some(START)
            );
            let mut input = InputState::new();
            for tick in 1..=23 {
                input.begin_frame();
                if early_hold || tick == 1 {
                    input.press(GbButton::A);
                } else {
                    input.release(GbButton::A);
                }
                game.update(&input);
                if tick < 23 {
                    assert_eq!(game.state.screen, GameScreen::StartMenu);
                    assert_eq!(
                        game.overworld.source_foreground().unwrap().last(),
                        Some(START),
                        "preparation must never execute Joypad"
                    );
                }
            }
            if early_hold {
                assert_eq!(game.state.screen, GameScreen::PartyScreen);
            } else {
                assert_eq!(game.state.screen, GameScreen::StartMenu);
                assert_eq!(game.overworld.source_foreground().unwrap().last(), Some(0));
                input.begin_frame();
                input.press(GbButton::A);
                game.update(&input);
                assert_eq!(game.state.screen, GameScreen::PartyScreen);
            }
        }
    });
}
#[test]
fn start_menu_entry_cached_and_full_frames_agree_through_every_publication() {
    on_stack(|| {
        let mut game = field_game();
        let mut session = crate::render::session::RenderSession::new();
        let config = RenderConfig::new(160, 144);
        let mut cached = FrameBuffer::new(config, Rgba::WHITE);
        let mut full = FrameBuffer::new(config, Rgba::WHITE);
        let mut scroll = |_: &mut [u8], _: usize, _: usize, _: i32, _: i32, _: u8| {};
        for tick in 0..=26 {
            if tick != 0 {
                game.update(&InputState::new());
            }
            session.render(&mut game, &mut cached, &mut scroll);
            game.draw(&mut full);
            assert_eq!(cached.packed(), full.packed(), "publication tick={tick}");
            assert_eq!(cached.display_palette(), full.display_palette());
        }
        assert!(game.start_menu.input_ready());
        assert_eq!(
            game.overworld.source_foreground().unwrap().pressed(),
            Some(0)
        );
    });
}
