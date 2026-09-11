use crate::alloc_prelude::*;
use pokered_core::battle::state::Pokemon;
use pokered_data::ui_layout::schema::BattlePartyDefaultLayout;

use crate::engine::{InkColor, Painter, Rgba, TilePos, Ui};

const MAX_VISIBLE: usize = 4;

fn visible_start(party_len: usize, cursor: usize) -> usize {
    if party_len <= MAX_VISIBLE {
        0
    } else {
        cursor
            .saturating_sub(1)
            .min(party_len.saturating_sub(MAX_VISIBLE))
    }
}

/// Return the selected Pokémon's row in the four-entry viewport.
pub fn cursor_visual_row(party_len: usize, cursor: usize) -> Option<usize> {
    (cursor < party_len).then(|| cursor - visible_start(party_len, cursor))
}

pub fn draw<P: Painter>(party: &[Pokemon], cursor: usize, layout: &BattlePartyDefaultLayout, ui: &mut Ui<P>, is_zh: bool) {
    let party_len = party.len();
    if party_len == 0 {
        return;
    }

    let visible_start = visible_start(party_len, cursor);

    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        let cursor_def = &layout.cursor;
        for i in 0..MAX_VISIBLE {
            let party_idx = visible_start + i;
            if party_idx >= party_len {
                break;
            }
            let mon = &party[party_idx];
            let row = i as u32;

            let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
            let name = mon.display_name(&mut name_buf);
            let label = if mon.hp == 0 {
                if is_zh {
                    format!("{} 倒下", name)
                } else {
                    format!("{} FNT", name)
                }
            } else {
                format!("{} {}/{}", name, mon.hp, mon.max_hp)
            };
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
