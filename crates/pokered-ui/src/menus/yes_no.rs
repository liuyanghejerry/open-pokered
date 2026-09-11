// Yes/No widget — thin wrapper around dotzuki-ui generic yes_no.
//
// Keeps the pokered-specific public API (taking YesNoDefaultLayout)
// while delegating the actual rendering to dotzuki_ui::widgets::yes_no.

use crate::alloc_prelude::*;
use dotzuki_engine::menu::{CursorStyle, MenuConfig};
use dotzuki_engine::render::TileRect;
use pokered_data::ui_layout::schema::YesNoDefaultLayout;

use crate::engine::{Painter, Rgba, TilePos, Ui};
use dotzuki_ui::widgets::yes_no;

/// Draw a yes/no choice box using a pokered layout definition.
pub fn draw<P: Painter>(options: &[String], selected: u32, layout: &YesNoDefaultLayout, ui: &mut Ui<P>) {
    if options.is_empty() {
        return;
    }
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
    let cursor = CursorStyle::new(Some(223), Default::default());
    let config = MenuConfig::new(area, None, content, cursor);

    let opt_vec: Vec<String> = options.to_vec();
    yes_no::draw_yes_no(&opt_vec, selected as usize, &[config], ui.painter());
}

/// Repaint only the changed cursor cells of an already-rendered choice box.
pub fn redraw_cursor<P: Painter>(
    previous_selected: usize,
    current_selected: usize,
    layout: &YesNoDefaultLayout,
    painter: &mut P,
) {
    let position = |selected: usize| {
        TilePos::new(
            layout.box_0.rect.tx + 1,
            layout.box_0.rect.ty + 1 + selected as u32 * 2,
        )
    };
    let old = position(previous_selected);
    painter.draw_pixel_rect(old.tx * 8, old.ty * 8, 8, 9, Rgba::INK_WHITE);
    painter.draw_glyph(
        position(current_selected),
        layout.cursor.glyph,
        layout.cursor.color.into(),
    );
}
