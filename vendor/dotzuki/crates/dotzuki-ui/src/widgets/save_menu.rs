//! Save menu widget. Shows save info + phase-specific boxes.
//! Uses `&[MenuConfig]` — configs[0] is the info box. Additional
//! configs handle different phases (ask prompt, saving, done, etc.).

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct SaveEntry {
    pub slot: String,
    pub exists: bool,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SaveMenuData {
    pub title: String,
    pub slots: Vec<SaveEntry>,
    pub cursor: usize,
    pub ask_confirm: Option<String>,
    pub ask_cursor: usize,
}

pub fn draw_save_menu<P: Painter>(data: &SaveMenuData, configs: &[MenuConfig], painter: &mut P) {
    let mut ui = Ui::new(painter);

    // Info box (first config)
    if let Some(info) = configs.first() {
        ui.text_box(info.area, Rgba::INK_BLACK, true, |frame| {
            let rel_tx = info.content.tx.saturating_sub(info.area.tx + 1);
            let rel_ty = info.content.ty.saturating_sub(info.area.ty + 1);
            frame.label(rel_tx, rel_ty, &data.title, Rgba::INK_BLACK);
            for (i, entry) in data.slots.iter().enumerate() {
                let row = rel_ty + 2 + (i as u32) * 2;
                let mut slot_text = entry.slot.clone();
                if entry.exists {
                    slot_text.push_str("  *");
                }
                frame.label(rel_tx, row, &slot_text, Rgba::INK_BLACK);
                if let Some(ref summary) = entry.summary {
                    frame.label(rel_tx + 2, row + 1, summary, Rgba::INK_BLACK);
                }
                if i == data.cursor && info.cursor.tile.is_some() {
                    frame.cursor_glyph_at(
                        rel_tx.saturating_sub(1),
                        row,
                        '\u{25B6}',
                        Rgba::INK_BLACK,
                    );
                }
            }
        });
    }

    // Confirm overlay (ask prompt box)
    if let Some(ref prompt) = data.ask_confirm {
        if let Some(cfg) = configs.get(1) {
            ui.text_box(cfg.area, Rgba::INK_BLACK, true, |frame| {
                let rel_tx = cfg.content.tx.saturating_sub(cfg.area.tx + 1);
                let rel_ty = cfg.content.ty.saturating_sub(cfg.area.ty + 1);
                frame.label(rel_tx, rel_ty, prompt, Rgba::INK_BLACK);
                let yes_row = rel_ty + 2;
                let no_row = rel_ty + 3;
                frame.label(rel_tx + 2, yes_row, "YES", Rgba::INK_BLACK);
                frame.label(rel_tx + 2, no_row, "NO", Rgba::INK_BLACK);
                if data.ask_cursor == 0 {
                    frame.cursor_glyph_at(rel_tx, yes_row, '\u{25B6}', Rgba::INK_BLACK);
                } else {
                    frame.cursor_glyph_at(rel_tx, no_row, '\u{25B6}', Rgba::INK_BLACK);
                }
            });
        }
    }
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

    fn info_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(3, 2, 14, 14),
            None,
            TileRect::new(4, 3, 12, 12),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn prompt_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(5, 5, 10, 7),
            None,
            TileRect::new(6, 6, 8, 5),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn test_data() -> SaveMenuData {
        SaveMenuData {
            title: "SAVE".into(),
            slots: vec![
                SaveEntry {
                    slot: "SAVE SLOT 1".into(),
                    exists: true,
                    summary: Some("HERO Lv.7".into()),
                },
                SaveEntry {
                    slot: "SAVE SLOT 2".into(),
                    exists: false,
                    summary: Some("EMPTY".into()),
                },
            ],
            cursor: 0,
            ask_confirm: None,
            ask_cursor: 0,
        }
    }

    #[test]
    fn draws_info_box() {
        let mut painter = RecordingPainter::default();
        draw_save_menu(&test_data(), &[info_config()], &mut painter);
        assert_eq!(painter.text_boxes.len(), 1);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "SAVE"));
    }
    #[test]
    fn draws_slots() {
        let mut painter = RecordingPainter::default();
        draw_save_menu(&test_data(), &[info_config()], &mut painter);
        assert!(painter
            .texts
            .iter()
            .any(|(_, t, _)| t.starts_with("SAVE SLOT 1")));
    }
    #[test]
    fn confirm_overlay() {
        let mut data = test_data();
        data.ask_confirm = Some("OVERWRITE?".into());
        data.ask_cursor = 1;
        let mut painter = RecordingPainter::default();
        draw_save_menu(&data, &[info_config(), prompt_config()], &mut painter);
        assert_eq!(painter.text_boxes.len(), 2);
        assert!(painter
            .texts
            .iter()
            .any(|(_, t, _)| t.contains("OVERWRITE")));
    }
    #[test]
    fn no_configs() {
        let mut painter = RecordingPainter::default();
        draw_save_menu(&test_data(), &[], &mut painter);
        assert!(painter.text_boxes.is_empty());
    }
}
