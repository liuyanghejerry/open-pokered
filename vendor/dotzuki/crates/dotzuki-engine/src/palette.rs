use core::fmt::Debug;
use core::hash::Hash;

// ============================================================================
// SGB Color Types (shared between engine, data, and renderer)
// ============================================================================

/// A single SGB color in 5-bit-per-channel RGB format.
/// Stored as the original 5-bit values (0-31 per channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl SgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// An SGB palette entry: 4 colors (color0..color3).
/// color0 is typically the lightest, color3 the darkest.
pub type SgbPaletteEntry = [SgbColor; 4];

// ============================================================================
// Palette Trait & Provider
// ============================================================================

/// Marker trait for palette identifiers.
///
/// Implementations are typically lightweight enums or numeric IDs
/// that uniquely identify a colour palette used for rendering
/// backgrounds, sprites, or UI elements.
pub trait PaletteTrait: Copy + Eq + Hash + Debug + 'static {}

/// Provider trait that supplies palette data to the engine.
///
/// The renderer queries this provider to obtain the correct colour
/// palette for the current scene, overworld map, or monster sprite.
pub trait PaletteProvider<P: PaletteTrait> {
    /// Returns the 4-colour background palette as an array of colour indices.
    fn bg_palette(&self, palette: P) -> [u8; 4];

    /// Returns the first 4-colour object (sprite) palette.
    fn obj_palette0(&self, palette: P) -> [u8; 4];

    /// Returns the second 4-colour object (sprite) palette.
    fn obj_palette1(&self, palette: P) -> [u8; 4];

    /// Returns the overworld palette for a given tileset and map combination.
    ///
    /// The `last_map` parameter is used for palette transition smoothing
    /// when the player moves between maps with different palettes.
    fn overworld_palette_for(&self, tileset_id: u8, map_id: u8, last_map: u8) -> P;

    /// Returns the palette used to colour a monster sprite.
    fn monster_palette(&self, species_index: u8) -> P;

    /// Look up the SGB palette entry (4 SGB colors) for the given palette ID.
    ///
    /// Used by SGB rendering mode to convert palette IDs to actual colors.
    /// Default returns a black fallback.
    fn sgb_palette_data(&self, _id: P, _is_red: bool) -> SgbPaletteEntry {
        [SgbColor::new(0, 0, 0); 4]
    }

    /// Convert HP bar color index (0=green, 1=yellow, 2=red) to a palette ID.
    fn hp_bar_to_palette_id(&self, hp_bar_color: u8) -> P;
}
