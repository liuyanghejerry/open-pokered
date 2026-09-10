//! Party list widget. Party members + optional action overlay.
//! Uses `&[MenuConfig]` — configs[0] is the party list, configs[1] is action menu.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

#[derive(Debug, Clone)]
pub struct PartyMemberEntry {
    pub name: String,
    pub level: u8,
    pub hp: u32,
    pub max_hp: u32,
    pub status: Option<String>,
    pub species_icon: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct PartyListData {
    pub members: Vec<PartyMemberEntry>,
    pub cursor: usize,
    pub action_menu: Option<Vec<String>>,
    pub action_cursor: usize,
}

pub fn draw_party_list<P: Painter>(data: &PartyListData, configs: &[MenuConfig], painter: &mut P) {
    let Some(list_config) = configs.first() else {
        return;
    };
    let mut ui = Ui::new(painter);

    ui.text_box(list_config.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx = list_config
            .content
            .tx
            .saturating_sub(list_config.area.tx + 1);
        let rel_ty = list_config
            .content
            .ty
            .saturating_sub(list_config.area.ty + 1);
        let row_step = 2u32;

        for (i, member) in data.members.iter().enumerate() {
            let row = rel_ty + (i as u32) * row_step;
            if row + 1 >= rel_ty + list_config.content.th {
                break;
            }

            if i == data.cursor && list_config.cursor.tile.is_some() {
                frame.cursor_glyph_at(rel_tx, row, '\u{25B6}', Rgba::INK_BLACK);
            }

            let mut col = rel_tx + 1;
            if let Some(ref icon) = member.species_icon {
                frame.label(col, row, icon, Rgba::INK_BLACK);
                col += icon.len() as u32 + 1;
            }

            let display_name: &str = if member.name.len() > 10 {
                &member.name[..10]
            } else {
                &member.name
            };
            frame.label(col, row, display_name, Rgba::INK_BLACK);
            col += display_name.len() as u32 + 1;

            let lv_str = format!(":L{}", member.level);
            frame.label(col, row, &lv_str, Rgba::INK_BLACK);

            if let Some(ref status) = member.status {
                if !status.is_empty() {
                    frame.label(rel_tx + 10, row, status, Rgba::INK_BLACK);
                }
            }

            let hp_str = format!("{}/{}", member.hp, member.max_hp);
            frame.label(rel_tx + 15, row, &hp_str, Rgba::INK_BLACK);
        }
    });

    if let Some(ref actions) = data.action_menu {
        if let Some(action_config) = configs.get(1) {
            ui.text_box(action_config.area, Rgba::INK_BLACK, true, |frame| {
                let rel_tx = action_config
                    .content
                    .tx
                    .saturating_sub(action_config.area.tx + 1);
                let rel_ty = action_config
                    .content
                    .ty
                    .saturating_sub(action_config.area.ty + 1);
                for (i, action) in actions.iter().enumerate() {
                    let row = rel_ty + (i as u32) * 2;
                    frame.label(rel_tx + 1, row, action, Rgba::INK_BLACK);
                    if i == data.action_cursor && action_config.cursor.tile.is_some() {
                        frame.cursor_glyph_at(rel_tx, row, '\u{25B6}', Rgba::INK_BLACK);
                    }
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

    fn list_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(0, 0, 20, 18),
            None,
            TileRect::new(1, 1, 18, 16),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn action_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(12, 4, 8, 8),
            None,
            TileRect::new(13, 5, 6, 6),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn test_data() -> PartyListData {
        PartyListData {
            members: vec![
                PartyMemberEntry {
                    name: "SPARKIT".into(),
                    level: 25,
                    hp: 60,
                    max_hp: 60,
                    status: None,
                    species_icon: None,
                    active: true,
                },
                PartyMemberEntry {
                    name: "LEAFKIT".into(),
                    level: 20,
                    hp: 10,
                    max_hp: 50,
                    status: Some("SLP".into()),
                    species_icon: None,
                    active: true,
                },
            ],
            cursor: 0,
            action_menu: Some(vec!["STATS".into(), "SWITCH".into(), "CANCEL".into()]),
            action_cursor: 0,
        }
    }

    #[test]
    fn draws_members() {
        let mut painter = RecordingPainter::default();
        draw_party_list(
            &test_data(),
            &[list_config(), action_config()],
            &mut painter,
        );
        assert!(painter.texts.iter().any(|(_, t, _)| t == "SPARKIT"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "LEAFKIT"));
    }
    #[test]
    fn draws_action_menu() {
        let mut painter = RecordingPainter::default();
        draw_party_list(
            &test_data(),
            &[list_config(), action_config()],
            &mut painter,
        );
        assert!(painter.texts.iter().any(|(_, t, _)| t == "STATS"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "SWITCH"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "CANCEL"));
    }
    #[test]
    fn draws_levels() {
        let mut painter = RecordingPainter::default();
        draw_party_list(&test_data(), &[list_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("L25")));
    }
    #[test]
    fn draws_status() {
        let mut painter = RecordingPainter::default();
        draw_party_list(&test_data(), &[list_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "SLP"));
    }
    #[test]
    fn draws_cursor() {
        let mut painter = RecordingPainter::default();
        draw_party_list(&test_data(), &[list_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
}
