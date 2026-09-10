//! Battle party list widget.
//! Renders party members with name, HP, and optional status indicators.
//! Uses `&[MenuConfig]` — first config is the party list box.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct BattlePartyEntry {
    pub name: String,
    pub hp: u32,
    pub max_hp: u32,
    pub status: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct BattlePartyData {
    pub entries: Vec<BattlePartyEntry>,
    pub cursor: usize,
}

pub fn draw_battle_party<P: Painter>(
    data: &BattlePartyData,
    configs: &[MenuConfig],
    painter: &mut P,
) {
    let Some(config) = configs.first() else {
        return;
    };
    if data.entries.is_empty() {
        return;
    }
    let mut ui = Ui::new(painter);
    ui.text_box(config.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = config.content.tx.saturating_sub(config.area.tx + 1);
        let rel_ty = config.content.ty.saturating_sub(config.area.ty + 1);
        for (i, entry) in data.entries.iter().enumerate() {
            let row = rel_ty + (i as u32);
            if row >= rel_ty + config.content.th {
                break;
            }
            let label = if entry.hp == 0 {
                format!("{} FNT", entry.name)
            } else {
                format!("{} {}/{}", entry.name, entry.hp, entry.max_hp)
            };
            frame.label(rel_tx + 1, row, &label, Rgba::INK_BLACK);
            if i == data.cursor && config.cursor.tile.is_some() {
                frame.cursor_glyph_at(rel_tx, row, '\u{25B6}', Rgba::INK_BLACK);
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
            TileRect::new(1, 3, 18, 8),
            None,
            TileRect::new(2, 4, 16, 6),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn test_data() -> BattlePartyData {
        BattlePartyData {
            entries: vec![
                BattlePartyEntry {
                    name: "SPARKIT".into(),
                    hp: 20,
                    max_hp: 35,
                    status: Some("BRN".into()),
                    active: true,
                },
                BattlePartyEntry {
                    name: "LEAFKIT".into(),
                    hp: 45,
                    max_hp: 45,
                    status: None,
                    active: true,
                },
                BattlePartyEntry {
                    name: "FLAMBIT".into(),
                    hp: 0,
                    max_hp: 39,
                    status: Some("FNT".into()),
                    active: false,
                },
            ],
            cursor: 0,
        }
    }

    #[test]
    fn draws_box() {
        let mut painter = RecordingPainter::default();
        draw_battle_party(&test_data(), &[test_config()], &mut painter);
        assert_eq!(painter.text_boxes.len(), 1);
    }
    #[test]
    fn draws_names() {
        let mut painter = RecordingPainter::default();
        draw_battle_party(&test_data(), &[test_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("SPARKIT")));
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("LEAFKIT")));
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("FNT")));
    }
    #[test]
    fn draws_cursor() {
        let mut painter = RecordingPainter::default();
        draw_battle_party(&test_data(), &[test_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
        assert_eq!(painter.glyphs[0].1, '\u{25B6}');
    }
    #[test]
    fn empty_party() {
        let mut painter = RecordingPainter::default();
        draw_battle_party(
            &BattlePartyData {
                entries: vec![],
                cursor: 0,
            },
            &[test_config()],
            &mut painter,
        );
        assert_eq!(painter.text_boxes.len(), 0);
    }
    #[test]
    fn no_configs() {
        let mut painter = RecordingPainter::default();
        draw_battle_party(&test_data(), &[], &mut painter);
        assert!(painter.text_boxes.is_empty());
    }
}
