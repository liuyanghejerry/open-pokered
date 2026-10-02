use crate::alloc_prelude::*;
use dotzuki_engine::render_data::RenderData;
use pokered_core::battle::menu::BagMenuState;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::ui_layout::schema::{BattleBagDefaultLayout, SizeMode};

use crate::engine::{InkColor, Painter, Rgba, TilePos, TileRect, Ui};

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
    // PrintListMenuEntries prints the name and the separate × / two-digit
    // quantity (home/list_menu.asm:364, 479-491). As in the ordinary bag,
    // reserve the quantity columns before measuring mixed-width name glyphs.
    let name_x = (rect.tx + 3) * 8;
    let right_x = (rect.tx + rect.tw - 1) * 8;
    let mut rows = Vec::new();
    for (i, (item_id, qty)) in items.iter().enumerate() {
        let quantity = format!("×{:>width$}", (*qty).min(99), width = list_child.qty_width as usize);
        let quantity_x = right_x.saturating_sub(ui.painter().measure_text_px(&quantity));
        let name_width = (list_child.item_name_width * 8)
            .min(quantity_x.saturating_sub(name_x + 8));
        let item_name = render_data.item_name(*item_id);
        let mut name = String::new();
        if ui.painter().measure_text_px(item_name) <= name_width {
            name.push_str(item_name);
        } else {
            for ch in item_name.chars() {
                if ui.painter().measure_text_px(&format!("{name}{ch}…")) > name_width {
                    break;
                }
                name.push(ch);
            }
            if ui.painter().measure_text_px("…") <= name_width {
                name.push('…');
            }
        }
        rows.push((start_y + i as u32 * (1 + list_child.gap), name, quantity, quantity_x));
    }

    ui.text_box(rect, list_child.color, true, |frame| {
        for (y, name, _, _) in &rows {
            frame.label(2, *y, name, InkColor::Black);
        }

        let cancel_y = start_y + items.len() as u32 * (1 + list_child.gap);
        frame.label(2, cancel_y, "CANCEL", InkColor::Black);

        if let Some(c) = &list_child.cursor {
            let cur_y = start_y + cursor as u32 * (1 + list_child.gap);
            frame.cursor_glyph_at(1, cur_y, c.glyph, c.color);
        }
    });
    for (y, _, quantity, quantity_x) in rows {
        ui.painter().draw_text_px(quantity_x, (rect.ty + 1 + y) * 8, &quantity, InkColor::Black.into());
    }
}

/// Repaint only the changed cursor cells of an already-rendered battle bag.
pub fn redraw_cursor<P: Painter>(
    previous_cursor: usize,
    state: &BagMenuState,
    layout: &BattleBagDefaultLayout,
    painter: &mut P,
) {
    let Some(cursor) = &layout.list.cursor else {
        return;
    };
    let position = |selected: usize| {
        TilePos::new(
            layout.list.rect.tx + 2,
            layout.list.rect.ty
                + 1
                + layout.list.padding.top
                + selected as u32 * (1 + layout.list.gap),
        )
    };
    let old = position(previous_cursor);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    painter.draw_glyph(
        position(state.cursor()),
        cursor.glyph,
        cursor.color.into(),
    );
}

/// Cursor ink region for the authored battle-bag layout.
pub fn cursor_damage(selected: usize, layout: &BattleBagDefaultLayout) -> crate::DamageRect {
    crate::DamageRect::cursor(TilePos::new(
        layout.list.rect.tx + 2,
        layout.list.rect.ty
            + 1
            + layout.list.padding.top
            + selected as u32 * (1 + layout.list.gap),
    ))
}

fn clamp(val: u32, min: Option<u32>, max: Option<u32>) -> u32 {
    let v = min.map_or(val, |m| val.max(m));
    max.map_or(v, |m| v.min(m))
}

#[cfg(all(test, feature = "framebuffer"))]
mod pixel_tests {
    use super::*;
    use crate::backends::FrameBufferPainter;
    use pokered_core::game_state::Lang;
    use pokered_data::impl_traits::PokemonRenderData;
    use pokered_data::ui_layout::schema::BATTLE_BAG_DEFAULT_LAYOUT;
    use pokered_renderer::{FrameBuffer, RenderConfig};

    #[test]
    fn long_item_quantities_are_visible_inside_the_original_tile_border() {
        for lang in [Lang::En, Lang::Zh] {
            for item in [ItemId::SuperPotion, ItemId::ThunderStone] {
                for qty in [1, 99] {
                    let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                    let state = BagMenuState::new(vec![(item, qty)]);
                    let mut painter = FrameBufferPainter::new(&mut fb).with_lang(lang);
                    draw(&state, &BATTLE_BAG_DEFAULT_LAYOUT, &mut Ui::new(&mut painter),
                        &PokemonRenderData::new(lang == Lang::Zh));
                    let y = if lang == Lang::Zh { 95 } else { 96 };
                    // The existing Fusion Pixel provider is the typography
                    // oracle. Quantities have their own right-aligned region;
                    // neither long names nor the quantity may overwrite it.
                    let quantity = format!("×{qty:>2}");
                    let quantity_x = 152 - dotzuki_renderer::embedded_font::measure_text(&quantity);
                    let mut expected = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                    dotzuki_renderer::embedded_font::draw_text(
                        &quantity, quantity_x, y, Rgba::BLACK, &mut expected);
                    for row in 0..10 {
                        for x in quantity_x..152 {
                            assert_eq!(fb.get_pixel(x, y + row), expected.get_pixel(x, y + row),
                                "quantity must use the project font: {lang:?} {item:?} ×{qty} at ({x},{row})");
                        }
                        for x in quantity_x - 8..quantity_x {
                            assert_eq!(fb.get_pixel(x, y + row), Some(Rgba::WHITE),
                                "name and quantity must have a clear separating cell");
                        }
                    }
                    let border = pokered_renderer::embedded_font::box_tiles::VERTICAL_RIGHT;
                    for row in 0..8 {
                        for col in 0..8 {
                            let expected = if border[row] & (0x80 >> col) != 0 { Rgba::BLACK } else { Rgba::WHITE };
                            assert_eq!(fb.get_pixel(152 + col, 96 + row as u32), Some(expected),
                                "item text must preserve the right border");
                        }
                    }
                }
            }
        }
    }
}
