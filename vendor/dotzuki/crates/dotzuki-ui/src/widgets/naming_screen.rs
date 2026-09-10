//! Naming screen widget. Title, name display, and keyboard grid.
//! Uses `&[MenuConfig]` — configs[0] is the main naming box.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct NamingScreenRow {
    pub keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NamingScreenData {
    pub title: String,
    pub name: String,
    pub cursor_pos: usize,
    pub keyboard_rows: Vec<NamingScreenRow>,
    pub keyboard_cursor: (usize, usize),
    pub max_length: usize,
}

const NAME_BOX_TX: u32 = 10;
const NAME_BOX_TY: u32 = 2;
const UNDERSCORE_TY: u32 = NAME_BOX_TY + 1;
const KEYBOARD_TX: u32 = 2;
const KEYBOARD_TY: u32 = 5;
const KEYBOARD_COL_STEP: u32 = 2;
const TITLE_TX: u32 = 1;
const TITLE_TY: u32 = 1;

pub fn draw_naming_screen<P: Painter>(
    data: &NamingScreenData,
    configs: &[MenuConfig],
    painter: &mut P,
) {
    let Some(config) = configs.first() else {
        return;
    };
    let mut ui = Ui::new(painter);
    ui.text_box(config.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = config.content.tx.saturating_sub(config.area.tx + 1);
        let rel_ty = config.content.ty.saturating_sub(config.area.ty + 1);

        // Title
        frame.label(
            rel_tx + TITLE_TX,
            rel_ty + TITLE_TY,
            &data.title,
            Rgba::INK_BLACK,
        );

        // Name display
        frame.label(
            rel_tx + NAME_BOX_TX,
            rel_ty + NAME_BOX_TY,
            &data.name,
            Rgba::INK_BLACK,
        );

        // Underscore row
        let name_len = data.name.len() as u32;
        let max_len = data.max_length as u32;
        let cursor_char = if data.cursor_pos < data.max_length {
            '_'
        } else {
            ' '
        };
        for i in 0..max_len {
            let ch = if i < name_len {
                data.name.chars().nth(i as usize).unwrap_or('_')
            } else if i == name_len && data.cursor_pos == data.name.len() {
                '_'
            } else {
                ' '
            };
            let glyph = if i < name_len || (i == name_len && data.cursor_pos == data.name.len()) {
                ch
            } else {
                ' '
            };
            let s = glyph.to_string();
            if !s.trim().is_empty() || glyph == '_' || glyph == ' ' {
                frame.label(
                    rel_tx + NAME_BOX_TX + i,
                    rel_ty + UNDERSCORE_TY,
                    &s,
                    Rgba::INK_BLACK,
                );
            }
        }

        // Keyboard grid
        let alphabet = &data.keyboard_rows;
        for (row_i, row) in alphabet.iter().enumerate() {
            let ty = rel_ty + KEYBOARD_TY + row_i as u32;
            for (col_i, key) in row.keys.iter().enumerate() {
                let tx = rel_tx + KEYBOARD_TX + col_i as u32 * KEYBOARD_COL_STEP;
                if row_i == data.keyboard_cursor.0 && col_i == data.keyboard_cursor.1 {
                    frame.cursor_glyph_at(tx.saturating_sub(1), ty, '\u{25B6}', Rgba::INK_BLACK);
                }
                frame.label(tx, ty, key, Rgba::INK_BLACK);
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
            TileRect::new(1, 1, 18, 16),
            None,
            TileRect::new(2, 2, 16, 14),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn test_data() -> NamingScreenData {
        NamingScreenData {
            title: "YOUR NAME?".into(),
            name: "ASH".into(),
            cursor_pos: 3,
            keyboard_rows: vec![
                NamingScreenRow {
                    keys: vec![
                        "A".into(),
                        "B".into(),
                        "C".into(),
                        "D".into(),
                        "E".into(),
                        "F".into(),
                        "G".into(),
                        "H".into(),
                    ],
                },
                NamingScreenRow {
                    keys: vec![
                        "I".into(),
                        "J".into(),
                        "K".into(),
                        "L".into(),
                        "M".into(),
                        "N".into(),
                        "O".into(),
                        "P".into(),
                    ],
                },
            ],
            keyboard_cursor: (0, 0),
            max_length: 7,
        }
    }

    #[test]
    fn draws_box() {
        let mut painter = RecordingPainter::default();
        draw_naming_screen(&test_data(), &[test_config()], &mut painter);
        assert_eq!(painter.text_boxes.len(), 1);
    }
    #[test]
    fn draws_title() {
        let mut painter = RecordingPainter::default();
        draw_naming_screen(&test_data(), &[test_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "YOUR NAME?"));
    }
    #[test]
    fn draws_keyboard() {
        let mut painter = RecordingPainter::default();
        draw_naming_screen(&test_data(), &[test_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "A"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "H"));
    }
    #[test]
    fn draws_cursor() {
        let mut painter = RecordingPainter::default();
        draw_naming_screen(&test_data(), &[test_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
}
