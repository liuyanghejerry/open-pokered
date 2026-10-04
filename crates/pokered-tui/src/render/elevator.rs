//! Renderer for the elevator floor-selection menu screen.
//!
//! A readable text/tile presentation: the "WHICH FLOOR?" prompt, a vertical
//! list of floor labels with the current selection marked, and a hint line.
//! The menu logic lives entirely in `pokered_core::elevator_screen`.

use pokered_core::elevator_screen::ElevatorScreen;
use pokered_core::game_state::Lang;
use pokered_data::lang_data;
use pokered_renderer::embedded_font::{draw_text, measure_text};
use pokered_renderer::{FrameBuffer, Rgba};

use pokered_data::battle_text::zh_name;

const BG: Rgba = Rgba::WHITE;
const FG: Rgba = Rgba::BLACK;

/// Draw the elevator floor menu to the 160x144 framebuffer.
pub fn draw_elevator(elevator: &ElevatorScreen, fb: &mut FrameBuffer, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    // DisplayElevatorFloorMenu leaves the map behind the usual list menu.
    super::draw_text_box(fb, 0, 12 * 8, 18, 4, FG);

    draw_text(lang_data::ui_label("WHICH FLOOR?", is_zh), 8, 14 * 8, FG, fb,
    );
    super::draw_text_box(fb, 4 * 8, 2 * 8, 14, 9, FG);

    let floors = elevator.menu_entries();
    let selected = elevator.selected_index();
    let offset = elevator.scroll_offset(3);
    // A fourth unselectable entry is visible below the three cursor rows.
    for (row, floor) in floors.skip(offset).take(4).enumerate() {
        draw_text(floor, 6 * 8, (4 + row as u32 * 2)* 8, FG, fb);
    }

    draw_text("▶", 5 * 8, (4 + (selected - offset)as u32 * 2)* 8, FG, fb);
}

/// Label for one filter-bag row. The drink flow passes internal item
/// constants (FRESH_WATER) — render display names in both languages (the
/// audit saw the raw constant in English mode). Floor labels ("1F") fall
/// through unchanged.
pub(crate) fn filter_label(item: &str, is_zh: bool) -> String {
    match pokered_data::items::ItemId::from_const_name(item) {
        Some(id) => lang_data::item_name(id, is_zh).to_string(),
        None if is_zh => zh_name(item),
        None => item.to_string(),
    }
}

/// Draw the filtered-bag menu ("WHICH ONE?" + carried item list).
pub fn draw_filter_bag(filter: &ElevatorScreen, fb: &mut FrameBuffer, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    fb.clear(BG);

    draw_text(lang_data::ui_label("WHICH ONE?", is_zh), 48, 10, FG, fb);

    let items = filter.floors();
    let sel = filter.selected_index();
    let start_y = 30;
    let row_h = 14;
    // Same scroll window as the elevator menu (see draw_elevator).
    let max_visible = 7;
    let offset = filter.scroll_offset(max_visible);
    for (row, (i, item)) in items.iter().enumerate().skip(offset).take(max_visible).enumerate() {
        let y = start_y + row as u32 * row_h;
        let marker = if i == sel { ">" } else { " " };
        let label = filter_label(item, is_zh);
        draw_text(&format!("{} {}", marker, label), 44, y, FG, fb);
    }

    draw_text(lang_data::ui_label("A SELECT", is_zh), 28, 128, FG, fb);
    draw_text(lang_data::ui_label("B BACK", is_zh), 88, 128, FG, fb);
}

/// Repaint only the `>` marker shared by elevator and filtered-bag menus.
pub fn redraw_elevator_cursor(previous: (u32, u32), current: (u32, u32), fb: &mut FrameBuffer) {
    let glyph = if current.0 == 40 { "▶" } else { ">" };
    for (x, y) in [previous, current] {
        for py in y..(y + 10).min(fb.height() ) {
        for px in x..(x + measure_text(glyph)).min(fb.width()) {
                fb.set_pixel(px, py, BG);

    }
}
}
    draw_text(glyph, current.0, current.1, FG, fb);
}
