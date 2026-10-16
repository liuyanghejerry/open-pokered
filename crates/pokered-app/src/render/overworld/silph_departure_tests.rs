use super::*;
use pokered_core::overworld::OverworldInput;
use pokered_core::snapshot::OverworldSnapshot;
use pokered_data::impl_traits::PokemonRedData;

#[test]
fn silph_departure_real_map_cached_draw_matches_raw_redraw_and_does_not_tick_the_owner() {
    let root = pokered_renderer::resource::AssetRoot::auto_detect().expect("test graphics");
    let mut full_resources = Some(ResourceManager::new(root.clone()));
    let mut cached_resources = Some(ResourceManager::new(root));
    let mut screen = OverworldScreen::new(MapId::SilphCo11F, None, PokemonRedData);
    screen.state.player.x = 6;
    screen.state.player.y = 14;
    let mut cache = OverworldBackgroundCache::new(160, 144);
    let mut full = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut cached = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    // Prime the normal composited background; entering/exiting the effect must
    // invalidate it, rather than reusing an already mapped palette stage.
    draw_overworld_cached(
        &mut screen,
        &mut cached_resources,
        &mut cached,
        pokered_core::game_state::Lang::En,
        &mut cache,
    );
    screen
        .reload_scene_source(
            "SilphCo11F",
            "game_scene SilphCo11F {\n  @load {\n    silphRocketDeparture()\n  }\n}",
        )
        .unwrap();
    screen.run_on_load();
    let mut black_frames = 0;
    let mut normal_stage_while_blocked = false;
    for elapsed in 0..=67 {
        let before = serde_json::to_value(OverworldSnapshot::capture(&screen)).unwrap();
        draw_overworld(
            &mut screen,
            &mut full_resources,
            &mut full,
            pokered_core::game_state::Lang::En,
        );
        draw_overworld_cached(
            &mut screen,
            &mut cached_resources,
            &mut cached,
            pokered_core::game_state::Lang::En,
            &mut cache,
        );
        assert_eq!(cached.packed(), full.packed(), "palette wait {elapsed}");
        let pixels = cached.packed().to_vec();
        draw_overworld_cached(
            &mut screen,
            &mut cached_resources,
            &mut cached,
            pokered_core::game_state::Lang::En,
            &mut cache,
        );
        assert_eq!(pixels, cached.packed(), "repeat draw wait {elapsed}");
        assert_eq!(
            serde_json::to_value(OverworldSnapshot::capture(&screen)).unwrap(),
            before,
            "draw cannot advance the host/VM"
        );
        if (24..=42).contains(&elapsed) {
            for y in 0..144 {
                for x in 0..160 {
                    assert_eq!(
                        cached.get_pixel(x, y),
                        Some(Rgba::BLACK),
                        "black wait {elapsed}: {x},{y}"
                    );
                }
            }
            black_frames += 1;
        }
        if elapsed == 59 {
            assert_eq!(screen.silph_rocket_departure_palette_index(), Some(3));
            assert!(!screen.script_engine_idle());
            assert!(
                cached.packed().iter().any(|byte| *byte != 0xff),
                "normal stage visible before final eight waits finish"
            );
            normal_stage_while_blocked = true;
        }
        if elapsed < 67 {
            screen.update_frame(OverworldInput::new(
                false, false, false, false, false, false, false, false,
            ));
        }
    }
    assert_eq!(black_frames, 19);
    assert!(normal_stage_while_blocked);
    assert!(screen.script_engine_idle());
    assert_eq!(screen.silph_rocket_departure_palette_index(), None);
}
