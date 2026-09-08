use dotzuki_engine::render_data::RenderData;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::ui_layout::schema::{BagDefaultLayout, SizeMode};

use crate::engine::{InkColor, Painter, TileRect, Ui};

pub fn draw<P: Painter>(
    items: &[(ItemId, u8)], cursor: usize, layout: &BagDefaultLayout, ui: &mut Ui<P>,
    render_data: &dyn RenderData<Move = MoveId, Item = ItemId, Species = Species>,
) {
    ui.clear(InkColor::White);

    let header = &layout.box_0;
    ui.text_box(header.rect, header.color, true, |frame| {
        for label in header.labels.iter() {
            frame.label(label.tx, label.ty, &label.text, label.color);
        }
    });

    let list_child = &layout.list;
    // Total selectable entries: items + the trailing CANCEL row.
    let total = items.len() as u32 + 1;
    let content_h = total + total.saturating_sub(1) * list_child.gap;
    // Auto height may never exceed the layout's own rect height: the bag box
    // has to stay on-screen (the audit saw the bottom border leave the screen
    // on long lists). The list becomes a scrolling window over the entries.
    let eff_h = match list_child.height_mode {
        SizeMode::Fixed => list_child.rect.th,
        SizeMode::Auto => clamp(
            content_h + list_child.padding.top + list_child.padding.bottom + 2,
            list_child.min_height,
            list_child.max_height,
        )
        .min(list_child.rect.th),
    };
    let rect = TileRect::new(list_child.rect.tx, list_child.rect.ty, list_child.rect.tw, eff_h);

    let start_y = list_child.padding.top;
    // Visible window: entries that fit in the box interior, scrolled to keep
    // the cursor row on-screen (the original bag list scrolls the same way).
    let body_rows = eff_h.saturating_sub(2); // interior rows below the top border
    let last_body_row = body_rows.saturating_sub(1);
    let row_pitch = 1 + list_child.gap;
    let visible_rows = if row_pitch == 0 {
        total
    } else {
        last_body_row.saturating_sub(start_y) / row_pitch + 1
    }
    .min(total);
    let offset = if (cursor as u32) >= visible_rows {
        cursor as u32 - (visible_rows - 1)
    } else {
        0
    } as usize;

    ui.text_box(rect, list_child.color, true, |frame| {
        for (i, (item_id, qty)) in items.iter().enumerate().skip(offset).take(visible_rows as usize) {
            let y = start_y + (i - offset) as u32 * row_pitch;
            let item_name = render_data.item_name(*item_id);
            let name: String = item_name.chars().take(layout.list.item_name_width as usize).collect();
            let label = format!(
                "{:<name_w$} ×{:<qty_w$}",
                name,
                qty,
                name_w = layout.list.item_name_width as usize,
                qty_w = layout.list.qty_width as usize,
            );
            frame.label(2, y, &label, InkColor::Black);
        }

        let cancel_index = items.len();
        if cancel_index >= offset && cancel_index < offset + visible_rows as usize {
            let cancel_y = start_y + (cancel_index - offset) as u32 * row_pitch;
            frame.label(2, cancel_y, "CANCEL", InkColor::Black);
        }

        if let Some(c) = &list_child.cursor {
            let cur_y = start_y + (cursor - offset) as u32 * row_pitch;
            frame.cursor_glyph_at(1, cur_y, c.glyph, c.color);
        }
    });
}

fn clamp(val: u32, min: Option<u32>, max: Option<u32>) -> u32 {
    let v = min.map_or(val, |m| val.max(m));
    max.map_or(v, |m| v.min(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{TilePos, Rgba};
    use pokered_data::ui_layout::schema::BAG_DEFAULT_LAYOUT;

    #[derive(Debug, Default)]
    struct Rec {
        ops: Vec<Op>,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Op {
        Box(TileRect, Rgba),
        Text(TilePos, String),
        Cursor(TilePos),
    }

    impl Painter for Rec {
        fn clear(&mut self, _color: Rgba) {}
        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.ops.push(Op::Box(rect, color));
        }
        fn draw_text(&mut self, pos: TilePos, text: &str, _color: Rgba) {
            self.ops.push(Op::Text(pos, text.to_string()));
        }
        fn draw_glyph(&mut self, pos: TilePos, _glyph: char, _color: Rgba) {
            self.ops.push(Op::Cursor(pos));
        }
        fn draw_pixel_rect(&mut self, _x: u32, _y: u32, _w: u32, _h: u32, _c: Rgba) {}
        fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
    }

    struct StubData;
    impl RenderData for StubData {
        type Move = MoveId;
        type Item = ItemId;
        type Species = Species;
        fn move_name(&self, _: MoveId) -> &str { "" }
        fn move_pp(&self, _: MoveId) -> (u8, u8) { (0, 0) }
        fn move_type(&self, _: MoveId) -> u8 { 0 }
        fn item_name(&self, _: ItemId) -> &str { "POTION" }
        fn species_name(&self, _: Species) -> &str { "" }
    }

    fn bag_items(n: usize) -> Vec<(ItemId, u8)> {
        vec![(ItemId::Potion, 1); n]
    }

    /// Long bag lists render as a scrolling window that stays inside the box:
    /// every label row sits on/below the top border but never on the bottom
    /// border, and the cursor row is always visible (the audit's HM03/HM04
    /// screenshots had the cursor off-screen and the border off-screen).
    #[test]
    fn long_bag_list_scrolls_and_stays_in_the_box() {
        let items = bag_items(20);
        let layout = &BAG_DEFAULT_LAYOUT;
        for &cursor in &[0usize, 5, 10, 15, items.len()] {
            let mut rec = Rec::default();
            let mut ui = Ui::new(&mut rec);
            draw(&items, cursor, layout, &mut ui, &StubData);

            let list_box = rec
                .ops
                .iter()
                .find_map(|op| match op {
                    Op::Box(r, _) if r.ty > 0 => Some(*r),
                    _ => None,
                })
                .expect("list box drawn");
            // The box bottom must stay on the 18-row screen.
            assert!(
                list_box.ty + list_box.th <= 18,
                "list box must fit on screen at cursor {cursor}"
            );
            let bottom_border = list_box.ty + list_box.th - 1;
            // Every text row must be inside the box interior.
            for op in &rec.ops {
                // Only rows inside the LIST box count (the header box has its
                // own labels above it).
                let in_list = |pos: &TilePos| pos.ty >= list_box.ty;
                if let Op::Text(pos, _) = op {
                    if !in_list(pos) {
                        continue;
                    }
                    assert!(
                        pos.ty > list_box.ty && pos.ty < bottom_border,
                        "label at row {} must be inside the box (cursor {cursor})",
                        pos.ty
                    );
                }
                if let Op::Cursor(pos) = op {
                    assert!(
                        pos.ty > list_box.ty && pos.ty < bottom_border,
                        "cursor at row {} must be inside the box (cursor {cursor})",
                        pos.ty
                    );
                }
            }
            // The cursor must be drawn for the selected entry.
            assert!(
                matches!(rec.ops.last(), Some(Op::Cursor(_))),
                "cursor drawn at cursor {cursor}"
            );
        }
    }

    /// CANCEL is always reachable: with the cursor on the last entry the
    /// CANCEL row is the one rendered.
    #[test]
    fn cancel_row_visible_when_cursor_on_it() {
        let items = bag_items(20);
        let mut rec = Rec::default();
        let mut ui = Ui::new(&mut rec);
        draw(&items, items.len(), &BAG_DEFAULT_LAYOUT, &mut ui, &StubData);
        let has_cancel = rec.ops.iter().any(|op| matches!(op, Op::Text(_, t) if t == "CANCEL"));
        assert!(has_cancel, "CANCEL row visible when the cursor is on it");
    }
}
