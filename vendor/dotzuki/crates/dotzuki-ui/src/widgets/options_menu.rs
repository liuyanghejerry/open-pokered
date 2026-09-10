//! Options menu widget. 4 boxes with dual cursor (▷ always, ▶ selected).
//! Uses `&[MenuConfig]` — configs[0..3] are bordered option boxes,
//! configs[3] is the unbordered cancel region.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct OptionEntry {
    pub label: String,
    pub value: String,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct OptionsMenuData {
    pub options: Vec<OptionEntry>,
    pub cursor: usize,
}

const INDICATOR: char = '\u{25B7}';
const CURSOR: char = '\u{25B6}';

/// Returns the x-offset for a value string within a given menu box.
/// In the original, enum_position_map provided pixel-level offsets.
fn value_offset_for(value: &str) -> u32 {
    match value {
        "Fast" | "Medium" | "Slow" => match value {
            "Fast" => 0,
            "Medium" => 1,
            _ => 2,
        },
        "On" | "Off" => {
            if value == "On" {
                0
            } else {
                1
            }
        }
        "Shift" | "Set" => {
            if value == "Shift" {
                0
            } else {
                1
            }
        }
        _ => 0,
    }
}

pub fn draw_options_menu<P: Painter>(
    data: &OptionsMenuData,
    configs: &[MenuConfig],
    painter: &mut P,
) {
    let mut ui = Ui::new(painter);

    // Options are distributed across 4 boxes: TEXT SPEED, BATTLE SCENE, BATTLE STYLE, CANCEL
    // Each config corresponds to one option entry.
    for (i, entry) in data.options.iter().enumerate() {
        let Some(cfg) = configs.get(i) else { continue };
        let has_border = i < 3; // first 3 boxes have borders, last is unbordered region

        ui.text_box(cfg.area, Rgba::INK_BLACK, has_border, |frame| {
            let rel_tx: u32 = if has_border {
                cfg.content.tx.saturating_sub(cfg.area.tx + 1)
            } else {
                cfg.content.tx.saturating_sub(cfg.area.tx)
            };
            let rel_ty: u32 = if has_border {
                cfg.content.ty.saturating_sub(cfg.area.ty + 1)
            } else {
                cfg.content.ty.saturating_sub(cfg.area.ty)
            };

            // Label at (rel_tx, rel_ty)
            frame.label(rel_tx, rel_ty, &entry.label, Rgba::INK_BLACK);

            // Value for first 3 options (TEXT SPEED, BATTLE SCENE, BATTLE STYLE)
            let val_offset = value_offset_for(&entry.value);
            let value_x = rel_tx + 10 + val_offset;
            frame.label(value_x, rel_ty, &entry.value, Rgba::INK_BLACK);

            // Dual cursor: ▷ always at value position, ▶ when this row is selected
            let is_selected = i == data.cursor;
            frame.cursor_glyph_at(value_x, rel_ty, INDICATOR, Rgba::INK_BLACK);
            if is_selected {
                frame.cursor_glyph_at(value_x, rel_ty, CURSOR, Rgba::INK_BLACK);
            }
        });
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

    fn box_configs() -> [MenuConfig; 4] {
        [
            MenuConfig::new(
                TileRect::new(2, 2, 16, 3),
                None,
                TileRect::new(3, 3, 14, 1),
                dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
            ),
            MenuConfig::new(
                TileRect::new(2, 5, 16, 3),
                None,
                TileRect::new(3, 6, 14, 1),
                dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
            ),
            MenuConfig::new(
                TileRect::new(2, 8, 16, 3),
                None,
                TileRect::new(3, 9, 14, 1),
                dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
            ),
            MenuConfig::new(
                TileRect::new(2, 11, 16, 3),
                None,
                TileRect::new(3, 12, 14, 1),
                dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
            ),
        ]
    }
    fn test_data() -> OptionsMenuData {
        OptionsMenuData {
            options: vec![
                OptionEntry {
                    label: "TEXT SPEED".into(),
                    value: "Fast".into(),
                    selected: false,
                },
                OptionEntry {
                    label: "BATTLE SCENE".into(),
                    value: "On".into(),
                    selected: false,
                },
                OptionEntry {
                    label: "BATTLE STYLE".into(),
                    value: "Shift".into(),
                    selected: false,
                },
                OptionEntry {
                    label: "CANCEL".into(),
                    value: "".into(),
                    selected: false,
                },
            ],
            cursor: 0,
        }
    }

    #[test]
    fn draws_all_boxes() {
        let configs = box_configs();
        let mut painter = RecordingPainter::default();
        draw_options_menu(&test_data(), &configs, &mut painter);
        assert_eq!(painter.text_boxes.len(), 3); // 3 bordered boxes, 4th is unbordered region
    }
    #[test]
    fn draws_labels() {
        let configs = box_configs();
        let mut painter = RecordingPainter::default();
        draw_options_menu(&test_data(), &configs, &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "TEXT SPEED"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "BATTLE SCENE"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "BATTLE STYLE"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "CANCEL"));
    }
    #[test]
    fn draws_values() {
        let configs = box_configs();
        let mut painter = RecordingPainter::default();
        draw_options_menu(&test_data(), &configs, &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "Fast"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "On"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "Shift"));
    }
    #[test]
    fn draws_dual_cursors() {
        let configs = box_configs();
        let mut painter = RecordingPainter::default();
        draw_options_menu(&test_data(), &configs, &mut painter);
        // Expect at least 4 ▷ (one per option) + 1 ▶ (for selected)
        let indicator_count = painter
            .glyphs
            .iter()
            .filter(|(_, g, _)| *g == INDICATOR)
            .count();
        let cursor_count = painter
            .glyphs
            .iter()
            .filter(|(_, g, _)| *g == CURSOR)
            .count();
        assert!(indicator_count >= 4, "Each option should have ▷ indicator");
        assert_eq!(cursor_count, 1, "Only selected option should have ▶ cursor");
    }
    #[test]
    fn no_configs() {
        let mut painter = RecordingPainter::default();
        draw_options_menu(&test_data(), &[], &mut painter);
        assert!(painter.text_boxes.is_empty());
    }
}
