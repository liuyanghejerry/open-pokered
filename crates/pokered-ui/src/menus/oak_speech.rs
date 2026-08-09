use pokered_data::ui_layout::schema::{
    OakSpeechNameChoiceLayout, OakSpeechTextPhaseLayout,
};

use crate::engine::{InkColor, Painter, Ui};

/// Draws the bottom dialog box used by the six text phases (Greeting,
/// ShowNidorino, Explanation, IntroducePlayer, IntroduceRival, FinalSpeech).
///
/// The Pokémon/trainer sprite for the active phase is intentionally NOT
/// painted here — the caller blits it via [`ResourceManager`] before invoking
/// this function. The dialog box at (0, 12)..(18, 16) does not overlap the
/// centered sprite area, so paint order is not safety-critical here (unlike
/// the Pokédex frame which interior-fills its sprite area).
pub fn draw_text_phase<P: Painter>(
    line1: &str,
    line2: &str,
    show_arrow: bool,
    layout: &OakSpeechTextPhaseLayout,
    ui: &mut Ui<P>,
) {
    ui.text_box(layout.dialog_box.rect, layout.dialog_box.color, true, |frame| {
        if !line1.is_empty() {
            frame.label(1, 1, line1, InkColor::Black);
        }
        if !line2.is_empty() {
            frame.label(1, 3, line2, InkColor::Black);
        }
        if show_arrow {
            let cursor = &layout.cursor;
            let rel_tx = cursor
                .tx
                .saturating_sub(layout.dialog_box.rect.tx + 1);
            let rel_ty = cursor
                .base_ty
                .saturating_sub(layout.dialog_box.rect.ty + 1);
            frame.cursor_glyph_at(rel_tx, rel_ty, cursor.glyph, cursor.color);
        }
    });
}

/// Draws the name-selection screen used by [`OakSpeechPhase::PlayerNameChoice`]
/// and [`OakSpeechPhase::RivalNameChoice`]. Renders the top-left 9×10 list box
/// with "NAME" header + four name choices + cursor on the selected row, plus a
/// bottom prompt box ("Your name?" or "His name?").
///
/// The portrait sprite (red or rival1) at tile (10, 4) is painted by the
/// caller; both boxes lie outside the sprite area so paint order is benign.
pub fn draw_name_choice<P: Painter>(
    names: &[&str; 4],
    cursor_index: usize,
    prompt: &str,
    layout: &OakSpeechNameChoiceLayout,
    ui: &mut Ui<P>,
) {
    let cursor = &layout.cursor;

    ui.text_box(layout.name_list.rect, layout.name_list.color, true, |frame| {
        for label in layout.name_list.labels.iter() {
            // JSON label coords are absolute tiles; convert to box-interior.
            let rel_tx = label.tx.saturating_sub(layout.name_list.rect.tx + 1);
            let rel_ty = label.ty.saturating_sub(layout.name_list.rect.ty + 1);
            frame.label(rel_tx, rel_ty, &label.text, label.color);
        }
        for (i, name) in names.iter().enumerate() {
            // Names are at JSON-absolute (2, 2+i*2); interior origin is (1, 1)
            // so interior coords are (1, 1+i*2).
            let row_ty = 1 + (i as u32) * 2;
            frame.label(1, row_ty, name, InkColor::Black);
            if i == cursor_index {
                // Cursor JSON tx=1 is absolute → interior tx=0.
                let rel_tx = cursor
                    .tx
                    .saturating_sub(layout.name_list.rect.tx + 1);
                frame.cursor_glyph_at(rel_tx, row_ty, cursor.glyph, cursor.color);
            }
        }
    });

    ui.text_box(layout.prompt_box.rect, layout.prompt_box.color, true, |frame| {
        frame.label(1, 1, prompt, InkColor::Black);
    });
}
