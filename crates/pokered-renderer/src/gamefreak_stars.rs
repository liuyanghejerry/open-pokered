//! Game Freak splash stars from `engine/movie/splash.asm`.
use pokered_core::gamefreak_splash::GameFreakSplashState;
use pokered_data::layout_constants::intro_scene;

use crate::palette::{PaletteState, GRAYSCALE_PALETTE};
use crate::resource::ResourceManager;
use crate::{FrameBuffer, Rgba};

/// Draw the original four big-star OAM entries and falling small-star waves.
pub fn draw_stars(state: &GameFreakSplashState, rm: &mut ResourceManager, fb: &mut FrameBuffer) {
    let mut palettes = PaletteState::new(GRAYSCALE_PALETTE);
    // Both kinds of star select OAM_PAL1. Only the small-star animation
    // toggles bits 5/7, mapping its lower star to white every other step.
    palettes.obp1 = if state.small_star_blink() { 0x04 } else { 0xa4 };
    let palette = palettes.obj_palette1();

    if let Some((x, y)) = state.big_star_oam() {
        if let Ok(star) = rm.load_battle("move_anim_1") {
            // LoadShootingStarGraphics copies tiles 3/19; the right-hand
            // OAM entries reuse them with OAM_XFLIP (no vertical flip).
            for (index, dx, dy, flip_x) in [
                (3, 0, 0, false),
                (3, 8, 0, true),
                (19, 0, 8, false),
                (19, 8, 8, true),
            ] {
                if index < star.tileset.len() {
                    fb.blit_gb_tile(
                        x - 8 + dx,
                        y - 16 + dy,
                        star.tileset.get(index),
                        &palette,
                        true,
                        flip_x,
                        false,
                    );
                }
            }
        }
    }

    let positions = state.small_stars_oam();
    if positions.is_empty() {
        return;
    }
    if let Ok(star) = rm.load_splash("falling_star") {
        if star.tileset.is_empty() {
            return;
        }
        let tile = star.tileset.get(0);
        for (x, y) in positions {
            // SmallStarsOAM uses OAM_PRIO: black letterbox BG pixels hide
            // these sprites as the waves fall off the white field.
            for row in 0..8 {
                let sy = y - 16 + row as i32;
                if sy < intro_scene::BLACK_BAR_TOP_PIXEL_H as i32
                    || sy >= intro_scene::BLACK_BAR_BOTTOM_PIXEL_Y as i32
                    || sy >= fb.height() as i32
                {
                    continue;
                }
                let colors = tile.render_row(row, &palette);
                for (col, color) in colors.into_iter().enumerate() {
                    let sx = x - 8 + col as i32;
                    if sx >= 0 && sx < fb.width() as i32 && color != Rgba::TRANSPARENT {
                        fb.set_pixel(sx as u32, sy as u32, color);
                    }
                }
            }
        }
    }
}

#[cfg(all(test, not(target_os = "none")))]
mod tests {
    use super::*;
    use crate::resource::AssetRoot;
    use crate::tile::{Tile, TileSet};
    use pokered_core::gamefreak_splash::SplashInput;

    fn background() -> FrameBuffer {
        let mut fb = FrameBuffer::new(
            dotzuki_engine::render_config::RenderConfig::new(160, 144),
            Rgba::WHITE,
        );
        fb.fill_rect(0, 0, 160, 32, Rgba::BLACK);
        fb.fill_rect(0, 112, 160, 32, Rgba::BLACK);
        fb
    }

    // Pixel oracle deliberately avoids the production palette/blit helpers.
    fn sprite(
        fb: &mut FrameBuffer,
        tile: &Tile,
        x: i32,
        y: i32,
        flip: bool,
        shades: [u8; 4],
        behind_bars: bool,
    ) {
        let colors = [
            Rgba::WHITE,
            Rgba::new(170, 170, 170, 255),
            Rgba::new(85, 85, 85, 255),
            Rgba::BLACK,
        ];
        for row in 0..8 {
            for col in 0..8 {
                let sx = x + col as i32;
                let sy = y + row as i32;
                let index = tile.get(row, if flip { 7 - col } else { col }) as usize;
                if index == 0
                    || !(0..160).contains(&sx)
                    || !(0..144).contains(&sy)
                    || (behind_bars && !(32..112).contains(&sy))
                {
                    continue;
                }
                fb.set_pixel(sx as u32, sy as u32, colors[shades[index] as usize]);
            }
        }
    }

    fn compare(frame: u16, actual: &FrameBuffer, expected: &FrameBuffer) {
        for y in 0..144 {
            for x in 0..160 {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "frame {frame}, pixel ({x},{y})"
                );
            }
        }
    }

    fn assets() -> (ResourceManager, TileSet, Tile) {
        let mut rm = ResourceManager::new(AssetRoot::auto_detect().unwrap());
        let big = rm.load_battle("move_anim_1").unwrap().tileset.clone();
        let small = rm
            .load_splash("falling_star")
            .unwrap()
            .tileset
            .get(0)
            .clone();
        assert!(big.len() > 19);
        assert!(
            small.pixels.iter().flatten().any(|&p| p == 1),
            "persistent upper star"
        );
        assert!(
            small.pixels.iter().flatten().any(|&p| p >= 2),
            "blinking lower star"
        );
        (rm, big, small)
    }

    #[test]
    fn big_star_matches_original_quadrants_palette_and_clipping_every_frame() {
        let (mut rm, big, _) = assets();
        let mut state = GameFreakSplashState::new();
        for frame in 0..284 {
            if frame >= 244 {
                let mut actual = background();
                draw_stars(&state, &mut rm, &mut actual);
                let mut expected = background();
                let step = i32::from(frame - 244);
                let x = 148 - 4 * step;
                let y = -12 + 4 * step;
                sprite(&mut expected, big.get(3), x, y, false, [0, 1, 2, 2], false);
                sprite(
                    &mut expected,
                    big.get(3),
                    x + 8,
                    y,
                    true,
                    [0, 1, 2, 2],
                    false,
                );
                sprite(
                    &mut expected,
                    big.get(19),
                    x,
                    y + 8,
                    false,
                    [0, 1, 2, 2],
                    false,
                );
                sprite(
                    &mut expected,
                    big.get(19),
                    x + 8,
                    y + 8,
                    true,
                    [0, 1, 2, 2],
                    false,
                );
                compare(frame, &actual, &expected);
            }
            state.update_frame(SplashInput::none());
        }
    }

    #[test]
    fn all_small_star_waves_keep_upper_star_and_blink_lower_behind_bars() {
        let (mut rm, _, small) = assets();
        let waves = [
            [48, 64, 88, 120],
            [56, 72, 96, 112],
            [52, 76, 84, 100],
            [60, 92, 108, 116],
        ];
        let mut state = GameFreakSplashState::new();
        for frame in 0..498 {
            if frame >= 314 {
                let mut actual = background();
                draw_stars(&state, &mut rm, &mut actual);
                let mut expected = background();
                let elapsed = frame - 314;
                if elapsed < 144 {
                    let wave = elapsed / 24;
                    let step = (elapsed % 24) / 3;
                    let shades = if step % 2 == 0 {
                        [0, 1, 0, 0]
                    } else {
                        [0, 1, 2, 2]
                    };
                    for (w, xs) in waves.iter().enumerate() {
                        if w > wave as usize {
                            break;
                        }
                        for &x in xs {
                            sprite(
                                &mut expected,
                                &small,
                                x - 8,
                                89 + 8 * (wave as i32 - w as i32) + step as i32,
                                false,
                                shades,
                                true,
                            );
                        }
                    }
                }
                compare(frame, &actual, &expected);
            }
            state.update_frame(SplashInput::none());
        }
    }
}
