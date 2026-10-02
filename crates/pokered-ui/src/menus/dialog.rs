//! Original text rows and eight-pixel English cells; Chinese keeps 10px glyphs.

use crate::engine::{InkColor, Painter, Ui};
use dotzuki_engine::render::TileRect;
use pokered_core::game_state::Lang;
use pokered_data::text_layout::wrap_hard_lines;
use pokered_data::ui_layout::schema::DialogDefaultLayout;

/// Draw one already-paginated dialogue page, preserving its hard row breaks.
pub fn draw<P: Painter>(
    text: &str,
    show_arrow: bool,
    layout: &DialogDefaultLayout,
    ui: &mut Ui<P>,
    _lang: Lang,
) {
    let area = TileRect::new(
        layout.box_0.rect.tx,
        layout.box_0.rect.ty,
        layout.box_0.rect.tw,
        layout.box_0.rect.th,
    );
    let interior_width = area.tw.saturating_sub(2);
    let interior_height = area.th.saturating_sub(2);
    let max_lines = (interior_height / 2).max(1) as usize;
    let lines = wrap_hard_lines(text, interior_width as usize * 8);
    ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
        for (i, line) in lines.iter().take(max_lines).enumerate() {
            frame.label(0, 1 + i as u32 * 2, line, InkColor::Black);
        }
        if show_arrow && !text.is_empty() {
            frame.cursor_glyph_at(
                interior_width.saturating_sub(1),
                interior_height.saturating_sub(1),
                '▼',
                InkColor::Black,
            );
        }
    });
}
