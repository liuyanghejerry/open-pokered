//! Bag menu widget. Header box + scrollable item list with CANCEL.
//! Uses `&[MenuConfig]` — configs[0] is the header box ("ITEM"),
//! configs[1] is the scrollable item list box.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, TileRect, Ui};

#[derive(Debug, Clone)]
pub struct BagItemEntry {
    pub name: String,
    pub quantity: u32,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct BagData {
    pub items: Vec<BagItemEntry>,
    pub cursor: usize,
}

pub fn draw_bag<P: Painter>(data: &BagData, configs: &[MenuConfig], painter: &mut P) {
    let mut ui = Ui::new(painter);

    // Header box — use label_positions if available
    if let Some(header) = configs.first() {
        ui.text_box(header.area, Rgba::INK_BLACK, true, |frame| {
            if let Some(label) = header.label_positions.first() {
                frame.label(label.0, label.1, &label.2, Rgba::INK_BLACK);
            } else {
                let rel_tx = header.content.tx.saturating_sub(header.area.tx + 1);
                let rel_ty = header.content.ty.saturating_sub(header.area.ty + 1);
                frame.label(rel_tx, rel_ty, "ITEM", Rgba::INK_BLACK);
            }
        });
    }

    // List box — use gap and padding from config
    let Some(list) = configs.get(1) else { return };
    let num_items = data.items.len() as u32 + 1; // +1 for CANCEL
    let gap = list.gap;
    let pad_top = list.padding.top;
    let pad_left = list.padding.left;
    let content_h = num_items + (num_items.saturating_sub(1)) * gap;
    let eff_h = content_h + pad_top + list.padding.bottom + 2; // +2 for border
    let rect = TileRect::new(list.area.tx, list.area.ty, list.area.tw, eff_h);

    ui.text_box(rect, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = list.content.tx.saturating_sub(rect.tx + 1);
        let rel_ty = list.content.ty.saturating_sub(rect.ty + 1);

        for (i, item) in data.items.iter().enumerate() {
            let y = rel_ty + pad_top + i as u32 * (1 + gap);
            let name: String = item.name.chars().take(12).collect();
            let label = format!("{:<12} ×{:<2}", name, item.quantity);
            frame.label(rel_tx + pad_left, y, &label, Rgba::INK_BLACK);
        }
        // CANCEL row
        let cancel_y = rel_ty + pad_top + data.items.len() as u32 * (1 + gap);
        frame.label(rel_tx + pad_left, cancel_y, "CANCEL", Rgba::INK_BLACK);

        // Cursor
        if list.cursor.tile.is_some() {
            let cur_y = rel_ty + pad_top + data.cursor as u32 * (1 + gap);
            frame.cursor_glyph_at(
                rel_tx + pad_left.saturating_sub(1),
                cur_y,
                '\u{25B6}',
                Rgba::INK_BLACK,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::TilePos;

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

    fn header_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(0, 0, 8, 3),
            None,
            TileRect::new(1, 1, 6, 1),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn list_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(0, 2, 9, 2),
            None,
            TileRect::new(1, 3, 7, 0),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn test_data() -> BagData {
        BagData {
            items: vec![
                BagItemEntry {
                    name: "POTION".into(),
                    quantity: 5,
                    index: 0,
                },
                BagItemEntry {
                    name: "BALL".into(),
                    quantity: 3,
                    index: 1,
                },
            ],
            cursor: 0,
        }
    }

    #[test]
    fn draws_header() {
        let mut painter = RecordingPainter::default();
        draw_bag(
            &test_data(),
            &[header_config(), list_config()],
            &mut painter,
        );
        assert!(painter.texts.iter().any(|(_, t, _)| t == "ITEM"));
        assert_eq!(painter.text_boxes.len(), 2);
    }
    #[test]
    fn draws_items() {
        let mut painter = RecordingPainter::default();
        draw_bag(
            &test_data(),
            &[header_config(), list_config()],
            &mut painter,
        );
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("POTION")));
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("BALL")));
    }
    #[test]
    fn draws_cancel() {
        let mut painter = RecordingPainter::default();
        draw_bag(
            &test_data(),
            &[header_config(), list_config()],
            &mut painter,
        );
        assert!(painter.texts.iter().any(|(_, t, _)| t == "CANCEL"));
    }
    #[test]
    fn draws_cursor() {
        let mut painter = RecordingPainter::default();
        draw_bag(
            &test_data(),
            &[header_config(), list_config()],
            &mut painter,
        );
        assert!(!painter.glyphs.is_empty());
    }
}
