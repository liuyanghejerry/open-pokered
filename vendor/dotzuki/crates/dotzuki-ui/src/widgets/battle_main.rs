//! Battle main menu widget — 2×2 grid with base box + menu box.
//! Uses `&[MenuConfig]` — configs[0] is the base background box,
//! configs[1] is the 2×2 menu grid.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct BattleMainData {
    pub options: Vec<String>,
    pub cursor: usize,
}

pub fn draw_battle_main<P: Painter>(
    data: &BattleMainData,
    configs: &[MenuConfig],
    painter: &mut P,
) {
    let mut ui = Ui::new(painter);

    // Base background box (just the border, no interior content)
    if let Some(base) = configs.first() {
        ui.text_box(base.area, Rgba::INK_BLACK, true, |_frame| {});
    }

    // 2×2 menu grid box
    let Some(menu) = configs.get(1) else { return };
    ui.text_box(menu.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = menu.content.tx.saturating_sub(menu.area.tx + 1);
        let rel_ty = menu.content.ty.saturating_sub(menu.area.ty + 1);

        // 2×2 grid: row/col from cursor index
        for (i, opt) in data.options.iter().enumerate() {
            let row = rel_ty + (i as u32 / 2);
            let col = rel_tx + (i as u32 % 2) * 6;

            // Use GB tile IDs for "PKMN" → composite glyphs at 0xE1/0xE2
            if opt == "PKMN" {
                frame.label(col, row, "PK", Rgba::INK_BLACK);
                frame.label(col + 2, row, "MN", Rgba::INK_BLACK);
            } else {
                frame.label(col, row, opt, Rgba::INK_BLACK);
            }
        }

        // Cursor at selected option: row=index/2, col=(index%2)*6
        if menu.cursor.tile.is_some() && data.cursor < data.options.len() {
            let cur_row = rel_ty + (data.cursor as u32 / 2);
            let cur_col = rel_tx.saturating_sub(1) + (data.cursor as u32 % 2) * 6;
            frame.cursor_glyph_at(cur_col, cur_row, '\u{25B6}', Rgba::INK_BLACK);
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

    fn base_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(8, 11, 12, 5),
            None,
            TileRect::new(9, 12, 10, 3),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn menu_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(8, 11, 12, 5),
            None,
            TileRect::new(9, 12, 10, 3),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn test_data() -> BattleMainData {
        BattleMainData {
            options: vec!["FIGHT".into(), "BAG".into(), "PKMN".into(), "RUN".into()],
            cursor: 0,
        }
    }

    #[test]
    fn draws_both_boxes() {
        let mut painter = RecordingPainter::default();
        draw_battle_main(&test_data(), &[base_config(), menu_config()], &mut painter);
        assert_eq!(painter.text_boxes.len(), 2);
    }
    #[test]
    fn draws_options() {
        let mut painter = RecordingPainter::default();
        draw_battle_main(&test_data(), &[base_config(), menu_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "FIGHT"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "BAG"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "RUN"));
    }
    #[test]
    fn draws_pkmn_composite() {
        let mut painter = RecordingPainter::default();
        draw_battle_main(&test_data(), &[base_config(), menu_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "PK"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "MN"));
    }
    #[test]
    fn draws_cursor() {
        let mut painter = RecordingPainter::default();
        draw_battle_main(&test_data(), &[base_config(), menu_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
    #[test]
    fn no_menu_config() {
        let mut painter = RecordingPainter::default();
        draw_battle_main(&test_data(), &[base_config()], &mut painter);
        assert_eq!(painter.text_boxes.len(), 1); // only base box
    }
}
