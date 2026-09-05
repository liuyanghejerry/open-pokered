use dotzuki_engine::render_data::RenderData;
use pokered_core::game_state::Lang;
use pokered_core::items::shop::ShopMenuState;
use pokered_data::item_data::get_item_data;
use pokered_data::items::ItemId;
use pokered_data::lang_data;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::ui_layout::schema::{MartBuyItemsWithMoneyLayout, MartConfirmLayout, MartMainMenuLayout, MartQuantityLayout, MartResultDialogLayout, MartSellItemsWithMoneyLayout};

use crate::engine::{InkColor, Painter, Ui};

/// Yes/No choice for confirmation dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    Yes,
    No,
}

/// Internal helper: draws the main-menu box + labels + cursor from a raw cursor index.
/// Shared by [`draw_main_menu`] (which takes `&ShopMenuState`) and
/// [`draw_main_with_money`] (which takes `cursor: usize`).
fn draw_main_menu_box<P: Painter>(cursor_index: usize, layout: &MartMainMenuLayout, ui: &mut Ui<P>, is_zh: bool) {
    let m = &layout.menu_box;
    ui.text_box(m.rect, m.color, true, |frame| {
        for label in m.labels.iter() {
            frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
        }
        let cursor_row = layout.cursor.base_ty + (cursor_index as u32 * layout.cursor.row_step);
        frame.cursor_at(layout.cursor.tx, cursor_row, layout.cursor.color);
    });
}

pub fn draw_main_menu<P: Painter>(state: &ShopMenuState, layout: &MartMainMenuLayout, ui: &mut Ui<P>, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    draw_main_menu_box(state.cursor(), layout, ui, is_zh);
}

pub fn draw_main_with_money<P: Painter>(cursor: usize, player_money: u32, layout: &MartMainMenuLayout, ui: &mut Ui<P>, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    draw_main_menu_box(cursor, layout, ui, is_zh);
    let money_label = if lang == Lang::Zh { format!("金钱${}元", player_money) } else { format!("MONEY ${}", player_money) };
    let money_pos = layout
        .dynamic_labels
        .iter()
        .find_map(|(k, v)| if k == "money_value" { Some((v.tx, v.ty, v.color)) } else { None })
        .unwrap_or((1, 1, InkColor::Black));
    ui.text_box(layout.money_box.rect, layout.money_box.color, true, |frame| {
        frame.label(money_pos.0, money_pos.1, &money_label, money_pos.2);
    });
}

pub fn draw_buy_items_with_money<P: Painter>(
    items: &[ItemId],
    cursor: usize,
    scroll_offset: usize,
    player_money: u32,
    layout: &MartBuyItemsWithMoneyLayout,
    ui: &mut Ui<P>,
    lang: Lang,
    render_data: &dyn RenderData<Move = MoveId, Item = ItemId, Species = Species>,
) {
    // No `ui.clear` — the mart draws over the live overworld map (see the
    // GameScreen::Shop dispatch in pokered-app's game.rs).
    let lb = &layout.list_box;
    ui.text_box(lb.rect, lb.color, true, |frame| {
        for (i, item_id) in items.iter().skip(scroll_offset).enumerate() {
            let row = 1 + (i as u32 * 2);
            // Interior rows run 0..=th-3; item rows are odd, so the last
            // visible row is th-3 (th-1 would let a row land on the border).
            if row >= lb.rect.th.saturating_sub(2) {
                break;
            }
            if let Some(data) = get_item_data(*item_id) {
                let name = render_data.item_name(*item_id);
                let label = format!("{:<12} ${:<5}", name, data.price);
                frame.label(2, row, &label, InkColor::Black);
            }
        }
        let cursor_row = layout.cursor.base_ty
            + ((cursor - scroll_offset) as u32 * layout.cursor.row_step);
        frame.cursor_at(layout.cursor.tx, cursor_row, layout.cursor.color);
    });
    let money_pos = layout
        .dynamic_labels
        .iter()
        .find_map(|(k, v)| if k == "money_value" { Some((v.tx, v.ty, v.color)) } else { None })
        .unwrap_or((1, 1, InkColor::Black));
    let money_label = if lang == Lang::Zh { format!("金钱${}元", player_money) } else { format!("MONEY ${}", player_money) };
    let m = &layout.money_box;
    ui.text_box(m.rect, m.color, true, |frame| {
        frame.label(money_pos.0, money_pos.1, &money_label, money_pos.2);
    });
}

pub fn draw_sell_items_with_money<P: Painter>(
    owned_items: &[(ItemId, u32)],
    cursor: usize,
    scroll_offset: usize,
    player_money: u32,
    layout: &MartSellItemsWithMoneyLayout,
    ui: &mut Ui<P>,
    lang: Lang,
    render_data: &dyn RenderData<Move = MoveId, Item = ItemId, Species = Species>,
) {
    // No `ui.clear` — the mart draws over the live overworld map (see the
    // GameScreen::Shop dispatch in pokered-app's game.rs).
    let lb = &layout.list_box;
    ui.text_box(lb.rect, lb.color, true, |frame| {
        let max_row = lb.rect.th.saturating_sub(2);
        for (i, (item_id, qty)) in owned_items.iter().skip(scroll_offset).enumerate() {
            let row = 1 + (i as u32 * 2);
            if row >= max_row {
                break;
            }
            if let Some(_data) = get_item_data(*item_id) {
                let name = render_data.item_name(*item_id);
                let label = format!("{:<12} ×{:<2}", name, qty);
                frame.label(2, row, &label, InkColor::Black);
            }
        }
        let cancel_row = 1 + ((owned_items.len() - scroll_offset) as u32 * 2);
        if cancel_row < max_row {
            frame.label(2, cancel_row, "CANCEL", InkColor::Black);
        }
        let cursor_row = layout.cursor.base_ty
            + ((cursor - scroll_offset) as u32 * layout.cursor.row_step);
        frame.cursor_at(layout.cursor.tx, cursor_row, layout.cursor.color);
    });
    let money_pos = layout
        .dynamic_labels
        .iter()
        .find_map(|(k, v)| if k == "money_value" { Some((v.tx, v.ty, v.color)) } else { None })
        .unwrap_or((1, 1, InkColor::Black));
    let money_label = if lang == Lang::Zh { format!("金钱${}元", player_money) } else { format!("MONEY ${}", player_money) };
    let m = &layout.money_box;
    ui.text_box(m.rect, m.color, true, |frame| {
        frame.label(money_pos.0, money_pos.1, &money_label, money_pos.2);
    });
}

pub fn draw_quantity<P: Painter>(
    item_name: &str,
    quantity: u8,
    unit_price: u32,
    total_cost: u32,
    player_money: u32,
    layout: &MartQuantityLayout,
    lang: Lang,
    ui: &mut Ui<P>,
) {
    let find = |key: &str| -> (u32, u32, InkColor) {
        layout
            .dynamic_labels
            .iter()
            .find_map(|(k, v)| if k == key { Some((v.tx, v.ty, v.color)) } else { None })
            .unwrap_or((1, 1, InkColor::Black))
    };
    let name_pos = find("item_name");
    let qty_pos = find("qty_value");
    let cost_pos = find("cost_value");
    let money_pos = find("money_value");

    let d = &layout.detail_box;
    ui.text_box(d.rect, d.color, true, |frame| {
        frame.label(name_pos.0, name_pos.1, item_name, name_pos.2);
        let qty_label = format!("×{:>2}", quantity);
        frame.label(qty_pos.0, qty_pos.1, &qty_label, qty_pos.2);
        let cost_label = format!("${}", total_cost);
        frame.label(cost_pos.0, cost_pos.1, &cost_label, cost_pos.2);
    });
    let money_label = if lang == Lang::Zh { format!("金钱${}元", player_money) } else { format!("MONEY ${}", player_money) };
    let m = &layout.money_box;
    ui.text_box(m.rect, m.color, true, |frame| {
        frame.label(money_pos.0, money_pos.1, &money_label, money_pos.2);
    });
    let _ = unit_price;
}

pub fn draw_confirm<P: Painter>(lang: Lang,message: &str, selected: ConfirmChoice, layout: &MartConfirmLayout, ui: &mut Ui<P>) {
    let is_zh = lang == Lang::Zh;
    // Bordered message box: over the live-map backdrop a borderless region
    // reads as stray glyphs floating on the scene (the original prints this
    // in a standard framed textbox at the top of the screen).
    ui.text_box(layout.message_region.rect, layout.message_region.color, true, |frame| {
        for (i, line) in message.lines().enumerate() {
            frame.label(1, (i as u32) * 2, line, InkColor::Black);
        }
    });
    let b = &layout.choice_box;
    ui.text_box(b.rect, b.color, true, |frame| {
        for label in b.labels.iter() {
            frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
        }
        let cursor_row = layout.cursor.base_ty + (match selected {
            ConfirmChoice::Yes => 0,
            ConfirmChoice::No => 1,
        } as u32 * layout.cursor.row_step);
        frame.cursor_at(layout.cursor.tx, cursor_row, layout.cursor.color);
    });
}

pub fn draw_result_dialog<P: Painter>(lines: &[&str], layout: &MartResultDialogLayout, ui: &mut Ui<P>) {
    let b = &layout.result_box;
    let msg = layout
        .dynamic_labels
        .iter()
        .find_map(|(k, v)| if k == "message_line" { Some((v.tx, v.ty)) } else { None })
        .unwrap_or((1, 1));
    ui.text_box(b.rect, b.color, true, |frame| {
        for (i, line) in lines.iter().enumerate() {
            frame.label(msg.0, msg.1 + (i as u32 * 2), line, InkColor::Black);
        }
    });
}
