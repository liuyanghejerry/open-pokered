//! The project's Fusion Pixel font, plus dedicated Game Boy UI graphics.
//!
//! Ordinary English, Chinese and mixed text keeps the engine's original
//! glyphs and metrics. The opaque tile helpers below serve only box borders.

use crate::{FbSurface, FrameBuffer};
use dotzuki_engine::render::Rgba;
use dotzuki_renderer::embedded_font as fusion;
use dotzuki_renderer::palette::GbColor;

pub use fusion::{
    char_advance, draw_box_tile, draw_char, draw_char_scaled, draw_text, draw_text_scaled,
    fill_tile, is_cjk, measure_text, measure_text_scaled, GLYPH_SIZE,
};

type LocalGlyph = (u32, u32, u32, i32, i32, u32, &'static [u16]);
const LOCAL_GLYPHS: &[LocalGlyph] = include!(concat!(env!("OUT_DIR"), "/cjk_glyphs.rs"));

fn text_ink(fb: &mut FrameBuffer, color: Rgba) -> GbColor {
    let display = *fb.display_palette();
    fb.reset_palette();
    let ink = dotzuki_renderer::indexed_framebuffer::quantize(fb.display_palette(), color);
    fb.set_palette(display);
    ink
}

fn draw_indexed_glyph(ch: char, x: u32, y: u32, ink: GbColor, fb: &mut FrameBuffer) -> Option<u32> {
    if ch == '▶' || ch == '▼' {
        return None;
    }
    let ascii = (ch as u32)
        .checked_sub(u32::from(' '))
        .map(|index| index as usize);
    let index = match ascii {
        Some(index) if index < 95 && LOCAL_GLYPHS[index].0 == ch as u32 => index,
        _ => LOCAL_GLYPHS
            .binary_search_by_key(&(ch as u32), |glyph| glyph.0)
            .ok()?,
    };
    let (_, width, height, x_off, y_off, advance, rows) = LOCAL_GLYPHS[index];
    let (fb_width, fb_height) = (fb.width(), fb.height());
    #[cfg(all(target_os = "none", target_arch = "arm"))]
    let indices = fb.indices_mut();
    let start_y = y as i32 + 2 + y_off;
    let glyph_x = x + x_off as u32;
    for ri in 0..height as usize {
        let row = rows[ri];
        let py = start_y + ri as i32;
        if py < 0 || py >= fb_height as i32 {
            continue;
        }
        #[cfg(all(target_os = "none", target_arch = "arm"))]
        let row_start = py as usize * fb_width as usize;
        let mut bits = row;
        while bits != 0 {
            let bit = 15 - bits.leading_zeros();
            let ci = width - 1 - bit;
            bits ^= 1 << bit;
            let px = glyph_x + ci;
            if px < fb_width {
                #[cfg(not(all(target_os = "none", target_arch = "arm")))]
                fb.set_pixel_index(px, py as u32, ink);
                #[cfg(all(target_os = "none", target_arch = "arm"))]
                {
                    indices[row_start + px as usize] = ink as u8;
                }
            }
        }
    }
    Some(advance)
}

pub fn draw_char_indexed(ch: char, x: u32, y: u32, color: Rgba, fb: &mut FrameBuffer) -> u32 {
    let ink = text_ink(fb, color);
    draw_indexed_glyph(ch, x, y, ink, fb).unwrap_or_else(|| fusion::draw_char(ch, x, y, color, fb))
}

pub fn draw_text_indexed(text: &str, mut x: u32, y: u32, color: Rgba, fb: &mut FrameBuffer) {
    let ink = text_ink(fb, color);
    for ch in text.chars() {
        if x >= fb.width() {
            break;
        }
        x += draw_indexed_glyph(ch, x, y, ink, fb)
            .unwrap_or_else(|| fusion::draw_char(ch, x, y, color, fb));
    }
}

#[inline]
pub fn char_advance_indexed(ch: char) -> u32 {
    if ch == '▶' || ch == '▼' {
        return fusion::char_advance(ch);
    }
    let ascii = (ch as u32)
        .checked_sub(u32::from(' '))
        .map(|index| index as usize);
    let index = match ascii {
        Some(index) if index < 95 && LOCAL_GLYPHS[index].0 == ch as u32 => Some(index),
        _ => LOCAL_GLYPHS
            .binary_search_by_key(&(ch as u32), |glyph| glyph.0)
            .ok(),
    };
    index.map_or_else(|| fusion::char_advance(ch), |i| LOCAL_GLYPHS[i].5)
}

pub fn measure_text_indexed(text: &str) -> u32 {
    text.chars().map(char_advance_indexed).sum()
}

const BOX_TILES: &[u8; 48] = include_bytes!("../fonts/pokered-box-tiles.bin");
/// Draw an opaque UI tile. Fill its paper once to avoid per-pixel color
/// quantization for all white pixels of corners, symbols and blank tiles.
pub fn draw_glyph(glyph: &[u8; 8], x: u32, y: u32, color: Rgba, bg: Rgba, fb: &mut impl FbSurface) {
    if x >= fb.width() || y >= fb.height() {
        return;
    }
    fb.fill_rect(x, y, 8.min(fb.width() - x), 8.min(fb.height() - y), bg);
    for (row, bits) in glyph.iter().enumerate() {
        if *bits == 0 {
            continue;
        }
        for col in 0..8 {
            if bits & (0x80 >> col) != 0 {
                fb.set_pixel(x.saturating_add(col), y.saturating_add(row as u32), color);
            }
        }
    }
}

const fn box_tile(index: usize) -> [u8; 8] {
    let mut glyph = [0; 8];
    let mut i = 0;
    while i < 8 {
        glyph[i] = BOX_TILES[index * 8 + i];
        i += 1;
    }
    glyph
}

/// TextBoxBorder uses the same $7A top/bottom and $7C left/right tile;
/// white pixels are opaque background tiles, including the corners.
pub mod box_tiles {
    pub const TOP_LEFT: [u8; 8] = super::box_tile(0);
    pub const HORIZONTAL: [u8; 8] = super::box_tile(1);
    pub const TOP_RIGHT: [u8; 8] = super::box_tile(2);
    pub const VERTICAL_LEFT: [u8; 8] = super::box_tile(3);
    pub const VERTICAL_RIGHT: [u8; 8] = VERTICAL_LEFT;
    pub const BOTTOM_LEFT: [u8; 8] = super::box_tile(4);
    pub const BOTTOM_RIGHT: [u8; 8] = super::box_tile(5);
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
    fn batched_opaque_ui_tiles_match_per_pixel_painter() {
        let graphics = [
            box_tiles::TOP_LEFT, box_tiles::HORIZONTAL, box_tiles::TOP_RIGHT,
            box_tiles::VERTICAL_LEFT, box_tiles::BOTTOM_LEFT, box_tiles::BOTTOM_RIGHT,
            [0; 8], [0xAA, 0x55, 0x81, 0x42, 0x24, 0x18, 0xFF, 0],
        ];
        for (tile, glyph) in graphics.iter().enumerate() {
            for (x, y) in [(0, 0), (1, 1), (15, 9), (16, 12), (17, 13), (u32::MAX, u32::MAX)] {
                for ink in [Rgba::BLACK, Rgba::INK_DARK_GRAY, Rgba::WHITE, Rgba::rgb(17, 119, 201), Rgba::TRANSPARENT] {
                    for paper in [Rgba::WHITE, Rgba::INK_LIGHT_GRAY, Rgba::BLACK] {
                        let mut expected = FrameBuffer::new(RenderConfig::new(17, 13), Rgba::INK_DARK_GRAY);
                        let mut actual = expected.clone();
                        fusion::draw_glyph(glyph, x, y, ink, paper, &mut expected);
                        draw_glyph(glyph, x, y, ink, paper, &mut actual);
                        for py in 0..13 {
                            for px in 0..17 {
                                assert_eq!(actual.get_pixel(px, py), expected.get_pixel(px, py),
                                    "UI tile {tile}, origin({x},{y}), {ink:?}/{paper:?}, pixel({px},{py})");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn project_font_metrics_keep_half_width_latin_and_full_width_chinese() {
        assert_eq!(char_advance('A'), 5);
        assert_eq!(char_advance('中'), 10);
        assert_eq!(measure_text("A中1"), 20);
        assert_eq!(measure_text_scaled("A中1", 2), 40);
    }

    #[test]
    fn english_chinese_and_mixed_text_keep_project_font_pixels() {
        for text in ["POKéMON 123!?", "中文皮卡丘", "Pikachu等级10", "№ $×…▷▶▼"] {
            for scale in [1, 2] {
                let mut actual = FrameBuffer::new(RenderConfig::new(96, 32), Rgba::WHITE);
                let mut expected = actual.clone();
                draw_text_scaled(text, 3, 2, scale, Rgba::BLACK, &mut actual);
                fusion::draw_text_scaled(text, 3, 2, scale, Rgba::BLACK, &mut expected);
                assert_eq!(measure_text(text), fusion::measure_text(text));
                assert_eq!(measure_text_indexed(text), fusion::measure_text(text));
                for y in 0..32 {
                    for x in 0..96 {
                        assert_eq!(actual.get_pixel(x, y), expected.get_pixel(x, y), "{text}, scale {scale}, pixel({x},{y})");
                    }
                }
            }
        }
    }

    #[test]
    fn indexed_text_matches_fusion_and_restores_the_display_palette() {
        let bases = [
            crate::palette::GRAYSCALE_PALETTE,
            crate::palette::Palette::<GbColor>::new(&[
                Rgba::BLACK,
                Rgba::WHITE,
                Rgba::INK_DARK_GRAY,
                Rgba::INK_LIGHT_GRAY,
            ]),
        ];
        for base in bases {
            for text in ["POKéMON 123!?", "中文皮卡丘", "Pikachu等级10", "№ $×…▷▶▼"]
            {
                let mut actual =
                    FrameBuffer::with_palette(RenderConfig::new(96, 32), Rgba::WHITE, base);
                actual.set_palette(crate::palette::GRAYSCALE_PALETTE);
                let mut expected = actual.clone();
                draw_text_indexed(text, 3, 2, Rgba::BLACK, &mut actual);
                fusion::draw_text(text, 3, 2, Rgba::BLACK, &mut expected);
                assert_eq!(actual.display_palette(), expected.display_palette());
                for y in 0..32 {
                    for x in 0..96 {
                        assert_eq!(
                            actual.get_pixel(x, y),
                            expected.get_pixel(x, y),
                            "{text}, pixel({x},{y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_local_ascii_glyph_matches_fusion() {
        for ch in ' '..='~' {
            let mut actual = FrameBuffer::new(RenderConfig::new(16, 16), Rgba::WHITE);
            let mut expected = actual.clone();
            assert_eq!(
                draw_char_indexed(ch, 3, 2, Rgba::BLACK, &mut actual),
                fusion::draw_char(ch, 3, 2, Rgba::BLACK, &mut expected),
                "{ch} advance",
            );
            for y in 0..16 {
                for x in 0..16 {
                    assert_eq!(
                        actual.get_pixel(x, y),
                        expected.get_pixel(x, y),
                        "{ch}, pixel({x},{y})"
                    );
                }
            }
        }
    }
}
