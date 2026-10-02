// Dialog widget — thin wrapper around dotzuki-ui generic dialog.
//
// Keeps the pokered-specific public API (taking DialogDefaultLayout)
// while delegating the actual rendering to dotzuki_ui::widgets::dialog.
//
// The wrap width is derived from the box interior (in pixels, for the
// proportional Fusion Pixel font) rather than a per-language character
// count: with Latin at 5px and CJK at 10px advance, an 18-tile interior
// fits ~28 Latin / 14 CJK characters, and dotzuki-ui's wrap_lines re-flows
// short authored lines so they fill the box.

use dotzuki_engine::menu::{CursorStyle, MenuConfig};
use dotzuki_engine::render::TileRect;
use pokered_core::game_state::Lang;
use pokered_data::ui_layout::schema::DialogDefaultLayout;

use crate::engine::{Painter, Ui};
use dotzuki_ui::widgets::dialog;

/// Draw a prepared overworld page without reflowing Chinese typewriter text.
/// English retains the generic dialog's existing wrapping behavior.
pub fn draw_paginated<P: Painter>(
    text: &str,
    show_arrow: bool,
    layout: &DialogDefaultLayout,
    ui: &mut Ui<P>,
    lang: Lang,
) {
    if lang == Lang::Zh {
        // Chinese overworld pages are already laid out before revealing text.
        // Reflowing a partially typed page moves words and can discard row 3.
        ui.text_box(layout.box_0.rect, layout.box_0.color, true, |frame| {
            for (row, line) in text.split('\n').enumerate() {
                frame.label(0, row as u32 * 2, line, crate::InkColor::Black);
            }
            if show_arrow && !text.is_empty() {
                let content = layout.box_0.rect;
                frame.cursor_glyph_at(
                    content.tw.saturating_sub(3),
                    content.th.saturating_sub(3),
                    '▼',
                    crate::InkColor::Black,
                );
            }
        });
        return;
    }
    draw(text, show_arrow, layout, ui, lang);
}

/// Draw ordinary dialog text with the generic engine's wrapping behavior.
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
    let content = TileRect::new(
        area.tx + 1,
        area.ty + 1,
        area.tw.saturating_sub(2),
        area.th.saturating_sub(2),
    );
    let cursor = if show_arrow {
        CursorStyle::new(Some(223), Default::default())
    } else {
        CursorStyle::new(None, Default::default())
    };
    let config = MenuConfig::new(area, None, content, cursor);

    dialog::draw_dialog(text, &[config], ui.painter());
}
