use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::BattleTextDefaultLayout;

use crate::engine::{InkColor, Painter, Ui};
use jrpg_ui::widgets::dialog::wrap_lines;

/// Draw the battle message box.
///
/// Text is wrapped to fill the box interior in pixels (Fusion Pixel font:
/// Latin 5px, CJK 10px advance). `lang` is retained for API compatibility —
/// the wrap width comes from the box geometry, and the language only affects
/// glyph baseline placement inside the painter.
pub fn draw<P: Painter>(text: &str, show_arrow: bool, layout: &BattleTextDefaultLayout, ui: &mut Ui<P>, _lang: Lang) {
    let max_width_px = (layout.box_0.rect.tw.saturating_sub(2) as usize) * 8;
    // Battle pages are capped by the caller (2 lines per page); wrapping here
    // must not silently drop content, so no line cap is applied.
    let wrapped = wrap_lines(text, max_width_px, usize::MAX);
    let start_ty = layout.box_0.text_start_ty.unwrap_or(1);
    let start_tx = layout.box_0.text_start_tx.unwrap_or(0);
    let line_h = layout.box_0.line_height.unwrap_or(2);

    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for (i, line) in wrapped.iter().enumerate() {
            frame.label(start_tx, start_ty + (i as u32) * line_h, line, InkColor::Black);
        }

        if show_arrow {
            let cursor = &layout.cursor;
            frame.cursor_glyph_at(cursor.tx, cursor.base_ty, cursor.glyph, cursor.color);
        }
    });
}
