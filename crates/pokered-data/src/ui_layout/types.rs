pub use dotzuki_engine::render::{BracketSides, TilePos, TileRect};

use dotzuki_engine::render::Rgba;

/// Legacy ink color used by the v1 layout schema and v1 menus.
///
/// The original Game Boy 4-shade palette plus three HP-bar shades. The engine
/// painter API takes [`Rgba`] directly; this enum survives only because the
/// v1 `ui_layouts/v1/*.json` schema serializes color names (`"Black"`,
/// `"DarkGray"`, …). It is deleted together with the v1 menu stack once the
/// v2 `.gui` migration completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum InkColor {
    #[default]
    Black,
    DarkGray,
    LightGray,
    White,
    HpFull,
    HpCaution,
    HpCritical,
}

impl InkColor {
    /// The RGBA value this shade has always rendered as.
    pub const fn to_rgba(self) -> Rgba {
        match self {
            InkColor::Black => Rgba::INK_BLACK,
            InkColor::DarkGray => Rgba::INK_DARK_GRAY,
            InkColor::LightGray => Rgba::INK_LIGHT_GRAY,
            InkColor::White => Rgba::INK_WHITE,
            InkColor::HpFull => Rgba::rgb(0x20, 0x20, 0x20),
            InkColor::HpCaution => Rgba::rgb(0x70, 0x70, 0x70),
            InkColor::HpCritical => Rgba::rgb(0x40, 0x40, 0x40),
        }
    }
}

impl From<InkColor> for Rgba {
    fn from(c: InkColor) -> Rgba {
        c.to_rgba()
    }
}
