//! Original Red/Blue surfing uses SeelSprite, with the standard six-frame layout.
use dotzuki_engine::{overworld::types::TransportMode, render_config::RenderConfig};
use pokered_app::render::draw_overworld;
use pokered_core::{
    game_state::Lang,
    overworld::{Direction, MovementState, OverworldScreen},
};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
use pokered_renderer::{
    resource::{AssetRoot, ResourceManager},
    FrameBuffer, Rgba,
};

fn screen() -> OverworldScreen {
    let mut s = OverworldScreen::new(MapId::SeafoamIslandsB3F, None, PokemonRedData);
    s.state.player.x = 18;
    s.state.player.y = 10;
    s.state.player.facing = Direction::Down;
    s.state.player.transport = TransportMode::Surfing;
    s
}

fn render(s: &mut OverworldScreen) -> FrameBuffer {
    let mut rm = Some(ResourceManager::new(
        AssetRoot::auto_detect().expect("gfx assets"),
    ));
    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    draw_overworld(s, &mut rm, &mut fb, Lang::En);
    fb
}

/// Explicitly run on master and the PR branch with identical fixed states.
#[test]
#[ignore = "explicit screenshot capture"]
fn capture_surf_comparisons() {
    let side = std::env::var("CAPTURE_SIDE").expect("CAPTURE_SIDE=before or after");
    assert!(side == "before" || side == "after");
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/screenshots/pr62-fixes");
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = screen();
    for (name, facing, moving) in [
        ("surf-down", Direction::Down, false),
        ("surf-right", Direction::Right, true),
    ] {
        s.state.player.facing = facing;
        if moving {
            s.state.player.movement_state = MovementState::Walking;
            s.state.walk_counter = 6;
        }
        render(&mut s)
            .save_png(&dir.join(format!("{name}-{side}.png")))
            .unwrap();
    }
}

fn assert_sprite(fb: &FrameBuffer, asset: &str, frame: usize, flip: bool) {
    use pokered_renderer::palette::{Palette, GRAYSCALE_PALETTE};
    let mut rm = ResourceManager::new(AssetRoot::auto_detect().unwrap());
    let sheet = rm.load_sprite(asset).unwrap().tileset.clone();
    let pal = Palette::new(&[
        Rgba::TRANSPARENT,
        GRAYSCALE_PALETTE.colors[1],
        GRAYSCALE_PALETTE.colors[2],
        GRAYSCALE_PALETTE.colors[3],
    ]);
    let mut ink = 0;
    for y in 0..16u32 {
        for x in 0..16u32 {
            let sx = if flip { 15 - x } else { x };
            let tile = sheet.get(frame * 4 + (y / 8 * 2 + sx / 8) as usize);
            let c = tile.render_row((y % 8) as usize, &pal)[(sx % 8) as usize];
            if c != Rgba::TRANSPARENT {
                ink += 1;
                assert_eq!(
                    fb.get_pixel(72 + x, 64 + y),
                    Some(c),
                    "{asset} frame={frame} flip={flip} at ({x},{y})"
                );
            }
        }
    }
    assert!(ink > 30, "sprite must have visible pixels");
}

#[test]
fn surfing_uses_seel_in_all_facings_and_animation_phases() {
    for (dir, stand, step, right) in [
        (Direction::Down, 0, 3, false),
        (Direction::Up, 1, 4, false),
        (Direction::Left, 2, 5, false),
        (Direction::Right, 2, 5, true),
    ] {
        let mut s = screen();
        s.state.player.facing = dir;
        assert_sprite(&render(&mut s), "seel", stand, right);
        s.state.player.movement_state = MovementState::Walking;
        for (counter, frame, mirror) in [
            (8, stand, false),
            (6, step, false),
            (4, stand, false),
            (2, step, matches!(dir, Direction::Down | Direction::Up)),
        ] {
            s.state.walk_counter = counter;
            assert_sprite(&render(&mut s), "seel", frame, right || mirror);
        }
    }
}

#[test]
fn leaving_surf_restores_red_and_biking_keeps_its_sprite() {
    let mut s = screen();
    s.state.player.transport = TransportMode::Walking;
    let walking = render(&mut s);
    s.state.player.transport = TransportMode::Surfing;
    let surfing = render(&mut s);
    assert!(
        (64..80).any(|y| (72..88).any(|x| walking.get_pixel(x, y) != surfing.get_pixel(x, y))),
        "surf must replace RedSprite"
    );
    assert_sprite(&surfing, "seel", 0, false);
    s.state.player.transport = TransportMode::Walking;
    let restored = render(&mut s);
    assert_sprite(&restored, "red", 0, false);
    for y in 0..144 {
        for x in 0..160 {
            assert_eq!(walking.get_pixel(x, y), restored.get_pixel(x, y));
        }
    }
    s.state.player.transport = TransportMode::Biking;
    assert_sprite(&render(&mut s), "red_bike", 0, false);
}
