use crate::alloc_prelude::*;
use dotzuki_engine::render_data::RenderData;
use pokered_core::battle::menu::MoveMenuState;
use pokered_data::moves::MoveId;
use pokered_data::ui_layout::schema::BattleMoveDefaultLayout;

use crate::engine::{InkColor, Painter, Rgba, TilePos, TileRect, Ui};

pub fn draw<P: Painter>(
    state: &MoveMenuState,
    layout: &BattleMoveDefaultLayout,
    ui: &mut Ui<P>,
    lang: pokered_core::game_state::Lang,
    render_data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
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

    let proportional = ui.painter().supports_proportional();
    // Frame coords below are RELATIVE to the box interior origin (rect.tx + 1, rect.ty + 1)
    // because text_box adds +1 padding for the border. Move list box rect (4, 12, 16, 6)
    // → interior origin (5, 13); native draws move names at screen (6, 13+i) → frame (1, i).
    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for (i, slot) in moves.iter().enumerate() {
            let name = render_data.move_name(slot.move_id);
            let truncated: String = name.chars().take(12).collect();
            if !proportional {
                frame.label(1, i as u32, &truncated, InkColor::Black);
            }
        }
        if !proportional {
            if let Some(list_cursor) = &layout.list_default.cursor {
                let cursor_row = list_cursor.base_ty + cursor as u32 * list_cursor.row_step;
                frame.cursor_glyph_at(
                    list_cursor.tx,
                    cursor_row,
                    list_cursor.glyph,
                    list_cursor.color,
                );
            }
        }
    });
    if proportional {
        let x = (layout.box_0.rect.tx + 2) * 8;
        let y = (layout.box_0.rect.ty + 1) * 8 - 2;
        for (i, slot) in moves.iter().enumerate() {
            let name: String = render_data
                .move_name(slot.move_id)
                .chars()
                .take(12)
                .collect();
            ui.painter()
                .draw_text_px(x, y + i as u32 * 8, &name, Rgba::INK_BLACK);
        }
        draw_en_cursor(cursor, layout, ui.painter());
    }

    draw_en_info(state, layout, ui, render_data);

    draw_connectors(layout, ui);
}

/// Redraw the portions of an already-rendered move menu that change when its
/// selection moves: the old/new cursor cells and the selected move's info box.
pub fn redraw_selection<P: Painter>(
    previous_cursor: usize,
    state: &MoveMenuState,
    layout: &BattleMoveDefaultLayout,
    ui: &mut Ui<P>,
    lang: pokered_core::game_state::Lang,
    render_data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
) {
    if lang == pokered_core::game_state::Lang::Zh {
        let old_y = 94 + previous_cursor as u32 * 10;
        ui.painter()
            .draw_pixel_rect(8, old_y, 8, 9, Rgba::INK_WHITE);
        draw_zh_cursor(state.cursor(), ui.painter());
        redraw_zh_selected_info(state, ui.painter(), render_data);
        return;
    }

    if let Some(cursor) = &layout.list_default.cursor {
        let old = TilePos::new(
            layout.box_0.rect.tx + 1 + cursor.tx,
            layout.box_0.rect.ty + 1 + cursor.base_ty + previous_cursor as u32 * cursor.row_step,
        );
        let inset = if ui.painter().supports_proportional() {
            2
        } else {
            0
        };
        ui.painter()
            .draw_pixel_rect(old.tx * 8, old.ty * 8 - inset, 8, 10, Rgba::INK_WHITE);
    }
    draw_en_cursor(state.cursor(), layout, ui.painter());
    redraw_en_selected_info(state, layout, ui.painter(), render_data);
}

/// Regions changed by [`redraw_selection`].
pub fn selection_damage(
    previous: usize,
    current: usize,
    lang: pokered_core::game_state::Lang,
) -> [crate::DamageRect; 3] {
    let (cursor_x, cursor_y, cursor_step, info_y, info_height) =
        if lang == pokered_core::game_state::Lang::Zh {
            (8, 94, 10, 72, 16)
        } else {
            (5 * 8, 13 * 8 - 2, 8, 72, 26)
        };
    let cursor = |selected: usize| {
        crate::DamageRect::new(cursor_x, cursor_y + selected as u32 * cursor_step, 8, 9)
    };
    [
        crate::DamageRect::new(8, info_y, 72, info_height),
        cursor(previous),
        cursor(current),
    ]
}

fn redraw_en_selected_info<P: Painter>(
    state: &MoveMenuState,
    layout: &BattleMoveDefaultLayout,
    painter: &mut P,
    render_data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
) {
    if painter.supports_proportional() {
        let x = (layout.box_1.rect.tx + 1) * 8;
        let y = (layout.box_1.rect.ty + 1) * 8;
        painter.draw_pixel_rect(x, y, 72, 24, Rgba::INK_WHITE);
        if let Some(slot) = state.current_move() {
            let kind = move_type_display_name(render_data.move_type(slot.move_id));
            painter.draw_text_px(x, y, &format!("TYPE/{kind}"), Rgba::INK_BLACK);
            painter.draw_text_px(
                x,
                y + 12,
                &format!(
                    "PP {:>2}/{:>2}",
                    slot.current_pp.min(99),
                    slot.max_pp.min(99)
                ),
                Rgba::INK_BLACK,
            );
        }
        return;
    }
    // Clear only the dynamic TYPE and PP-value ink. Latin glyphs can extend
    // outside their nominal eight-pixel row, so restore the fixed labels that
    // overlap the cleared pixels before drawing the new values.
    painter.draw_pixel_rect(8, 80, 72, 10, Rgba::INK_WHITE);
    painter.draw_pixel_rect(40, 88, 40, 10, Rgba::INK_WHITE);
    for label in layout.box_1.labels.iter() {
        painter.draw_text(
            TilePos::new(
                layout.box_1.rect.tx + 1 + label.tx,
                layout.box_1.rect.ty + 1 + label.ty,
            ),
            &label.text,
            label.color.into(),
        );
    }
    let Some(slot) = state.current_move() else {
        return;
    };
    let type_name = move_type_display_name(render_data.move_type(slot.move_id));
    painter.draw_text(TilePos::new(1, 10), &type_name, Rgba::INK_BLACK);
    painter.draw_text(
        TilePos::new(5, 11),
        &format!("{:>2}/{:>2}", slot.current_pp.min(99), slot.max_pp.min(99)),
        Rgba::INK_BLACK,
    );
}

fn redraw_zh_selected_info<P: Painter>(
    state: &MoveMenuState,
    painter: &mut P,
    data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
) {
    painter.draw_pixel_rect(8, 72, 72, 16, Rgba::INK_WHITE);
    if let Some(slot) = state.current_move() {
        let kind = pokered_data::types::PokemonType::from_id(data.move_type(slot.move_id));
        painter.draw_text_px(
            8,
            72,
            &format!("属性/{}", pokered_data::lang_data::type_name(kind, true)),
            Rgba::INK_BLACK,
        );
    }
}

fn draw_connectors<P: Painter>(layout: &BattleMoveDefaultLayout, ui: &mut Ui<P>) {
    // Connector tiles bridge the move-list box top border with the PP-info
    // box. Repaint them after the info box because their pixels overlap its
    // bottom edge.
    ui.text_box(
        TileRect::new(0, 0, 20, 18),
        InkColor::Black,
        false,
        |frame| {
            let left_tx = layout.box_0.rect.tx;
            let right_tx = layout.box_1.rect.tx + layout.box_1.rect.tw - 1;
            let connector_ty = layout.base.rect.ty;
            frame.gb_tile(left_tx, connector_ty, 0x7A, "", InkColor::Black);
            frame.gb_tile(right_tx, connector_ty, 0x7E, "", InkColor::Black);
        },
    );
}

fn draw_en_cursor<P: Painter>(selected: usize, layout: &BattleMoveDefaultLayout, painter: &mut P) {
    let Some(cursor) = &layout.list_default.cursor else {
        return;
    };
    let position = TilePos::new(
        layout.box_0.rect.tx + 1 + cursor.tx,
        layout.box_0.rect.ty + 1 + cursor.base_ty + selected as u32 * cursor.row_step,
    );
    if painter.supports_proportional() {
        let mut buf = [0u8; 4];
        painter.draw_text_px(
            position.tx * 8,
            position.ty * 8 - 2,
            cursor.glyph.encode_utf8(&mut buf),
            cursor.color.into(),
        );
    } else {
        painter.draw_glyph(position, cursor.glyph, cursor.color.into());
    }
}

fn draw_en_info<P: Painter>(
    state: &MoveMenuState,
    layout: &BattleMoveDefaultLayout,
    ui: &mut Ui<P>,
    render_data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
) {
    let Some(slot) = state.current_move() else {
        return;
    };
    if ui.painter().supports_proportional() {
        ui.text_box(layout.box_1.rect, layout.box_1.color, true, |_| {});
        redraw_en_selected_info(state, layout, ui.painter(), render_data);
        return;
    }
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

        let pp_text = format!("{:>2}/{:>2}", slot.current_pp.min(99), slot.max_pp.min(99));
        frame.label(4, 2, &pp_text, InkColor::Black);
    });
}

fn move_type_display_name(type_id: u8) -> String {
    // Type IDs use the internal enum ordering (non-sequential).
    // 0x00-0x08: Normal through Ghost
    // 0x14-0x1A: Fire through Dragon (index 9-15 in display order)
    let names: [&str; 15] = [
        "NORMAL", "FIGHTING", "FLYING", "POISON", "GROUND", "ROCK", "BIRD", "BUG", "GHOST", "FIRE",
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
    data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
) {
    draw_zh_info(state, ui, data);
    ui.text_box(TileRect::new(0, 11, 20, 7), InkColor::Black, true, |_| {});
    let painter = ui.painter();
    for (i, slot) in state.moves().iter().enumerate() {
        let y = 94 + i as u32 * 10;
        painter.draw_text_px(16, y, data.move_name(slot.move_id), InkColor::Black.into());
        painter.draw_text_px(
            104,
            y,
            &format!(
                "PP {:>2}/{:>2}",
                slot.current_pp.min(99),
                slot.max_pp.min(99)
            ),
            InkColor::Black.into(),
        );
        if i == state.cursor() {
            draw_zh_cursor(i, painter);
        }
    }
}

fn draw_zh_cursor<P: Painter>(selected: usize, painter: &mut P) {
    painter.draw_text_px(8, 94 + selected as u32 * 10, "▶", InkColor::Black.into());
}

fn draw_zh_info<P: Painter>(
    state: &MoveMenuState,
    ui: &mut Ui<P>,
    data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
) {
    ui.text_box(TileRect::new(0, 8, 11, 4), InkColor::Black, true, |_| {});
    if let Some(slot) = state.current_move() {
        let kind = pokered_data::types::PokemonType::from_id(data.move_type(slot.move_id));
        ui.painter().draw_text_px(
            8,
            72,
            &format!("属性/{}", pokered_data::lang_data::type_name(kind, true)),
            InkColor::Black.into(),
        );
    }
}

/// Gen-1 Mimic's separate MoveSelectionMenu ($01): enemy techniques at
/// hlcoord(0,7), without the normal TYPE/PP box (core.asm .mimicmenu).
pub fn draw_mimic<P: Painter>(
    state: &MoveMenuState,
    ui: &mut Ui<P>,
    lang: pokered_core::game_state::Lang,
    render_data: &dyn RenderData<
        Move = MoveId,
        Item = pokered_data::items::ItemId,
        Species = pokered_data::species::Species,
    >,
) {
    ui.text_box(
        TileRect::new(0, 12, 20, 6),
        InkColor::Black,
        true,
        |frame| {
            frame.label(
                0,
                1,
                if lang == pokered_core::game_state::Lang::Zh {
                    "模仿哪个招式？"
                } else {
                    "Which technique?"
                },
                InkColor::Black,
            );
        },
    );
    let proportional = ui.painter().supports_proportional();
    ui.text_box(
        TileRect::new(
            0,
            if proportional { 6 } else { 7 },
            16,
            if proportional { 8 } else { 6 },
        ),
        InkColor::Black,
        true,
        |frame| {
            if !proportional {
                for (row, slot) in state.moves().iter().enumerate() {
                    let name: String = render_data
                        .move_name(slot.move_id)
                        .chars()
                        .take(12)
                        .collect();
                    frame.label(1, row as u32, &name, InkColor::Black);
                }
                frame.cursor_glyph_at(0, state.cursor() as u32, '▶', InkColor::Black);
            }
        },
    );
    if proportional {
        for (row, slot) in state.moves().iter().enumerate() {
            let name: String = render_data
                .move_name(slot.move_id)
                .chars()
                .take(12)
                .collect();
            ui.painter()
                .draw_text_px(16, 56 + row as u32 * 12, &name, Rgba::INK_BLACK);
        }
        ui.painter()
            .draw_text_px(8, 56 + state.cursor() as u32 * 12, "▶", Rgba::INK_BLACK);
    }
}
