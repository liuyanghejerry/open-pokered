//! Mart / shop widget system — 6 sub-functions for the full buy/sell flow.
//! Uses `&[MenuConfig]` — different sub-functions use different config layouts:
//! - main: [menu_box, money_box]
//! - items: [list_box, money_box]
//! - quantity: [detail_box, money_box]
//! - confirm: [message_region, choice_box]
//! - result: [result_box]
//! - message: [message_box]

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, Ui};

// ---------------------------------------------------------------------------
// Shared data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MartItemEntry {
    pub name: String,
    pub price: u32,
    pub owned: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct MartMainData {
    pub greeting: String,
    pub options: Vec<String>,
    pub cursor: usize,
    pub balance: u32,
}

#[derive(Debug, Clone)]
pub struct MartItemsData {
    pub items: Vec<MartItemEntry>,
    pub cursor: usize,
    pub balance: u32,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct MartConfirmData {
    pub item_name: String,
    pub item_price: u32,
    pub balance: u32,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct MartQuantityData {
    pub item_name: String,
    pub item_price: u32,
    pub quantity: u32,
    pub total: u32,
    pub balance: u32,
    pub max_quantity: u32,
}

#[derive(Debug, Clone)]
pub struct MartResultData {
    pub message: String,
    pub item_name: String,
    pub item_quantity: u32,
    pub total_cost: u32,
    pub balance: u32,
}

#[derive(Debug, Clone)]
pub struct MartMessageData {
    pub message: String,
    pub balance: u32,
}

// ---------------------------------------------------------------------------
// Money helper
// ---------------------------------------------------------------------------

fn money_label(balance: u32) -> String {
    format!("MONEY ${}", balance)
}

fn draw_money_box<P: Painter>(
    balance: u32,
    config: &MenuConfig,
    ui: &mut Ui<P>,
    rel_tx_override: Option<u32>,
) {
    ui.text_box(config.area, Rgba::INK_BLACK, true, |frame| {
        let rel_tx =
            rel_tx_override.unwrap_or_else(|| config.content.tx.saturating_sub(config.area.tx + 1));
        let rel_ty = config.content.ty.saturating_sub(config.area.ty + 1);
        frame.label(rel_tx, rel_ty, &money_label(balance), Rgba::INK_BLACK);
    });
}

// ---------------------------------------------------------------------------
// draw_mart_main
// ---------------------------------------------------------------------------

pub fn draw_mart_main<P: Painter>(data: &MartMainData, configs: &[MenuConfig], painter: &mut P) {
    let mut ui = Ui::new(painter);

    // Menu box (configs[0]) with labels and cursor — no greeting
    if let Some(menu) = configs.first() {
        ui.text_box(menu.area, Rgba::INK_BLACK, true, |frame| {
            let labels = &menu.label_positions;

            for (i, opt) in data.options.iter().enumerate() {
                let (lx, ly) = if i < labels.len() {
                    (labels[i].0, labels[i].1)
                } else {
                    // Fallback: BUY at (1,1), SELL at (1,3), QUIT at (1,5)
                    (1, 1 + (i as u32) * 2)
                };
                frame.label(lx, ly, opt, Rgba::INK_BLACK);
            }

            // Cursor (left of selected label)
            if menu.cursor.tile.is_some() {
                let cur_y = if data.cursor < labels.len() {
                    labels[data.cursor].1
                } else {
                    1 + (data.cursor as u32) * 2
                };
                frame.cursor_glyph_at(0, cur_y, '\u{25B6}', Rgba::INK_BLACK);
            }
        });
    }

    // Money box (configs[1])
    if let Some(money_cfg) = configs.get(1) {
        draw_money_box(data.balance, money_cfg, &mut ui, Some(1));
    }
}

// ---------------------------------------------------------------------------
// draw_mart_items
// ---------------------------------------------------------------------------

pub fn draw_mart_items<P: Painter>(data: &MartItemsData, configs: &[MenuConfig], painter: &mut P) {
    let mut ui = Ui::new(painter);

    // List box (configs[0])
    if let Some(list) = configs.first() {
        ui.text_box(list.area, Rgba::INK_BLACK, true, |frame| {
            let rel_tx = list.content.tx.saturating_sub(list.area.tx + 1);
            let rel_ty = list.content.ty.saturating_sub(list.area.ty + 1);

            // Title at top
            frame.label(rel_tx + 1, rel_ty, &data.title, Rgba::INK_BLACK);

            for (i, entry) in data.items.iter().enumerate() {
                let row = rel_ty + 1 + (i as u32 * 2);
                if row >= rel_ty + list.content.th.saturating_sub(1) {
                    break;
                }
                let label = format!("{:<12} ${:<5}", entry.name, entry.price);
                frame.label(rel_tx + 1, row, &label, Rgba::INK_BLACK);
            }

            // Cursor
            if list.cursor.tile.is_some() {
                let cur_row = rel_ty + 1 + (data.cursor as u32 * 2);
                frame.cursor_glyph_at(rel_tx, cur_row, '\u{25B6}', Rgba::INK_BLACK);
            }
        });
    }

    // Money box (configs[1])
    if let Some(money_cfg) = configs.get(1) {
        draw_money_box(data.balance, money_cfg, &mut ui, Some(1));
    }
}

// ---------------------------------------------------------------------------
// draw_mart_confirm
// ---------------------------------------------------------------------------

pub fn draw_mart_confirm<P: Painter>(
    data: &MartConfirmData,
    configs: &[MenuConfig],
    painter: &mut P,
) {
    let mut ui = Ui::new(painter);

    // Message region (configs[0], no border)
    if let Some(msg) = configs.first() {
        ui.text_box(msg.area, Rgba::INK_BLACK, false, |frame| {
            let rel_tx = msg.content.tx.saturating_sub(msg.area.tx);
            let rel_ty = msg.content.ty.saturating_sub(msg.area.ty);
            let label = format!("{}  ${} -- Is that OK?", data.item_name, data.item_price);
            frame.label(rel_tx, rel_ty, &label, Rgba::INK_BLACK);
        });
    }

    // Choice box (configs[1], bordered) with YES/NO and cursor
    if let Some(choice) = configs.get(1) {
        ui.text_box(choice.area, Rgba::INK_BLACK, true, |frame| {
            let rel_tx = choice.content.tx.saturating_sub(choice.area.tx + 1);
            let rel_ty = choice.content.ty.saturating_sub(choice.area.ty + 1);
            frame.label(rel_tx + 1, rel_ty, "YES", Rgba::INK_BLACK);
            frame.label(rel_tx + 1, rel_ty + 2, "NO", Rgba::INK_BLACK);
            if choice.cursor.tile.is_some() {
                let cur_row = rel_ty + (data.cursor as u32) * 2;
                frame.cursor_glyph_at(rel_tx, cur_row, '\u{25B6}', Rgba::INK_BLACK);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// draw_mart_quantity
// ---------------------------------------------------------------------------

pub fn draw_mart_quantity<P: Painter>(
    data: &MartQuantityData,
    configs: &[MenuConfig],
    painter: &mut P,
) {
    let mut ui = Ui::new(painter);

    // Detail box (configs[0])
    if let Some(detail) = configs.first() {
        ui.text_box(detail.area, Rgba::INK_BLACK, true, |frame| {
            let rel_tx = detail.content.tx.saturating_sub(detail.area.tx + 1);
            let rel_ty = detail.content.ty.saturating_sub(detail.area.ty + 1);

            // Item name
            frame.label(rel_tx, rel_ty, &data.item_name, Rgba::INK_BLACK);

            // Quantity
            let qty_label = format!("x{:>2}", data.quantity);
            frame.label(rel_tx + 1, rel_ty + 2, &qty_label, Rgba::INK_BLACK);

            // Cost
            let cost_label = format!("${}", data.total);
            frame.label(rel_tx, rel_ty + 4, &cost_label, Rgba::INK_BLACK);
        });
    }

    // Money box (configs[1])
    if let Some(money_cfg) = configs.get(1) {
        draw_money_box(data.balance, money_cfg, &mut ui, Some(1));
    }
}

// ---------------------------------------------------------------------------
// draw_mart_result
// ---------------------------------------------------------------------------

pub fn draw_mart_result<P: Painter>(
    data: &MartResultData,
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

        // Message lines
        for (i, line) in data.message.lines().enumerate() {
            frame.label(rel_tx, rel_ty + (i as u32 * 2), line, Rgba::INK_BLACK);
        }

        // Item details
        let detail_y = rel_ty + (data.message.lines().count() as u32) * 2 + 1;
        let detail = format!("{}  x{}", data.item_name, data.item_quantity);
        frame.label(rel_tx, detail_y, &detail, Rgba::INK_BLACK);

        // Cost
        let cost_text = format!("${}", data.total_cost);
        frame.label(rel_tx, detail_y + 2, &cost_text, Rgba::INK_BLACK);

        // Balance
        let balance_text = money_label(data.balance);
        frame.label(
            rel_tx,
            rel_ty + config.content.th.saturating_sub(1),
            &balance_text,
            Rgba::INK_BLACK,
        );
    });
}

// ---------------------------------------------------------------------------
// draw_mart_message
// ---------------------------------------------------------------------------

pub fn draw_mart_message<P: Painter>(
    data: &MartMessageData,
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

        // Centred message
        let msg_lines: Vec<&str> = data.message.split('\n').collect();
        let msg_height = msg_lines.len() as u32;
        let vert_pad = config.content.th.saturating_sub(msg_height) / 2;
        for (i, line) in msg_lines.iter().enumerate() {
            let line_w = line.len() as u32;
            let pad_x = config.content.tw.saturating_sub(line_w) / 2;
            frame.label(
                rel_tx + pad_x,
                rel_ty + vert_pad + i as u32,
                line,
                Rgba::INK_BLACK,
            );
        }

        // Balance
        let balance_text = money_label(data.balance);
        let bal_y = rel_ty + config.content.th.saturating_sub(1);
        frame.label(rel_tx, bal_y, &balance_text, Rgba::INK_BLACK);
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    fn menu_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(1, 1, 18, 16),
            None,
            TileRect::new(2, 2, 16, 14),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }
    fn money_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(1, 14, 18, 3),
            None,
            TileRect::new(2, 15, 16, 1),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        )
    }
    fn choice_config() -> MenuConfig {
        MenuConfig::new(
            TileRect::new(10, 10, 8, 6),
            None,
            TileRect::new(11, 11, 6, 4),
            dotzuki_engine::menu::CursorStyle::new(Some(223), Default::default()),
        )
    }

    #[test]
    fn draw_main_shows_options() {
        let data = MartMainData {
            greeting: "Welcome!".into(),
            options: vec!["BUY".into(), "SELL".into(), "QUIT".into()],
            cursor: 0,
            balance: 5000,
        };
        let mut painter = RecordingPainter::default();
        draw_mart_main(&data, &[menu_config(), money_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "BUY"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "SELL"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "QUIT"));
    }
    #[test]
    fn draw_main_shows_money() {
        let data = MartMainData {
            greeting: "Hi!".into(),
            options: vec!["BUY".into()],
            cursor: 0,
            balance: 9999,
        };
        let mut painter = RecordingPainter::default();
        draw_mart_main(&data, &[menu_config(), money_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("9999")));
        assert_eq!(painter.text_boxes.len(), 2); // menu box + money box
    }
    #[test]
    fn draw_main_cursor() {
        let data = MartMainData {
            greeting: "Hi!".into(),
            options: vec!["BUY".into(), "SELL".into()],
            cursor: 1,
            balance: 100,
        };
        let mut painter = RecordingPainter::default();
        draw_mart_main(&data, &[menu_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
    #[test]
    fn draw_items_shows_entries() {
        let data = MartItemsData {
            items: vec![MartItemEntry {
                name: "POTION".into(),
                price: 300,
                owned: None,
            }],
            cursor: 0,
            balance: 5000,
            title: "BUY".into(),
        };
        let mut painter = RecordingPainter::default();
        draw_mart_items(&data, &[menu_config(), money_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("POTION")));
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("300")));
        assert_eq!(painter.text_boxes.len(), 2);
    }
    #[test]
    fn draw_confirm_yes_no() {
        let data = MartConfirmData {
            item_name: "POTION".into(),
            item_price: 300,
            balance: 1000,
            cursor: 0,
        };
        let msg_cfg = MenuConfig::new(
            TileRect::new(1, 10, 18, 3),
            None,
            TileRect::new(1, 10, 18, 2),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        );
        let mut painter = RecordingPainter::default();
        draw_mart_confirm(&data, &[msg_cfg, choice_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t == "YES"));
        assert!(painter.texts.iter().any(|(_, t, _)| t == "NO"));
    }
    #[test]
    fn draw_confirm_cursor() {
        let data = MartConfirmData {
            item_name: "POTION".into(),
            item_price: 300,
            balance: 500,
            cursor: 1,
        };
        let msg_cfg = MenuConfig::new(
            TileRect::new(1, 10, 18, 3),
            None,
            TileRect::new(1, 10, 18, 2),
            dotzuki_engine::menu::CursorStyle::new(None, Default::default()),
        );
        let mut painter = RecordingPainter::default();
        draw_mart_confirm(&data, &[msg_cfg, choice_config()], &mut painter);
        assert!(!painter.glyphs.is_empty());
    }
    #[test]
    fn draw_quantity() {
        let data = MartQuantityData {
            item_name: "POTION".into(),
            item_price: 300,
            quantity: 3,
            total: 900,
            balance: 5000,
            max_quantity: 10,
        };
        let mut painter = RecordingPainter::default();
        draw_mart_quantity(&data, &[menu_config(), money_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("POTION")));
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("900")));
        assert_eq!(painter.text_boxes.len(), 2);
    }
    #[test]
    fn draw_result() {
        let data = MartResultData {
            message: "Here you are!".into(),
            item_name: "POTION".into(),
            item_quantity: 3,
            total_cost: 900,
            balance: 4100,
        };
        let mut painter = RecordingPainter::default();
        draw_mart_result(&data, &[menu_config()], &mut painter);
        assert!(painter
            .texts
            .iter()
            .any(|(_, t, _)| t.contains("Here you are")));
    }
    #[test]
    fn draw_message() {
        let data = MartMessageData {
            message: "No money!".into(),
            balance: 50,
        };
        let mut painter = RecordingPainter::default();
        draw_mart_message(&data, &[menu_config()], &mut painter);
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("No money")));
        assert!(painter.texts.iter().any(|(_, t, _)| t.contains("50")));
    }
}
