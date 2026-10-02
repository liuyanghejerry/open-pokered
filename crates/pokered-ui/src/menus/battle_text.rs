use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::BattleTextDefaultLayout;

use crate::engine::{InkColor, Painter, Ui};
use pokered_data::text_layout::wrap_hard_lines;

/// Draw the battle message box.
///
/// Text preserves authored rows and uses actual glyph widths (original Latin
/// 8px, Fusion Pixel Chinese 10px). `lang` is retained for API compatibility —
/// the wrap width comes from the box geometry, and the language only affects
/// glyph baseline placement inside the painter.
pub fn draw<P: Painter>(
    text: &str,
    show_arrow: bool,
    layout: &BattleTextDefaultLayout,
    ui: &mut Ui<P>,
    _lang: Lang,
) {
    let max_width_px = (layout.box_0.rect.tw.saturating_sub(2) as usize) * 8;
    // Battle pages are capped by the caller (2 lines per page); wrapping here
    // must not silently drop content, so no line cap is applied.
    let wrapped = wrap_hard_lines(text, max_width_px);
    let start_ty = layout.box_0.text_start_ty.unwrap_or(1);
    let start_tx = layout.box_0.text_start_tx.unwrap_or(0);
    let line_h = layout.box_0.line_height.unwrap_or(2);

    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for (i, line) in wrapped.iter().enumerate() {
            frame.label(
                start_tx,
                start_ty + (i as u32) * line_h,
                line,
                InkColor::Black,
            );
        }

        if show_arrow {
            let cursor = &layout.cursor;
            frame.cursor_glyph_at(cursor.tx, cursor.base_ty, cursor.glyph, cursor.color);
        }
    });
}

/// Draw text while treating each authored newline as a hard row break.
///
/// Gen-1 battle-intro text explicitly moves `appeared!` to the second row.
/// Kept as a distinct entry point for callers that encode that control stream.
pub fn draw_hard_lines<P: Painter>(
    text: &str,
    show_arrow: bool,
    layout: &BattleTextDefaultLayout,
    ui: &mut Ui<P>,
    _lang: Lang,
) {
    let max_width_px = (layout.box_0.rect.tw.saturating_sub(2) as usize) * 8;
    let wrapped = wrap_hard_lines(text, max_width_px);
    let start_ty = layout.box_0.text_start_ty.unwrap_or(1);
    let start_tx = layout.box_0.text_start_tx.unwrap_or(0);
    let line_h = layout.box_0.line_height.unwrap_or(2);

    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for (i, line) in wrapped.iter().enumerate() {
            frame.label(
                start_tx,
                start_ty + (i as u32) * line_h,
                line,
                InkColor::Black,
            );
        }

        if show_arrow {
            let cursor = &layout.cursor;
            frame.cursor_glyph_at(cursor.tx, cursor.base_ty, cursor.glyph, cursor.color);
        }
    });
}
