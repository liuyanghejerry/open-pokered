use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::Painter;

use super::bag::{draw_bag, BagData, BagItemEntry};

pub fn draw_battle_bag<P: Painter>(data: &BagData, configs: &[MenuConfig], painter: &mut P) {
    let mut augmented = data.items.clone();
    let cancel_index = augmented.len();
    augmented.push(BagItemEntry {
        name: "CANCEL".to_string(),
        quantity: 0,
        index: cancel_index,
    });
    let battle_data = BagData {
        items: augmented,
        cursor: data.cursor,
    };
    draw_bag(&battle_data, configs, painter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::{Rgba, TilePos, TileRect};

    #[derive(Debug, Default)]
    struct RecordingPainter {
        texts: Vec<(TilePos, String, Rgba)>,
        text_boxes: Vec<(TileRect, Rgba)>,
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
            TileRect::new(0, 0, 8, 3),
            None,
            TileRect::new(1, 1, 6, 1),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn list_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(0, 2, 9, 5),
            None,
            TileRect::new(1, 3, 7, 3),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }

    #[test]
    fn includes_cancel() {
        let data = BagData {
            items: vec![BagItemEntry {
                name: "POTION".into(),
                quantity: 5,
                index: 0,
            }],
            cursor: 0,
        };
        let mut painter = RecordingPainter::default();
        draw_battle_bag(&data, &[test_config(), list_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "CANCEL"));
    }
    #[test]
    fn draws_original_items() {
        let data = BagData {
            items: vec![
                BagItemEntry {
                    name: "POTION".into(),
                    quantity: 5,
                    index: 0,
                },
                BagItemEntry {
                    name: "FULL HEAL".into(),
                    quantity: 3,
                    index: 1,
                },
            ],
            cursor: 0,
        };
        let mut painter = RecordingPainter::default();
        draw_battle_bag(&data, &[test_config(), list_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("POTION")));
        assert!(painter
            .texts
            .iter()
            .any(|(_, t, _)| t.contains("FULL HEAL")));
    }
}
