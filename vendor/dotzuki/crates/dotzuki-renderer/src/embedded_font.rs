//! Embedded 10px bitmap font — Fusion Pixel 10px Monospaced (OFL-1.1).
//!
//! Latin glyphs: 5px half-width (advance=5), CJK glyphs: 10px full-width (advance=10).
//! Glyph data is generated at build time from BDF files by `build.rs`.
//!
//! All text rendering funnels through `draw_text()` which auto-dispatches
//! Latin vs CJK based on character codepoint.

use crate::FbSurface;
use dotzuki_engine::render::Rgba;

/// Default line height in pixels (Fusion Pixel 10px baseline-to-baseline).
pub const GLYPH_SIZE: u32 = 10;
/// Baseline offset for CJK glyph positioning.
const CJK_BASELINE: i32 = 2;

// ── Generated Glyph Data ──────────────────────────────────────────

/// Compact binary glyph blob baked by `build.rs` from the Fusion Pixel BDF
/// (full ~24k-char repertoire). Layout (little-endian):
///   u32 count
///   count × { u32 codepoint, u32 data_offset }   (sorted ascending by codepoint)
///   per glyph @ data_offset:
///     u8 width, u8 nrows, i16 x_off, i16 y_off, u8 advance, (u16 × nrows) rows
static GLYPH_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/glyphs.bin"));

#[inline]
fn blob_u32(off: usize) -> u32 {
    u32::from_le_bytes([
        GLYPH_BLOB[off],
        GLYPH_BLOB[off + 1],
        GLYPH_BLOB[off + 2],
        GLYPH_BLOB[off + 3],
    ])
}

// ── Runtime Glyph Info ────────────────────────────────────────────

struct GlyphInfo {
    width: u32,
    height: u32,
    x_off: i32,
    y_off: i32,
    advance: u32,
    /// Raw little-endian u16 bitmap rows (`height` rows, 2 bytes each), borrowed
    /// from `GLYPH_BLOB`.
    rows: &'static [u8],
}

fn lookup_glyph(ch: char) -> Option<GlyphInfo> {
    let cp = ch as u32;
    let count = blob_u32(0) as usize;
    // Binary search the codepoint-sorted index (8 bytes/entry, starting at offset 4).
    let (mut lo, mut hi) = (0usize, count);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let entry = 4 + mid * 8;
        let mcp = blob_u32(entry);
        if mcp < cp {
            lo = mid + 1;
        } else if mcp > cp {
            hi = mid;
        } else {
            let off = blob_u32(entry + 4) as usize;
            let width = GLYPH_BLOB[off] as u32;
            let nrows = GLYPH_BLOB[off + 1] as usize;
            let x_off = i16::from_le_bytes([GLYPH_BLOB[off + 2], GLYPH_BLOB[off + 3]]) as i32;
            let y_off = i16::from_le_bytes([GLYPH_BLOB[off + 4], GLYPH_BLOB[off + 5]]) as i32;
            let advance = GLYPH_BLOB[off + 6] as u32;
            let rows = &GLYPH_BLOB[off + 7..off + 7 + nrows * 2];
            return Some(GlyphInfo {
                width,
                height: nrows as u32,
                x_off,
                y_off,
                advance,
                rows,
            });
        }
    }
    None
}

/// Returns true if the character has a CJK-width glyph (advance >= 10).
pub fn is_cjk(ch: char) -> bool {
    lookup_glyph(ch).map_or(false, |g| g.advance >= 10)
}

/// The pixel advance width of `ch` — the amount the cursor moves after drawing
/// it. Latin (half-width) → 5, CJK (full-width) → 10, the ▶/▼ cursor fallbacks →
/// 10, unknown → 0. This is the single source of truth for proportional text
/// measurement (see [`measure_text`] and the layout engine's proportional mode).
pub fn char_advance(ch: char) -> u32 {
    if ch == '▶' || ch == '▼' {
        return 10;
    }
    lookup_glyph(ch).map_or(0, |g| g.advance)
}

/// The total pixel width of `text` (sum of [`char_advance`] over its chars).
pub fn measure_text(text: &str) -> u32 {
    text.chars().map(char_advance).sum()
}

// ── Fallback Cursor Glyphs ────────────────────────────────────────

// 8×8 fallback bitmaps for the cursor arrows (not in the BDF blob). The
// Fusion Pixel baseline sinks text ink to tile rows 3..=9 (Latin) / 1..=9
// (CJK) — center 6.0 — so the ▶ triangle occupies rows 3..=7 and is drawn
// one extra pixel lower (see draw_char) to land its center on the same row
// 6.0 instead of floating ~2.5px above the text line.
const GLYPH_CURSOR_RIGHT: [u8; 8] = [
    0b00000000, 0b00000000, 0b00000000, 0b01111000, 0b01111100, 0b01111110, 0b01111100, 0b01111000,
];
const GLYPH_CURSOR_DOWN: [u8; 8] = [
    0b00000000, 0b00000000, 0b01111110, 0b01111110, 0b00111100, 0b00011000, 0b00000000, 0b00000000,
];

/// Extra vertical offset for the ▶ fallback (see the comment on
/// [`GLYPH_CURSOR_RIGHT`]). Applied per scale step in `draw_char_scaled`.
const CURSOR_RIGHT_DY: u32 = 1;

// ── Public Drawing API ────────────────────────────────────────────

fn draw_glyph_pixel(x: u32, y: u32, color: Rgba, fb: &mut impl FbSurface) {
    if x < fb.width() && y < fb.height() {
        fb.set_pixel(x, y, color);
    }
}

/// Draw a single character at pixel (x, y). Returns advance width for cursor positioning.
/// Latin (half-width): returns 5. CJK (full-width): returns 10. Unknown: returns 0.
pub fn draw_char(ch: char, x: u32, y: u32, color: Rgba, fb: &mut impl FbSurface) -> u32 {
    let fb_h = fb.height();
    // Fallback for cursor arrows (not in BDF)
    if ch == '▶' {
        let y = y + CURSOR_RIGHT_DY;
        for row in 0..8u32 {
            let py = y + row;
            if py >= fb_h {
                break;
            }
            let byte = GLYPH_CURSOR_RIGHT[row as usize];
            for col in 0..8u32 {
                if byte & (0x80 >> col) != 0 {
                    draw_glyph_pixel(x + col, py, color, fb);
                }
            }
        }
        return 10;
    }
    if ch == '▼' {
        for row in 0..8u32 {
            let py = y + row;
            if py >= fb_h {
                break;
            }
            let byte = GLYPH_CURSOR_DOWN[row as usize];
            for col in 0..8u32 {
                if byte & (0x80 >> col) != 0 {
                    draw_glyph_pixel(x + col, py, color, fb);
                }
            }
        }
        return 10;
    }

    if let Some(g) = lookup_glyph(ch) {
        let start_y = y as i32 + CJK_BASELINE + g.y_off;
        for ri in 0..g.height as usize {
            let row = u16::from_le_bytes([g.rows[ri * 2], g.rows[ri * 2 + 1]]);
            let py = start_y + ri as i32;
            if py < 0 || py >= fb_h as i32 {
                continue;
            }
            for ci in 0..g.width {
                if row & (1 << (g.width - 1 - ci)) != 0 {
                    let px = x + ci + g.x_off as u32;
                    if px < fb.width() {
                        fb.set_pixel(px, py as u32, color);
                    }
                }
            }
        }
        return g.advance;
    }
    0
}

/// Draw text at pixel (x, y). Auto-dispatches Latin (5px half-width) vs CJK (10px full-width).
/// Cursor position is tracked in pixels. Text wraps at framebuffer width.
pub fn draw_text(text: &str, mut x: u32, y: u32, color: Rgba, fb: &mut impl FbSurface) {
    let fb_w = fb.width();
    for ch in text.chars() {
        if x >= fb_w {
            break;
        }
        let adv = draw_char(ch, x, y, color, fb);
        x += adv;
    }
}

/// Fill a `scale × scale` block of one glyph pixel (clipped to the framebuffer).
fn fill_glyph_block(x: i32, y: i32, scale: u32, color: Rgba, fb: &mut impl FbSurface) {
    for dy in 0..scale as i32 {
        let py = y + dy;
        if py < 0 || py >= fb.height() as i32 {
            continue;
        }
        for dx in 0..scale as i32 {
            let px = x + dx;
            if px < 0 || px >= fb.width() as i32 {
                continue;
            }
            fb.set_pixel(px as u32, py as u32, color);
        }
    }
}

/// Draw a single character scaled by an integer factor: every source glyph pixel
/// becomes a `scale × scale` block. `scale == 1` is identical to [`draw_char`].
/// Returns the *scaled* advance width (`char_advance(ch) * scale`).
pub fn draw_char_scaled(
    ch: char,
    x: u32,
    y: u32,
    scale: u32,
    color: Rgba,
    fb: &mut impl FbSurface,
) -> u32 {
    let scale = scale.max(1);
    if scale == 1 {
        return draw_char(ch, x, y, color, fb);
    }
    let s = scale as i32;
    // Cursor-arrow fallbacks (not in the BDF blob) — 8×8 bitmaps, 10px advance.
    if ch == '▶' || ch == '▼' {
        let (bmp, dy) = if ch == '▶' {
            (&GLYPH_CURSOR_RIGHT, CURSOR_RIGHT_DY as i32)
        } else {
            (&GLYPH_CURSOR_DOWN, 0)
        };
        for row in 0..8i32 {
            let byte = bmp[row as usize];
            for col in 0..8i32 {
                if byte & (0x80 >> col) != 0 {
                    fill_glyph_block(x as i32 + col * s, y as i32 + (row + dy) * s, scale, color, fb);
                }
            }
        }
        return 10 * scale;
    }
    if let Some(g) = lookup_glyph(ch) {
        let start_y = y as i32 + (CJK_BASELINE + g.y_off) * s;
        for ri in 0..g.height as usize {
            let row = u16::from_le_bytes([g.rows[ri * 2], g.rows[ri * 2 + 1]]);
            let py = start_y + ri as i32 * s;
            for ci in 0..g.width {
                if row & (1 << (g.width - 1 - ci)) != 0 {
                    let px = x as i32 + (ci as i32 + g.x_off) * s;
                    fill_glyph_block(px, py, scale, color, fb);
                }
            }
        }
        return g.advance * scale;
    }
    0
}

/// Draw text scaled by an integer factor (see [`draw_char_scaled`]). Advances the
/// cursor by the scaled per-glyph width. No wrapping (a title/heading is a single
/// measured line — use [`measure_text_scaled`] to centre it).
pub fn draw_text_scaled(
    text: &str,
    mut x: u32,
    y: u32,
    scale: u32,
    color: Rgba,
    fb: &mut impl FbSurface,
) {
    for ch in text.chars() {
        x += draw_char_scaled(ch, x, y, scale, color, fb);
    }
}

/// The pixel width of `text` when drawn at `scale` (i.e. [`measure_text`] × scale).
pub fn measure_text_scaled(text: &str, scale: u32) -> u32 {
    measure_text(text) * scale.max(1)
}

/// 8×8 bitmap tiles for Game Boy-style text box borders (rounded corners, 1px lines).
pub mod box_tiles {
    /// ┌
    pub const TOP_LEFT: [u8; 8] = [
        0b00000000, 0b00001111, 0b00011111, 0b00110000, 0b00100000, 0b01100000, 0b01000000,
        0b01000000,
    ];
    /// ┐
    pub const TOP_RIGHT: [u8; 8] = [
        0b00000000, 0b11110000, 0b11111000, 0b00001100, 0b00000100, 0b00000110, 0b00000010,
        0b00000010,
    ];
    /// └
    pub const BOTTOM_LEFT: [u8; 8] = [
        0b01000000, 0b01000000, 0b01100000, 0b00100000, 0b00110000, 0b00011111, 0b00001111,
        0b00000000,
    ];
    /// ┘
    pub const BOTTOM_RIGHT: [u8; 8] = [
        0b00000010, 0b00000010, 0b00000110, 0b00000100, 0b00001100, 0b11111000, 0b11110000,
        0b00000000,
    ];
    /// ─ (top edge)
    pub const HORIZONTAL: [u8; 8] = [
        0b00000000, 0b11111111, 0b11111111, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
        0b00000000,
    ];
    /// ─ (bottom edge)
    pub const HORIZONTAL_BOTTOM: [u8; 8] = [
        0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b11111111, 0b11111111,
        0b00000000,
    ];
    /// │ (left edge)
    pub const VERTICAL_LEFT: [u8; 8] = [
        0b01100000, 0b01100000, 0b01100000, 0b01100000, 0b01100000, 0b01100000, 0b01100000,
        0b01100000,
    ];
    /// │ (right edge)
    pub const VERTICAL_RIGHT: [u8; 8] = [
        0b00000110, 0b00000110, 0b00000110, 0b00000110, 0b00000110, 0b00000110, 0b00000110,
        0b00000110,
    ];

    /// Transparency masks for the border tiles above: set bits mark pixels that
    /// lie OUTSIDE the box outline. [`crate::embedded_font::draw_box_tile`]
    /// leaves these pixels untouched so the scene behind the box shows through
    /// instead of being painted over with the background color.
    pub mod outside {
        /// ┌ — everything above/left of the rounded corner stroke.
        pub const TOP_LEFT: [u8; 8] = [
            0b11111111, 0b11110000, 0b11100000, 0b11000000, 0b11000000, 0b10000000, 0b10000000,
            0b10000000,
        ];
        /// ┐ — everything above/right of the rounded corner stroke.
        pub const TOP_RIGHT: [u8; 8] = [
            0b11111111, 0b00001111, 0b00000111, 0b00000011, 0b00000011, 0b00000001, 0b00000001,
            0b00000001,
        ];
        /// └ — everything below/left of the rounded corner stroke.
        pub const BOTTOM_LEFT: [u8; 8] = [
            0b10000000, 0b10000000, 0b10000000, 0b11000000, 0b11000000, 0b11100000, 0b11110000,
            0b11111111,
        ];
        /// ┘ — everything below/right of the rounded corner stroke.
        pub const BOTTOM_RIGHT: [u8; 8] = [
            0b00000001, 0b00000001, 0b00000001, 0b00000011, 0b00000011, 0b00000111, 0b00001111,
            0b11111111,
        ];
        /// ─ (top edge) — the row above the stroke.
        pub const HORIZONTAL: [u8; 8] = [
            0b11111111, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
            0b00000000,
        ];
        /// ─ (bottom edge) — the row below the stroke.
        pub const HORIZONTAL_BOTTOM: [u8; 8] = [
            0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
            0b11111111,
        ];
        /// │ (left edge) — the column left of the stroke.
        pub const VERTICAL_LEFT: [u8; 8] = [
            0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b10000000, 0b10000000,
            0b10000000,
        ];
        /// │ (right edge) — the column right of the stroke.
        pub const VERTICAL_RIGHT: [u8; 8] = [
            0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001,
            0b00000001,
        ];
    }
}

pub fn draw_glyph(glyph: &[u8; 8], x: u32, y: u32, color: Rgba, bg: Rgba, fb: &mut impl FbSurface) {
    let fb_h = fb.height();
    let fb_w = fb.width();
    for row in 0..8u32 {
        let py = y + row;
        if py >= fb_h {
            break;
        }
        let byte = glyph[row as usize];
        for col in 0..8u32 {
            let px = x + col;
            if px >= fb_w {
                break;
            }
            if byte & (0x80 >> col) != 0 {
                fb.set_pixel(px, py, color);
            } else {
                fb.set_pixel(px, py, bg);
            }
        }
    }
}

pub fn fill_tile(x: u32, y: u32, color: Rgba, fb: &mut impl FbSurface) {
    let fb_h = fb.height();
    let fb_w = fb.width();
    for row in 0..8u32 {
        let py = y + row;
        if py >= fb_h {
            break;
        }
        for col in 0..8u32 {
            let px = x + col;
            if px >= fb_w {
                break;
            }
            fb.set_pixel(px, py, color);
        }
    }
}

/// Draw a box-border tile (see [`box_tiles`]): `color` on stroke pixels, `bg`
/// on interior pixels, and leaves pixels marked in `outside` untouched so the
/// scene behind the box shows through beyond the border line. Pair each tile
/// with its mask from [`box_tiles::outside`].
pub fn draw_box_tile(
    glyph: &[u8; 8],
    outside: &[u8; 8],
    x: u32,
    y: u32,
    color: Rgba,
    bg: Rgba,
    fb: &mut impl FbSurface,
) {
    let fb_h = fb.height();
    let fb_w = fb.width();
    for row in 0..8u32 {
        let py = y + row;
        if py >= fb_h {
            break;
        }
        let byte = glyph[row as usize];
        let out = outside[row as usize];
        for col in 0..8u32 {
            let px = x + col;
            if px >= fb_w {
                break;
            }
            let mask = 0x80 >> col;
            if out & mask != 0 {
                continue;
            }
            if byte & mask != 0 {
                fb.set_pixel(px, py, color);
            } else {
                fb.set_pixel(px, py, bg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FrameBuffer;
    use dotzuki_engine::render_config::RenderConfig;

    #[test]
    fn draw_text_does_not_panic() {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_text("Hello, World!", 0, 0, Rgba::BLACK, &mut fb);
        draw_text("Edge", 150, 140, Rgba::BLACK, &mut fb);
        draw_text("", 0, 0, Rgba::BLACK, &mut fb);
    }

    #[test]
    fn draw_text_sets_pixels() {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_text("A", 0, 0, Rgba::BLACK, &mut fb);
        let has_black = (0..12).any(|y| (0..10).any(|x| fb.get_pixel(x, y) == Some(Rgba::BLACK)));
        assert!(has_black, "Drawing 'A' should set at least one black pixel");
    }

    #[test]
    fn cjk_lookup_and_is_cjk() {
        assert!(is_cjk('你'));
        assert!(is_cjk('好'));
        assert!(!is_cjk('A'));
        assert!(!is_cjk(' '));
    }

    #[test]
    fn draw_char_returns_advance() {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        let adv_a = draw_char('A', 0, 0, Rgba::BLACK, &mut fb);
        let adv_ni = draw_char('你', 50, 0, Rgba::BLACK, &mut fb);
        assert_eq!(adv_a, 5); // Latin half-width
        assert_eq!(adv_ni, 10); // CJK full-width
    }

    #[test]
    fn cursor_right_fallback_centers_on_text_ink() {
        // The Fusion Pixel baseline sinks glyph ink below the tile top (Latin
        // caps span tile rows 3..=9 → center 6.0); the hardcoded ▶ fallback
        // must center on the same row or menu cursors float above the line.
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_char('▶', 0, 0, Rgba::BLACK, &mut fb);
        draw_char('M', 16, 0, Rgba::BLACK, &mut fb);
        let ink_center = |x0: u32| {
            let rows: Vec<u32> = (0..12)
                .filter(|&y| (0..8).any(|x| fb.get_pixel(x0 + x, y) == Some(Rgba::BLACK)))
                .collect();
            (*rows.first().unwrap() + *rows.last().unwrap()) as f64 / 2.0
        };
        let (cursor, text) = (ink_center(0), ink_center(16));
        assert!(
            (cursor - text).abs() <= 0.5,
            "▶ ink center {cursor} should match text ink center {text}"
        );
    }

    #[test]
    fn draw_text_mixed_pixels() {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_text("Hello你好", 0, 0, Rgba::BLACK, &mut fb);
        // 'H' at x=0: Latin 5px half-width, '你' starts at x=5*5=25
        assert!(fb.get_pixel(0, 3) == Some(Rgba::BLACK), "H left stroke");
        let has_cjk = (25..45).any(|x| (0..12).any(|y| fb.get_pixel(x, y) == Some(Rgba::BLACK)));
        assert!(has_cjk, "你/好 should render in x=25..44");
    }

    #[test]
    fn box_tiles_outside_masks_disjoint_from_strokes() {
        let pairs = [
            (&box_tiles::TOP_LEFT, &box_tiles::outside::TOP_LEFT),
            (&box_tiles::TOP_RIGHT, &box_tiles::outside::TOP_RIGHT),
            (&box_tiles::BOTTOM_LEFT, &box_tiles::outside::BOTTOM_LEFT),
            (&box_tiles::BOTTOM_RIGHT, &box_tiles::outside::BOTTOM_RIGHT),
            (&box_tiles::HORIZONTAL, &box_tiles::outside::HORIZONTAL),
            (
                &box_tiles::HORIZONTAL_BOTTOM,
                &box_tiles::outside::HORIZONTAL_BOTTOM,
            ),
            (
                &box_tiles::VERTICAL_LEFT,
                &box_tiles::outside::VERTICAL_LEFT,
            ),
            (
                &box_tiles::VERTICAL_RIGHT,
                &box_tiles::outside::VERTICAL_RIGHT,
            ),
        ];
        for (glyph, outside) in pairs {
            for row in 0..8 {
                assert_eq!(
                    glyph[row] & outside[row],
                    0,
                    "outside mask overlaps the stroke at row {row}"
                );
            }
        }
    }

    #[test]
    fn draw_box_tile_leaves_outside_pixels_untouched() {
        let green = Rgba::rgb(0x20, 0x60, 0x20);
        let mut fb = FrameBuffer::new(RenderConfig::new(16, 16), green);
        draw_box_tile(
            &box_tiles::TOP_LEFT,
            &box_tiles::outside::TOP_LEFT,
            0,
            0,
            Rgba::BLACK,
            Rgba::WHITE,
            &mut fb,
        );
        // (0,0) is outside the rounded corner arc: untouched.
        assert_eq!(fb.get_pixel(0, 0), Some(green));
        // (4,1) is on the stroke: ink.
        assert_eq!(fb.get_pixel(4, 1), Some(Rgba::BLACK));
        // (7,7) is interior: background fill.
        assert_eq!(fb.get_pixel(7, 7), Some(Rgba::WHITE));
    }

    #[test]
    fn full_cjk_repertoire_baked() {
        // Characters absent from the old hand-picked subset — guards against the
        // build regressing to a curated list. (陈 is why the wuxia protagonist 陈墨
        // had to be abbreviated to 墨 before the full repertoire was baked.)
        for ch in "陈剑江湖金土星令传奇侠君臣岂".chars() {
            assert!(
                lookup_glyph(ch).is_some(),
                "CJK glyph {ch:?} should be baked"
            );
            assert!(
                is_cjk(ch),
                "{ch:?} should be full-width CJK (advance >= 10)"
            );
        }
        // Full-width CJK punctuation is in the set too.
        for ch in "：，。！？「」、".chars() {
            assert!(
                lookup_glyph(ch).is_some(),
                "punctuation {ch:?} should be baked"
            );
        }
        // The blob carries full GB/CJK coverage, not a few hundred glyphs.
        assert!(
            blob_u32(0) > 20_000,
            "expected full CJK repertoire, got {} glyphs",
            blob_u32(0)
        );
    }
}
