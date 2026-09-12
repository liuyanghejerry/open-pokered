//! Renderer for the elevator floor-selection menu screen.
//!
//! A readable text/tile presentation: the "WHICH FLOOR?" prompt, a vertical
//! list of floor labels with the current selection marked, and a hint line.
//! The menu logic lives entirely in `pokered_core::elevator_screen`.

use crate::alloc_prelude::*;
use pokered_core::elevator_screen::ElevatorScreen;
use pokered_core::game_state::Lang;
use pokered_data::lang_data;
use pokered_renderer::embedded_font::{draw_text, measure_text};
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
#[cfg(any(test, target_os = "none"))]
pub fn redraw_elevator_cursor(
    previous: (u32, u32),
    current: (u32, u32),
    fb: &mut FrameBuffer,
) {
    for (x, y) in [previous, current] {
        for py in y..(y + 10).min(fb.height()) {
            for px in x..(x + measure_text(">")).min(fb.width()) {
                fb.set_pixel(px, py, BG);
            }
        }
    }
    draw_text(">", current.0, current.1, FG, fb);
}

#[cfg(test)]
mod filter_label_tests {
    use super::*;
    use dotzuki_engine::render_config::RenderConfig;
    use pokered_core::elevator_screen::{ElevatorInput, ElevatorScreen};

    fn menu_at(entries: &[&str], selected: usize) -> ElevatorScreen {
        let mut menu = ElevatorScreen::new(entries.iter().map(|entry| (*entry).into()).collect());
        for _ in 0..selected {
            menu.update_frame(ElevatorInput {
                down: true,
                ..ElevatorInput::none()
            });
        }
        menu
    }

    fn render_menu(menu: &ElevatorScreen, filter: bool, lang: Lang) -> FrameBuffer {
        let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), BG);
        if filter {
            draw_filter_bag(menu, &mut fb, lang);
        } else {
            draw_elevator(menu, &mut fb, lang);
        }
        fb
    }

    fn assert_framebuffers_equal(actual: &FrameBuffer, expected: &FrameBuffer) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());
        for y in 0..actual.height() {
            for x in 0..actual.width() {
                assert_eq!(
                    actual.get_pixel(x, y),
                    expected.get_pixel(x, y),
                    "framebuffer mismatch at ({x}, {y})",
                );
            }
        }
    }

    fn assert_cursor_repaint(
        entries: &[&str],
        previous: usize,
        current: usize,
        filter: bool,
        lang: Lang,
    ) {
        let previous_menu = menu_at(entries, previous);
        let current_menu = menu_at(entries, current);
        let offset = previous_menu.scroll_offset(7);
        assert_eq!(offset, current_menu.scroll_offset(7));
        let x = if filter { 44 } else { 60 };
        let position = |selected| (x, 30 + (selected - offset) as u32 * 14);

        let mut actual = render_menu(&previous_menu, filter, lang);
        redraw_elevator_cursor(position(previous), position(current), &mut actual);
        let expected = render_menu(&current_menu, filter, lang);
        assert_framebuffers_equal(&actual, &expected);
    }

    /// The drink list carries internal item constants; English mode must show
    /// display names, not the raw constants (audit: drink-filter-first.png).
    #[test]
    fn filter_label_maps_item_constants_to_display_names() {
        assert_eq!(filter_label("FRESH_WATER", false), "FRESH WATER");
        assert_eq!(filter_label("SODA_POP", false), "SODA POP");
        assert_eq!(filter_label("FRESH_WATER", true), "新鲜水");
        // Elevator floors are not items: unchanged.
        assert_eq!(filter_label("1F", false), "1F");
        assert_eq!(filter_label("B1F", true), "B1F");
    }

    #[test]
    fn short_menu_cursor_repaint_matches_full_redraw_for_every_transition() {
        let elevator_entries = &["1F", "2F", "3F"][..];
        let filter_entries = &["FRESH_WATER", "SODA_POP", "LEMONADE"][..];
        for language in [Lang::En, Lang::Zh] {
            for (filter, entries) in [(false, elevator_entries), (true, filter_entries)] {
                for previous in 0..entries.len() {
                    for current in 0..entries.len() {
                        if previous != current {
                            assert_cursor_repaint(
                                entries, previous, current, filter, language,
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn long_elevator_cursor_repaint_matches_full_redraw_within_each_viewport() {
        let entries = &[
            "1F", "2F", "3F", "4F", "5F", "6F", "7F", "8F", "9F", "10F", "11F",
        ];
        for language in [Lang::En, Lang::Zh] {
            for previous in 0..entries.len() {
                for current in 0..entries.len() {
                    if previous != current
                        && menu_at(entries, previous).scroll_offset(7)
                            == menu_at(entries, current).scroll_offset(7)
                    {
                        assert_cursor_repaint(
                            entries, previous, current, false, language,
                        );
                    }
                }
            }
        }
    }
}
