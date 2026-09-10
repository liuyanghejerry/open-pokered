/// Tile-grid position (8×8 pixel tiles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct TilePos {
    /// Column in tile units (0-based).
    pub tx: u32,
    /// Row in tile units (0-based).
    pub ty: u32,
}

impl TilePos {
    /// Create a new tile position.
    #[inline]
    pub const fn new(tx: u32, ty: u32) -> Self {
        Self { tx, ty }
    }

    /// Convert to pixel coordinates (each tile is 8×8 pixels).
    #[inline]
    pub const fn to_pixels(self) -> (u32, u32) {
        (self.tx * 8, self.ty * 8)
    }
}

/// Tile-grid rectangle (position + extent in tile units).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct TileRect {
    /// Left column in tile units.
    pub tx: u32,
    /// Top row in tile units.
    pub ty: u32,
    /// Width in tile units.
    pub tw: u32,
    /// Height in tile units.
    pub th: u32,
}

impl TileRect {
    /// Create a new tile rectangle.
    #[inline]
    pub const fn new(tx: u32, ty: u32, tw: u32, th: u32) -> Self {
        Self { tx, ty, tw, th }
    }

    /// Return the top-left corner as a [`TilePos`].
    #[inline]
    pub const fn pos(&self) -> TilePos {
        TilePos::new(self.tx, self.ty)
    }

    /// Return a rectangle shifted by `(dx, dy)` tiles.
    #[inline]
    pub fn translated(&self, dx: u32, dy: u32) -> Self {
        Self::new(self.tx + dx, self.ty + dy, self.tw, self.th)
    }
}

/// Bitmap of which sides of a bracket box should be drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BracketSides {
    /// Draw the top edge.
    pub top: bool,
    /// Draw the bottom edge.
    pub bottom: bool,
    /// Draw the left edge.
    pub left: bool,
    /// Draw the right edge.
    pub right: bool,
}

impl BracketSides {
    /// Only the right and bottom edges (corner bracket).
    pub const RIGHT_BOTTOM: Self = Self {
        top: false,
        bottom: true,
        left: false,
        right: true,
    };
    /// All four edges.
    pub const ALL: Self = Self {
        top: true,
        bottom: true,
        left: true,
        right: true,
    };
}
