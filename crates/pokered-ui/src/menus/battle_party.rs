use crate::alloc_prelude::*;
use pokered_core::battle::state::Pokemon;
use pokered_data::ui_layout::schema::BattlePartyDefaultLayout;

use crate::engine::{InkColor, Painter, Rgba, TilePos, Ui};

const MAX_VISIBLE: usize = 4;

/// Return the first party index shown in the four-entry viewport.
pub fn viewport_start(party_len: usize, cursor: usize) -> usize {
    if party_len <= MAX_VISIBLE {
        0
    } else {
        cursor
            .saturating_sub(1)
            .min(party_len.saturating_sub(MAX_VISIBLE))
    }
}

fn party_label(mon: &Pokemon, is_zh: bool) -> String {
    let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
    let name = mon.display_name(&mut name_buf);
    if mon.hp == 0 {
        if is_zh {
            format!("{} 倒下", name)
        } else {
            format!("{} FNT", name)
        }
    } else {
        format!("{} {}/{}", name, mon.hp, mon.max_hp)
    }
}

/// Return the selected Pokémon's row in the four-entry viewport.
pub fn cursor_visual_row(party_len: usize, cursor: usize) -> Option<usize> {
    (cursor < party_len).then(|| cursor - viewport_start(party_len, cursor))
}

pub fn draw<P: Painter>(party: &[Pokemon], cursor: usize, layout: &BattlePartyDefaultLayout, ui: &mut Ui<P>, is_zh: bool) {
    let party_len = party.len();
    if party_len == 0 {
        return;
    }

    let visible_start = viewport_start(party_len, cursor);

    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        let cursor_def = &layout.cursor;
        for i in 0..MAX_VISIBLE {
            let party_idx = visible_start + i;
            if party_idx >= party_len {
                break;
            }
            let mon = &party[party_idx];
            let row = i as u32;

            let label = party_label(mon, is_zh);
            frame.label(1, row, &label, InkColor::Black);

            if party_idx == cursor {
                let cursor_row = cursor_def.base_ty + i as u32 * cursor_def.row_step;
                frame.cursor_glyph_at(cursor_def.tx, cursor_row, cursor_def.glyph, cursor_def.color);
            }
        }
    });
}

/// Repaint only the changed cursor cells when the party viewport did not
/// scroll. `previous_row` and `current_row` are viewport-relative rows.
pub fn redraw_cursor<P: Painter>(
    previous_row: usize,
    current_row: usize,
    layout: &BattlePartyDefaultLayout,
    painter: &mut P,
) {
    let cursor = &layout.cursor;
    let position = |row: usize| {
        TilePos::new(
            layout.box_0.rect.tx + 1 + cursor.tx,
            layout.box_0.rect.ty + 1 + cursor.base_ty + row as u32 * cursor.row_step,
        )
    };
    let old = position(previous_row);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    painter.draw_glyph(
        position(current_row),
        cursor.glyph,
        cursor.color.into(),
    );
}

/// Cursor ink region for a viewport-relative party row.
pub fn cursor_damage(row: usize, layout: &BattlePartyDefaultLayout) -> crate::DamageRect {
    let cursor = &layout.cursor;
    crate::DamageRect::cursor(TilePos::new(
        layout.box_0.rect.tx + 1 + cursor.tx,
        layout.box_0.rect.ty + 1 + cursor.base_ty + row as u32 * cursor.row_step,
    ))
}

/// Label/cursor band changed when the four-entry viewport scrolls.
pub fn viewport_damage(layout: &BattlePartyDefaultLayout) -> crate::DamageRect {
    let rect = layout.box_0.rect;
    crate::DamageRect::new(
        (rect.tx + 1) * 8,
        (rect.ty + 1) * 8 - 1,
        rect.tw.saturating_sub(2) * 8,
        rect.th.saturating_sub(2) * 8 + 6,
    )
}

/// Finish a viewport scroll after the caller has shifted the retained label
/// pixels. This clears and redraws only the newly exposed rows plus the cursor.
pub fn redraw_viewport_edges<P: Painter>(
    party: &[Pokemon],
    cursor: usize,
    previous_start: usize,
    layout: &BattlePartyDefaultLayout,
    painter: &mut P,
    is_zh: bool,
) {
    if party.is_empty() {
        return;
    }

    let rect = layout.box_0.rect;
    let band_y = ((rect.ty + 1) * 8).saturating_sub(1);
    let band_height = rect.th.saturating_sub(2) * 8 + 6;
    let current_start = viewport_start(party.len(), cursor);
    let shifted_rows = current_start.abs_diff(previous_start).min(MAX_VISIBLE);
    if shifted_rows == 0 {
        return;
    }
    let shifted_pixels = shifted_rows as u32 * 8;

    // The label band starts one pixel above the nominal tile row because the
    // Chinese backend applies that offset. Its bottom includes the font's
    // descender spill but stops before the horizontal border stroke.
    let moving_forward = current_start > previous_start;
    let (clear_y, rows, overlap_y, overlap_row) = if moving_forward {
        (
            band_y + band_height - shifted_pixels,
            MAX_VISIBLE - shifted_rows..MAX_VISIBLE,
            band_y,
            0,
        )
    } else {
        (
            band_y,
            0..shifted_rows,
            band_y + band_height - 6,
            MAX_VISIBLE - 1,
        )
    };
    painter.draw_pixel_rect(
        (rect.tx + 2) * 8,
        clear_y,
        rect.tw.saturating_sub(3) * 8,
        shifted_pixels,
        Rgba::INK_WHITE,
    );
    // Adjacent proportional-font rows overlap their 8 px cells. The shifted
    // edge can therefore contain a few pixels from the row just outside the
    // copied range; clear that spill and redraw the retained boundary row.
    painter.draw_pixel_rect(
        (rect.tx + 2) * 8,
        overlap_y,
        rect.tw.saturating_sub(3) * 8,
        6,
        Rgba::INK_WHITE,
    );
    painter.draw_pixel_rect(
        (rect.tx + 1) * 8,
        band_y,
        8,
        band_height,
        Rgba::INK_WHITE,
    );

    for row in rows.chain(core::iter::once(overlap_row)) {
        let Some(mon) = party.get(current_start + row) else {
            continue;
        };
        painter.draw_text(
            TilePos::new(rect.tx + 2, rect.ty + 1 + row as u32),
            &party_label(mon, is_zh),
            InkColor::Black.into(),
        );
    }

    if let Some(row) = cursor_visual_row(party.len(), cursor) {
        let cursor_def = &layout.cursor;
        painter.draw_glyph(
            TilePos::new(
                rect.tx + 1 + cursor_def.tx,
                rect.ty + 1 + cursor_def.base_ty + row as u32 * cursor_def.row_step,
            ),
            cursor_def.glyph,
            cursor_def.color.into(),
        );
    }
}
