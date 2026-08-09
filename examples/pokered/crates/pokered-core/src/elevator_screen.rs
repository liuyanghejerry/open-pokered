//! Elevator floor-selection menu screen.
//!
//! A pure-logic, I/O-free state machine: a vertical list of floor labels the
//! player scrolls with Up/Down and confirms with A (B cancels). The chosen
//! floor index is delivered back to the caller, which resumes the suspended
//! overworld script via `OverworldScreen::resume_script_after_elevator`.
//!
//! Like the other screen state machines (`slots_screen`, `options_menu`), this
//! crate is deterministic and I/O-free; rendering lives in the app layer.

/// Per-frame input for the elevator screen (edge-triggered by the caller).
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

/// Result of a single frame update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorAction {
    /// Stay on the elevator screen.
    Continue,
    /// The player confirmed floor `selected`; the caller should resume the
    /// script with that index (0-based).
    Select(usize),
    /// The player cancelled (B); resume the script with -1.
    Cancel,
}

/// Elevator floor-menu screen state.
#[derive(Debug, Clone)]
pub struct ElevatorScreen {
    floors: Vec<String>,
    selected: usize,
    /// Kept for potential blink/scroll effects; currently unused by logic.
    frame_counter: u32,
}

impl ElevatorScreen {
    pub fn new(floors: Vec<String>) -> Self {
        Self {
            floors,
            selected: 0,
            frame_counter: 0,
        }
    }

    pub fn floors(&self) -> &[String] {
        &self.floors
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// First list index of the visible scroll window when at most
    /// `max_visible` rows fit on screen. The window follows the selection so
    /// the selected row is always visible; lists that fit entirely return 0.
    pub fn scroll_offset(&self, max_visible: usize) -> usize {
        if max_visible == 0 || self.floors.len() <= max_visible {
            return 0;
        }
        self.selected
            .saturating_sub(max_visible / 2)
            .min(self.floors.len() - max_visible)
    }

    pub fn update_frame(&mut self, input: ElevatorInput) -> ElevatorAction {
        self.frame_counter = self.frame_counter.wrapping_add(1);
        if self.floors.is_empty() {
            return ElevatorAction::Cancel;
        }
        if input.up {
            self.selected = (self.selected + self.floors.len() - 1) % self.floors.len();
        }
        if input.down {
            self.selected = (self.selected + 1) % self.floors.len();
        }
        if input.a {
            return ElevatorAction::Select(self.selected);
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

    fn floors() -> Vec<String> {
        vec!["1F".into(), "2F".into(), "3F".into()]
    }

    #[test]
    fn starts_at_first_floor() {
        let mut s = ElevatorScreen::new(floors());
        assert_eq!(s.selected_index(), 0);
        assert_eq!(s.floors(), &["1F", "2F", "3F"]);
    }

    #[test]
    fn down_wraps_and_selects() {
        let mut s = ElevatorScreen::new(floors());
        s.update_frame(ElevatorInput {
            down: true,
            ..ElevatorInput::none()
        });
        s.update_frame(ElevatorInput {
            down: true,
            ..ElevatorInput::none()
        });
        assert_eq!(s.selected_index(), 2);
        let action = s.update_frame(ElevatorInput {
            a: true,
            ..ElevatorInput::none()
        });
        assert_eq!(action, ElevatorAction::Select(2));
    }

    #[test]
    fn up_wraps() {
        let mut s = ElevatorScreen::new(floors());
        let action = s.update_frame(ElevatorInput {
            up: true,
            ..ElevatorInput::none()
        });
        assert_eq!(action, ElevatorAction::Continue);
        assert_eq!(s.selected_index(), 2);
    }

    #[test]
    fn b_cancels() {
        let mut s = ElevatorScreen::new(floors());
        let action = s.update_frame(ElevatorInput {
            b: true,
            ..ElevatorInput::none()
        });
        assert_eq!(action, ElevatorAction::Cancel);
    }

    #[test]
    fn empty_floors_cancels_immediately() {
        let mut s = ElevatorScreen::new(Vec::new());
        let action = s.update_frame(ElevatorInput::none());
        assert_eq!(action, ElevatorAction::Cancel);
    }

    fn eleven_floors() -> Vec<String> {
        (1..=11).map(|i| format!("{i}F")).collect()
    }

    #[test]
    fn scroll_offset_zero_for_short_lists() {
        let s = ElevatorScreen::new(floors());
        assert_eq!(s.scroll_offset(7), 0);
    }

    #[test]
    fn scroll_offset_follows_selection() {
        let mut s = ElevatorScreen::new(eleven_floors());
        // Selection at the top: no scrolling.
        assert_eq!(s.scroll_offset(7), 0);
        // Walk to the middle: the window centers on the selection.
        for _ in 0..5 {
            s.update_frame(ElevatorInput {
                down: true,
                ..ElevatorInput::none()
            });
        }
        assert_eq!(s.selected_index(), 5);
        assert_eq!(s.scroll_offset(7), 2);
        // Walk to the last floor: the window clamps at the end so the
        // selection stays visible.
        for _ in 0..5 {
            s.update_frame(ElevatorInput {
                down: true,
                ..ElevatorInput::none()
            });
        }
        assert_eq!(s.selected_index(), 10);
        assert_eq!(s.scroll_offset(7), 4);
        // Wrapping back to the first floor snaps the window to the top.
        s.update_frame(ElevatorInput {
            down: true,
            ..ElevatorInput::none()
        });
        assert_eq!(s.selected_index(), 0);
        assert_eq!(s.scroll_offset(7), 0);
    }
}
