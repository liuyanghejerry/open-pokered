use jrpg_engine::render_data::RenderData;
use pokered_core::battle::menu::BagMenuState;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::ui_layout::schema::{BattleBagDefaultLayout, SizeMode};

use crate::engine::{InkColor, Painter, TileRect, Ui};

pub fn draw<P: Painter>(
    state: &BagMenuState, layout: &BattleBagDefaultLayout, ui: &mut Ui<P>,
    render_data: &dyn RenderData<Move = MoveId, Item = ItemId, Species = Species>,
) {
    let items = state.items();
    let cursor = state.cursor();

    let list_child = &layout.list;
    let num_items = items.len() as u32 + 1;
    let content_h = num_items + num_items.saturating_sub(1) * list_child.gap;
    let eff_h = match list_child.height_mode {
        SizeMode::Fixed => list_child.rect.th,
        SizeMode::Auto => clamp(
            content_h + list_child.padding.top + list_child.padding.bottom + 2,
            list_child.min_height,
            list_child.max_height,
        ),
    };
    let rect = TileRect::new(list_child.rect.tx, list_child.rect.ty, list_child.rect.tw, eff_h);

    let start_y = list_child.padding.top;

    ui.text_box(rect, list_child.color, true, |frame| {
        for (i, (item_id, qty)) in items.iter().enumerate() {
            let item_name = render_data.item_name(*item_id);
            let truncated: String = item_name.chars().take(layout.list.item_name_width as usize).collect();
            let label = format!("{} \u{00D7}{}", truncated, qty);
            let y = start_y + i as u32 * (1 + list_child.gap);
            frame.label(2, y, &label, InkColor::Black);
        }

        let cancel_y = start_y + items.len() as u32 * (1 + list_child.gap);
        frame.label(2, cancel_y, "CANCEL", InkColor::Black);

        if let Some(c) = &list_child.cursor {
            let cur_y = start_y + cursor as u32 * (1 + list_child.gap);
            frame.cursor_glyph_at(1, cur_y, c.glyph, c.color);
        }
    });
}

fn clamp(val: u32, min: Option<u32>, max: Option<u32>) -> u32 {
    let v = min.map_or(val, |m| val.max(m));
    max.map_or(v, |m| v.min(m))
}
