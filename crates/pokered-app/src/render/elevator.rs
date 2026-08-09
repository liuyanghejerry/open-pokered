//! Renderer for the elevator floor-selection menu screen.
//!
//! A readable text/tile presentation: the "WHICH FLOOR?" prompt, a vertical
//! list of floor labels with the current selection marked, and a hint line.
//! The menu logic lives entirely in `pokered_core::elevator_screen`.

use pokered_core::elevator_screen::ElevatorScreen;
use pokered_core::game_state::Lang;
use pokered_data::lang_data;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::{FrameBuffer, Rgba};

use crate::render::battle_i18n::zh_name;

const BG: Rgba = Rgba::WHITE;
const FG: Rgba = Rgba::BLACK;

/// Draw the elevator floor menu to the 160x144 framebuffer.
pub fn draw_elevator(elevator: &ElevatorScreen, fb: &mut FrameBuffer, lang: Lang) {
    let is_zh = lang == Lang::Zh;
    fb.clear(BG);

    draw_text(lang_data::ui_label("WHICH FLOOR?", is_zh), 40, 10, FG, fb);

    let floors = elevator.floors();
    let sel = elevator.selected_index();
    let start_y = 30;
    let row_h = 14;
    // Rows between the prompt (y=10) and the footer (y=128): 7 fit on screen.
    // Long floor lists (e.g. Silph Co's 11) scroll with the selection cursor.
    let max_visible = 7;
    let offset = elevator.scroll_offset(max_visible);
    for (row, (i, floor)) in floors.iter().enumerate().skip(offset).take(max_visible).enumerate() {
        let y = start_y + row as u32 * row_h;
        let marker = if i == sel { ">" } else { " " };
        // Floor labels ("1F"/"B1F" etc.) are option values — kept as-is.
        draw_text(&format!("{} {}", marker, floor), 60, y, FG, fb);
    }

    draw_text(lang_data::ui_label("A SELECT", is_zh), 28, 128, FG, fb);
    draw_text(lang_data::ui_label("B BACK", is_zh), 88, 128, FG, fb);
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
        let label = if is_zh { zh_name(item) } else { item.clone() };
        draw_text(&format!("{} {}", marker, label), 44, y, FG, fb);
    }

    draw_text(lang_data::ui_label("A SELECT", is_zh), 28, 128, FG, fb);
    draw_text(lang_data::ui_label("B BACK", is_zh), 88, 128, FG, fb);
}
