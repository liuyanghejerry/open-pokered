// Minimal resource loading abstraction for dotzuki-renderer.
// Games provide a ResourceProvider implementation for asset loading.

use crate::tile::TileSet;

/// Trait for loading graphical assets.
///
/// The game-specific implementation handles filesystem access,
/// PNG decoding, and caching.
pub trait ResourceProvider {
    /// Load a tileset from a category directory and filename.
    /// Returns a reference to a cached TileSet.
    fn load_asset(&mut self, category: &str, filename: &str) -> Result<&TileSet, String>;

    /// Load a tileset with 2bpp decoding (for font assets).
    fn load_asset_2bpp(&mut self, category: &str, filename: &str) -> Result<&TileSet, String>;

    /// Load a font tileset.
    fn load_font(&mut self, name: &str) -> Result<&TileSet, String>;
}
