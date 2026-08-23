use pokered_core::battle::state::Pokemon;
use pokered_data::ui_layout::schema::BattlePartyDefaultLayout;

use crate::engine::{InkColor, Painter, Ui};

const MAX_VISIBLE: usize = 4;

pub fn draw<P: Painter>(party: &[Pokemon], cursor: usize, layout: &BattlePartyDefaultLayout, ui: &mut Ui<P>, is_zh: bool) {
    let party_len = party.len();
    if party_len == 0 {
        return;
    }

    let visible_start = if party_len <= MAX_VISIBLE {
        0
    } else {
        let ideal_start = cursor.saturating_sub(1);
        ideal_start.min(party_len - MAX_VISIBLE)
    };

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
