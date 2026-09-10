//! Tile map constants and tile cache for background rendering.
//!
//! The Game Boy background map is 32×32 tiles (256×256 pixels).
//! The old 8-bit `TileMap` has been replaced by [`dotzuki_engine::tilemap::Tilemap`];
//! the `pub use` alias below provides backward-compatible naming.
//!
//! [`TileCache`] stores pre-rendered RGBA tiles for a (tileset, palette) pair
//! to avoid per-pixel palette lookups on repeat renders.

use crate::palette::{ColorIndex, GbColor, Palette};
use crate::tile::{RgbaTile, TileFormat, TileSet, TILE_PIXELS};
use crate::TILE_SIZE;

/// Game Boy background map dimensions in tiles.
pub const BG_MAP_WIDTH: u32 = 32;
pub const BG_MAP_HEIGHT: u32 = 32;
/// Background map size in pixels (256×256).
pub const BG_MAP_PIXEL_WIDTH: u32 = BG_MAP_WIDTH * TILE_SIZE;
pub const BG_MAP_PIXEL_HEIGHT: u32 = BG_MAP_HEIGHT * TILE_SIZE;
/// Total tile entries in one background map (1024).
pub const BG_MAP_SIZE: usize = (BG_MAP_WIDTH * BG_MAP_HEIGHT) as usize;

/// Pre-rendered RGBA tile cache for a (tileset, palette) pair.
///
/// Stores each tile from a [`TileSet`] rendered through a [`Palette`]
/// as an [`RgbaTile`], avoiding per-pixel palette lookups on repeat renders.
pub struct TileCache {
    cached: Vec<RgbaTile>,
    valid: bool,
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            cached: Vec::new(),
            valid: false,
        }
    }

    /// Populate the cache from a tileset and palette, unless already valid.
    pub fn ensure(&mut self, tileset: &TileSet, palette: &Palette) {
        if self.valid {
            return;
        }
        let count = tileset.len();
        self.cached.clear();
        self.cached.reserve(count);
        for i in 0..count {
            let tile = tileset.get(i);
            let mut rgba = RgbaTile::blank();
            for row in 0..TILE_PIXELS {
                for col in 0..TILE_PIXELS {
                    let color_idx = tile.pixels[row][col];
                    rgba.pixels[row][col] = palette.color(GbColor::from_u8(color_idx));
                }
            }
            self.cached.push(rgba);
        }
        self.valid = true;
    }

    /// Populate the cache from a tileset and palette using a generic [`ColorIndex`].
    ///
    /// Unlike [`ensure`](Self::ensure), this method is format-aware:
    /// - [`TileFormat::FullColor`]: uses [`TileSet::get_rgba`] directly — no palette lookup.
    /// - [`TileFormat::Gb2bpp`] / [`TileFormat::Gba4bpp`]: applies `palette.color(C::from_u8(...))`.
    pub fn ensure_with_format<C: ColorIndex>(&mut self, tileset: &TileSet, palette: &Palette<C>) {
        if self.valid {
            return;
        }
        let format = tileset.tile_format();
        let count = tileset.len();
        self.cached.clear();
        self.cached.reserve(count);
        for i in 0..count {
            match format {
                TileFormat::FullColor => {
                    if let Some(rgba_tile) = tileset.get_rgba(i) {
                        self.cached.push(rgba_tile.clone());
                    } else {
                        self.cached.push(RgbaTile::blank());
                    }
                }
                _ => {
                    let tile = tileset.get(i);
                    let mut rgba = RgbaTile::blank();
                    for row in 0..TILE_PIXELS {
                        for col in 0..TILE_PIXELS {
                            let color_idx = tile.pixels[row][col];
                            rgba.pixels[row][col] = palette.color(C::from_u8(color_idx));
                        }
                    }
                    self.cached.push(rgba);
                }
            }
        }
        self.valid = true;
    }

    /// Invalidate the cache so the next `ensure` rebuilds it.
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Get a cached RGBA tile by tile index, or `None` if out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&RgbaTile> {
        self.cached.get(index)
    }
}

impl Default for TileCache {
    fn default() -> Self {
        Self::new()
    }
}
