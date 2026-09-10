//! Menu system abstractions for JRPG engine.
//!
//! This module defines:
//!
//! * **`MenuProvider` trait** — game-data provider for menu definitions
//!   (titles, options, layouts). Implemented by the data crate.
//! * **`MenuSystem<M>` struct** — stateful menu controller that manages
//!   cursor position, scroll offset, input handling, and rendering.
//! * **`MenuInput` / `MenuAction`** — input abstraction and action results.
//! * **`NamedMenuInputSource` trait** — platform-level input conversion.
//!
//! # Design
//!
//! The menu system is **data-driven**: the provider supplies static layout
//! and option data, while `MenuSystem` owns the runtime state (cursor,
//! scroll, open/closed).  Rendering uses the [`Painter`](crate::render::Painter)
//! trait, so menus work identically across pixel and recording backends.

use crate::render::{Painter, Rgba, TilePos, TileRect, Ui};

// ---------------------------------------------------------------------------
// MenuInput — abstracted directional + confirm/cancel input
// ---------------------------------------------------------------------------

/// Abstract menu input, decoupled from any specific platform or key binding.
///
/// A platform input layer (keyboard, gamepad, touch) converts its events
/// into a `MenuInput` struct and passes it to [`MenuSystem::handle_input`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuInput {
    /// Up / D-Pad Up pressed this frame.
    pub up: bool,
    /// Down / D-Pad Down pressed this frame.
    pub down: bool,
    /// Confirm / A button pressed this frame.
    pub confirm: bool,
    /// Cancel / B button pressed this frame.
    pub cancel: bool,
}

// ---------------------------------------------------------------------------
// MenuAction — result of processing one frame of input
// ---------------------------------------------------------------------------

/// Outcome of [`MenuSystem::handle_input`].
///
/// The caller (game loop) uses this to transition game state — e.g.
/// `Selected(0)` on the main menu means "New Game".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// No input or no state change this frame.
    None,
    /// The player confirmed on option `N` (0-based index).
    Selected(u8),
    /// The player pressed Cancel / B.
    Cancelled,
    /// Cursor moved up (may be used for sound effects).
    Up,
    /// Cursor moved down (may be used for sound effects).
    Down,
    /// Scroll position changed to `N` (for scrollable menus).
    Scroll(u8),
}

// ---------------------------------------------------------------------------
// MenuOption — a single selectable entry
// ---------------------------------------------------------------------------

/// A single option in a menu list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuOption {
    /// Display text shown to the player.
    pub label: String,
    /// If `false`, this option is visually dimmed and cannot be selected.
    pub enabled: bool,
    /// Optional longer description / tooltip (not normally displayed).
    pub description: Option<String>,
}

impl MenuOption {
    /// Create an enabled option with just a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            description: None,
        }
    }

    /// Create a disabled option.
    pub fn disabled(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: false,
            description: None,
        }
    }
}

// ---------------------------------------------------------------------------
// MenuLayout — static positioning and appearance
// ---------------------------------------------------------------------------

/// Static layout descriptor for a menu.
///
/// All coordinates are in **tile units** (8x8 pixels per tile).  The
/// rendering code uses [`TilePos`] and [`TileRect`] to position the
/// menu box on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuLayout {
    /// Screen position of the menu (top-left corner of the border box in
    /// tile coordinates).
    pub position: TilePos,
    /// Size of the menu box including its border (in tile units).
    pub size: TileRect,
    /// Vertical spacing between option rows (in tile units).  Typically
    /// `2` for single-spaced lists.
    pub option_spacing: u32,
    /// If `true`, draw a cursor next to the currently-selected option.
    pub show_cursor: bool,
}

impl MenuLayout {
    /// Create a layout at the given tile position, with `tw * th` size
    /// (INCLUDING the 1-tile border on each side).
    pub fn new(tx: u32, ty: u32, tw: u32, th: u32) -> Self {
        Self {
            position: TilePos::new(tx, ty),
            size: TileRect::new(tx, ty, tw, th),
            option_spacing: 2,
            show_cursor: true,
        }
    }

    /// Builder: set vertical option spacing.
    pub fn with_spacing(mut self, spacing: u32) -> Self {
        self.option_spacing = spacing;
        self
    }

    /// Builder: show or hide the cursor indicator.
    pub fn with_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }
}

// ---------------------------------------------------------------------------
// TileSlot — border position identifier
// ---------------------------------------------------------------------------

/// Position slot in a border tile set. Used by editors to identify which
/// border tile to swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TileSlot {
    TopLeftCorner,
    TopRightCorner,
    BottomLeftCorner,
    BottomRightCorner,
    TopEdge,
    BottomEdge,
    LeftEdge,
    RightEdge,
    Fill,
}

// ---------------------------------------------------------------------------
// BorderStyle — 9-slot tile border configuration
// ---------------------------------------------------------------------------

/// A configurable 9-slot border for menus. Each slot references a tile index
/// in the currently active tileset. Editors can swap these to change the
/// visual style without code changes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BorderStyle {
    pub corner_tl: u16,
    pub corner_tr: u16,
    pub corner_bl: u16,
    pub corner_br: u16,
    pub edge_top: u16,
    pub edge_bottom: u16,
    pub edge_left: u16,
    pub edge_right: u16,
    pub fill_bg: u16,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            corner_tl: 192,
            corner_tr: 193, // GB standard menu border
            corner_bl: 198,
            corner_br: 199,
            edge_top: 194,
            edge_bottom: 197,
            edge_left: 195,
            edge_right: 196,
            fill_bg: 200,
        }
    }
}

// ---------------------------------------------------------------------------
// CursorAnchor — cursor positioning relative to a menu entry
// ---------------------------------------------------------------------------

/// Cursor anchor point relative to a menu entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CursorAnchor {
    #[default]
    CenterLeft,
    TopLeft,
    BottomLeft,
    TopRight,
    CenterRight,
}

// ---------------------------------------------------------------------------
// CursorStyle — cursor indicator appearance
// ---------------------------------------------------------------------------

/// Cursor indicator style for menus.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CursorStyle {
    /// Tile index for the cursor glyph. `None` → invisible cursor.
    pub tile: Option<u16>,
    /// How the cursor is positioned relative to the menu entry.
    pub anchor: CursorAnchor,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self {
            tile: Some(223),
            anchor: CursorAnchor::CenterLeft,
        }
    }
}

impl CursorStyle {
    /// Create a cursor style with the given tile index and anchor.
    pub fn new(tile: Option<u16>, anchor: CursorAnchor) -> Self {
        Self { tile, anchor }
    }
}

// ---------------------------------------------------------------------------
// EdgeInsets — layout padding
// ---------------------------------------------------------------------------

/// Padding inside a menu container (in tiles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeInsets {
    /// Padding from the top of the container (in tiles).
    pub top: u32,
    /// Padding from the bottom of the container (in tiles).
    pub bottom: u32,
    /// Padding from the left side of the container (in tiles).
    pub left: u32,
    /// Padding from the right side of the container (in tiles).
    pub right: u32,
}

impl Default for EdgeInsets {
    fn default() -> Self {
        Self {
            top: 1,
            bottom: 1,
            left: 1,
            right: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// MenuConfig — rendering-level layout configuration
// ---------------------------------------------------------------------------

/// Rendering-level layout for a menu. Describes where and how a menu is
/// drawn on the 160×144 canvas.
///
/// Unlike [`MenuLayout`] (which is a game-data provider interface),
/// `MenuConfig` is the concrete rendering configuration consumed by
/// [`dotzuki_ui`](crate) widget drawing functions.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MenuConfig {
    /// The outer bounding box of the menu in tile coordinates.
    pub area: TileRect,
    /// Border tile configuration. `None` → no border (transparent background).
    pub border: Option<BorderStyle>,
    /// The inner content area (text, icons, etc.) relative to `area`.
    pub content: TileRect,
    /// Cursor appearance.
    pub cursor: CursorStyle,
    /// Pre-positioned labels at frame-relative tile coordinates.
    /// Each entry is `(tx, ty, text)`.
    pub label_positions: Vec<(u32, u32, String)>,
    /// Vertical gap between list items in tile rows.
    pub gap: u32,
    /// Interior padding inside the border.
    pub padding: EdgeInsets,
}

impl MenuConfig {
    pub fn new(
        area: TileRect,
        border: Option<BorderStyle>,
        content: TileRect,
        cursor: CursorStyle,
    ) -> Self {
        Self {
            area,
            border,
            content,
            cursor,
            label_positions: Vec::new(),
            gap: 1,
            padding: EdgeInsets::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// MenuProvider trait
// ---------------------------------------------------------------------------

/// Provider of static menu definitions (titles, options, layouts).
///
/// `MenuProvider` decouples menu **data** from menu **state** and
/// **rendering**.  The implementing crate supplies concrete menu IDs and
/// the associated data; the engine's [`MenuSystem`] consumes this data
/// without knowing anything about the game's menu hierarchy.
///
/// # Associated Types
///
/// * `MenuId` — an enum (or other `Copy + PartialEq + Debug` type) that
///   identifies which menu screen to display.
pub trait MenuProvider {
    /// The type that identifies a specific menu screen.
    type MenuId: Copy + core::fmt::Debug + PartialEq;

    /// Title string displayed at the top of the menu box.
    fn title(&self, menu: Self::MenuId) -> &str;

    /// The list of options for this menu.  The returned slice must live
    /// at least as long as `self`.
    fn options(&self, menu: Self::MenuId) -> &[MenuOption];

    /// Number of **visible** options (for cursor bounds checking and
    /// scroll window size).
    fn option_count(&self, menu: Self::MenuId) -> u8;

    /// Whether the option list can scroll when it exceeds the visible
    /// window.
    fn scrollable(&self, menu: Self::MenuId) -> bool;

    /// Static layout descriptor (position, size, spacing, cursor policy).
    fn layout(&self, menu: Self::MenuId) -> MenuLayout;
}

// ---------------------------------------------------------------------------
// MenuSystem — stateful menu controller
// ---------------------------------------------------------------------------

/// Stateful menu controller that owns cursor position, scroll offset,
/// and open/closed state.
///
/// `MenuSystem` is parameterised over the `MenuProvider` implementation,
/// so it can call into the provider for data without any downcasting or
/// dynamic dispatch overhead.
pub struct MenuSystem<'prov, M: MenuProvider> {
    /// Reference to the menu data provider.
    provider: &'prov M,
    /// Which menu is currently active.
    pub current_menu: M::MenuId,
    /// 0-based index of the currently-highlighted option.
    pub cursor: u8,
    /// Scroll offset (for scrollable menus).  0 = first visible option
    /// is at index 0 in the full option list.
    pub scroll_offset: u8,
    /// Whether the menu is currently open / visible.
    is_open: bool,
}

impl<'prov, M: MenuProvider> MenuSystem<'prov, M> {
    /// Create a new `MenuSystem` backed by `provider`.  The menu starts
    /// closed; call [`open`](Self::open) to activate it.
    pub fn new(provider: &'prov M) -> Self {
        Self {
            provider,
            // We need a default for current_menu.  Use open() to set it.
            current_menu: unsafe { core::mem::zeroed() },
            cursor: 0,
            scroll_offset: 0,
            is_open: false,
        }
    }

    /// Open a specific menu, resetting cursor and scroll to their defaults.
    pub fn open(&mut self, menu: M::MenuId) {
        self.current_menu = menu;
        self.cursor = 0;
        self.scroll_offset = 0;
        self.is_open = true;
    }

    /// Close the menu.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Returns `true` if the menu is currently open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Process one frame of abstract input and return a [`MenuAction`].
    ///
    /// The caller should call this once per frame with the aggregated
    /// directional / confirm / cancel state.
    pub fn handle_input(&mut self, input: &MenuInput) -> MenuAction {
        if !self.is_open {
            return MenuAction::None;
        }

        let total = self.provider.option_count(self.current_menu);
        if total == 0 {
            // No options -- only cancel is valid.
            if input.cancel {
                self.is_open = false;
                return MenuAction::Cancelled;
            }
            return MenuAction::None;
        }

        // --- direction ---
        if input.up && self.cursor > 0 {
            // Find previous enabled option (skip disabled).
            let mut next = self.cursor;
            loop {
                if next == 0 {
                    break;
                }
                next -= 1;
                let opts = self.provider.options(self.current_menu);
                if opts[next as usize].enabled {
                    self.cursor = next;
                    return MenuAction::Up;
                }
            }
        }

        if input.down {
            let max = (total as usize).saturating_sub(1);
            let mut next = self.cursor;
            loop {
                if (next as usize) >= max {
                    break;
                }
                next += 1;
                let opts = self.provider.options(self.current_menu);
                if opts[next as usize].enabled {
                    self.cursor = next;
                    return MenuAction::Down;
                }
            }
        }

        // --- confirm ---
        if input.confirm {
            let opts = self.provider.options(self.current_menu);
            if let Some(opt) = opts.get(self.cursor as usize) {
                if opt.enabled {
                    return MenuAction::Selected(self.cursor);
                }
            }
        }

        // --- cancel ---
        if input.cancel {
            self.is_open = false;
            return MenuAction::Cancelled;
        }

        MenuAction::None
    }

    /// Return the currently-selected option, or `None` if the cursor is
    /// out of range or the menu is closed.
    pub fn selected_option(&self) -> Option<&MenuOption> {
        if !self.is_open {
            return None;
        }
        self.provider
            .options(self.current_menu)
            .get(self.cursor as usize)
    }

    /// Render the menu onto `painter` using the [`Painter`] trait.
    ///
    /// This draws the text box, title, options, and cursor indicator
    /// through the backend-agnostic `Painter` interface.  TUI and pixel
    /// backends produce identical visual output (modulo resolution).
    pub fn render<P: Painter>(&self, painter: &mut P) {
        if !self.is_open {
            return;
        }

        let layout = self.provider.layout(self.current_menu);
        let title = self.provider.title(self.current_menu);
        let options = self.provider.options(self.current_menu);
        let visible_count = self.provider.option_count(self.current_menu);

        let mut ui = Ui::new(painter);

        ui.text_box(layout.size, Rgba::INK_BLACK, true, |f| {
            // Title row (row 0 inside the border).
            f.label(0, 0, title, Rgba::INK_BLACK);

            // Options start at row 1 (just below the title), skip scroll_offset.
            let start = self.scroll_offset as usize;
            let end = (start + visible_count as usize).min(options.len());

            for i in start..end {
                let row_inner = 1 + ((i - start) as u32) * layout.option_spacing;
                let opt = &options[i];
                let color = if opt.enabled {
                    Rgba::INK_BLACK
                } else {
                    Rgba::INK_LIGHT_GRAY
                };
                f.label(1, row_inner, &opt.label, color);

                // Cursor
                if layout.show_cursor && i == self.cursor as usize && opt.enabled {
                    f.cursor_at(0, row_inner, Rgba::INK_BLACK);
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// NamedMenuInputSource trait -- platform input to MenuInput
// ---------------------------------------------------------------------------

/// Trait for converting platform-specific input (keyboard, gamepad,
/// touch) into abstract [`MenuInput`].
///
/// Implementors read their hardware / event state and produce a
/// `MenuInput` struct once per frame.
pub trait NamedMenuInputSource {
    /// Read the current input state and return a [`MenuInput`].
    fn read_input(&self) -> MenuInput;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Painter, Rgba, TilePos, TileRect};

    // -- Mock types -------------------------------------------------------

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockMenuId {
        MainMenu,
    }

    struct MockMenuProvider {
        title: &'static str,
        options: Vec<MenuOption>,
        layout: MenuLayout,
    }

    impl MenuProvider for MockMenuProvider {
        type MenuId = MockMenuId;

        fn title(&self, _menu: Self::MenuId) -> &str {
            self.title
        }

        fn options(&self, _menu: Self::MenuId) -> &[MenuOption] {
            &self.options
        }

        fn option_count(&self, _menu: Self::MenuId) -> u8 {
            self.options.len() as u8
        }

        fn scrollable(&self, _menu: Self::MenuId) -> bool {
            false
        }

        fn layout(&self, _menu: Self::MenuId) -> MenuLayout {
            self.layout
        }
    }

    // -- Recording painter for tests -------------------------------------

    /// A `Painter` that records every call for later inspection.
    #[derive(Debug, Default)]
    struct RecordingPainter {
        text_boxes: Vec<(TileRect, Rgba)>,
        texts: Vec<(TilePos, String, Rgba)>,
        glyphs: Vec<(TilePos, char, Rgba)>,
    }

    impl Painter for RecordingPainter {
        fn clear(&mut self, _color: Rgba) {}
        fn draw_text_box(&mut self, rect: TileRect, color: Rgba) {
            self.text_boxes.push((rect, color));
        }
        fn draw_text(&mut self, pos: TilePos, text: &str, color: Rgba) {
            self.texts.push((pos, text.to_string(), color));
        }
        fn draw_glyph(&mut self, pos: TilePos, glyph: char, color: Rgba) {
            self.glyphs.push((pos, glyph, color));
        }
        fn draw_pixel_rect(&mut self, _px: u32, _py: u32, _pw: u32, _ph: u32, _color: Rgba) {}
        fn draw_gb_tile(&mut self, _pos: TilePos, _tile_id: u8, _fallback: &str, _color: Rgba) {}
    }

    fn make_provider() -> MockMenuProvider {
        MockMenuProvider {
            title: "POKERED",
            options: vec![
                MenuOption::new("New Game"),
                MenuOption::new("Continue"),
                MenuOption::new("Quit"),
            ],
            layout: MenuLayout::new(5, 3, 10, 9),
        }
    }

    // -- Tests -----------------------------------------------------------

    #[test]
    fn open_menu_sets_cursor_to_zero() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);
        assert!(system.is_open());
        assert_eq!(system.cursor, 0);
        assert_eq!(system.scroll_offset, 0);
    }

    #[test]
    fn cursor_moves_down_on_down_input() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        let action = system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Down);
        assert_eq!(system.cursor, 1);
    }

    #[test]
    fn cursor_stops_at_last_option() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        // Move to last option (index 2).
        system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(system.cursor, 2);

        // One more down should do nothing (stay at 2).
        let action = system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::None);
        assert_eq!(system.cursor, 2);
    }

    #[test]
    fn cursor_moves_up_on_up_input() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        // Move down first, then up.
        system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(system.cursor, 1);

        let action = system.handle_input(&MenuInput {
            up: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Up);
        assert_eq!(system.cursor, 0);
    }

    #[test]
    fn confirm_on_enabled_option_returns_selected() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        // Move to "Continue" (index 1).
        system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });

        let action = system.handle_input(&MenuInput {
            confirm: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Selected(1));
    }

    #[test]
    fn cancel_returns_cancelled_and_closes_menu() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        let action = system.handle_input(&MenuInput {
            cancel: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Cancelled);
        assert!(!system.is_open());
    }

    #[test]
    fn disabled_option_is_skipped_by_cursor() {
        let provider = MockMenuProvider {
            title: "TEST",
            options: vec![
                MenuOption::new("A"),
                MenuOption::disabled("B"),
                MenuOption::new("C"),
            ],
            layout: MenuLayout::new(0, 0, 6, 5),
        };
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        // Cursor starts at "A" (index 0). Down should skip disabled "B" and land on "C".
        let action = system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::Down);
        assert_eq!(system.cursor, 2);
    }

    #[test]
    fn confirm_on_disabled_option_does_nothing() {
        let provider = MockMenuProvider {
            title: "TEST",
            options: vec![MenuOption::disabled("X"), MenuOption::new("Y")],
            layout: MenuLayout::new(0, 0, 6, 5),
        };
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        // Cursor starts at 0 (disabled). Confirm should do nothing.
        let action = system.handle_input(&MenuInput {
            confirm: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::None);
    }

    #[test]
    fn selected_option_returns_correct_option() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        // Move to "Continue".
        system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });

        let opt = system.selected_option();
        assert!(opt.is_some());
        assert_eq!(opt.unwrap().label, "Continue");
    }

    #[test]
    fn closed_menu_ignores_input() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        // Menu is NOT opened.

        let action = system.handle_input(&MenuInput {
            down: true,
            confirm: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::None);
    }

    #[test]
    fn render_draws_text_box_at_layout_position() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        let mut painter = RecordingPainter::default();
        system.render(&mut painter);

        // Should draw one text box.
        assert_eq!(painter.text_boxes.len(), 1);
        let (rect, _) = painter.text_boxes[0];
        assert_eq!(rect.tx, 5);
        assert_eq!(rect.ty, 3);
        assert_eq!(rect.tw, 10);
        assert_eq!(rect.th, 9);
    }

    #[test]
    fn render_draws_title_and_options() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        let mut painter = RecordingPainter::default();
        system.render(&mut painter);

        // Title should be drawn.
        let titles: Vec<_> = painter
            .texts
            .iter()
            .filter(|(_, t, _)| t == "POKERED")
            .collect();
        assert!(!titles.is_empty(), "Title 'POKERED' was not drawn");

        // All three options should be drawn.
        for expected in &["New Game", "Continue", "Quit"] {
            let found = painter.texts.iter().any(|(_, t, _)| t == expected);
            assert!(found, "Option '{}' was not drawn", expected);
        }
    }

    #[test]
    fn render_draws_cursor_at_correct_position() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        // Move cursor to index 1 ("Continue").
        system.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });

        let mut painter = RecordingPainter::default();
        system.render(&mut painter);

        // Should have exactly one cursor glyph.
        assert_eq!(painter.glyphs.len(), 1, "Expected exactly 1 cursor glyph");
        let (glyph_pos, glyph_char, _) = painter.glyphs[0];
        assert_eq!(glyph_char, '\u{25B6}', "Cursor glyph should be U+25B6");

        // Cursor should be on the row corresponding to option index 1.
        // Layout: title at inner row 0, options start at inner row 1,
        // option_spacing = 2, so option 1 is at inner row 3.
        // Frame origin = layout.pos + 1-tile border inset = (6, 4).
        // So absolute tile pos for cursor = (6 + 0, 4 + 1 + 1*2) = (6, 7).
        assert_eq!(glyph_pos.tx, 6);
        assert_eq!(glyph_pos.ty, 7);
    }

    #[test]
    fn closed_menu_renders_nothing() {
        let provider = make_provider();
        let system = MenuSystem::new(&provider);
        // NOT opened.

        let mut painter = RecordingPainter::default();
        system.render(&mut painter);

        assert!(painter.text_boxes.is_empty());
        assert!(painter.texts.is_empty());
        assert!(painter.glyphs.is_empty());
    }

    #[test]
    fn empty_menu_returns_none_on_confirm() {
        let provider = MockMenuProvider {
            title: "EMPTY",
            options: vec![],
            layout: MenuLayout::new(0, 0, 6, 3),
        };
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);

        let action = system.handle_input(&MenuInput {
            confirm: true,
            ..Default::default()
        });
        assert_eq!(action, MenuAction::None);
    }

    #[test]
    fn menu_option_new_and_disabled() {
        let opt = MenuOption::new("Hello");
        assert!(opt.enabled);
        assert_eq!(opt.label, "Hello");
        assert!(opt.description.is_none());

        let opt = MenuOption::disabled("Locked");
        assert!(!opt.enabled);
        assert_eq!(opt.label, "Locked");
    }

    #[test]
    fn menu_layout_builders() {
        let layout = MenuLayout::new(1, 2, 8, 6)
            .with_spacing(3)
            .with_cursor(false);

        assert_eq!(layout.position.tx, 1);
        assert_eq!(layout.position.ty, 2);
        assert_eq!(layout.size.tx, 1);
        assert_eq!(layout.size.ty, 2);
        assert_eq!(layout.size.tw, 8);
        assert_eq!(layout.size.th, 6);
        assert_eq!(layout.option_spacing, 3);
        assert!(!layout.show_cursor);
    }

    #[test]
    fn menu_input_default_is_all_false() {
        let input = MenuInput::default();
        assert!(!input.up);
        assert!(!input.down);
        assert!(!input.confirm);
        assert!(!input.cancel);
    }

    #[test]
    fn menu_system_close_and_reopen() {
        let provider = make_provider();
        let mut system = MenuSystem::new(&provider);
        system.open(MockMenuId::MainMenu);
        assert!(system.is_open());

        system.close();
        assert!(!system.is_open());

        system.open(MockMenuId::MainMenu);
        assert!(system.is_open());
        assert_eq!(system.cursor, 0); // reset
    }
}
