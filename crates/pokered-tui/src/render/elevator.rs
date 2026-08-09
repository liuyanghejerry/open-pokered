//! Renderer for the elevator floor-selection menu screen.
//!
//! A readable text/tile presentation: the "WHICH FLOOR?" prompt, a vertical
//! list of floor labels with the current selection marked, and a hint line.
//! The menu logic lives entirely in `pokered_core::elevator_screen` (shared
//! with the native app; this is a pixel-for-pixel mirror of the app's
//! `render/elevator.rs`).

use pokered_core::elevator_screen::ElevatorScreen;
use pokered_renderer::embedded_font::draw_text;
use pokered_renderer::{FrameBuffer, Rgba};

const BG: Rgba = Rgba::WHITE;
const FG: Rgba = Rgba::BLACK;

/// Draw the elevator floor menu to the 160x144 framebuffer.
pub fn draw_elevator(elevator: &ElevatorScreen, fb: &mut FrameBuffer) {
    fb.clear(BG);

    draw_text("WHICH FLOOR?", 40, 10, FG, fb);

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
        draw_text(&format!("{} {}", marker, floor), 60, y, FG, fb);
    }

    draw_text("A SELECT", 28, 128, FG, fb);
    draw_text("B BACK", 88, 128, FG, fb);
}

/// Draw the filtered-bag menu ("WHICH ONE?" + carried item list).
pub fn draw_filter_bag(filter: &ElevatorScreen, fb: &mut FrameBuffer) {
    fb.clear(BG);

    draw_text("WHICH ONE?", 48, 10, FG, fb);

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
        draw_text(&format!("{} {}", marker, item), 44, y, FG, fb);
    }

    draw_text("A SELECT", 28, 128, FG, fb);
    draw_text("B BACK", 88, 128, FG, fb);
}
