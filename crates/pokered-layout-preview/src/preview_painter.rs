//! Preview painter wrapping dotzuki-ui's `FrameBufferPainter` with the
//! pokered-specific GB tile mappings the engine painter lacks.
//!
//! The engine painter renders unmapped tile ids as `[NNN]` placeholder text.
//! pokered's own backend (`pokered_ui::backends::framebuffer::draw_gb_tile`)
//! additionally maps 0xE1/0xE2 (the battle-menu `<PK><MN>` ligature) — keep
//! this interception in sync with that file.

use dotzuki_engine::render::{Painter, Rgba, TilePos, TileRect};
use dotzuki_ui::FrameBufferPainter;

pub struct PreviewPainter<'fb> {
    inner: FrameBufferPainter<'fb>,
}

impl<'fb> PreviewPainter<'fb> {
    pub fn new(inner: FrameBufferPainter<'fb>) -> Self {
        Self { inner }
    }
}

impl Painter for PreviewPainter<'_> {
    fn clear(&mut self, color: Rgba) {
        self.inner.clear(color);
    }

    fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
        self.inner.draw_text_box(rect, color);
    }

    fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
        self.inner.draw_text(pos, text, color);
    }

    fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
        self.inner.draw_glyph(pos, glyph, color);
    }

    fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba) {
        self.inner.draw_pixel_rect(px, py, pw, ph, color);
    }

    fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: Rgba) {
        match tile_id {
            // Battle-menu "PKMN" ligature pair (0xE1 = Pk, 0xE2 = Mn), matching
            // pokered-ui's framebuffer backend.
            0xE1 => {
                let (px, py) = pos.to_pixels();
                self.inner.draw_text_px(px, py, "PK", color);
            }
            0xE2 => {
                let (px, py) = pos.to_pixels();
                self.inner.draw_text_px(px, py, "MN", color);
            }
            _ => self.inner.draw_gb_tile(pos, tile_id, fallback, color),
        }
    }

    // Forward the proportional-text overrides so the wrapper doesn't silently
    // downgrade to the trait defaults (which would disable proportional CJK).
    fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: Rgba) {
        self.inner.draw_text_px(px, py, text, color);
    }

    fn measure_text_px(&self, text: &str) -> u32 {
        self.inner.measure_text_px(text)
    }

    fn draw_text_px_scaled(&mut self, px: u32, py: u32, text: &str, scale: u32, color: Rgba) {
        self.inner.draw_text_px_scaled(px, py, text, scale, color);
    }

    fn measure_text_px_scaled(&self, text: &str, scale: u32) -> u32 {
        self.inner.measure_text_px_scaled(text, scale)
    }

    fn supports_proportional(&self) -> bool {
        self.inner.supports_proportional()
    }
}
