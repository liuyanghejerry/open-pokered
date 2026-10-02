//! Pokémon Red's tile font (8×8 and 8-pixel advance), with Fusion Pixel kept
//! for Chinese and characters outside the original English character map.
//!
//! The row-major bitmaps are exact conversions of pret/pokered's font PNGs;
//! see fonts/POKERED.md. Runtime drawing needs no PNG decoder or heap storage.

use crate::FbSurface;
use dotzuki_engine::render::Rgba;
use dotzuki_renderer::embedded_font as fusion;

pub use fusion::{draw_box_tile, draw_glyph, fill_tile, is_cjk, GLYPH_SIZE};

const FONT: &[u8; 1024] = include_bytes!("../fonts/pokered-font.bin");
const EXTRA: &[u8; 256] = include_bytes!("../fonts/pokered-font-extra.bin");
const NUMBER_SYMBOL: &[u8; 8] = include_bytes!("../fonts/pokered-number-symbol.bin");
const NAMING_UNDERSCORES: &[u8; 16] = include_bytes!("../fonts/pokered-naming-underscores.bin");

pub use pokered_data::text_layout::{char_advance, measure_text, tile_for_char};

/// One raw original tile. Includes the PK/MN ligatures at $E1/$E2.
pub fn original_tile_glyph(tile: u8) -> Option<&'static [u8; 8]> {
    let (data, offset): (&[u8], usize) = match tile {
        0x60..=0x7F => (EXTRA, (tile - 0x60) as usize * 8),
        0x80..=0xFF => (FONT, (tile - 0x80) as usize * 8),
        _ => return None,
    };
    data[offset..offset + 8].try_into().ok()
}

/// Naming first loads HpBarAndStatusGraphics at $62. Its underscores replace
/// the unused hiragana glyphs in the regular extra font (naming_screen.asm:93).
pub fn naming_underscore_glyph(raised: bool) -> &'static [u8; 8] {
    let offset = if raised { 8 } else { 0 };
    NAMING_UNDERSCORES[offset..offset + 8].try_into().unwrap()
}

fn draw_bitmap(glyph: &[u8; 8], x: u32, y: u32, scale: u32, color: Rgba, fb: &mut impl FbSurface) {
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            if bits & (0x80 >> col) != 0 {
                let px = x.saturating_add(col * scale);
                let py = y.saturating_add(row as u32 * scale);
                if px < fb.width() && py < fb.height() {
                    fb.fill_rect(
                        px,
                        py,
                        scale.min(fb.width() - px),
                        scale.min(fb.height() - py),
                        color,
                    );
                }
            }
        }
    }
}

pub fn draw_char(ch: char, x: u32, y: u32, color: Rgba, fb: &mut impl FbSurface) -> u32 {
    draw_char_scaled(ch, x, y, 1, color, fb)
}

pub fn draw_char_scaled(
    ch: char,
    x: u32,
    y: u32,
    scale: u32,
    color: Rgba,
    fb: &mut impl FbSurface,
) -> u32 {
    let scale = scale.max(1);
    if let Some(tile) = tile_for_char(ch) {
        let glyph = if ch == '№' {
            NUMBER_SYMBOL
        } else {
            original_tile_glyph(tile).unwrap()
        };
        draw_bitmap(glyph, x, y, scale, color, fb);
        8 * scale
    } else {
        fusion::draw_char_scaled(ch, x, y, scale, color, fb);
        char_advance(ch) * scale
    }
}

pub fn draw_text(text: &str, mut x: u32, y: u32, color: Rgba, fb: &mut impl FbSurface) {
    for ch in text.chars() {
        if x >= fb.width() {
            break;
        }
        x += draw_char(ch, x, y, color, fb);
    }
}

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

pub fn measure_text_scaled(text: &str, scale: u32) -> u32 {
    measure_text(text) * scale.max(1)
}

const fn extra_tile(tile: u8) -> [u8; 8] {
    let mut glyph = [0; 8];
    let mut i = 0;
    while i < 8 {
        glyph[i] = EXTRA[(tile - 0x60) as usize * 8 + i];
        i += 1;
    }
    glyph
}

/// TextBoxBorder uses the same $7A top/bottom and $7C left/right tile;
/// white pixels are opaque Game Boy background tiles, including the corners.
pub mod box_tiles {
    pub const TOP_LEFT: [u8; 8] = super::extra_tile(0x79);
    pub const HORIZONTAL: [u8; 8] = super::extra_tile(0x7A);
    pub const TOP_RIGHT: [u8; 8] = super::extra_tile(0x7B);
    pub const VERTICAL_LEFT: [u8; 8] = super::extra_tile(0x7C);
    pub const VERTICAL_RIGHT: [u8; 8] = VERTICAL_LEFT;
    pub const BOTTOM_LEFT: [u8; 8] = super::extra_tile(0x7D);
    pub const BOTTOM_RIGHT: [u8; 8] = super::extra_tile(0x7E);
    pub const HORIZONTAL_BOTTOM: [u8; 8] = HORIZONTAL;
    pub mod outside {
        pub const TOP_LEFT: [u8; 8] = [0; 8];
        pub const TOP_RIGHT: [u8; 8] = [0; 8];
        pub const BOTTOM_LEFT: [u8; 8] = [0; 8];
        pub const BOTTOM_RIGHT: [u8; 8] = [0; 8];
        pub const HORIZONTAL: [u8; 8] = [0; 8];
        pub const HORIZONTAL_BOTTOM: [u8; 8] = [0; 8];
        pub const VERTICAL_LEFT: [u8; 8] = [0; 8];
        pub const VERTICAL_RIGHT: [u8; 8] = [0; 8];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FrameBuffer;
    use dotzuki_engine::render_config::RenderConfig;

    #[test]
    fn original_font_uses_eight_pixel_cells_and_preserves_chinese_advance() {
        assert_eq!(measure_text("POKéMON 123!?"), 13 * 8);
        assert_eq!(char_advance('▶'), 8);
        assert_eq!(char_advance('▷'), 8);
        assert_eq!(char_advance('_'), 8);
        assert_eq!(char_advance('+'), 8);
        assert_eq!(char_advance('№'), 8);
        assert_eq!(char_advance('中'), fusion::char_advance('中'));
        assert_eq!(char_advance('中'), 10);
        assert_eq!(measure_text("A中1"), 26);
    }

    #[test]
    fn original_letter_pixels_fill_the_eight_by_eight_cell() {
        let mut fb = FrameBuffer::new(RenderConfig::new(24, 16), Rgba::WHITE);
        assert_eq!(draw_char('A', 8, 4, Rgba::BLACK, &mut fb), 8);
        // The original A tile is [10,28,28,44,7C,82,82,00]. Its rightmost
        // pixels occupy column 6, beyond the previous five-pixel Latin cell.
        assert_eq!(
            original_tile_glyph(0x80).unwrap(),
            &[16, 40, 40, 68, 124, 130, 130, 0]
        );
        assert_eq!(fb.get_pixel(14, 9), Some(Rgba::BLACK));
        assert_eq!(fb.get_pixel(15, 9), Some(Rgba::WHITE));
        assert_eq!(fb.get_pixel(8, 12), Some(Rgba::WHITE));
    }
    #[test]
    fn chinese_glyph_pixels_remain_identical_to_fusion_pixel() {
        let mut actual = FrameBuffer::new(RenderConfig::new(24, 16), Rgba::WHITE);
        let mut expected = FrameBuffer::new(RenderConfig::new(24, 16), Rgba::WHITE);
        assert_eq!(draw_char('中', 8, 2, Rgba::BLACK, &mut actual), 10);
        fusion::draw_char('中', 8, 2, Rgba::BLACK, &mut expected);
        for y in 0..16 {
            for x in 0..24 {
                assert_eq!(actual.get_pixel(x, y), expected.get_pixel(x, y));
            }
        }
    }
}
