// Re-exported from pokered-data so all existing call sites compile unchanged.
pub use pokered_data::ui_layout::{BracketSides, InkColor, TilePos, TileRect};

// Painter, Ui, Frame, LabelValue live in dotzuki-engine; the painter API takes
// `Rgba` directly (legacy `InkColor` converts via `Into<Rgba>`).
pub use dotzuki_engine::render::painter::{Frame, LabelValue, Painter, Ui};
pub use dotzuki_engine::render::Rgba;
