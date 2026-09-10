use crate::alloc_prelude::*;
use dotzuki_engine::render_data::RenderData;
use pokered_core::battle::menu::MoveMenuState;
use pokered_data::moves::MoveId;
use pokered_data::ui_layout::schema::BattleMoveDefaultLayout;

use crate::engine::{InkColor, Painter, TileRect, Ui};

pub fn draw<P: Painter>(
    state: &MoveMenuState,
    layout: &BattleMoveDefaultLayout,
    ui: &mut Ui<P>,
    lang: pokered_core::game_state::Lang,
    render_data: &dyn RenderData<Move = MoveId, Item = pokered_data::items::ItemId, Species = pokered_data::species::Species>,
) {
    if lang == pokered_core::game_state::Lang::Zh {
        draw_zh(state, ui, render_data);
        return;
    }
    let moves = state.moves();
    let cursor = state.cursor();

    // Draw the full-width base dialog box first so the bottom area has a complete
    // border at row 12; the move-list box (box_0, starting at col 4) overlays it.
    // Restores pre-unification layering where standard_dialog was always underneath.
    ui.text_box(layout.base.rect, layout.base.color, true, |_frame| {});

    // Frame coords below are RELATIVE to the box interior origin (rect.tx + 1, rect.ty + 1)
    // because text_box adds +1 padding for the border. Move list box rect (4, 12, 16, 6)
    // → interior origin (5, 13); native draws move names at screen (6, 13+i) → frame (1, i).
    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for (i, slot) in moves.iter().enumerate() {
            let name = render_data.move_name(slot.move_id);
            let truncated: String = name.chars().take(12).collect();
            frame.label(1, i as u32, &truncated, InkColor::Black);
        }
        if let Some(list_cursor) = &layout.list_default.cursor {
            let cursor_row = list_cursor.base_ty + cursor as u32 * list_cursor.row_step;
            frame.cursor_glyph_at(list_cursor.tx, cursor_row, list_cursor.glyph, list_cursor.color);
        }
    });

    if cursor < moves.len() {
        let slot = &moves[cursor];
        // PP info box rect (0, 8, 11, 5) → interior origin (1, 9).
        // Native: TYPE/ at (1,9)=frame(0,0); type at (1,10)=frame(0,1);
        //         PP at (2,11)=frame(1,2); PP value at (5,11)=frame(4,2).
        ui.text_box(layout.box_1.rect, layout.box_1.color, true, |frame| {
            for label in layout.box_1.labels.iter() {
                frame.label(label.tx, label.ty, &label.text, label.color);
            }
            let type_id = render_data.move_type(slot.move_id);
            let type_str = move_type_display_name(type_id);
            frame.label(0, 1, &type_str, InkColor::Black);

            let pp_text = format!(
                "{:>2}/{:>2}",
                slot.current_pp.min(99),
                slot.max_pp.min(99)
            );
            frame.label(4, 2, &pp_text, InkColor::Black);
        });
    }

    // Connector tiles bridging the move-list box top border with the PP-info box.
    // Positions derived from layout boxes instead of hardcoded screen coordinates.
    ui.text_box(TileRect::new(0, 0, 20, 18), InkColor::Black, false, |frame| {
        let left_tx = layout.box_0.rect.tx;
        let right_tx = layout.box_1.rect.tx + layout.box_1.rect.tw - 1;
        let connector_ty = layout.base.rect.ty;
        frame.gb_tile(left_tx, connector_ty, 0x7A, "", InkColor::Black);
        frame.gb_tile(right_tx, connector_ty, 0x7E, "", InkColor::Black);
    });
}

fn move_type_display_name(type_id: u8) -> String {
    // Type IDs use the internal enum ordering (non-sequential).
    // 0x00-0x08: Normal through Ghost
    // 0x14-0x1A: Fire through Dragon (index 9-15 in display order)
    let names: [&str; 15] = [
        "NORMAL", "FIGHTING", "FLYING", "POISON", "GROUND",
        "ROCK", "BIRD", "BUG", "GHOST", "FIRE",
        "WATER", "GRASS", "ELECTRIC", "PSYCHIC", "ICE",
    ];
    let idx = match type_id {
        0x00..=0x08 => type_id as usize,
        0x14..=0x19 => (type_id - 0x14 + 9) as usize,
        _ => return "???".to_string(),
    };
    if idx < names.len() {
        names[idx].to_string()
    } else {
        "???".to_string()
    }
}

// Four 10px CJK rows need a 40px interior; the English box only has 32px.
fn draw_zh<P: Painter>(
    state: &MoveMenuState,
    ui: &mut Ui<P>,
    data: &dyn RenderData<Move = MoveId, Item = pokered_data::items::ItemId, Species = pokered_data::species::Species>,
) {
    ui.text_box(TileRect::new(0, 8, 11, 4), InkColor::Black, true, |_| {});
    ui.text_box(TileRect::new(0, 11, 20, 7), InkColor::Black, true, |_| {});
    let painter = ui.painter();
    if let Some(slot) = state.current_move() {
        let kind = pokered_data::types::PokemonType::from_id(data.move_type(slot.move_id));
        painter.draw_text_px(8, 72, &format!("属性/{}", pokered_data::lang_data::type_name(kind, true)), InkColor::Black.into());
    }
    for (i, slot) in state.moves().iter().enumerate() {
        let y = 96 + i as u32 * 10;
        painter.draw_text_px(16, y, data.move_name(slot.move_id), InkColor::Black.into());
        painter.draw_text_px(104, y, &format!("PP {:>2}/{:>2}", slot.current_pp.min(99), slot.max_pp.min(99)), InkColor::Black.into());
        if i == state.cursor() {
            painter.draw_text_px(8, y, "▶", InkColor::Black.into());
        }
    }
}
