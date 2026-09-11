//! Generic title-screen menu driver — the reusable, render-agnostic logic behind
//! a game's title screen.
//!
//! It owns the vertical menu (labels + which entries are enabled), the selection
//! cursor, and the "press start" blink timer. Games pair it with a `.gui` title
//! layout: each frame drive it with [`TitleMenu::update`], write its state into a
//! [`DataContext`] with [`TitleMenu::write_context`] (`items` / `cursor` /
//! `show_blink`), then render the layout.
//!
//! It deliberately knows nothing about *what* the entries mean (New Game /
//! Continue / Quit) — the game maps the returned [`TitleEvent::Selected`] index to
//! its own action, and decides which entries are enabled (e.g. disable "Continue"
//! when there is no save file). This mirrors how [`crate::menu`]-style screens are
//! driven elsewhere in the engine, so a title screen is authored the same way as
//! any other `.gui` screen — no bespoke per-game title machinery.

use crate::input::{GbButton, InputState};
use crate::layout_engine::types::{DataContext, DataValue};

/// One title-menu entry.
#[derive(Debug, Clone)]
pub struct TitleItem {
    pub label: String,
    /// Disabled entries are skipped by the cursor and cannot be selected (e.g.
    /// "Continue" with no save). The layout may still render them dimmed.
    pub enabled: bool,
}

impl TitleItem {
    /// An enabled entry.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
        }
    }

    /// An entry whose enabled state is decided at build time (e.g. `has_save`).
    pub fn with_enabled(label: impl Into<String>, enabled: bool) -> Self {
        Self {
            label: label.into(),
            enabled,
        }
    }
}

/// What [`TitleMenu::update`] reports for a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleEvent {
    /// Nothing happened this frame.
    None,
    /// The cursor moved to a new entry.
    Moved,
    /// The player confirmed the entry at this index (always an enabled entry).
    Selected(usize),
}

/// Default blink period in frames: the prompt is shown for N frames, hidden for N.
pub const DEFAULT_BLINK_PERIOD: u32 = 30;

/// A vertical title-screen menu with a blinking "press start" prompt.
#[derive(Debug, Clone)]
pub struct TitleMenu {
    items: Vec<TitleItem>,
    cursor: usize,
    blink_period: u32,
    blink_frame: u32,
}

impl TitleMenu {
    /// Build a menu from entries, landing the cursor on the first enabled one.
    pub fn new(items: Vec<TitleItem>) -> Self {
        let mut menu = Self {
            items,
            cursor: 0,
            blink_period: DEFAULT_BLINK_PERIOD,
            blink_frame: 0,
        };
        // If the first entry is disabled, seek forward to the first enabled one.
        if menu.items.first().map(|i| !i.enabled).unwrap_or(false) {
            if let Some(i) = menu.next_enabled(0, 1) {
                menu.cursor = i;
            }
        }
        menu
    }

    /// Override the blink period (frames on = frames off). Clamped to ≥1.
    pub fn with_blink_period(mut self, frames: u32) -> Self {
        self.blink_period = frames.max(1);
        self
    }

    /// Advance one frame: Up/Down move the cursor to the nearest enabled entry,
    /// A/Start confirm the current entry (if enabled), and the blink timer ticks.
    pub fn update(&mut self, input: &InputState) -> TitleEvent {
        // Blink timer ticks regardless of input.
        let cycle = (self.blink_period * 2).max(1);
        self.blink_frame = (self.blink_frame + 1) % cycle;

        if self.items.is_empty() {
            return TitleEvent::None;
        }

        let up = input.is_just_pressed(GbButton::Up);
        let down = input.is_just_pressed(GbButton::Down);
        if up || down {
            let dir = if up { -1 } else { 1 };
            if let Some(next) = self.next_enabled(self.cursor, dir) {
                if next != self.cursor {
                    self.cursor = next;
                    return TitleEvent::Moved;
                }
            }
        }

        if input.is_just_pressed(GbButton::A) || input.is_just_pressed(GbButton::Start) {
            if self.items[self.cursor].enabled {
                return TitleEvent::Selected(self.cursor);
            }
        }
        TitleEvent::None
    }

    /// The next enabled index from `from` stepping by `dir` (±1), wrapping around.
    /// `None` if no entry is enabled.
    fn next_enabled(&self, from: usize, dir: i32) -> Option<usize> {
        let n = self.items.len();
        if n == 0 {
            return None;
        }
        let mut i = from as i32;
        for _ in 0..n {
            i = (i + dir).rem_euclid(n as i32);
            if self.items[i as usize].enabled {
                return Some(i as usize);
            }
        }
        None
    }

    /// The selected entry index.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The menu entries.
    pub fn items(&self) -> &[TitleItem] {
        &self.items
    }

    /// Whether the blink prompt is currently visible (first half of the period).
    pub fn show_blink(&self) -> bool {
        self.blink_frame < self.blink_period
    }

    /// Write this menu's state into `ctx` for the `.gui` title layout:
    /// - `items`: the list of entry labels (bind a `list` to `source = "{items}"`),
    /// - `cursor`: the selected index (`selected = "{cursor}"`),
    /// - `show_blink`: whether a "press start" prompt is visible this frame
    ///   (`visible = "{show_blink}"`).
    pub fn write_context(&self, ctx: &mut DataContext) {
        let labels: Vec<DataValue> = self
            .items
            .iter()
            .map(|it| DataValue::from(it.label.as_str()))
            .collect();
        ctx.set("items", DataValue::List(labels));
        ctx.set("cursor", self.cursor as i64);
        ctx.set("show_blink", self.show_blink());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A single just-press of `button` on a fresh frame.
    fn tap(menu: &mut TitleMenu, button: GbButton) -> TitleEvent {
        let mut input = InputState::new();
        input.begin_frame();
        input.press(button);
        menu.update(&input)
    }

    fn menu3() -> TitleMenu {
        TitleMenu::new(vec![
            TitleItem::new("新的征程"),
            TitleItem::new("继续江湖"),
            TitleItem::new("退出"),
        ])
    }

    #[test]
    fn cursor_moves_and_wraps() {
        let mut m = menu3();
        assert_eq!(m.cursor(), 0);
        assert_eq!(tap(&mut m, GbButton::Down), TitleEvent::Moved);
        assert_eq!(m.cursor(), 1);
        tap(&mut m, GbButton::Down);
        assert_eq!(m.cursor(), 2);
        // Down from the last entry wraps to the first.
        assert_eq!(tap(&mut m, GbButton::Down), TitleEvent::Moved);
        assert_eq!(m.cursor(), 0);
        // Up from the first wraps to the last.
        tap(&mut m, GbButton::Up);
        assert_eq!(m.cursor(), 2);
    }

    #[test]
    fn confirm_reports_selected_index() {
        let mut m = menu3();
        tap(&mut m, GbButton::Down); // → 1
        assert_eq!(tap(&mut m, GbButton::A), TitleEvent::Selected(1));
        // Start also confirms.
        assert_eq!(tap(&mut m, GbButton::Start), TitleEvent::Selected(1));
    }

    #[test]
    fn disabled_entries_are_skipped_and_unselectable() {
        // "Continue" disabled (no save): cursor starts on the first enabled entry
        // and Down skips over the disabled middle entry.
        let mut m = TitleMenu::new(vec![
            TitleItem::new("新的征程"),
            TitleItem::with_enabled("继续江湖", false),
            TitleItem::new("退出"),
        ]);
        assert_eq!(m.cursor(), 0, "starts on the first enabled entry");
        assert_eq!(tap(&mut m, GbButton::Down), TitleEvent::Moved);
        assert_eq!(m.cursor(), 2, "Down skips the disabled entry");
        // A on the disabled entry can never fire, because the cursor can't land
        // there; confirm on an enabled entry works.
        assert_eq!(tap(&mut m, GbButton::A), TitleEvent::Selected(2));
    }

    #[test]
    fn first_entry_disabled_seeks_forward() {
        let m = TitleMenu::new(vec![
            TitleItem::with_enabled("继续江湖", false),
            TitleItem::new("新的征程"),
        ]);
        assert_eq!(m.cursor(), 1, "cursor skips a disabled first entry");
    }

    #[test]
    fn blink_toggles_over_the_period() {
        let mut m = TitleMenu::new(vec![TitleItem::new("开始")]).with_blink_period(2);
        // Fresh: frame 0 → visible.
        assert!(m.show_blink());
        let idle = InputState::new();
        m.update(&idle); // frame 1 → still visible (< period 2)
        assert!(m.show_blink());
        m.update(&idle); // frame 2 → hidden (>= period)
        assert!(!m.show_blink());
        m.update(&idle); // frame 3 → hidden
        assert!(!m.show_blink());
        m.update(&idle); // frame 4 → wraps to 0 → visible
        assert!(m.show_blink());
    }
}
