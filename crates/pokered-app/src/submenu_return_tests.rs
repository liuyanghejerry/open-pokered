//! Bag and in-game Options return through their actual resource wait.
use super::*;
use pokered_core::foreground_input::{A, B, DOWN, RIGHT};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn frame(game: &mut PokemonGame, input: &mut InputState, keys: u8) {
    input.begin_frame();
    for (bit, key) in [
        (A, GbButton::A),
        (B, GbButton::B),
        (DOWN, GbButton::Down),
        (RIGHT, GbButton::Right),
    ] {
        if keys & bit != 0 {
            input.press(key);
        } else {
            input.release(key);
        }
    }
    game.update(input);
    if let Some(flow) = &game.field_flash {
        assert!(flow.error.is_none(), "{:?}", flow.error);
    }
}

fn fixture(map: MapId, screen: GameScreen) -> (PokemonGame, InputState) {
    fixture_at(map, screen, (7, 6))
}

fn fixture_at(map: MapId, screen: GameScreen, position: (u16, u16)) -> (PokemonGame, InputState) {
    let mut game = PokemonGame::new(GameVersion::Red);
    game.audio = Some(AudioOutput::new_pcm());
    game.audio
        .as_ref()
        .unwrap()
        .play_music(pokered_audio::music_data::MusicId::PALLET_TOWN);
    game.overworld = OverworldScreen::new(map, None, PokemonRedData);
    game.overworld.state.player.x = position.0;
    game.overworld.state.player.y = position.1;
    game.overworld.publish_npc_frame(false);
    game.save_data
        .game_data
        .bag
        .add_item(pokered_data::items::ItemId::Potion, 3)
        .unwrap();
    game.handle_transition(GameScreen::StartMenu);
    game.handle_transition(screen);
    let mut input = InputState::new();
    for _ in 0..32 {
        frame(&mut game, &mut input, 0);
    }
    (game, input)
}

#[test]
fn submenu_cancel_reloads_patterns_and_rejects_input_until_start_returns() {
    run(|| {
        for map in [MapId::ViridianCity, MapId::IndigoPlateauLobby] {
            for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
                let (mut game, mut input) = fixture(map, screen.clone());
                let player = (
                    game.overworld.state.player.x,
                    game.overworld.state.player.y,
                    game.overworld.state.player.facing,
                );
                let bag = game.save_data.game_data.bag.items().to_vec();
                let cursor = game.start_menu.cursor();
                let mut before = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                game.draw(&mut before);
                frame(&mut game, &mut input, B);
                assert!(game.field_flash.is_some());
                assert_eq!(game.state.screen, screen);
                let mut saw_copy = false;
                for tick in 1..80 {
                    frame(
                        &mut game,
                        &mut input,
                        if tick == 1 || tick == 2 { DOWN } else { 0 },
                    );
                    if let Some(flow) = &game.field_flash {
                        let copying = flow.irq.as_ref().is_some_and(|irq| irq.copy.is_some());
                        saw_copy |= copying;
                        let mut during = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                        game.draw(&mut during);
                        if copying {
                            assert_eq!(during.packed(), before.packed(),
                                "the old UI remains visible while the textbox transfer is pending");
                        }
                    }
                    assert_eq!(
                        (
                            game.overworld.state.player.x,
                            game.overworld.state.player.y,
                            game.overworld.state.player.facing
                        ),
                        player
                    );
                    assert_eq!(game.save_data.game_data.bag.items(), bag);
                    assert!(game.source_display.device().lcd_enabled());
                    assert!(!game.field_flash_white_frame());
                }
                assert!(
                    saw_copy,
                    "must actually service the textbox resource transfer"
                );
                assert!(game.field_flash.is_none());
                assert_eq!(game.state.screen, GameScreen::StartMenu);
                assert!(game.start_menu.input_ready());
                assert_eq!(
                    game.start_menu.cursor(),
                    cursor,
                    "short pulse cannot leak through restoration"
                );
                frame(&mut game, &mut input, DOWN);
                assert_ne!(
                    game.start_menu.cursor(),
                    cursor,
                    "START must accept a later fresh press"
                );
            }
        }
    });
}

#[test]
fn submenu_handoff_counts_play_time_once_per_update() {
    run(|| {
        for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
            let (mut game, mut input) = fixture(MapId::ViridianCity, screen.clone());
            let mut expected = game.save_data.game_data.play_time;
            for tick in 0..80 {
                expected.tick();
                frame(&mut game, &mut input, if tick == 0 { B } else { 0 });
                assert_eq!(game.save_data.game_data.play_time, expected,
                    "{screen:?} must keep one timer tick across the native/IRQ handoff at {tick}");
            }
        }
    });
}

#[test]
fn submenu_restoration_cached_frames_match_full_render_and_keep_options() {
    run(|| {
        for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
            let (mut game, mut input) = fixture(MapId::ViridianCity, screen.clone());
            if screen == GameScreen::OptionsMenu {
                frame(&mut game, &mut input, RIGHT);
                frame(&mut game, &mut input, 0);
            }
            let options = game.options_menu.options;
            let mut session = crate::render::session::RenderSession::new();
            let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            let mut reused_during_restoration = false;
            for tick in 0..80 {
                frame(&mut game, &mut input, if tick == 0 { B } else { 0 });
                let update = session.render(&mut game, &mut cached, &mut |_, _, _, _, _, _| {});
                reused_during_restoration |= game.field_flash.is_some()
                    && matches!(update, crate::render::session::FrameUpdate::Reuse);
                game.draw(&mut full);
                assert_eq!(
                    cached.packed(),
                    full.packed(),
                    "{screen:?} return frame {tick}"
                );
            }
            assert!(reused_during_restoration, "unchanged scanout must reuse its existing pixels");
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            if screen == GameScreen::OptionsMenu {
                assert_eq!(game.save_data.game_data.options, options);
            }
        }
    });
}

/// AutoBgMapTransfer keeps the old menu in portions that have not reached
/// VRAM yet. Check map pixels away from the player and START's text so font
/// metrics and menu layout cannot turn an instant whole-screen swap green.
#[test]
fn submenu_return_preserves_untransferred_background_bands() {
    run(|| {
        let mut failures = Vec::new();
        for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
            let (mut game, mut input) = fixture_at(MapId::ViridianCity, screen.clone(), (20, 30));
            let mut image = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            game.draw(&mut image);
            let bands = |frame: &FrameBuffer| -> Vec<Vec<u8>> {
                (0..3).map(|band| {
                    (band * 48..(band + 1) * 48).flat_map(|y| {
                        (0..32).map(move |x| frame.get_index(x, y).unwrap() as u8)
                    }).collect()
                }).collect()
            };
            let before = bands(&image);
            let mut observed = Vec::new();
            for tick in 0..24 {
                frame(&mut game, &mut input, if tick < 2 { B } else { 0 });
                game.draw(&mut image);
                observed.push(bands(&image));
            }
            assert_eq!(game.state.screen, GameScreen::StartMenu);
            assert!(game.start_menu.input_ready());
            let after = observed.last().unwrap();
            assert!((0..3).all(|band| before[band] != after[band]),
                "fixture must distinguish each old menu band from its restored map");
            let masks: Vec<u8> = observed.iter().map(|image| {
                (0..3).fold(0, |mask, band| {
                    mask | (u8::from(image[band] == after[band]) << band)
                })
            }).collect();
            // Three actual six-row transfers are visible in successive
            // completed frames; the initial portion may be top, middle or
            // bottom. Do not bake a menu-specific order into this regression.
            let changed: Vec<u8> = masks.iter().copied().enumerate()
                .filter(|&(i, mask)| i == 0 || masks[i - 1] != mask)
                .map(|(_, mask)| mask).collect();
            assert_eq!(masks.iter().position(|&mask| mask != 0), Some(6),
                "{screen:?}: original cancellation publishes its first restored band at t+6");
            if changed.len() != 4 || changed[0] != 0 || changed[3] != 7
                || changed.windows(2).any(|pair| (pair[0] ^ pair[1]).count_ones() != 1)
            {
                failures.push(format!("{screen:?}: published background masks {changed:?}"));
            }
            for (tick, (image, mask)) in observed.iter().zip(masks).enumerate() {
                for band in 0..3 {
                    if mask & (1 << band) == 0 {
                        assert!(image[band] == before[band],
                            "{screen:?}: tick {tick}, band {band} was partially overwritten; masks {changed:?}");
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("; "));
    });
}

#[test]
fn submenu_return_preserves_the_phase_advanced_while_waiting_in_native_ui() {
    run(|| {
        for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
            let mut first_bands = Vec::new();
            for extra_idle in 0..4 {
                let (mut game, mut input) =
                    fixture_at(MapId::ViridianCity, screen.clone(), (20, 30));
                for _ in 0..extra_idle {
                    frame(&mut game, &mut input, 0);
                }
                let mut first = None;
                for tick in 0..24 {
                    frame(&mut game, &mut input, if tick < 2 { B } else { 0 });
                    if let Some(raster) = game.source_display.device().background_transfer_raster() {
                        let mask = (0..3).fold(0u8, |mask, band| {
                            mask | (u8::from(raster.rows()[band * 48] != 0) << band)
                        });
                        if mask != 0 { first.get_or_insert(mask); }
                    }
                }
                assert_eq!(game.state.screen, GameScreen::StartMenu);
                first_bands.push(first.expect("the live transfer must publish its first band"));
            }
            assert!(first_bands.iter().all(|mask| mask.count_ones() == 1));
            for pair in first_bands.windows(2) {
                assert_eq!(pair[1], ((pair[0] << 1) & 7) | (pair[0] >> 2),
                    "{screen:?}: one more idle VBlank must rotate the first published region; {first_bands:?}");
            }
        }
    });
}

#[test]
fn submenu_player_returns_through_scanned_oam_before_the_last_background_band() {
    run(|| {
        for screen in [GameScreen::Bag, GameScreen::OptionsMenu] {
            let (mut game, mut input) = fixture_at(MapId::ViridianCity, screen.clone(), (20, 30));
            let mut image = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
            let mut first_visible_oam = None;
            let mut player_frames = Vec::new();
            let mut opaque_player_pixels = Vec::new();
            for tick in 0..16 {
                frame(&mut game, &mut input, if tick < 2 { B } else { 0 });
                game.draw(&mut image);
                let player: Vec<_> = (60..76).flat_map(|y| {
                    let image = &image;
                    (64..80).map(move |x| image.get_index(x, y).unwrap())
                }).collect();
                player_frames.push(player);
                if let Some(raster) = game.source_display.device().native_object_raster() {
                    if raster.spans().iter().any(|span| span.oam.chunks_exact(4)
                        .any(|object| object[0] > 0 && object[0] < 160)) {
                        first_visible_oam.get_or_insert(tick);
                        assert!(game.field_flash.is_some(), "objects must be independent of the final menu switch");
                        if opaque_player_pixels.is_empty() {
                            let span = &raster.spans()[0];
                            let flow = game.field_flash.as_ref().unwrap();
                            for object in span.oam.chunks_exact(4).take(4) {
                                assert!(object[0] >= 16 && object[1] >= 8);
                                assert_eq!(object[3] & 0x80, 0, "this fixture's player is in front of the background");
                                for dy in 0..8u8 {
                                    let row = if object[3] & 0x40 == 0 { dy } else { 7 - dy };
                                    let address = 0x8000 + u16::from(object[2]) * 16 + u16::from(row) * 2;
                                    let low = flow.patterns.read(address).unwrap();
                                    let high = flow.patterns.read(address + 1).unwrap();
                                    for dx in 0..8u8 {
                                        let bit = if object[3] & 0x20 == 0 { 7 - dx } else { dx };
                                        let color = ((low >> bit) & 1) | (((high >> bit) & 1) << 1);
                                        if color == 0 { continue; }
                                        let palette = span.palettes[usize::from((object[3] >> 4) & 1)];
                                        let shade = dotzuki_renderer::palette::GbColor::from_u8((palette >> (color * 2)) & 3);
                                        let (x, y) = (u32::from(object[1] - 8 + dx), u32::from(object[0] - 16 + dy));
                                        assert_eq!(image.get_index(x, y), Some(shade), "first scanned player pixel at ({x}, {y})");
                                        opaque_player_pixels.push((x, y, shade));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            assert_eq!(first_visible_oam, Some(7), "{screen:?}: the reference player's first completed scan is t+7");
            assert_ne!(player_frames[6], player_frames[7], "the player must actually become visible");
            // A bottom-first transfer leaves the old middle background under
            // the visible player at t+7. Compare opaque sprite pixels, not the
            // transparent holes through which that legitimate old UI shows.
            assert!(!opaque_player_pixels.is_empty());
            for (x, y, shade) in opaque_player_pixels {
                assert_eq!(image.get_index(x, y), Some(shade), "standing player must match the final native image at ({x}, {y})");
            }
        }
    });
}

#[test]
fn title_options_still_returns_to_main_menu_without_field_restoration() {
    run(|| {
        let (mut game, mut input) = fixture(MapId::ViridianCity, GameScreen::OptionsMenu);
        game.main_menu.last_choice = Some(pokered_core::game_state::MainMenuChoice::Option);
        frame(&mut game, &mut input, B);
        assert_eq!(game.state.screen, GameScreen::MainMenu);
        assert!(game.field_flash.is_none());
        let audio = game.audio.as_ref().unwrap().manager.lock().unwrap();
        assert_eq!(audio.sequencer.current_sfx_id, SfxId::PressAB as u8);
        assert!(audio.is_sfx_playing());
    });
}
