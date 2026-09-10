//! Game Boy-style tile border rendering.
//!
//! A [`Border`] draws a bordered rectangle using configurable tile indices.
//! Each of the 4 corners, 4 edge segments, and the fill area can use
//! independent tile IDs. When no tile style is set (`tiles` is `None`),
//! the entire rectangle is filled with a solid background color.

use dotzuki_engine::render::{Painter, Rgba, TilePos, TileRect};

// ── BorderTiles ──────────────────────────────────────────────────────────

/// Tile indices for a Game Boy-style box border.
///
/// Each field corresponds to a specific position in the border grid:
///
/// ```text
/// ┌──────┬──────┬──────┬──────┬──────┐
/// │ tl   │ top  │ top  │ top  │ tr   │
/// ├──────┼──────┼──────┼──────┼──────┤
/// │ left │ fill │ fill │ fill │ right│
/// ├──────┼──────┼──────┼──────┼──────┤
/// │ left │ fill │ fill │ fill │ right│
/// ├──────┼──────┼──────┼──────┼──────┤
/// │ bl   │ bot  │ bot  │ bot  │ br   │
/// └──────┴──────┴──────┴──────┴──────┘
/// ```
///
/// Defaults match the box-drawing tile IDs from the embedded font
/// (0x79–0x7F).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderTiles {
    /// Top-left corner tile.
    pub top_left: u8,
    /// Top edge tile (repeated across the top).
    pub top: u8,
    /// Top-right corner tile.
    pub top_right: u8,
    /// Left edge tile (repeated down the left side).
    pub left: u8,
    /// Right edge tile (repeated down the right side).
    pub right: u8,
    /// Bottom-left corner tile.
    pub bottom_left: u8,
    /// Bottom edge tile (repeated across the bottom).
    pub bottom: u8,
    /// Bottom-right corner tile.
    pub bottom_right: u8,
    /// Fill tile for the interior.
    pub fill: u8,
}

impl Default for BorderTiles {
    fn default() -> Self {
        Self {
            top_left: 0x79,     // ┌
            top: 0x7A,          // ─
            top_right: 0x7B,    // ┐
            left: 0x7C,         // │
            right: 0x7C,        // │
            bottom_left: 0x7D,  // └
            bottom: 0x7A,       // ─
            bottom_right: 0x7E, // ┘
            fill: 0x7F,         // space
        }
    }
}

// ── Border ───────────────────────────────────────────────────────────────

/// A Game Boy-style bordered rectangle.
///
/// # Examples
///
/// ```
/// use dotzuki_renderer::layout_engine::elements::border::{Border, BorderTiles};
/// use dotzuki_engine::render::{TileRect, Rgba};
///
/// let rect = TileRect::new(1, 1, 10, 5);
/// let border = Border::new(rect, Rgba::INK_BLACK);
///
/// // Use custom tiles if needed
/// let custom_tiles = BorderTiles {
///     top_left: 0x01,
///     top_right: 0x02,
///     bottom_left: 0x03,
///     bottom_right: 0x04,
///     ..BorderTiles::default()
/// };
/// let styled = Border::with_tiles(rect, custom_tiles, Rgba::INK_DARK_GRAY);
/// ```
#[derive(Debug, Clone)]
pub struct Border {
    /// The area this border occupies (in tiles).
    pub rect: TileRect,
    /// Optional tile indices for the border and fill.
    /// When `None`, the border renders as a solid color fill.
    pub tiles: Option<BorderTiles>,
    /// The ink color used for drawing tiles or the solid fill.
    pub color: Rgba,
}

impl Border {
    /// Create a border with default tile indices.
    #[inline]
    pub fn new(rect: TileRect, color: Rgba) -> Self {
        Self {
            rect,
            tiles: Some(BorderTiles::default()),
            color,
        }
    }

    /// Create a border with custom tile indices.
    #[inline]
    pub fn with_tiles(rect: TileRect, tiles: BorderTiles, color: Rgba) -> Self {
        Self {
            rect,
            tiles: Some(tiles),
            color,
        }
    }

    /// Create a borderless fill (no tiles, just a solid color rectangle).
    ///
    /// Useful as a background fill behind other elements.
    #[inline]
    pub fn fill(rect: TileRect, color: Rgba) -> Self {
        Self {
            rect,
            tiles: None,
            color,
        }
    }

    /// Whether this border has a tile style set.
    #[inline]
    pub fn has_style(&self) -> bool {
        self.tiles.is_some()
    }

    // ── Rendering ──────────────────────────────────────────────────────

    /// Render this border into the given [`Painter`].
    ///
    /// When `tiles` is `Some`, draws the border using tile IDs:
    /// corners, edges, and fill. When `tiles` is `None`, fills the
    /// entire rectangle with `color` using a single pixel-rect call.
    pub fn render(&self, painter: &mut dyn Painter) {
        let rect = self.rect;

        match &self.tiles {
            Some(tiles) => {
                // Corners (drawn even for 1×1 rects)
                painter.draw_gb_tile(
                    TilePos::new(rect.tx, rect.ty),
                    tiles.top_left,
                    " ",
                    self.color,
                );
                if rect.tw > 1 {
                    painter.draw_gb_tile(
                        TilePos::new(rect.tx + rect.tw - 1, rect.ty),
                        tiles.top_right,
                        " ",
                        self.color,
                    );
                }
                if rect.th > 1 {
                    painter.draw_gb_tile(
                        TilePos::new(rect.tx, rect.ty + rect.th - 1),
                        tiles.bottom_left,
                        " ",
                        self.color,
                    );
                }
                if rect.tw > 1 && rect.th > 1 {
                    painter.draw_gb_tile(
                        TilePos::new(rect.tx + rect.tw - 1, rect.ty + rect.th - 1),
                        tiles.bottom_right,
                        " ",
                        self.color,
                    );
                }

                // Top edge
                for x in 1..rect.tw.saturating_sub(1) {
                    painter.draw_gb_tile(
                        TilePos::new(rect.tx + x, rect.ty),
                        tiles.top,
                        " ",
                        self.color,
                    );
                }

                // Bottom edge
                if rect.th > 1 {
                    for x in 1..rect.tw.saturating_sub(1) {
                        painter.draw_gb_tile(
                            TilePos::new(rect.tx + x, rect.ty + rect.th - 1),
                            tiles.bottom,
                            " ",
                            self.color,
                        );
                    }
                }

                // Left edge
                for y in 1..rect.th.saturating_sub(1) {
                    painter.draw_gb_tile(
                        TilePos::new(rect.tx, rect.ty + y),
                        tiles.left,
                        " ",
                        self.color,
                    );
                }

                // Right edge
                if rect.tw > 1 {
                    for y in 1..rect.th.saturating_sub(1) {
                        painter.draw_gb_tile(
                            TilePos::new(rect.tx + rect.tw - 1, rect.ty + y),
                            tiles.right,
                            " ",
                            self.color,
                        );
                    }
                }

                // Fill interior
                for y in 1..rect.th.saturating_sub(1) {
                    for x in 1..rect.tw.saturating_sub(1) {
                        painter.draw_gb_tile(
                            TilePos::new(rect.tx + x, rect.ty + y),
                            tiles.fill,
                            " ",
                            self.color,
                        );
                    }
                }
            }
            None => {
                // Solid color fill — no border tiles
                painter.draw_pixel_rect(
                    rect.tx * 8,
                    rect.ty * 8,
                    rect.tw * 8,
                    rect.th * 8,
                    self.color,
                );
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::render::TileRect;

    // ── BorderTiles tests ──────────────────────────────────────────────

    #[test]
    fn default_tiles_match_textbox_constants() {
        let t = BorderTiles::default();
        assert_eq!(t.top_left, 0x79);
        assert_eq!(t.top, 0x7A);
        assert_eq!(t.top_right, 0x7B);
        assert_eq!(t.left, 0x7C);
        assert_eq!(t.right, 0x7C);
        assert_eq!(t.bottom_left, 0x7D);
        assert_eq!(t.bottom, 0x7A);
        assert_eq!(t.bottom_right, 0x7E);
        assert_eq!(t.fill, 0x7F);
    }

    #[test]
    fn custom_tiles_support() {
        let t = BorderTiles {
            top_left: 0x01,
            top: 0x02,
            top_right: 0x03,
            left: 0x04,
            right: 0x05,
            bottom_left: 0x06,
            bottom: 0x07,
            bottom_right: 0x08,
            fill: 0x09,
        };
        assert_eq!(t.top_left, 0x01);
        assert_eq!(t.right, 0x05);
        assert_eq!(t.fill, 0x09);
    }

    #[test]
    fn border_tiles_copy_and_eq() {
        let a = BorderTiles::default();
        let b = a;
        assert_eq!(a, b);

        let mut c = a;
        c.top_left = 0x00;
        assert_ne!(a, c);
    }

    // ── Border construction tests ─────────────────────────────────────

    #[test]
    fn new_border_has_default_tiles() {
        let rect = TileRect::new(0, 0, 10, 5);
        let b = Border::new(rect, Rgba::INK_BLACK);
        assert!(b.has_style());
        assert_eq!(b.tiles, Some(BorderTiles::default()));
        assert_eq!(b.rect, rect);
    }

    #[test]
    fn with_tiles_sets_custom_tiles() {
        let rect = TileRect::new(2, 3, 8, 4);
        let tiles = BorderTiles {
            top_left: 0x10,
            ..BorderTiles::default()
        };
        let b = Border::with_tiles(rect, tiles, Rgba::INK_DARK_GRAY);
        assert!(b.has_style());
        assert_eq!(b.tiles.unwrap().top_left, 0x10);
    }

    #[test]
    fn fill_border_has_no_style() {
        let rect = TileRect::new(0, 0, 20, 18);
        let b = Border::fill(rect, Rgba::INK_WHITE);
        assert!(!b.has_style());
        assert_eq!(b.tiles, None);
        assert_eq!(b.color, Rgba::INK_WHITE);
    }

    #[test]
    fn has_style_reflects_tiles_presence() {
        let rect = TileRect::new(0, 0, 4, 4);

        let styled = Border::new(rect, Rgba::INK_BLACK);
        assert!(styled.has_style());

        let fill_only = Border::fill(rect, Rgba::INK_WHITE);
        assert!(!fill_only.has_style());
    }

    #[test]
    fn minimal_1x1_border() {
        // A 1×1 border should just draw the top-left corner
        let rect = TileRect::new(5, 5, 1, 1);
        let b = Border::new(rect, Rgba::INK_BLACK);
        assert!(b.has_style());
        assert_eq!(b.rect.tw, 1);
        assert_eq!(b.rect.th, 1);
    }

    #[test]
    fn thin_horizontal_border() {
        // 10-wide × 1-high: just top edge between two corners
        let rect = TileRect::new(0, 0, 10, 1);
        let b = Border::new(rect, Rgba::INK_BLACK);
        assert_eq!(b.rect.tw, 10);
        assert_eq!(b.rect.th, 1);
    }

    #[test]
    fn thin_vertical_border() {
        // 1-wide × 5-high: just left edge between two corners
        let rect = TileRect::new(0, 0, 1, 5);
        let b = Border::new(rect, Rgba::INK_BLACK);
        assert_eq!(b.rect.tw, 1);
        assert_eq!(b.rect.th, 5);
    }
}
