//! Battle move menu widget. Lists moves with name and PP.
//! Uses `&[MenuConfig]` — configs[0] is the move list box,
//! configs[1] is the PP detail box.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct MoveEntry {
    pub name: String,
    pub pp_current: u8,
    pub pp_max: u8,
    pub disabled: bool,
    pub move_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MoveMenuData {
    pub moves: Vec<MoveEntry>,
    pub cursor: usize,
}

pub fn draw_move_menu<P: Painter>(data: &MoveMenuData, configs: &[MenuConfig], painter: &mut P) {
    let Some(list_config) = configs.first() else {
        return;
    };
    let mut ui = Ui::new(painter);

    // Move list box
    ui.text_box(list_config.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = list_config
            .content
            .tx
            .saturating_sub(list_config.area.tx + 1);
        let rel_ty = list_config
            .content
            .ty
            .saturating_sub(list_config.area.ty + 1);
        for (i, slot) in data.moves.iter().enumerate() {
            let name: String = slot.name.chars().take(12).collect();
            frame.label(rel_tx + 1, rel_ty + i as u32, &name, Rgba::INK_BLACK);
        }
        if data.cursor < data.moves.len() && list_config.cursor.tile.is_some() {
            let cur_row = rel_ty + data.cursor as u32;
            frame.cursor_glyph_at(rel_tx, cur_row, '\u{25B6}', Rgba::INK_BLACK);
        }
    });

    // PP info box (second config)
    if let Some(pp_config) = configs.get(1) {
        if data.cursor < data.moves.len() {
            let slot = &data.moves[data.cursor];
            ui.text_box(pp_config.area, Rgba::INK_BLACK, true, |frame| {
                let rel_tx = pp_config.content.tx.saturating_sub(pp_config.area.tx + 1);
                let rel_ty = pp_config.content.ty.saturating_sub(pp_config.area.ty + 1);
                frame.label(rel_tx, rel_ty, "TYPE/", Rgba::INK_BLACK);
                if let Some(ref t) = slot.move_type {
                    frame.label(rel_tx, rel_ty + 1, t, Rgba::INK_BLACK);
                }
                let pp_text = format!("{:>2}/{:>2}", slot.pp_current.min(99), slot.pp_max.min(99));
                frame.label(rel_tx + 1, rel_ty + 2, "PP", Rgba::INK_BLACK);
                frame.label(rel_tx + 4, rel_ty + 2, &pp_text, Rgba::INK_BLACK);
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

    fn list_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(4, 12, 16, 6),
            None,
            TileRect::new(5, 13, 14, 4),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn pp_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(0, 8, 11, 5),
            None,
            TileRect::new(1, 9, 9, 3),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn test_data() -> MoveMenuData {
        MoveMenuData {
            moves: vec![
                MoveEntry {
                    name: "THUNDERBOLT".into(),
                    pp_current: 5,
                    pp_max: 15,
                    disabled: false,
                    move_type: Some("ELEC".into()),
                },
                MoveEntry {
                    name: "QUICK ATTACK".into(),
                    pp_current: 0,
                    pp_max: 30,
                    disabled: true,
                    move_type: Some("NORM".into()),
                },
            ],
            cursor: 0,
        }
    }

    #[test]
    fn draws_moves() {
        let mut painter = RecordingPainter::default();
        draw_move_menu(&test_data(), &[list_config(), pp_config()], &mut painter);
        assert!(painter
            .texts
            .iter()
            .any(|(_, t, _)| t.contains("THUNDERBOLT")));
    }
    #[test]
    fn draws_pp_info() {
        let mut painter = RecordingPainter::default();
        draw_move_menu(&test_data(), &[list_config(), pp_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("5/15")));
    }
    #[test]
    fn draws_cursor() {
        let mut painter = RecordingPainter::default();
        draw_move_menu(&test_data(), &[list_config(), pp_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
    #[test]
    fn no_configs() {
        let mut painter = RecordingPainter::default();
        draw_move_menu(&test_data(), &[], &mut painter);
        assert!(painter.text_boxes.is_empty());
    }
}
