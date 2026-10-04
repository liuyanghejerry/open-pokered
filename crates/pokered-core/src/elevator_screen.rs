//! Elevator floor-selection menu screen.
//!
//! The original list keeps three selectable rows and a fourth preview row.
//! Up/Down move within the list without wrapping; A selects a floor or the final
//! CANCEL entry, and B cancels. The caller resumes the suspended map script.
//! Filtered-bag callers use a separate constructor to retain their list policy.

use crate::alloc_prelude::*;

/// Per-frame input, edge-triggered by the caller.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ElevatorInput {
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
}

impl ElevatorInput {
    pub fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorAction {
    Continue,
    /// Zero-based floor index, excluding the CANCEL entry.
    Select(usize),
    /// B or A on CANCEL; the caller resumes the script with -1.
    Cancel,
}

#[derive(Debug, Clone)]
pub struct ElevatorScreen {
    floors: Vec<String>,
    selected: usize,
    scroll: usize,
    floor_menu: bool,
    frame_counter: u32,
}

impl ElevatorScreen {
    pub fn new(floors: Vec<String>) -> Self {
        Self {
            floors,
            selected: 0,
            scroll: 0,
            floor_menu: true,
            frame_counter: 0,
        }
    }

    /// The drink filter reuses this screen but supplies item entries, not floors.
    pub fn new_filtered(items: Vec<String>) -> Self {
        Self {
            floor_menu: false,
            ..Self::new(items)
        }
    }

    /// Real destinations only; indices passed back to the script stay unchanged.
    pub fn floors(&self) -> &[String] {
        &self.floors
    }

    /// Display entries, including the elevator's final CANCEL sentinel.
    pub fn menu_entries(&self) -> impl Iterator<Item = &str> + '_ {
        self.floors
            .iter()
            .map(String::as_str)
            .chain(core::iter::once("CANCEL").filter(move |_| self.floor_menu))
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// First displayed entry. The elevator keeps its list window stationary
    /// until the cursor crosses row 0 or row 2 (home/list_menu.asm:177-195).
    /// Filtered-bag menus retain their centered window.
    pub fn scroll_offset(&self, max_visible: usize) -> usize {
        let count = self.floors.len() + usize::from(self.floor_menu);
        if max_visible == 0 || count <= max_visible {
            return 0;
        }
        let max_offset = count - max_visible;
        if self.floor_menu {
            self.scroll
                .max(self.selected.saturating_sub(max_visible - 1))
                .min(max_offset)
        } else {
            self.selected
                .saturating_sub(max_visible / 2)
                .min(max_offset)
        }
    }

    pub fn update_frame(&mut self, input: ElevatorInput) -> ElevatorAction {
        self.frame_counter = self.frame_counter.wrapping_add(1);
        if self.floors.is_empty() {
            return ElevatorAction::Cancel;
        }
        if self.floor_menu {
            if input.up && self.selected > 0 {
                self.selected -= 1;
                if self.selected < self.scroll {
                    self.scroll = self.selected;
                }
            } else if input.down && self.selected < self.floors.len() {
                self.selected += 1;
                if self.selected > self.scroll + 2 {
                    self.scroll = self.selected - 2;
                }
            }
        } else {
            if input.up {
                self.selected = (self.selected + self.floors.len() - 1) % self.floors.len();
            }
            if input.down {
                self.selected = (self.selected + 1) % self.floors.len();
            }
        }
        if input.a {
            return if self.floor_menu && self.selected == self.floors.len() {
                ElevatorAction::Cancel
            } else {
                ElevatorAction::Select(self.selected)
            };
        }
        if input.b {
            return ElevatorAction::Cancel;
        }
        ElevatorAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floors(count: usize) -> Vec<String> {
        (1..=count).map(|i| format!("{i}F")).collect()
    }
    fn down(menu: &mut ElevatorScreen) {
        menu.update_frame(ElevatorInput {
            down: true,
            ..ElevatorInput::none()
        });
    }
    fn up(menu: &mut ElevatorScreen) {
        menu.update_frame(ElevatorInput {
            up: true,
            ..ElevatorInput::none()
        });
    }
    fn confirm(menu: &mut ElevatorScreen) -> ElevatorAction {
        menu.update_frame(ElevatorInput {
            a: true,
            ..ElevatorInput::none()
        })
    }

    #[test]
    fn floor_destinations_exclude_cancel_but_display_entries_include_it() {
        let menu = ElevatorScreen::new(floors(3));
        assert_eq!(menu.floors(), &["1F", "2F", "3F"]);
        assert_eq!(
            menu.menu_entries().collect::<Vec<_>>(),
            ["1F", "2F", "3F", "CANCEL"]
        );
        assert_eq!(menu.selected_index(), 0);
    }

    #[test]
    fn every_real_floor_preserves_its_script_index() {
        let mut menu = ElevatorScreen::new(floors(11));
        for floor in 0..11 {
            assert_eq!(confirm(&mut menu), ElevatorAction::Select(floor));
            down(&mut menu);
        }
        assert_eq!(confirm(&mut menu), ElevatorAction::Cancel);
    }

    #[test]
    fn endpoints_do_not_wrap_and_cancel_is_selectable() {
        let mut menu = ElevatorScreen::new(floors(5));
        up(&mut menu);
        assert_eq!(menu.selected_index(), 0);
        for _ in 0..8 {
            down(&mut menu);
        }
        assert_eq!(menu.selected_index(), 5);
        assert_eq!(confirm(&mut menu), ElevatorAction::Cancel);
        assert_eq!(menu.scroll_offset(3), 3);
    }

    #[test]
    fn b_cancels_from_a_floor_or_cancel() {
        let mut menu = ElevatorScreen::new(floors(1));
        for selected in 0..=1 {
            assert_eq!(menu.selected_index(), selected);
            assert_eq!(
                menu.update_frame(ElevatorInput {
                    b: true,
                    ..ElevatorInput::none()
                }),
                ElevatorAction::Cancel
            );
            down(&mut menu);
        }
    }

    #[test]
    fn single_floor_shows_cancel_without_scrolling() {
        let mut menu = ElevatorScreen::new(floors(1));
        down(&mut menu);
        assert_eq!(menu.selected_index(), 1);
        assert_eq!(menu.scroll_offset(3), 0);
        assert_eq!(confirm(&mut menu), ElevatorAction::Cancel);
    }

    #[test]
    fn fourth_entry_is_a_preview_until_the_window_scrolls() {
        let mut menu = ElevatorScreen::new(floors(11));
        down(&mut menu);
        down(&mut menu);
        assert_eq!(menu.selected_index(), 2);
        assert_eq!(menu.scroll_offset(3), 0);
        assert_eq!(
            menu.menu_entries()
                .skip(menu.scroll_offset(3))
                .take(4)
                .collect::<Vec<_>>(),
            ["1F", "2F", "3F", "4F"]
        );
        down(&mut menu);
        assert_eq!(menu.selected_index(), 3);
        assert_eq!(menu.scroll_offset(3), 1);
    }

    #[test]
    fn scroll_window_keeps_its_position_when_moving_back_up_within_it() {
        let mut menu = ElevatorScreen::new(floors(11));
        for _ in 0..5 {
            down(&mut menu);
        }
        assert_eq!((menu.selected_index(), menu.scroll_offset(3)), (5, 3));
        up(&mut menu);
        up(&mut menu);
        assert_eq!((menu.selected_index(), menu.scroll_offset(3)), (3, 3));
        up(&mut menu);
        assert_eq!((menu.selected_index(), menu.scroll_offset(3)), (2, 2));
    }

    #[test]
    fn all_long_list_positions_fit_the_three_cursor_rows_in_both_directions() {
        let mut menu = ElevatorScreen::new(floors(11));
        for expected in 0..=11 {
            assert_eq!(menu.selected_index(), expected);
            assert!(menu.selected_index() - menu.scroll_offset(3) < 3);
            down(&mut menu);
        }
        for expected in (0..=11).rev() {
            assert_eq!(menu.selected_index(), expected);
            assert!(menu.selected_index() - menu.scroll_offset(3) < 3);
            up(&mut menu);
        }
        assert_eq!(menu.scroll_offset(3), 0);
    }

    #[test]
    fn filtered_bag_keeps_its_existing_entries_and_input_policy() {
        let mut menu = ElevatorScreen::new_filtered(floors(11));
        assert_eq!(menu.menu_entries().count(), 11);
        up(&mut menu);
        assert_eq!(menu.selected_index(), 10);
        assert_eq!(menu.scroll_offset(7), 4);
        assert_eq!(confirm(&mut menu), ElevatorAction::Select(10));
        down(&mut menu);
        assert_eq!(menu.selected_index(), 0);
    }

    #[test]
    fn empty_floors_cancel_immediately() {
        let mut menu = ElevatorScreen::new(Vec::new());
        assert_eq!(
            menu.update_frame(ElevatorInput::none()),
            ElevatorAction::Cancel
        );
        assert_eq!(menu.scroll_offset(0), 0);
    }
}
