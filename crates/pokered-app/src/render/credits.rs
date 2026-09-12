//! Renderer for the end credits (`pokered_core::credits::CreditsState`).
//!
//! Port of `Credits` (engine/movie/credits.asm:184-273): credit text over a
//! white band between black letterbox bars; every mon-command screen ends
//! with the mon scrolling left as a black silhouette
//! (`DisplayCreditsMon`); the roll closes on "THE END".

use crate::alloc_prelude::*;
use pokered_core::credits::{CreditsPhase, CreditsState};
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::{Palette, GRAYSCALE_SPRITE_PALETTE};
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba, TILE_SIZE};

use super::species_to_sprite_name;

const FG: Rgba = Rgba::BLACK;
const T: u32 = 8;

/// `HoFGBPalettes` fade ramp (credits.asm:135-140): 4 steps from white to
/// full black text.
const FADE_SHADES: [Rgba; 5] = [
    Rgba::WHITE,
    Rgba::rgb(0xC0, 0xC0, 0xC0),
    Rgba::rgb(0x80, 0x80, 0x80),
    Rgba::rgb(0x40, 0x40, 0x40),
    Rgba::BLACK,
];

/// Compact description of every value consumed by [`draw_credits`].
///
/// Credits holds and each two-frame scroll step contain many consecutive
/// pixel-identical frames. The GBA frontend compares this key while the
/// logical roll continues advancing at 60 Hz.
#[cfg(any(test, target_os = "none"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditsVisualKey {
    visual_phase: u8,
    screen_hash: u32,
    fade_step: u8,
    mon_scroll_step: u8,
}

#[cfg(any(test, target_os = "none"))]
pub fn credits_visual_key(roll: &CreditsState) -> CreditsVisualKey {
    let mut key = CreditsVisualKey {
        visual_phase: 0, // Letterbox bars only: hidden THE END or Done.
        screen_hash: 0x811c_9dc5,
        fade_step: 0,
        mon_scroll_step: 0,
    };
    match roll.phase() {
        CreditsPhase::TheEnd if roll.the_end_visible() => {
            key.visual_phase = 3;
            return key;
        }
        CreditsPhase::TheEnd | CreditsPhase::Done => return key,
        CreditsPhase::Hold | CreditsPhase::MonScroll => {}
    }

    let Some(screen) = roll.current_screen() else {
        return key;
    };
    let hash_byte = |hash: &mut u32, byte: u8| {
        *hash = (*hash ^ byte as u32).wrapping_mul(0x0100_0193);
    };
    hash_byte(&mut key.screen_hash, screen.lines.len() as u8);
    for line in screen.lines {
        hash_byte(&mut key.screen_hash, line.x_off as u8);
        for byte in line.text.bytes() {
            hash_byte(&mut key.screen_hash, byte);
        }
        hash_byte(&mut key.screen_hash, 0xff);
    }
    if let Some(species) = screen.mon() {
        hash_byte(&mut key.screen_hash, 1);
        hash_byte(&mut key.screen_hash, species as u8);
    } else {
        hash_byte(&mut key.screen_hash, 0);
    }
    key.visual_phase = if roll.phase() == CreditsPhase::Hold { 1 } else { 2 };
    key.fade_step = roll.fade_step();
    key.mon_scroll_step = roll.mon_scroll_step();
    key
}

/// Solid-black silhouette palette for the scrolling mon
/// (`ld a, %11111100 / ldh [rBGP]`, credits.asm:104-106).
fn silhouette_palette() -> Palette {
    let mut p = GRAYSCALE_SPRITE_PALETTE;
    p.colors[1] = Rgba::BLACK;
    p.colors[2] = Rgba::BLACK;
    p.colors[3] = Rgba::BLACK;
    p
}

/// Expand the original `$54` "POKé insertion" control char (`#`,
/// constants/charmap.asm:16, pokered-data charmap CHAR_POKE) into its display
/// form before drawing.
fn expand_poke(text: &str) -> String {
    if text.contains('#') {
        text.replace('#', "POKé")
    } else {
        text.to_string()
    }
}

/// Draw the credits roll to the 160x144 framebuffer.
pub fn draw_credits(
    roll: &CreditsState,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    // Black letterbox bars over a white middle band (FillFourRowsWithBlack ×
    // 2, credits.asm:14-17).
    fb.clear(Rgba::WHITE);
    for y in 0..(4 * T) {
        for x in 0..fb.width() {
            fb.set_pixel(x, y, Rgba::BLACK);
            fb.set_pixel(x, fb.height() - 1 - y, Rgba::BLACK);
        }
    }

    match roll.phase() {
        CreditsPhase::Hold | CreditsPhase::MonScroll => {
            if let Some(screen) = roll.current_screen() {
                let ink = FADE_SHADES[roll.fade_step() as usize];
                draw_scrolling_band(screen, ink, roll, resources, fb);
            }
        }
        CreditsPhase::TheEnd => {
            if roll.the_end_visible() {
                // hlcoord 4,8 "T H E  E N D" (TheEndTextString).
                draw_text("T H E  E N D", 4 * T, 8 * T, FG, fb);
            }
        }
        CreditsPhase::Done => {}
    }
}

/// Draw the middle band during Hold / MonScroll, modelled on the original's
/// per-scanline SCX scroll (`DisplayCreditsMon` + `ScrollCreditsMonLeft`,
/// credits.asm:56-125):
///
/// - Scanlines 0-31 and 112-143 (tiles 0-3 / 14-17) keep SCX=0 — the black
///   letterbox bars never move.
/// - Scanlines 32-111 (tiles 4-13) scroll left by `b = step * 8` px: the
///   credit text (tiles 6-8) slides with the band, and the mon silhouette
///   (tiles 6-12) crosses from the right edge (x = 160-b) to the left edge
///   over 27 steps (7 + 20 `ScrollCreditsMonLeft` calls). The tilemap copies
///   at vBGMap0 columns 12-31 make the text strip repeat seamlessly every
///   160 px while the mon's copy at columns 20-27 slides through.
/// - A white "window" (vBGMap1, middle rows filled white) sweeps in from the
///   right edge from step 7 on, covering the wrap-around text copy:
///   everything right of x = 216-b is white (the sweep tracks the mon's
///   right edge, credits.asm:107-114).
///
/// What cannot be exact on a 160×144 framebuffer: the LCD's mid-frame
/// per-scanline SCX writes (a software framebuffer has no scanline timing and
/// no tearing — only the final image matters), and the hardware window is
/// reproduced as an equivalent white fill. During the Hold phase `step` is 0
/// and the band collapses to the plain static text.
fn draw_scrolling_band(
    screen: &pokered_core::credits::CreditsScreen,
    ink: Rgba,
    roll: &CreditsState,
    resources: &mut Option<ResourceManager>,
    fb: &mut FrameBuffer,
) {
    let step = roll.mon_scroll_step() as i32;
    let b = step * 8; // SCX offset for scanlines 32-111
    let erase_edge = 216 - b; // white window sweep line (>= 160 until step 7)

    // 1. The text strip scrolls with the band, repeating every 160 px (the
    //    original's vBGMap0 copies at columns 0-19 / 12-31 tile seamlessly).
    for (i, line) in screen.lines.iter().enumerate() {
        let tx = (9i32 + line.x_off as i32).max(0) as i32;
        let y = (6 + 2 * i as u32) * T;
        // Original charmap: '#' is the $54 POKé insertion control
        // (constants/charmap.asm:16, charmap.rs CHAR_POKE) — expand to its
        // display form before measuring/clipping; clip math is char-based
        // because 'é' is multi-byte in UTF-8.
        let glyphs: Vec<char> = expand_poke(line.text).chars().collect();
        let text_w = glyphs.len() as i32 * 8;
        for k in 0..=2 {
            let x = tx * 8 - b + 160 * k;
            if x >= erase_edge || x >= fb.width() as i32 || x + text_w <= 0 {
                continue;
            }
            // Left-edge clip: drop whole glyphs that start off-screen.
            let skip = if x < 0 { ((-x) as usize).div_ceil(8) } else { 0 };
            let visible: String = glyphs[skip.min(glyphs.len())..].iter().collect();
            if visible.is_empty() {
                continue;
            }
            draw_text(&visible, x.max(0) as u32, y, ink, fb);
        }
    }

    // 2. The mon silhouette at vBGMap0 columns 20-27 → x = 160-b, rows 6-12.
    if roll.phase() == CreditsPhase::MonScroll {
        if let Some(species) = screen.mon() {
            if let Some(rm) = resources.as_mut() {
                let sprite = species_to_sprite_name(&format!("{}", species));
                if let Ok(cached) = rm.load_pokemon_front(&sprite) {
                    let w_tiles = cached.source_size.0 / TILE_SIZE;
                    let w_px = cached.source_size.0 as i32;
                    let x = 160 - b;
                    if x + w_px > 0 && x < fb.width() as i32 {
                        let pal = silhouette_palette();
                        blit_silhouette_left_clipped(
                            fb,
                            &cached.tileset,
                            x,
                            6 * T,
                            w_tiles,
                            &pal,
                        );
                    }
                }
            }
        }
    }

    // 3. White window sweep: tiles 4-13 right of the mon's right edge.
    if erase_edge < fb.width() as i32 {
        let x0 = erase_edge.max(0) as u32;
        for y in 4 * T..14 * T {
            for x in x0..fb.width() {
                fb.set_pixel(x, y, Rgba::WHITE);
            }
        }
    }
}

/// `blit_tileset` with a left-edge clip: pixels at negative x are dropped
/// instead of being shifted to x=0 (the mon slides in/out at the screen
/// edges during the credits scroll).
fn blit_silhouette_left_clipped(
    fb: &mut FrameBuffer,
    tileset: &pokered_renderer::tile::TileSet,
    x: i32,
    y: u32,
    tiles_per_row: u32,
    palette: &Palette,
) {
    let x0 = x.max(0) as u32;
    // Preserve the release-build clipping calculation without relying on an
    // overflowing signed-to-unsigned subtraction in debug builds.
    let skip_px = if x < 0 { x.unsigned_abs() } else { 0 };
    for idx in 0..tileset.len() {
        let tile = tileset.get(idx);
        let tcol = (idx as u32) % tiles_per_row;
        let trow = (idx as u32) / tiles_per_row;
        let px = x0 + tcol * TILE_SIZE;
        if px + TILE_SIZE <= skip_px {
            continue; // whole tile off-screen left
        }
        let py = y + trow * TILE_SIZE;
        for row in 0..TILE_SIZE {
            let rgba_row = tile.render_row(row as usize, palette);
            for col in 0..TILE_SIZE {
                let sx = px + col;
                if sx < skip_px || sx >= fb.width() || py + row >= fb.height() {
                    continue;
                }
                let c = rgba_row[col as usize];
                if c != Rgba::TRANSPARENT {
                    fb.set_pixel(sx, py + row, c);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::credits::CreditsInput;
    use pokered_data::wild_data::GameVersion;
    use pokered_renderer::resource::AssetRoot;

    fn new_fb() -> FrameBuffer {
        FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
    }

    fn test_resources() -> Option<ResourceManager> {
        let candidate = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gfx");
        if candidate.is_dir() {
            AssetRoot::new(candidate).ok().map(ResourceManager::new)
        } else {
            None
        }
    }

    fn assert_framebuffers_equal(
        actual: &FrameBuffer,
        expected: &FrameBuffer,
        context: &str,
    ) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());
        for y in 0..actual.height() {
            for x in 0..actual.width() {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "framebuffer mismatch at ({x}, {y}); {context}",
                );
            }
        }
    }

    /// The credits text data keeps the original `$54` control char verbatim;
    /// the renderer expands it to the display form (charmap.rs:20).
    #[test]
    fn poke_control_char_expands_to_display_form() {
        assert_eq!(expand_poke("#MON"), "POKéMON");
        assert_eq!(expand_poke("THE END"), "THE END");
        assert_eq!(expand_poke(""), "");
    }

    #[test]
    fn visual_key_only_reuses_pixel_identical_credits_frames() {
        for version in [GameVersion::Red, GameVersion::Blue] {
            let mut resources = test_resources();
            let mut roll = CreditsState::new(version);
            let mut previous: Option<(CreditsVisualKey, CreditsPhase, usize, FrameBuffer)> = None;
            let mut reused = 0;
            let mut ticks = 0;

            loop {
                let key = credits_visual_key(&roll);
                let mut current = new_fb();
                draw_credits(&roll, &mut resources, &mut current);
                if let Some((previous_key, previous_phase, previous_screen, previous_frame)) =
                    previous.as_ref()
                {
                    if *previous_key == key {
                        let context = format!(
                            "version={version:?}, previous={previous_phase:?}/screen{previous_screen}, current={:?}/screen{}, key={key:?}",
                            roll.phase(),
                            roll.screen_index(),
                        );
                        assert_framebuffers_equal(previous_frame, &current, &context);
                        reused += 1;
                    }
                }
                previous = Some((key, roll.phase(), roll.screen_index(), current));

                if roll.phase() == CreditsPhase::Done {
                    break;
                }
                let input = if roll.awaiting_final_button() {
                    CreditsInput { a: true, b: false }
                } else {
                    CreditsInput::none()
                };
                roll.update_frame(input);
                ticks += 1;
                assert!(ticks < 10_000, "credits roll must terminate");
            }

            assert!(reused > 3_000, "credits holds should be reusable");
        }
    }
}
