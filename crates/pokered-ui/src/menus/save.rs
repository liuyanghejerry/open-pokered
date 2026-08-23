use pokered_core::game_state::Lang;
use pokered_core::save_menu::{SaveMenuState, SavePhase, YesNoChoice};
use pokered_data::lang_data;
use pokered_data::ui_layout::schema::{SaveAskPromptLayout, SaveDefaultLayout};

use crate::engine::{InkColor, Painter, Ui};

pub fn draw<P: Painter>(state: &SaveMenuState, layout: &SaveDefaultLayout, ask_layout: &SaveAskPromptLayout, ui: &mut Ui<P>, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    let info = &layout.box_0;
    ui.text_box(info.rect, info.color, true, |frame| {
        let labels = info.labels.as_ref();
        // Each static label is followed by its dynamic value to preserve
        // operation order (verified by save test suite).
        frame.label(labels[0].tx, labels[0].ty, lang_data::ui_label(&labels[0].text, is_zh), labels[0].color);
        frame.label(4, 1, &state.info.player_name, InkColor::Black);

        frame.label(labels[1].tx, labels[1].ty, lang_data::ui_label(&labels[1].text, is_zh), labels[1].color);
        let badges = format!("{}", state.info.num_badges);
        frame.label(8, 3, &badges, InkColor::Black);

        frame.label(labels[2].tx, labels[2].ty, lang_data::ui_label(&labels[2].text, is_zh), labels[2].color);
        let dex = format!("{}", state.info.pokedex_owned);
        frame.label(7, 5, &dex, InkColor::Black);

        frame.label(labels[3].tx, labels[3].ty, lang_data::ui_label(&labels[3].text, is_zh), labels[3].color);
        let time = format!(
            "{:>3}:{:02}",
            state.info.play_time_hours, state.info.play_time_minutes
        );
        frame.label(3, 7, &time, InkColor::Black);
    });

    // ── Phase-specific drawing ────────────────────────────────────
    match &state.phase {
        SavePhase::AskSave | SavePhase::ConfirmOverwrite => {
            draw_ask_prompt(state.cursor, ask_layout, ui, is_zh);
        }
        SavePhase::Saving { .. } => {
            let saving_box = &layout.box_1;
            ui.text_box(saving_box.rect, saving_box.color, true, |frame| {
                for label in saving_box.labels.iter() {
                    frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
                }
            });
        }
        SavePhase::SaveComplete | SavePhase::WaitAfterSave { .. } => {
            let done_box = &layout.box_2;
            ui.text_box(done_box.rect, done_box.color, true, |frame| {
                // Dynamic "X saved" text (computed from state)
                let msg = if is_zh {
                    format!("{}已保存", state.info.player_name)
                } else {
                    format!("{} saved", state.info.player_name)
                };
                frame.label(0, 0, &msg, InkColor::Black);
                for label in done_box.labels.iter() {
                    frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
                }
            });
        }
    }
}

fn draw_ask_prompt<P: Painter>(cursor: YesNoChoice, layout: &SaveAskPromptLayout, ui: &mut Ui<P>, is_zh: bool) {

    // Prompt box with labels
    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for label in layout.box_0.labels.iter() {
            frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
        }
    });

    // YES/NO box: border + labels + cursor
    ui.text_box(layout.box_1.rect, layout.box_1.color, true, |frame| {
        for label in layout.box_1.labels.iter() {
            frame.label(label.tx, label.ty, lang_data::ui_label(&label.text, is_zh), label.color);
        }

        // Look up cursor offset from enum_position_map
        let cursor_key = match cursor {
            YesNoChoice::Yes => "Yes",
            YesNoChoice::No => "No",
        };
        let offset = layout
            .enum_position_map
            .iter()
            .find_map(|(key, val)| if key == cursor_key { Some(*val as u32) } else { None })
            .unwrap_or(0);

        // Cursor is at screen-absolute position (col 15 = box_1 left border edge).
        // Since frame origin is (16,8) for this bordered box, a frame-relative tx
        // would be -1 → inexpressible as u32. Use abs_glyph instead.
        let abs_ty = layout.cursor.base_ty + offset * layout.cursor.row_step;
        frame.abs_glyph(layout.cursor.tx, abs_ty, layout.cursor.glyph, layout.cursor.color);
    });
}
