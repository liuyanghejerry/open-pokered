//! Stats screen widget. Two-page monster stats display.
//! Uses `&[MenuConfig]` — configs[0] is the main area (stats page 1),
//! configs[1] is the stats box, configs[2] is the moves box.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Frame, Painter, Rgba, Ui};

// Gen-I HP bar shades — picked at >50% / >20% / below (the classic GB HUD
// convention). Game-specific, so they live with this widget rather than in
// the engine's color set.
const HP_FULL: Rgba = Rgba::rgb(0x20, 0x20, 0x20);
const HP_CAUTION: Rgba = Rgba::rgb(0x70, 0x70, 0x70);
const HP_CRITICAL: Rgba = Rgba::rgb(0x40, 0x40, 0x40);

/// Game Boy HP bar at tile `(tx, ty)`, `width_tiles` wide, 4px tall
/// (offset +2 from tile top), tri-color fill (full / caution / critical).
fn hp_bar<P: Painter>(
    frame: &mut Frame<'_, P>,
    tx: u32,
    ty: u32,
    width_tiles: u32,
    current: u16,
    max: u16,
) {
    let bar_x = tx * 8;
    let bar_y = ty * 8 + 2;
    let bar_w = width_tiles * 8;
    const BAR_H: u32 = 4;

    frame.pixel_rect(bar_x, bar_y, bar_w, BAR_H, Rgba::INK_BLACK);
    frame.pixel_rect(bar_x + 1, bar_y + 1, bar_w - 2, BAR_H - 2, Rgba::INK_WHITE);

    if max == 0 {
        return;
    }
    let inner_w = bar_w - 2;
    let fill = (current as u32 * inner_w) / max as u32;
    if fill == 0 {
        return;
    }
    let color = if current * 2 > max {
        HP_FULL
    } else if current * 5 > max {
        HP_CAUTION
    } else {
        HP_CRITICAL
    };
    frame.pixel_rect(bar_x + 1, bar_y + 1, fill.min(inner_w), BAR_H - 2, color);
}

#[derive(Debug, Clone)]
pub struct StatValue {
    pub label: String,
    pub value: u16,
    pub max_value: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct MoveSummary {
    pub name: String,
    pub pp: u8,
    pub max_pp: u8,
}

#[derive(Debug, Clone)]
pub struct StatsData {
    pub name: String,
    pub level: u8,
    pub species: String,
    pub hp: u32,
    pub max_hp: u32,
    pub status: Option<String>,
    pub stats: Vec<StatValue>,
    pub moves: Vec<MoveSummary>,
    pub page: usize,
}

/// Draw stats screen. For page 0, uses configs[0] (main area) + configs[1] (stats box).
/// For page 1, uses configs[0] (main area) + configs[2] (moves box).
pub fn draw_stats_screen<P: Painter>(data: &StatsData, configs: &[MenuConfig], painter: &mut P) {
    let Some(main_config) = configs.first() else {
        return;
    };
    let mut ui = Ui::new(painter);

    // Main area (region_0 in old code, no border)
    ui.text_box(main_config.area, Rgba::INK_BLACK, false, |frame| {
        let rel_tx = main_config.content.tx.saturating_sub(main_config.area.tx);
        let rel_ty = main_config.content.ty.saturating_sub(main_config.area.ty);

        // Name (interior col 9, row 1 → rel_tx+8, rel_ty+0 after subtracting area origin)
        frame.label(rel_tx + 8, rel_ty, &data.name, Rgba::INK_BLACK);
        // Level
        frame.label(
            rel_tx + 13,
            rel_ty + 1,
            &format!(":L{:2}", data.level),
            Rgba::INK_BLACK,
        );

        // HP bar and fraction
        frame.label(rel_tx, rel_ty + 3, "HP:", Rgba::INK_BLACK);
        hp_bar(
            frame,
            rel_tx + 3,
            rel_ty + 3,
            8,
            data.hp as u16,
            data.max_hp as u16,
        );
        frame.label(
            rel_tx + 3,
            rel_ty + 4,
            &format!("{:>3}/{:<3}", data.hp, data.max_hp),
            Rgba::INK_BLACK,
        );

        // Status
        let status_text = data.status.as_deref().unwrap_or("OK");
        frame.label(rel_tx + 15, rel_ty + 5, status_text, Rgba::INK_BLACK);

        // Dex number placeholder
        frame.label(rel_tx + 2, rel_ty + 6, "No.000", Rgba::INK_BLACK);

        if data.page == 0 {
            frame.label(rel_tx + 11, rel_ty + 9, "TYPE1/", Rgba::INK_BLACK);
            frame.label(rel_tx + 11, rel_ty + 11, "TYPE2", Rgba::INK_BLACK);
        } else {
            // Page 1: EXP display
            frame.label(rel_tx + 11, rel_ty + 3, "EXP", Rgba::INK_BLACK);
        }
    });

    match data.page {
        0 => {
            if let Some(stats_config) = configs.get(1) {
                ui.text_box(stats_config.area, Rgba::INK_BLACK, true, |frame| {
                    let rel_tx = stats_config
                        .content
                        .tx
                        .saturating_sub(stats_config.area.tx + 1);
                    let rel_ty = stats_config
                        .content
                        .ty
                        .saturating_sub(stats_config.area.ty + 1);
                    for (i, stat) in data.stats.iter().enumerate() {
                        let row = rel_ty + (i as u32) * 2;
                        frame.label(rel_tx, row, &stat.label, Rgba::INK_BLACK);
                        frame.label(
                            rel_tx + 5,
                            row + 1,
                            &format!("{:3}", stat.value),
                            Rgba::INK_BLACK,
                        );
                    }
                });
            }
        }
        1 => {
            if let Some(moves_config) = configs.get(2) {
                ui.text_box(moves_config.area, Rgba::INK_BLACK, true, |frame| {
                    let rel_tx = moves_config
                        .content
                        .tx
                        .saturating_sub(moves_config.area.tx + 1);
                    let rel_ty = moves_config
                        .content
                        .ty
                        .saturating_sub(moves_config.area.ty + 1);
                    for (i, mv) in data.moves.iter().enumerate().take(4) {
                        let name_row = rel_ty + (i as u32 * 2 + 1);
                        let pp_row = rel_ty + (i as u32 * 2 + 2);
                        frame.label(rel_tx + 1, name_row, &mv.name, Rgba::INK_BLACK);
                        frame.label(rel_tx + 10, pp_row, "PP", Rgba::INK_BLACK);
                        frame.label(
                            rel_tx + 13,
                            pp_row,
                            &format!("{:>2}/{:>2}", mv.pp, mv.max_pp),
                            Rgba::INK_BLACK,
                        );
                    }
                });
            }
        }
        _ => {}
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
        pixel_rects: Vec<(u32, u32, u32, u32, Rgba)>,
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
        fn draw_pixel_rect(&mut self, px: u32, py: u32, pw: u32, ph: u32, color: Rgba) {
            self.pixel_rects.push((px, py, pw, ph, color));
        }
        fn draw_gb_tile(&mut self, _: TilePos, _: u8, _: &str, _: Rgba) {}
    }

    fn main_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(6, 0, 14, 18),
            None,
            TileRect::new(7, 1, 12, 16),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn stats_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(6, 10, 10, 9),
            None,
            TileRect::new(7, 11, 8, 7),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn moves_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(6, 10, 12, 11),
            None,
            TileRect::new(7, 11, 10, 9),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn test_data() -> StatsData {
        StatsData {
            name: "SPARKIT".into(),
            level: 25,
            species: "SPARKIT".into(),
            hp: 60,
            max_hp: 60,
            status: None,
            stats: vec![
                StatValue {
                    label: "ATTACK".into(),
                    value: 55,
                    max_value: Some(120),
                },
                StatValue {
                    label: "DEFENSE".into(),
                    value: 40,
                    max_value: Some(110),
                },
                StatValue {
                    label: "SPEED".into(),
                    value: 90,
                    max_value: Some(140),
                },
                StatValue {
                    label: "SPECIAL".into(),
                    value: 50,
                    max_value: Some(130),
                },
            ],
            moves: vec![
                MoveSummary {
                    name: "THUNDERBOLT".into(),
                    pp: 5,
                    max_pp: 15,
                },
                MoveSummary {
                    name: "QUICK ATTACK".into(),
                    pp: 30,
                    max_pp: 30,
                },
            ],
            page: 0,
        }
    }

    #[test]
    fn draws_page0() {
        let mut painter = RecordingPainter::default();
        draw_stats_screen(
            &test_data(),
            &[main_config(), stats_config(), moves_config()],
            &mut painter,
        );
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("SPARKIT")));
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("ATTACK")));
    }
    #[test]
    fn draws_page1() {
        let mut data = test_data();
        data.page = 1;
        let mut painter = RecordingPainter::default();
        draw_stats_screen(
            &data,
            &[main_config(), stats_config(), moves_config()],
            &mut painter,
        );
        assert!(painter
            .texts
            .iter()
            .any(|(_, t, _)| t.contains("THUNDERBOLT")));
    }
    #[test]
    fn draws_hp_bar() {
        let mut painter = RecordingPainter::default();
        draw_stats_screen(&test_data(), &[main_config()], &mut painter);
        assert!(!painter.pixel_rects.is_empty());
    }
    #[test]
    fn no_configs() {
        let mut painter = RecordingPainter::default();
        draw_stats_screen(&test_data(), &[], &mut painter);
        assert!(painter.text_boxes.is_empty());
    }
}
