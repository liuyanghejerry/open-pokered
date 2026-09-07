//! Regression coverage and deterministic captures for the PR #62 corrections.
use dotzuki_engine::render_config::RenderConfig;
use pokered_app::render::draw_overworld;
use pokered_core::game_state::Lang;
use pokered_core::overworld::presentation::EnterMapFlyState;
use pokered_core::overworld::{OverworldInput, OverworldScreen};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};

fn idle() -> OverworldInput {
    OverworldInput::new(false, false, false, false, false, false, false, false)
}

fn render(screen: &mut OverworldScreen) -> FrameBuffer {
    let mut rm = Some(ResourceManager::new(
        AssetRoot::auto_detect().expect("gfx assets"),
    ));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(screen, &mut rm, &mut fb, Lang::En);
    fb
}

fn fly_screen() -> OverworldScreen {
    let mut screen = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
    screen.fly_warp_to(MapId::PalletTown, 5, 6);
    for _ in 0..400 {
        screen.update_frame(idle());
        if screen
            .enter_map_fly_anim
            .as_ref()
            .is_some_and(|s| s.frame == 15)
        {
            return screen;
        }
    }
    panic!("FLY arrival did not start");
}

fn current_screen(mask: u8, start: (u16, u16)) -> OverworldScreen {
    let mut screen = OverworldScreen::new(MapId::SeafoamIslandsB3F, None, PokemonRedData);
    screen.set_flag_live("EVENT_SEAFOAM3_BOULDER1_DOWN_HOLE", mask & 1 != 0);
    screen.set_flag_live("EVENT_SEAFOAM3_BOULDER2_DOWN_HOLE", mask & 2 != 0);
    screen.pending_warp = Some(pokered_core::overworld::PendingWarp {
        dest_map: MapId::SeafoamIslandsB3F,
        dest_x: start.0 as u8,
        dest_y: start.1 as u8,
        save_last_map: false,
        arrival_spin: false,
    });
    screen.warp_fade_state = pokered_core::overworld::WarpFadeState::FadingOut {
        frames_remaining: 1,
    };
    for _ in 0..200 {
        screen.update_frame(idle());
        if screen.pending_warp.is_none()
            && screen.warp_fade_state == pokered_core::overworld::WarpFadeState::Idle
        {
            break;
        }
    }
    assert_eq!((screen.state.player.x, screen.state.player.y), start);
    screen.state.player.transport = dotzuki_engine::overworld::TransportMode::Surfing;
    screen
}

fn save_frame(screen: &mut OverworldScreen, name: &str) {
    let fb = render(screen);
    let mut img = image::RgbaImage::new(160, 144);
    for y in 0..144 {
        for x in 0..160 {
            img.put_pixel(x, y, image::Rgba(fb.get_pixel(x, y).unwrap().to_array()));
        }
    }
    let side = std::env::var("CAPTURE_SIDE").expect("CAPTURE_SIDE=before or after");
    assert!(side == "before" || side == "after");
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/screenshots/pr62-fixes");
    std::fs::create_dir_all(&dir).unwrap();
    img.save(dir.join(format!("{name}-{side}.png"))).unwrap();
}

/// Run unchanged on master and the fix branch with CAPTURE_SIDE=before/after.
#[test]
#[ignore = "explicit screenshot capture"]
fn capture_comparisons() {
    let mut screen = fly_screen();
    for frame in [15, 18, 33] {
        screen.enter_map_fly_anim = Some(EnterMapFlyState { frame });
        save_frame(&mut screen, &format!("fly-{frame}"));
    }
    for mask in [0, 3] {
        let mut screen = current_screen(mask, (15, 8));
        for _ in 0..60 {
            screen.update_frame(idle());
        }
        save_frame(&mut screen, &format!("current-{mask}"));
    }
}

#[test]
fn b3f_current_stops_only_when_both_boulders_are_down() {
    for start in [(15, 8), (18, 7), (19, 7)] {
        for mask in 0..4 {
            let mut screen = current_screen(mask, start);
            let mut crossed_channel = false;
            for _ in 0..600 {
                screen.update_frame(idle());
                crossed_channel |= (screen.state.player.x, screen.state.player.y) == (20, 11);
                // Stop before the stairs warp to B4F and its separate current.
                if mask != 3 && (screen.state.player.x, screen.state.player.y) == (20, 17) {
                    break;
                }
            }
            assert_eq!(
                crossed_channel,
                mask != 3,
                "current follows reversed RLE through (20,11)"
            );
            let position = (screen.state.player.x, screen.state.player.y);
            assert_eq!(
                position,
                if mask == 3 { start } else { (20, 17) },
                "B3F current: flags={mask}, start={start:?}"
            );
        }
    }
}

fn assert_frame_eq(actual: &FrameBuffer, expected: &FrameBuffer, context: &str) {
    for y in 0..144 {
        for x in 0..160 {
            assert_eq!(
                actual.get_pixel(x, y),
                expected.get_pixel(x, y),
                "{context}: pixel ({x},{y})"
            );
        }
    }
}

#[test]
fn fly_uses_side_wings_and_lands_at_the_player() {
    use pokered_renderer::palette::{Palette, GRAYSCALE_PALETTE};
    let mut screen = fly_screen();
    screen.enter_map_fly_anim = Some(EnterMapFlyState { frame: 0 });
    // The first coordinate is entirely beyond the right edge after anchoring
    // to our viewport. It supplies the unobstructed background for compositing.
    let background = render(&mut screen);
    screen.enter_map_fly_anim = None;
    let landed = render(&mut screen);
    assert!(
        (64..80).any(|y| (72..88).any(|x| background.get_pixel(x, y) != landed.get_pixel(x, y))),
        "the player must be hidden before the bird arrives"
    );

    let mut rm = ResourceManager::new(AssetRoot::auto_detect().unwrap());
    let bird = rm.load_sprite("bird").unwrap().tileset.clone();
    let pal = Palette::new(&[
        Rgba::TRANSPARENT,
        GRAYSCALE_PALETTE.colors[1],
        GRAYSCALE_PALETTE.colors[2],
        GRAYSCALE_PALETTE.colors[3],
    ]);
    // Independent expected poses/positions: original indexes $8/$9 select
    // sheet frames 2/5; ($40,$3c) must land exactly at viewport (72,64).
    for (frame, bx, by, sheet_frame) in [(15, 120, 49, 5), (18, 112, 54, 2), (33, 72, 64, 5)] {
        let mut expected = background.clone();
        for y in 0..16u32 {
            for x in 0..16u32 {
                let tile = bird.get(sheet_frame * 4 + (y / 8 * 2 + x / 8) as usize);
                let color = tile.render_row((y % 8) as usize, &pal)[(x % 8) as usize];
                if color != Rgba::TRANSPARENT {
                    expected.set_pixel(bx + x, by + y, color);
                }
            }
        }
        screen.enter_map_fly_anim = Some(EnterMapFlyState { frame });
        assert_frame_eq(
            &render(&mut screen),
            &expected,
            &format!("FLY frame {frame}"),
        );
    }
    // Completion must also show the player if a caller renders the done state
    // before the update loop removes it.
    screen.enter_map_fly_anim = Some(EnterMapFlyState { frame: 36 });
    assert_frame_eq(
        &render(&mut screen),
        &landed,
        "completed FLY restores player",
    );
}

#[test]
fn fly_lifecycle_freezes_input_then_restores_player() {
    let mut screen = fly_screen();
    let start = (screen.state.player.x, screen.state.player.y);
    while screen.enter_map_fly_anim.is_some() {
        screen.update_frame(OverworldInput::new(
            false, true, false, false, false, false, false, false,
        ));
        assert_eq!((screen.state.player.x, screen.state.player.y), start);
    }
    let completed = render(&mut screen);
    screen.enter_map_fly_anim = Some(EnterMapFlyState { frame: 36 });
    assert_frame_eq(&render(&mut screen), &completed, "lifecycle completion");
}
