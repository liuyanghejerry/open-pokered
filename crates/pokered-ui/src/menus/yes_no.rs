// Yes/No widget — thin wrapper around jrpg-ui generic yes_no.
//
// Keeps the pokered-specific public API (taking YesNoDefaultLayout)
// while delegating the actual rendering to jrpg_ui::widgets::yes_no.

use jrpg_engine::menu::{CursorStyle, MenuConfig};
use jrpg_engine::render::TileRect;
use pokered_data::ui_layout::schema::YesNoDefaultLayout;

use crate::engine::{Painter, Ui};
use jrpg_ui::widgets::yes_no;

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
