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
        let inner_px_w = inner_w * t;
        let inner_px_h = inner_h * t;

        draw_box_tile(
            &box_tiles::TOP_LEFT,
            &box_tiles::outside::TOP_LEFT,
            bx,
            by,
            ink,
            bg,
            self.fb,
        );
        // The repeated edge tiles are solid horizontal/vertical runs. Batch
        // those runs while retaining the four transparent corner masks.
        if inner_w > 0 {
            self.fb.fill_rect(bx + t, by + 1, inner_px_w, 2, ink);
            self.fb.fill_rect(bx + t, by + 3, inner_px_w, 5, bg);
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

        if inner_w > 0 && inner_h > 0 {
            self.fb
                .fill_rect(bx + t, by + t, inner_w * t, inner_h * t, bg);
        }
        if inner_h > 0 {
            self.fb.fill_rect(bx + 1, by + t, 2, inner_px_h, ink);
            self.fb.fill_rect(bx + 3, by + t, 5, inner_px_h, bg);
            self.fb.fill_rect(right_x, by + t, 5, inner_px_h, bg);
            self.fb
                .fill_rect(right_x + 5, by + t, 2, inner_px_h, ink);
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
        if inner_w > 0 {
            self.fb.fill_rect(bx + t, bot_y, inner_px_w, 5, bg);
            self.fb
                .fill_rect(bx + t, bot_y + 5, inner_px_w, 2, ink);
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

    fn draw_text(&mut self, pos: TilePos, text: &str, color: EngineRgba) {
        let (px, mut py) = pos.to_pixels();
        if self.lang == Lang::Zh {
            py = py.saturating_sub(1);
        }
        draw_text(text, px, py, color, self.fb);
    }

    // Pixel-precise text: proportional ASCII glyphs (5 px advance) cannot
    // share a flush right edge when snapped to the 8 px tile grid, so callers
    // that need exact alignment (e.g. the CONTINUE info values) draw through
    // this path. Layouts must also opt in through their theme; ordinary
    // Game Boy layouts continue to use the tile path.
    fn draw_text_px(&mut self, px: u32, py: u32, text: &str, color: EngineRgba) {
        let py = if self.lang == Lang::Zh {
            py.saturating_sub(1)
        } else {
            py
        };
        draw_text(text, px, py, color, self.fb);
    }

    fn supports_proportional(&self) -> bool {
        true
    }

    fn measure_text_px(&self, text: &str) -> u32 {
        embedded_font::measure_text(text)
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
        // Matches the mapping in dotzuki-ui/src/lib.rs.
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
            // Naming-screen underscore tiles. The BDF fallback glyph for '_'
            // is drawn below the 8×8 tile grid (10px cell, y_off -1), so it
            // would land on the row below; draw a crisp full-width underline
            // instead. 0x76 = normal slot, 0x77 = raised (current editing slot).
            0x76 | 0x77 => {
                fill_tile(px, py, bg, self.fb);
                let line_y = if tile_id == 0x77 { py + 4 } else { py + 6 };
                self.fb.fill_rect(px, line_y, TILE_SIZE_PX, 1, ink);
            }
            // Unknown tile id — fall back to the placeholder text glyph.
            _ => draw_text(fallback, px, py, ink, self.fb),
        }
    }
}
