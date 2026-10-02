//! Renderer for the end credits (`pokered_core::credits::CreditsState`).
//!
//! Port of `Credits` (engine/movie/credits.asm:184-273): credit text over a
//! white band between black letterbox bars; every mon-command screen ends
//! with the mon scrolling left as a black silhouette
//! (`DisplayCreditsMon`); the roll closes on "THE END".

use std::borrow::Cow;
use pokered_core::credits::{CreditsPhase, CreditsState};
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::palette::Palette;
use pokered_renderer::resource::ResourceManager;
use pokered_renderer::{FrameBuffer, Rgba};

use super::species_to_sprite_name;

const T: u32 = 8;

/// `HoFGBPalettes` fade ramp (credits.asm:135-140): 4 steps from white to
/// full black text.
const FADE_SHADES: [Rgba; 4] = [
    Rgba::WHITE,
    Rgba::rgb(0xAA, 0xAA, 0xAA),
    Rgba::rgb(0x55, 0x55, 0x55),
    Rgba::BLACK,
];

/// Compact description of every value consumed by [`draw_credits`].
///
/// Credits holds and each two-frame scroll step contain many consecutive
/// pixel-identical frames. The GBA frontend compares this key while the
/// logical roll continues advancing at 60 Hz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditsVisualKey {
    visual_phase: u8,
    screen_hash: u32,
    fade_step: u8,
    mon_scroll_step: u8,
}

pub fn credits_visual_key(roll: &CreditsState) -> CreditsVisualKey {
    let mut key = CreditsVisualKey {
        visual_phase: 0, // Letterbox bars only: hidden THE END or Done.
        screen_hash: 0x811c_9dc5,
        fade_step: 0,
        mon_scroll_step: 0,
    };
    if roll.opening_clear() {
        key.visual_phase = 4;
        return key;
    }
    match roll.phase() {
        CreditsPhase::TheEnd if roll.the_end_visible() => {
            key.visual_phase = 3;
            key.fade_step = roll.fade_step();
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
    key.visual_phase = if roll.phase() == CreditsPhase::Hold {
        1
    } else {
        2
    };
    key.fade_step = roll.fade_step();
    key.mon_scroll_step = roll.mon_scroll_step();
    key
}

/// Expand the original `$54` "POKé insertion" control char (`#`,
/// constants/charmap.asm:16, pokered-data charmap CHAR_POKE) into its display
/// form before drawing.
fn expand_poke(text: &str) -> Cow<'_, str> {
    if text.contains('#') {
        Cow::Owned(text.replace('#', "POKé"))
    } else {
        Cow::Borrowed(text)
    }
}

fn skip_glyphs(text: &str, count: usize) -> &str {
    if text.is_ascii() {
        return &text[count.min(text.len())..];
    }
    let byte = text
        .char_indices()
        .nth(count)
        .map_or(text.len(), |(byte, _)| byte);
    &text[byte..]
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
    if roll.opening_clear() {
        return;
    }
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
                if let Some(rm) = resources.as_mut() {
                    if let Ok(cached) = rm.load(
                        pokered_renderer::resource::AssetCategory::Credits,
                        "the_end",
                    ) {
                        let ink = FADE_SHADES[roll.fade_step() as usize];
                        let palette = Palette::new(&[Rgba::WHITE, ink, ink, ink]);
                        for (column, index) in [0, 1, 2, 2, 3, 4].into_iter().enumerate() {
                            let x = [4, 6, 8, 11, 13, 15][column] * T;
                            super::blit_single_tile(fb, &cached.tileset, index, x, 8 * T, &palette);
                            super::blit_single_tile(
                                fb,
                                &cached.tileset,
                                index + 5,
                                x,
                                9 * T,
                                &palette,
                            );
                        }
                    }
                }
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
        let text = expand_poke(line.text);
        let glyph_count = text.chars().count();
        let text_w = glyph_count as i32 * 8;
        for k in 0..=2 {
            let x = tx * 8 - b + 160 * k;
            if x >= erase_edge || x >= fb.width() as i32 || x + text_w <= 0 {
                continue;
            }
            // Left-edge clip: drop whole glyphs that start off-screen.
            let skip = if x < 0 {
                ((-x) as usize).div_ceil(8)
            } else {
                0
            };
            let visible = skip_glyphs(&text, skip.min(glyph_count));
            if visible.is_empty() {
                continue;
            }
            draw_text(visible, x.max(0) as u32, y, ink, fb);
        }
    }

    // 2. The mon silhouette at vBGMap0 columns 20-27 → x = 160-b, rows 6-12.
    if roll.phase() == CreditsPhase::MonScroll {
        if let Some(species) = screen.mon() {
            if let Some(rm) = resources.as_mut() {
                let sprite = species_to_sprite_name(&format!("{}", species));
                if let Ok(cached) = rm.load_pokemon_front(&sprite) {
                    let x = 160 - b;
                    if x + 56 > 0 && x < fb.width() as i32 {
                        super::blit_front_pic(fb, cached, x, (6 * T) as i32, false);
                    }
                }
            }
        }
    }

    if roll.phase() == CreditsPhase::MonScroll {
        fb.apply_bgp(0xfc);
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
