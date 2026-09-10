use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

use crate::widgets::dialog::wrap_lines;

pub fn draw_battle_text<P: Painter>(text: &str, configs: &[MenuConfig], painter: &mut P) {
    let Some(config) = configs.first() else {
        return;
    };
    let mut ui = Ui::new(painter);
    ui.text_box(config.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = config.content.tx.saturating_sub(config.area.tx + 1);
        let rel_ty = config.content.ty.saturating_sub(config.area.ty + 1);
        // Box interior width in pixels (8px tiles) — the wrap budget for the
        // proportional Fusion Pixel font (Latin 5px, CJK 10px advance).
        let max_width_px = (config.content.tw as usize) * 8;
        let line_height = 2u32;
        let max_lines = (config.content.th / line_height).max(1) as usize;
        let lines = wrap_lines(text, max_width_px, max_lines);
        for (i, line) in lines.iter().enumerate() {
            frame.label(
                rel_tx,
                rel_ty + (i as u32) * line_height,
                line,
                Rgba::INK_BLACK,
            );
        }
        if config.cursor.tile.is_some() && !text.is_empty() {
            let arrow_tx = rel_tx + config.content.tw.saturating_sub(1);
            let arrow_ty = rel_ty + config.content.th.saturating_sub(1);
            frame.cursor_glyph_at(arrow_tx, arrow_ty, '\u{25BC}', Rgba::INK_BLACK);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::{TilePos, TileRect};

    #[derive(Debug, Default)]
    struct RecordingPainter {
        text_boxes: Vec<(TileRect, Rgba)>,
        texts: Vec<(TilePos, String, Rgba)>,
        glyphs: Vec<(TilePos, char, Rgba)>,
    }
    impl Painter for RecordingPainter {
        fn clear(&mut self, _: Rgba) {}
        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.text_boxes.push((rect, color));
        }
        fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
            self.texts.push((pos, text.to_string(), color));
        }
        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            self.glyphs.push((pos, glyph, color));
        }
        fn draw_pixel_rect(&mut self, _: u32, _: u32, _: u32, _: u32, _: Rgba) {}
        fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
    }

    fn test_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(0, 14, 20, 4),
            None,
            TileRect::new(1, 15, 18, 2),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }

    #[test]
    fn draws_text() {
        let mut painter = RecordingPainter::default();
        draw_battle_text("Hello!", &[test_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "Hello!"));
    }
    #[test]
    fn draws_arrow() {
        let mut painter = RecordingPainter::default();
        draw_battle_text("Hi", &[test_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
    #[test]
    fn empty_no_arrow() {
        let mut painter = RecordingPainter::default();
        draw_battle_text("", &[test_config()], &mut painter);
        assert!(painter.glyphs.is_empty());
    }
}
