//! Component controls only; BIOS/physical SRAM must be tested on actual GBA.
use super::*;

fn game() -> PokemonGame {
    PokemonGame::new_with_options(
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
    )
}

#[test]
fn reset_combo_requires_exact_raw_buttons_not_extra_dpad_or_missing_buttons() {
    // Original engine/joypad.asm compares hJoyInput to PAD_BUTTONS ($0f),
    // before either ignored or disabled-input masks apply.
    for raw in 0u8..=255 {
        let mut input = InputState::new();
        input.set_from_bitmask(raw);
        assert_eq!(soft_reset_combo_held(&input), raw == 0x0f, "raw={raw:02x}");
    }
}

#[test]
fn raw_nonconsecutive_reset_polls_reach_reset_while_real_script_ignores_joypad() {
    let mut game = game();
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
    game.overworld
        .reload_scene_with_config(
            "RedsHouse2F",
            r#"
game_scene RedsHouse2F {
  @storyline("ignoreProbe") {
    setJoyIgnore(255)
    delay(1000)
  }
}
"#,
            Some(r#"{"onLoad":"ignoreProbe"}"#),
        )
        .unwrap();
    for _ in 0..20 {
        game.update(&InputState::new());
        if game.overworld.active_script_effect_label().as_deref() == Some("Delay") {
            break;
        }
    }
    assert_eq!(
        game.overworld.active_script_effect_label().as_deref(),
        Some("Delay")
    );
    let mut input = InputState::new();
    let step = |game: &mut PokemonGame, input: &mut InputState, raw: u8| {
        input.begin_frame();
        input.set_from_bitmask(raw);
        game.update(input);
    };
    for _ in 0..8 {
        step(&mut game, &mut input, 0x0f);
    }
    step(&mut game, &mut input, 0);
    assert_eq!(game.state.screen, GameScreen::Overworld);
    for _ in 0..7 {
        step(&mut game, &mut input, 0x0f);
        assert_eq!(game.state.screen, GameScreen::Overworld);
    }
    step(&mut game, &mut input, 0x0f);
    // Host reset behavior remains intact. This proves the global raw check
    // precedes script ignoring; it is not proof of the GBA BIOS restart.
    assert_eq!(game.state.screen, GameScreen::TitleScreen);
}

#[test]
fn platform_request_is_sticky_and_freezes_the_old_session_without_saving() {
    let mut game = game();
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::SaffronCity, None, PokemonRedData);
    game.save_data.game_data.player_money = 4567;
    game.pending_screen = Some(GameScreen::MainMenu);
    let frame = game.frame_count;
    game.request_platform_soft_reset();
    assert!(game.platform_reset_requested());
    assert!(game.platform_reset_requested());
    game.request_platform_soft_reset();
    for _ in 0..3 {
        game.update(&InputState::new());
    }
    assert_eq!(game.frame_count, frame);
    assert_eq!(game.state.screen, GameScreen::Overworld);
    assert_eq!(game.overworld.state.current_map, MapId::SaffronCity);
    assert_eq!(game.save_data.game_data.player_money, 4567);
    assert_eq!(game.pending_screen, Some(GameScreen::MainMenu));
}

#[test]
fn reset_white_frame_clears_pixels_and_all_display_shades_including_border() {
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::BLACK);
    fb.set_palette(pokered_renderer::palette::Palette::new(&[Rgba::BLACK; 4]));
    PokemonGame::draw_soft_reset_white_frame(&mut fb);
    for y in 0..144 {
        for x in 0..160 {
            assert_eq!(fb.get_pixel(x, y), Some(Rgba::WHITE));
        }
    }
    assert_eq!(&fb.display_palette().colors[..4], &[Rgba::WHITE; 4]);
}
