// no_std port (GBA / thumbv4t): see dotzuki-engine's header for the pattern.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", feature(prelude_import))]
#![cfg_attr(target_os = "none", allow(internal_features))]

extern crate alloc;

#[allow(unused_imports)]
mod alloc_prelude {
    pub use core::prelude::v1::*;
    pub use core::convert::{TryFrom, TryInto};
    pub use alloc::borrow::ToOwned;
    pub use core::iter::FromIterator;
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
    // Macros that std injects via `#[macro_use] extern crate std` but core
    // does not export through its prelude.
    pub use core::{assert_eq, assert_ne, matches, todo, unimplemented, write, writeln};
    pub use core::debug_assert;
}

#[cfg_attr(target_os = "none", prelude_import)]
#[allow(unused_imports)]
use alloc_prelude::*;

pub mod widgets;

use dotzuki_engine::hash::HashMap;

use dotzuki_renderer::embedded_font::{self, box_tiles, draw_box_tile};
use dotzuki_renderer::FrameBuffer;

pub use dotzuki_engine::menu::{BorderStyle, CursorStyle, EdgeInsets, MenuConfig};
pub use dotzuki_engine::render::{Frame, Painter, Rgba, TilePos, TileRect, Ui};

pub struct FrameBufferPainter<'fb> {
    fb: &'fb mut FrameBuffer,
    custom_tiles: HashMap<u8, &'static str>,
}

impl<'fb> FrameBufferPainter<'fb> {
    pub fn new(fb: &'fb mut FrameBuffer) -> Self {
        Self {
            fb,
            custom_tiles: HashMap::default(),
        }
    }

    /// Register game-specific GB tile ids that should render as short text
    /// instead of the `[NNN]` placeholder (e.g. a game might map its
    /// battle-menu ligature tiles 0xE1/0xE2 to "PK"/"MN"). The engine ships
    /// no game-specific tile glyphs itself; games inject their own here.
    pub fn with_custom_tiles(mut self, tiles: impl IntoIterator<Item = (u8, &'static str)>) -> Self {
        self.custom_tiles.extend(tiles);
        self
    }
}

impl Painter for FrameBufferPainter<'_> {
    fn clear(&mut self, color: Rgba) {
        self.fb.clear(color);
    }

    fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
        if rect.tw < 2 || rect.th < 2 {
            return;
        }
        let ink = color;
        let bg = Rgba::INK_WHITE;
        let t: u32 = 8;
        let bx = rect.tx * t;
        let by = rect.ty * t;
        let inner_w = rect.tw - 2;
        let inner_h = rect.th - 2;
        let right_x = bx + (rect.tw - 1) * t;
        let bot_y = by + (rect.th - 1) * t;

        draw_box_tile(
            &box_tiles::TOP_LEFT,
            &box_tiles::outside::TOP_LEFT,
            bx,
            by,
            ink,
            bg,
            self.fb,
        );
        for col in 0..inner_w {
            draw_box_tile(
                &box_tiles::HORIZONTAL,
                &box_tiles::outside::HORIZONTAL,
                bx + (1 + col) * t,
                by,
                ink,
                bg,
                self.fb,
            );
        }
        draw_box_tile(
            &box_tiles::TOP_RIGHT,
            &box_tiles::outside::TOP_RIGHT,
            right_x,
            by,
            ink,
            bg,
            self.fb,
        );

        for row in 0..inner_h {
            let y = by + (1 + row) * t;
            draw_box_tile(
                &box_tiles::VERTICAL_LEFT,
                &box_tiles::outside::VERTICAL_LEFT,
                bx,
                y,
                ink,
                bg,
                self.fb,
            );
            for col in 0..inner_w {
                embedded_font::fill_tile(bx + (1 + col) * t, y, bg, self.fb);
            }
            draw_box_tile(
                &box_tiles::VERTICAL_RIGHT,
                &box_tiles::outside::VERTICAL_RIGHT,
                right_x,
                y,
                ink,
                bg,
                self.fb,
            );
        }

        draw_box_tile(
            &box_tiles::BOTTOM_LEFT,
            &box_tiles::outside::BOTTOM_LEFT,
            bx,
            bot_y,
            ink,
            bg,
            self.fb,
        );
        for col in 0..inner_w {
            draw_box_tile(
                &box_tiles::HORIZONTAL_BOTTOM,
                &box_tiles::outside::HORIZONTAL_BOTTOM,
                bx + (1 + col) * t,
                bot_y,
                ink,
                bg,
                self.fb,
            );
        }
        draw_box_tile(
            &box_tiles::BOTTOM_RIGHT,
            &box_tiles::outside::BOTTOM_RIGHT,
            right_x,
            bot_y,
            ink,
            bg,
            self.fb,
        );
    }

    fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
        let (px, py) = pos.to_pixels();
        embedded_font::draw_text(text, px, py, color, self.fb);
    }

    // ── Proportional pixel-precise text (high-resolution path) ──────────────
    // Overrides the tile-grid defaults so the layout engine's proportional mode
    // renders CJK at true pixel precision with per-glyph advance.
    fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
        embedded_font::draw_text(text, px, py, color, self.fb);
    }

    fn measure_text_px(&self, text: &str) -> u32 {
        embedded_font::measure_text(text)
    }

    fn draw_text_px_scaled(&mut self, px: u32, py: u32, text: &str, scale: u32, color: Rgba) {
        embedded_font::draw_text_scaled(text, px, py, scale, color, self.fb);
    }

    fn measure_text_px_scaled(&self, text: &str, scale: u32) -> u32 {
        embedded_font::measure_text_scaled(text, scale)
    }

    fn supports_proportional(&self) -> bool {
        true
    }

    fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
        let (px, py) = pos.to_pixels();
        embedded_font::draw_char(glyph, px, py, color, self.fb);
    }

    fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba) {
        self.fb.fill_rect(px, py, pw, ph, color);
    }

    fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: Rgba) {
        let (px, py) = pos.to_pixels();
        let ink = color;
        let bg = Rgba::INK_WHITE;
        // Map the common Game Boy UI tile ids to real glyphs so cursors,
        // arrows and box borders render as graphics rather than `[NNN]` text.
        match tile_id {
            // Menu cursor ▶
            223 => {
                embedded_font::draw_char('\u{25B6}', px, py, ink, self.fb);
            }
            // Text-box "more text" down arrow ▼
            31 => {
                embedded_font::draw_char('\u{25BC}', px, py, ink, self.fb);
            }
            // Default box-border tile set (0x79–0x7F)
            0x79 => draw_box_tile(
                &box_tiles::TOP_LEFT,
                &box_tiles::outside::TOP_LEFT,
                px,
                py,
                ink,
                bg,
                self.fb,
            ),
            0x7A => draw_box_tile(
                &box_tiles::HORIZONTAL,
                &box_tiles::outside::HORIZONTAL,
                px,
                py,
                ink,
                bg,
                self.fb,
            ),
            0x7B => draw_box_tile(
                &box_tiles::TOP_RIGHT,
                &box_tiles::outside::TOP_RIGHT,
                px,
                py,
                ink,
                bg,
                self.fb,
            ),
            0x7C => draw_box_tile(
                &box_tiles::VERTICAL_LEFT,
                &box_tiles::outside::VERTICAL_LEFT,
                px,
                py,
                ink,
                bg,
                self.fb,
            ),
            0x7D => draw_box_tile(
                &box_tiles::BOTTOM_LEFT,
                &box_tiles::outside::BOTTOM_LEFT,
                px,
                py,
                ink,
                bg,
                self.fb,
            ),
            0x7E => draw_box_tile(
                &box_tiles::BOTTOM_RIGHT,
                &box_tiles::outside::BOTTOM_RIGHT,
                px,
                py,
                ink,
                bg,
                self.fb,
            ),
            0x7F => embedded_font::fill_tile(px, py, bg, self.fb),
            // Unknown tile id — game-registered custom text if any, else the
            // placeholder text glyph.
            _ => {
                let custom = self.custom_tiles.get(&tile_id).copied();
                embedded_font::draw_text(custom.unwrap_or(fallback), px, py, ink, self.fb);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_renderer::RenderConfig;

    fn fb() -> FrameBuffer {
        FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
    }

    #[test]
    fn custom_tile_renders_registered_text() {
        // A registered custom tile renders identically to drawing its text
        // directly (e.g. a game's 0xE1 → "PK" ligature tile).
        let mut a = fb();
        let mut b = fb();
        FrameBufferPainter::new(&mut a)
            .with_custom_tiles([(0xE1, "PK")])
            .draw_gb_tile(TilePos::new(2, 3), 0xE1, "[225]", Rgba::INK_BLACK);
        FrameBufferPainter::new(&mut b).draw_text(TilePos::new(2, 3), "PK", Rgba::INK_BLACK);
        assert_eq!(a.data, b.data);
    }

    #[test]
    fn unregistered_tile_falls_back_to_placeholder() {
        let mut a = fb();
        let mut b = fb();
        FrameBufferPainter::new(&mut a)
            .with_custom_tiles([(0xE1, "PK")])
            .draw_gb_tile(TilePos::new(2, 3), 0x99, "[153]", Rgba::INK_BLACK);
        FrameBufferPainter::new(&mut b).draw_text(TilePos::new(2, 3), "[153]", Rgba::INK_BLACK);
        assert_eq!(a.data, b.data);
    }
}
