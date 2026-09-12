// Re-exported from pokered-data so all existing call sites compile unchanged.
pub use pokered_data::ui_layout::{BracketSides, InkColor, TilePos, TileRect};

// Painter, Ui, Frame, LabelValue live in dotzuki-engine; the painter API takes
// `Rgba` directly (legacy `InkColor` converts via `Into<Rgba>`).
pub use dotzuki_engine::render::painter::{Frame, LabelValue, Painter, Ui};
pub use dotzuki_engine::render::Rgba;

/// Pixel-space region changed by a UI incremental update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl DamageRect {
    /// Current cursor glyphs occupy one tile plus one ink row below it.
    pub const fn cursor(pos: TilePos) -> Self {
        let tile = pokered_data::TILE_SIZE_PX;
        Self {
            x: pos.tx * tile,
            y: pos.ty * tile,
            width: tile,
            height: tile + 1,
        }
    }
}
