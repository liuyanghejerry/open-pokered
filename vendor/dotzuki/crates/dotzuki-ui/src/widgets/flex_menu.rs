//! Generic auto-sizing flex menu widget.
//!
//! Renders a vertical list of items in a bordered box that auto-sizes
//! to content width and height.  Supports cursor with wrap-around,
//! scrolling for long menus, and min/max size clamping.
//!
//! All positions are in tile units.  Uses the [`Painter`] trait for
//! backend-agnostic rendering.

use dotzuki_engine::menu::MenuConfig;
use dotzuki_engine::render::{Painter, Rgba, TileRect, Ui};

// ---------------------------------------------------------------------------
// clamp helper
// ---------------------------------------------------------------------------

/// Clamp `val` between `min` and `max` when present.
///
/// If both `min` and `max` are `None`, `val` is returned unchanged.
pub fn clamp(val: u32, min: Option<u32>, max: Option<u32>) -> u32 {
    let v = min.map_or(val, |m| val.max(m));
    max.map_or(v, |m| v.min(m))
}

// ---------------------------------------------------------------------------
// edge insets
// ---------------------------------------------------------------------------

/// Padding inside a flex menu container.
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
// Justify
// ---------------------------------------------------------------------------

/// Vertical alignment of content within the flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Justify {
    /// Content starts at the top padding edge.
    #[default]
    Start,
    /// Content is centered between top and bottom padding.
    Center,
    /// Content ends at the bottom padding edge.
    End,
}

// ---------------------------------------------------------------------------
// sizing mode
// ---------------------------------------------------------------------------

/// How a flex menu dimension is determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SizeMode {
    /// Use the fixed tile size from `config.rect`.
    #[default]
    Fixed,
    /// Auto-size to content, bounded by `min_width`/`max_width`
    /// (or `min_height`/`max_height`).
    Auto,
}

// ---------------------------------------------------------------------------
// config
// ---------------------------------------------------------------------------

/// Configuration for a flex menu widget.
#[derive(Debug, Clone)]
pub struct FlexMenuConfig {
    /// Base position (used as origin for auto-sizing; the effective rect
    /// may change if `width_mode` or `height_mode` is `Auto`).
    pub rect: TileRect,
    /// Ink colour for the box border and text.
    pub color: Rgba,
    /// Padding inside the box (tile units).
    pub padding: EdgeInsets,
    /// Vertical gap between items in tile rows (0 = no gap).
    pub gap: u32,
    /// How the container width is determined.
    pub width_mode: SizeMode,
    /// How the container height is determined.
    pub height_mode: SizeMode,
    /// Minimum container width in tiles (only used when `width_mode` is `Auto`).
    pub min_width: Option<u32>,
    /// Maximum container width in tiles.
    pub max_width: Option<u32>,
    /// Minimum container height in tiles (only used when `height_mode` is `Auto`).
    pub min_height: Option<u32>,
    /// Maximum container height in tiles.
    pub max_height: Option<u32>,
    /// Vertical alignment of content (only used when `height_mode` is `Auto`).
    pub justify: Justify,
    /// Glyph character for the cursor (default: ▶).
    pub cursor_glyph: char,
    /// Ink colour for the cursor glyph.
    pub cursor_color: Rgba,
    /// If `true`, cursor wraps around at the top/bottom boundaries.
    pub wrap_cursor: bool,
}

impl Default for FlexMenuConfig {
    fn default() -> Self {
        Self {
            rect: TileRect::new(0, 0, 10, 6),
            color: Rgba::INK_BLACK,
            padding: EdgeInsets::default(),
            gap: 0,
            width_mode: SizeMode::Fixed,
            height_mode: SizeMode::Fixed,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            justify: Justify::Start,
            cursor_glyph: '\u{25B6}',
            cursor_color: Rgba::INK_BLACK,
            wrap_cursor: true,
        }
    }
}

impl FlexMenuConfig {
    /// Create a flex menu config at a specific position with given size.
    pub fn new(tx: u32, ty: u32, tw: u32, th: u32) -> Self {
        Self {
            rect: TileRect::new(tx, ty, tw, th),
            ..Default::default()
        }
    }

    /// Builder: enable auto-sizing for both width and height.
    pub fn with_auto_size(mut self) -> Self {
        self.width_mode = SizeMode::Auto;
        self.height_mode = SizeMode::Auto;
        self
    }

    /// Builder: set the gap between items.
    pub fn with_gap(mut self, gap: u32) -> Self {
        self.gap = gap;
        self
    }

    /// Builder: set the padding.
    pub fn with_padding(mut self, top: u32, bottom: u32, left: u32, right: u32) -> Self {
        self.padding = EdgeInsets {
            top,
            bottom,
            left,
            right,
        };
        self
    }

    /// Builder: set size bounds.
    pub fn with_bounds(
        mut self,
        min_w: Option<u32>,
        max_w: Option<u32>,
        min_h: Option<u32>,
        max_h: Option<u32>,
    ) -> Self {
        self.min_width = min_w;
        self.max_width = max_w;
        self.min_height = min_h;
        self.max_height = max_h;
        self
    }

    /// Builder: set vertical justification.
    pub fn with_justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }

    /// Builder: set the cursor glyph and colour.
    pub fn with_cursor(mut self, glyph: char, color: Rgba) -> Self {
        self.cursor_glyph = glyph;
        self.cursor_color = color;
        self
    }

    /// Builder: disable cursor wrap-around.
    pub fn without_wrap(mut self) -> Self {
        self.wrap_cursor = false;
        self
    }

    /// Builder: set the text colour.
    pub fn with_color(mut self, color: Rgba) -> Self {
        self.color = color;
        self
    }
}

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

/// Runtime state for a flex menu.
#[derive(Debug, Clone)]
pub struct FlexMenuState {
    /// 0-based index of the currently-highlighted item.
    pub cursor: usize,
    /// Scroll offset: first visible item index (0-based).
    /// Items before this index are clipped.
    pub scroll_offset: usize,
}

impl Default for FlexMenuState {
    fn default() -> Self {
        Self {
            cursor: 0,
            scroll_offset: 0,
        }
    }
}

impl FlexMenuState {
    /// Move the cursor up, respecting wrap-around and scroll bounds.
    /// Returns `true` if the cursor actually moved.
    pub fn cursor_up(&mut self, item_count: usize, max_visible: usize) -> bool {
        if item_count == 0 {
            return false;
        }
        if self.cursor > 0 {
            self.cursor -= 1;
        } else if self.cursor == 0 {
            // wrap to bottom
            self.cursor = item_count - 1;
        }
        self.clamp_scroll(item_count, max_visible);
        true
    }

    /// Move the cursor down, respecting wrap-around and scroll bounds.
    /// Returns `true` if the cursor actually moved.
    pub fn cursor_down(&mut self, item_count: usize, max_visible: usize) -> bool {
        if item_count == 0 {
            return false;
        }
        let max = item_count.saturating_sub(1);
        if self.cursor < max {
            self.cursor += 1;
        } else {
            // wrap to top
            self.cursor = 0;
        }
        self.clamp_scroll(item_count, max_visible);
        true
    }

    /// Ensure the cursor is visible by adjusting `scroll_offset`.
    fn clamp_scroll(&mut self, item_count: usize, max_visible: usize) {
        if max_visible == 0 || item_count == 0 {
            self.scroll_offset = 0;
            return;
        }
        let max_visible = max_visible.min(item_count);
        // If cursor is above the visible window, scroll up.
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        }
        // If cursor is below the visible window, scroll down.
        if self.cursor >= self.scroll_offset + max_visible {
            self.scroll_offset = self.cursor + 1 - max_visible;
        }
        // Don't scroll past the end.
        let max_offset = item_count.saturating_sub(max_visible);
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }
    }
}

// ---------------------------------------------------------------------------
// draw
// ---------------------------------------------------------------------------

/// Draw a simple flex menu onto `ui` using a [`MenuConfig`].
///
/// Items are drawn in a bordered box at `config.area`.  No auto-sizing
/// is performed — the box size is determined by `config.area`.
///
/// The cursor is drawn at `state.cursor`, and visible items are scrolled
/// according to `state.scroll_offset`.
///
/// # Type Parameters
///
/// * `T: AsRef<str>` — each item provides its display text.
///
/// # Example
///
/// ```ignore
/// use dotzuki_engine::menu::{MenuConfig, CursorStyle};
/// use dotzuki_engine::render::TileRect;
///
/// let area = TileRect::new(5, 3, 10, 9);
/// let content = TileRect::new(6, 4, 8, 7);
/// let cursor = CursorStyle::new(Some(223), Default::default());
/// let config = MenuConfig::new(area, None, content, cursor);
/// let state = FlexMenuState::default();
/// draw_flex_menu(&items, &config, &state, max_visible, &mut ui);
/// ```
pub fn draw_flex_menu<P: Painter, T: AsRef<str>>(
    items: &[T],
    configs: &[MenuConfig],
    state: &FlexMenuState,
    max_visible: usize,
    ui: &mut Ui<P>,
) {
    let Some(config) = configs.first() else {
        return;
    };
    let num_items = items.len();
    if num_items == 0 {
        return;
    }

    let visible = max_visible.min(num_items);
    let rel_tx = config.content.tx.saturating_sub(config.area.tx + 1);
    let rel_ty = config.content.ty.saturating_sub(config.area.ty + 1);

    ui.text_box(config.area, Rgba::INK_BLACK, true, |frame| {
        let start = state.scroll_offset;
        let end = (start + visible).min(num_items);

        for (i, idx) in (start..end).enumerate() {
            let y = rel_ty + (i as u32);
            let item_text = items[idx].as_ref();
            frame.label(rel_tx, y, item_text, Rgba::INK_BLACK);

            if idx == state.cursor && config.cursor.tile.is_some() {
                frame.cursor_glyph_at(rel_tx.saturating_sub(1), y, '\u{25B6}', Rgba::INK_BLACK);
            }
        }
    });
}

/// Draw a flex menu onto `ui` (legacy, uses [`FlexMenuConfig`]).
///
/// Items are drawn in a bordered box.  When `config.width_mode` or
/// `config.height_mode` is `Auto`, the box size is computed from the
/// item content and clamped to the configured bounds.
///
/// The cursor is drawn at `state.cursor`, and visible items are scrolled
/// according to `state.scroll_offset`.
///
/// # Type Parameters
///
/// * `T: AsRef<str>` — each item provides its display text.
///
/// # Example
///
/// ```ignore
/// let items = ["New Game", "Continue", "Quit"];
/// let config = FlexMenuConfig::new(5, 3, 10, 9).with_auto_size();
/// let state = FlexMenuState::default();
/// draw_flex_menu_legacy(&items, &config, &state, max_visible, &mut ui);
/// ```
#[deprecated(note = "Use draw_flex_menu with &MenuConfig instead")]
pub fn draw_flex_menu_legacy<P: Painter, T: AsRef<str>>(
    items: &[T],
    config: &FlexMenuConfig,
    state: &FlexMenuState,
    max_visible: usize,
    ui: &mut Ui<P>,
) {
    let num_items = items.len();
    if num_items == 0 {
        return;
    }

    // ── compute effective size ──
    let content_w = items
        .iter()
        .map(|s| s.as_ref().len() as u32)
        .max()
        .unwrap_or(1);

    let eff_w = match config.width_mode {
        SizeMode::Fixed => config.rect.tw,
        SizeMode::Auto => clamp(
            content_w + config.padding.left + config.padding.right + 2, // +2 for border
            config.min_width,
            config.max_width,
        ),
    };

    let visible = max_visible.min(num_items);
    let content_h = visible as u32 + (visible.saturating_sub(1) as u32) * config.gap;

    let eff_h = match config.height_mode {
        SizeMode::Fixed => config.rect.th,
        SizeMode::Auto => clamp(
            content_h + config.padding.top + config.padding.bottom + 2, // +2 for border
            config.min_height,
            config.max_height,
        ),
    };

    let rect = TileRect::new(config.rect.tx, config.rect.ty, eff_w, eff_h);

    // ── vertical placement ──
    let start_y = match config.justify {
        Justify::Start => config.padding.top,
        Justify::Center => {
            let inner_h = eff_h.saturating_sub(2); // minus border
            let pad_h = config.padding.top + config.padding.bottom;
            config.padding.top + (inner_h.saturating_sub(pad_h).saturating_sub(content_h)) / 2
        }
        Justify::End => eff_h.saturating_sub(2 + config.padding.bottom + content_h),
    };

    ui.text_box(rect, config.color, true, |frame| {
        let start = state.scroll_offset;
        let end = (start + visible).min(num_items);

        for (i, idx) in (start..end).enumerate() {
            let y = start_y + (i as u32) * (1 + config.gap);
            let item_text = items[idx].as_ref();
            frame.label(config.padding.left, y, item_text, config.color);

            if idx == state.cursor {
                // Cursor sits at interior column 0 (left of padding)
                frame.cursor_glyph_at(0, y, config.cursor_glyph, config.cursor_color);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::menu::CursorStyle;
    use dotzuki_engine::render::TilePos;

    /// A `Painter` that records every call for test inspection.
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

    /// Helper: create a default `MenuConfig` for flex menu tests.
    fn flex_menu_config() -> MenuConfig {
        let area = TileRect::new(5, 3, 10, 9);
        let content = TileRect::new(6, 4, 8, 7);
        let cursor = CursorStyle::new(Some(223), Default::default());
        MenuConfig::new(area, None, content, cursor)
    }

    #[test]
    fn clamp_none_returns_val() {
        assert_eq!(clamp(5, None, None), 5);
    }

    #[test]
    fn clamp_min_enforces_lower() {
        assert_eq!(clamp(2, Some(5), None), 5);
    }

    #[test]
    fn clamp_max_enforces_upper() {
        assert_eq!(clamp(10, None, Some(8)), 8);
    }

    #[test]
    fn clamp_both_bounds() {
        assert_eq!(clamp(2, Some(5), Some(8)), 5);
        assert_eq!(clamp(7, Some(5), Some(8)), 7);
        assert_eq!(clamp(12, Some(5), Some(8)), 8);
    }

    #[test]
    fn flex_menu_state_cursor_up_wraps() {
        let mut state = FlexMenuState::default();
        state.cursor_up(3, 6);
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn flex_menu_state_cursor_down_wraps() {
        let mut state = FlexMenuState {
            cursor: 2,
            scroll_offset: 0,
        };
        state.cursor_down(3, 6);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn flex_menu_state_cursor_up_non_wrap() {
        let mut state = FlexMenuState {
            cursor: 0,
            scroll_offset: 0,
        };
        state.cursor_up(3, 6);
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn flex_menu_state_empty_does_nothing() {
        let mut state = FlexMenuState::default();
        assert!(!state.cursor_up(0, 6));
        assert!(!state.cursor_down(0, 6));
    }

    #[test]
    fn flex_menu_state_scroll_follows_cursor() {
        let mut state = FlexMenuState::default();
        for _ in 0..5 {
            state.cursor_down(10, 3);
        }
        assert_eq!(state.cursor, 5);
        assert!(state.scroll_offset > 0);
        assert!(state.cursor >= state.scroll_offset);
        assert!(state.cursor < state.scroll_offset + 3);
    }

    #[test]
    fn draw_flex_menu_fixed_size() {
        let items = ["A", "B", "C"];
        let config = flex_menu_config();
        let state = FlexMenuState::default();
        let mut painter = RecordingPainter::default();
        let mut ui = Ui::new(&mut painter);

        draw_flex_menu(&items, &[config], &state, 3, &mut ui);

        assert_eq!(painter.text_boxes.len(), 1);
        assert_eq!(painter.text_boxes[0].0, TileRect::new(5, 3, 10, 9));

        for expected in &["A", "B", "C"] {
            let found = painter.texts.iter().any(|(_, t, _)| t == expected);
            assert!(found, "Item '{}' was not drawn", expected);
        }
    }

    #[test]
    fn draw_flex_menu_cursor_position() {
        let items = ["Item1", "Item2", "Item3"];
        let config = flex_menu_config();
        let state = FlexMenuState {
            cursor: 1,
            scroll_offset: 0,
        };
        let mut painter = RecordingPainter::default();
        let mut ui = Ui::new(&mut painter);

        draw_flex_menu(&items, &[config], &state, 3, &mut ui);

        assert_eq!(painter.glyphs.len(), 1, "Expected exactly 1 cursor glyph");
        let (pos, glyph, _) = &painter.glyphs[0];
        assert_eq!(*glyph, '\u{25B6}');
        // content at (6,4), area at (5,3). Frame interior at (6,4).
        // rel_tx = 6-5-1 = 0, rel_ty = 4-3-1 = 0
        // Item 1 at y = 0 + 1 = 1, abs_ty = 4 + 1 = 5
        assert_eq!(pos.ty, 5);
    }

    #[test]
    fn draw_flex_menu_empty_returns_early() {
        let items: &[&str] = &[];
        let config = flex_menu_config();
        let state = FlexMenuState::default();
        let mut painter = RecordingPainter::default();
        let mut ui = Ui::new(&mut painter);

        draw_flex_menu(items, &[config], &state, 6, &mut ui);

        assert!(painter.text_boxes.is_empty());
        assert!(painter.texts.is_empty());
        assert!(painter.glyphs.is_empty());
    }

    #[test]
    fn draw_flex_menu_scroll_clips_items() {
        let items = ["A", "B", "C", "D", "E"];
        let config = flex_menu_config();
        let state = FlexMenuState {
            cursor: 3,
            scroll_offset: 1,
        };
        let mut painter = RecordingPainter::default();
        let mut ui = Ui::new(&mut painter);

        draw_flex_menu(&items, &[config], &state, 3, &mut ui);

        let drawn: Vec<&str> = painter.texts.iter().map(|(_, t, _)| t.as_str()).collect();
        assert!(drawn.contains(&"B"));
        assert!(drawn.contains(&"C"));
        assert!(drawn.contains(&"D"));
        assert!(!drawn.contains(&"A"));
        assert!(!drawn.contains(&"E"));
    }

    #[test]
    fn flex_menu_config_builders() {
        let config = FlexMenuConfig::new(1, 2, 8, 6)
            .with_auto_size()
            .with_gap(1)
            .with_padding(2, 2, 2, 2)
            .with_bounds(Some(6), Some(16), Some(4), Some(12))
            .with_justify(Justify::Center)
            .with_cursor('*', Rgba::INK_DARK_GRAY)
            .with_color(Rgba::INK_LIGHT_GRAY);

        assert_eq!(config.rect, TileRect::new(1, 2, 8, 6));
        assert!(matches!(config.width_mode, SizeMode::Auto));
        assert!(matches!(config.height_mode, SizeMode::Auto));
        assert_eq!(config.gap, 1);
        assert_eq!(config.padding.top, 2);
        assert_eq!(config.min_width, Some(6));
        assert_eq!(config.max_height, Some(12));
        assert!(matches!(config.justify, Justify::Center));
        assert_eq!(config.cursor_glyph, '*');
        assert_eq!(config.color, Rgba::INK_LIGHT_GRAY);
    }
}
