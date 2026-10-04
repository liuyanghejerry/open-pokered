use crate::alloc_prelude::*;
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::BattleTextDefaultLayout;

use crate::engine::{InkColor, Painter, TilePos, Ui};
use pokered_data::text_layout::wrap_hard_lines;

/// Draw the battle message box.
///
/// Text preserves authored rows and the project's Fusion Pixel glyphs.
/// `lang` is retained for API compatibility —
/// the wrap width comes from the box geometry, and the language only affects
/// glyph baseline placement inside the painter.
pub fn draw<P: Painter>(
    text: &str,
    show_arrow: bool,
    layout: &BattleTextDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    let max_width_px = (layout.box_0.rect.tw.saturating_sub(2) as usize) * 8;
    // Battle pages are capped by the caller (2 lines per page); wrapping here
    // must not silently drop content, so no line cap is applied.
    let wrapped = if lang == Lang::Zh { text.split('\n').map(str::to_owned).collect() } else { wrap_hard_lines(text, max_width_px) };
    let start_ty = if lang == Lang::Zh { 0 } else { layout.box_0.text_start_ty.unwrap_or(1) };
    let start_tx = layout.box_0.text_start_tx.unwrap_or(0);
    let line_h = layout.box_0.line_height.unwrap_or(2);

    let proportional = ui.painter().supports_proportional();
    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for (i, line) in wrapped.iter().enumerate() {
            if !proportional {
                frame.label(start_tx, start_ty + i as u32 * line_h, line, InkColor::Black);
            }
        }

        if !proportional && show_arrow {
            let cursor = &layout.cursor;
            frame.cursor_glyph_at(cursor.tx, cursor.base_ty, cursor.glyph, cursor.color);
        }
    });
    if proportional {
        let rect = layout.box_0.rect;
        let painter = ui.painter();
        // Preserve custom line spacing; the standard two-row text box uses
        // 12px spacing to keep 10px descenders clear of the bottom border.
        let pitch = if line_h == 2 { 12 } else { line_h * 8 };
        for (i, line) in wrapped.iter().enumerate() {
            painter.draw_text_px((rect.tx + 1 + start_tx) * 8,
                (rect.ty + 1 + start_ty) * 8 + i as u32 * pitch, line, InkColor::Black.into());
        }
        if show_arrow {
            let cursor = &layout.cursor;
            painter.draw_glyph(TilePos::new(rect.tx + 1 + cursor.tx,
                rect.ty + 1 + cursor.base_ty), cursor.glyph, cursor.color.into());
        }
    }
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
    draw(text, show_arrow, layout, ui, _lang);
}
