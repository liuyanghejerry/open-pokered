use pokered_data::TILE_SIZE_PX;
use pokered_renderer::embedded_font::{self, box_tiles, draw_box_tile, draw_text, fill_tile};
use pokered_renderer::{FrameBuffer, Rgba};
use pokered_core::game_state::Lang;

use crate::engine::{Painter, Rgba as EngineRgba, TilePos, TileRect};

pub struct FrameBufferPainter<'fb> {
    fb: &'fb mut FrameBuffer,
    pub lang: Lang,
}

impl<'fb> FrameBufferPainter<'fb> {
    pub fn new(fb: &'fb mut FrameBuffer) -> Self {
        Self { fb, lang: Lang::default() }
    }

    pub fn with_lang(mut self, lang: Lang) -> Self {
        self.lang = lang;
        self
    }
}

impl<'fb> Painter for FrameBufferPainter<'fb> {
    fn clear(&mut self, color: EngineRgba) {
        self.fb.clear(color);
    }

    fn draw_text_box(&mut self, rect: TileRect, color: EngineRgba) {
        // `rect.tw`/`rect.th` are TOTAL tile dimensions including borders, matching
        // the canonical `pokered_renderer::textbox::TextBoxFrame::draw_frame` semantics
        // and the convention used by all `ui_layouts/*.json` files. A box at
        // `tx=8, ty=12, tw=12, th=6` therefore occupies tile columns 8..=19 and
        // rows 12..=17 (the full screen is 20×18 tiles).
        //
        // Borders are drawn at the rect's outer edge; the interior runs from
        // `(tx+1, ty+1)` to `(tx+tw-2, ty+th-2)` inclusive — which matches the
        // `+1`/`+1` interior origin set by `Ui::text_box` for label placement.
        if rect.tw < 2 || rect.th < 2 {
            return;
        }
        let bg = Rgba::WHITE;
        let ink = color;
        let t = TILE_SIZE_PX;
        let bx = rect.tx * t;
        let by = rect.ty * t;
        let inner_w = rect.tw - 2;
        let inner_h = rect.th - 2;
        let right_x = bx + (rect.tw - 1) * t;
        let bot_y = by + (rect.th - 1) * t;

        draw_box_tile(&box_tiles::TOP_LEFT, &box_tiles::outside::TOP_LEFT, bx, by, ink, bg, self.fb);
        for col in 0..inner_w {
            draw_box_tile(&box_tiles::HORIZONTAL, &box_tiles::outside::HORIZONTAL, bx + (1 + col) * t, by, ink, bg, self.fb);
        }
        draw_box_tile(&box_tiles::TOP_RIGHT, &box_tiles::outside::TOP_RIGHT, right_x, by, ink, bg, self.fb);

        for row in 0..inner_h {
            let y = by + (1 + row) * t;
            draw_box_tile(&box_tiles::VERTICAL_LEFT, &box_tiles::outside::VERTICAL_LEFT, bx, y, ink, bg, self.fb);
            for col in 0..inner_w {
                fill_tile(bx + (1 + col) * t, y, bg, self.fb);
            }
            draw_box_tile(&box_tiles::VERTICAL_RIGHT, &box_tiles::outside::VERTICAL_RIGHT, right_x, y, ink, bg, self.fb);
        }

        draw_box_tile(&box_tiles::BOTTOM_LEFT, &box_tiles::outside::BOTTOM_LEFT, bx, bot_y, ink, bg, self.fb);
        for col in 0..inner_w {
            draw_box_tile(&box_tiles::HORIZONTAL_BOTTOM, &box_tiles::outside::HORIZONTAL_BOTTOM, bx + (1 + col) * t, bot_y, ink, bg, self.fb);
        }
        draw_box_tile(&box_tiles::BOTTOM_RIGHT, &box_tiles::outside::BOTTOM_RIGHT, right_x, bot_y, ink, bg, self.fb);
    }

    fn draw_text(&mut self, pos: TilePos, text: &str, color: EngineRgba) {
        let (px, mut py) = pos.to_pixels();
        if self.lang == Lang::Zh {
            py = py.saturating_sub(1);
        }
        draw_text(text, px, py, color, self.fb);
    }

    fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: EngineRgba) {
        let (px, mut py) = pos.to_pixels();
        if self.lang == Lang::Zh {
            py = py.saturating_sub(1);
        }
        let mut buf = [0u8; 4];
        let s = glyph.encode_utf8(&mut buf);
        draw_text(s, px, py, color, self.fb);
    }

    fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: EngineRgba) {
        self.fb.fill_rect(px, py, pw, ph, color);
    }

    fn draw_gb_tile(&mut self, pos: TilePos, tile_id: u8, fallback: &str, color: EngineRgba) {
        let (px, py) = pos.to_pixels();
        let ink = color;
        let bg = Rgba::INK_WHITE;
        // Map common Game Boy tile IDs to Fusion Pixel glyphs.
        // Matches the mapping in jrpg-ui/src/lib.rs.
        match tile_id {
            // Menu cursor ▶
            223 => {
                embedded_font::draw_char('\u{25B6}', px, py, ink, self.fb);
            }
            // Text-box "more text" down arrow ▼
            31 => {
                embedded_font::draw_char('\u{25BC}', px, py, ink, self.fb);
            }
            // Battle-menu "PKMN" ligature pair (0xE1 = Pk, 0xE2 = Mn). The v1
            // menu drew these as "PK"/"MN" text; the v2 tile element only knows
            // the tile id, so map them here to keep the framebuffer rendering.
            0xE1 => draw_text("PK", px, py, ink, self.fb),
            0xE2 => draw_text("MN", px, py, ink, self.fb),
            // Default box-border tile set (0x79–0x7F)
            0x79 => draw_box_tile(&box_tiles::TOP_LEFT, &box_tiles::outside::TOP_LEFT, px, py, ink, bg, self.fb),
            0x7A => draw_box_tile(&box_tiles::HORIZONTAL, &box_tiles::outside::HORIZONTAL, px, py, ink, bg, self.fb),
            0x7B => draw_box_tile(&box_tiles::TOP_RIGHT, &box_tiles::outside::TOP_RIGHT, px, py, ink, bg, self.fb),
            0x7C => draw_box_tile(&box_tiles::VERTICAL_LEFT, &box_tiles::outside::VERTICAL_LEFT, px, py, ink, bg, self.fb),
            0x7D => draw_box_tile(&box_tiles::BOTTOM_LEFT, &box_tiles::outside::BOTTOM_LEFT, px, py, ink, bg, self.fb),
            0x7E => draw_box_tile(&box_tiles::BOTTOM_RIGHT, &box_tiles::outside::BOTTOM_RIGHT, px, py, ink, bg, self.fb),
            0x7F => fill_tile(px, py, bg, self.fb),
            // Unknown tile id — fall back to the placeholder text glyph.
            _ => draw_text(fallback, px, py, ink, self.fb),
        }
    }
}
