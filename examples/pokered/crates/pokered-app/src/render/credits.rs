//! Renderer for the end credits (`pokered_core::credits::CreditsState`).
//!
//! Port of `Credits` (engine/movie/credits.asm:184-273): credit text over a
//! white band between black letterbox bars; every mon-command screen ends
//! with the mon scrolling left as a black silhouette
//! (`DisplayCreditsMon`); the roll closes on "THE END".

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

/// Solid-black silhouette palette for the scrolling mon
/// (`ld a, %11111100 / ldh [rBGP]`, credits.asm:104-106).
fn silhouette_palette() -> Palette {
    let mut p = GRAYSCALE_SPRITE_PALETTE;
    p.colors[1] = Rgba::BLACK;
    p.colors[2] = Rgba::BLACK;
    p.colors[3] = Rgba::BLACK;
    p
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
        let text_w = line.text.len() as i32 * 8;
        for k in 0..=2 {
            let x = tx * 8 - b + 160 * k;
            if x >= erase_edge || x >= fb.width() as i32 || x + text_w <= 0 {
                continue;
            }
            // Left-edge clip: drop whole glyphs that start off-screen.
            let skip = if x < 0 { ((-x) as usize).div_ceil(8) } else { 0 };
            let visible = &line.text[skip.min(line.text.len())..];
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
                    let ts = cached.tileset.clone();
                    let w_tiles = cached.source_size.0 / TILE_SIZE;
                    let w_px = cached.source_size.0 as i32;
                    let x = 160 - b;
                    if x + w_px > 0 && x < fb.width() as i32 {
                        let pal = silhouette_palette();
                        blit_silhouette_left_clipped(fb, &ts, x, 6 * T, w_tiles, &pal);
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
    let skip_px = x0 - x as u32; // pixels hidden off the left edge (0 when x >= 0)
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
