//! Generic list menu widget — covers main menu and start menu use-cases.
//! Uses `&[MenuConfig]` — configs[0] is the menu box.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct ListMenuData {
    pub title: Option<String>,
    pub items: Vec<String>,
    pub cursor: usize,
}

pub fn draw_list_menu<P: Painter>(data: &ListMenuData, configs: &[MenuConfig], painter: &mut P) {
    let Some(config) = configs.first() else {
        return;
    };
    let mut ui = Ui::new(painter);
    let num_items = data.items.len() as u32;
    if num_items == 0 {
        return;
    }

    let content_h = num_items; // 1 row per item, no gap
    let eff_h = content_h + 2; // +2 for border
    let rect = dotzuki_engine::render::TileRect::new(
        config.area.tx,
        config.area.ty,
        config.area.tw,
        eff_h,
    );

    ui.text_box(rect, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = config.content.tx.saturating_sub(rect.tx + 1);
        let rel_ty = config.content.ty.saturating_sub(rect.ty + 1);

        // Title
        if let Some(ref title) = data.title {
            if !title.is_empty() {
                frame.label(rel_tx, rel_ty, title, Rgba::INK_BLACK);
                let sep_y = rel_ty + 1;
                for col in 0..config.area.tw.saturating_sub(2) {
                    frame.label(rel_tx + col, sep_y, "-", Rgba::INK_BLACK);
                }
            }
        }

        let start_y = if data.title.as_ref().map_or(false, |t| !t.is_empty()) {
            rel_ty + 2
        } else {
            rel_ty
        };

        for (i, item) in data.items.iter().enumerate() {
            let y = start_y + i as u32;
            frame.label(rel_tx + 1, y, item, Rgba::INK_BLACK);
            if i == data.cursor && config.cursor.tile.is_some() {
                frame.cursor_glyph_at(rel_tx, y, '\u{25B6}', Rgba::INK_BLACK);
            }
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
            TileRect::new(5, 3, 10, 1),
            None,
            TileRect::new(6, 4, 8, 0),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn test_data() -> ListMenuData {
        ListMenuData {
            title: None,
            items: vec!["New Game".into(), "Continue".into(), "Quit".into()],
            cursor: 0,
        }
    }

    #[test]
    fn draws_items() {
        let mut painter = RecordingPainter::default();
        draw_list_menu(&test_data(), &[test_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "New Game"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "Continue"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "Quit"));
    }
    #[test]
    fn draws_title() {
        let mut data = test_data();
        data.title = Some("POKERED".into());
        let mut painter = RecordingPainter::default();
        draw_list_menu(&data, &[test_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "POKERED"));
    }
    #[test]
    fn draws_cursor() {
        let mut painter = RecordingPainter::default();
        draw_list_menu(&test_data(), &[test_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
    #[test]
    fn no_configs() {
        let mut painter = RecordingPainter::default();
        draw_list_menu(&test_data(), &[], &mut painter);
        assert!(painter.text_boxes.is_empty());
    }
}
