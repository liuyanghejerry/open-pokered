//! `tileset.png` atlas slicing for zero-Rust game maps.
//!
//! A map directory holds a hand-authored `tileset.png`: a row-major atlas of
//! fixed-size tiles referenced by the TMX layers' 1-based Tiled GIDs (GID 0
//! marks an empty cell). [`PngTileset`] decodes the PNG and slices it into
//! per-tile RGBA pixels, offering [`PngTileset::gid_pixel`] — a lookup with
//! the exact shape `dotzuki_renderer::layer_renderer::render_layers_sized`
//! expects from its `tile_color` callback.

use std::path::Path;

use anyhow::{bail, Context, Result};
use dotzuki_engine::render::Rgba;

/// A PNG tileset atlas sliced into per-tile RGBA pixels.
///
/// Tiles are addressed by 1-based Tiled GID and stored flat as
/// `count * tile_w * tile_h` pixels, row-major within each tile. Ported from
/// wuxia's proven `WuxiaTileset`, generalised to rectangular tiles (the TMX
/// `tilewidth`/`tileheight`).
pub struct PngTileset {
    pixels: Vec<Rgba>,
    tile_w: u32,
    tile_h: u32,
    count: usize,
}

impl PngTileset {
    /// Decode `bytes` as a PNG and slice it into `(w/tile_w) * (h/tile_h)`
    /// row-major tiles of `tile_w`×`tile_h` pixels.
    ///
    /// # Errors
    ///
    /// Fails when the PNG cannot be decoded, either tile dimension is zero,
    /// or the image dimensions are not exact multiples of the tile size.
    pub fn from_png_bytes(bytes: &[u8], tile_w: u32, tile_h: u32) -> Result<Self> {
        if tile_w == 0 || tile_h == 0 {
            bail!("tileset tile size must be non-zero (got {tile_w}x{tile_h})");
        }
        let img = image::load_from_memory(bytes)
            .context("failed to decode tileset PNG")?
            .to_rgba8();
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 || w % tile_w != 0 || h % tile_h != 0 {
            bail!("tileset image {w}x{h} is not a multiple of tile size {tile_w}x{tile_h}");
        }
        let rgba: Vec<Rgba> = img
            .pixels()
            .map(|p| Rgba::new(p.0[0], p.0[1], p.0[2], p.0[3]))
            .collect();
        Ok(Self::from_rgba(&rgba, w, h, tile_w, tile_h))
    }

    /// Load and slice a `tileset.png` from disk.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read, or on the same conditions as
    /// [`from_png_bytes`](Self::from_png_bytes).
    pub fn load(path: &Path, tile_w: u32, tile_h: u32) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read tileset {}", path.display()))?;
        Self::from_png_bytes(&bytes, tile_w, tile_h)
            .with_context(|| format!("invalid tileset {}", path.display()))
    }

    /// Slice an already-decoded RGBA image (`w`×`h`, both exact multiples of
    /// the tile size) into row-major tiles.
    fn from_rgba(rgba: &[Rgba], w: u32, h: u32, tile_w: u32, tile_h: u32) -> Self {
        let cols = (w / tile_w) as usize;
        let rows = (h / tile_h) as usize;
        let count = cols * rows;
        let (tw, th) = (tile_w as usize, tile_h as usize);
        let mut pixels = vec![Rgba::TRANSPARENT; count * tw * th];
        for ty in 0..rows {
            for tx in 0..cols {
                let tile_index = ty * cols + tx;
                for py in 0..th {
                    let src_y = ty * th + py;
                    for px in 0..tw {
                        let src_x = tx * tw + px;
                        pixels[tile_index * tw * th + py * tw + px] =
                            rgba[src_y * w as usize + src_x];
                    }
                }
            }
        }
        Self {
            pixels,
            tile_w,
            tile_h,
            count,
        }
    }

    /// Number of tiles in the atlas.
    #[inline]
    pub fn tile_count(&self) -> usize {
        self.count
    }

    /// Tile size in pixels `(width, height)`.
    #[inline]
    pub fn tile_size(&self) -> (u32, u32) {
        (self.tile_w, self.tile_h)
    }

    /// RGBA pixel at intra-tile `(px, py)` of the tile with 1-based Tiled
    /// `gid`. GID 0 (empty cell) and out-of-range GIDs render transparent.
    #[inline]
    pub fn gid_pixel(&self, gid: u16, px: u8, py: u8) -> Rgba {
        if gid == 0 {
            return Rgba::TRANSPARENT;
        }
        let idx = (gid - 1) as usize;
        if idx >= self.count {
            return Rgba::TRANSPARENT;
        }
        let (tw, th) = (self.tile_w as usize, self.tile_h as usize);
        let (px, py) = (px as usize, py as usize);
        if px >= tw || py >= th {
            return Rgba::TRANSPARENT;
        }
        self.pixels[idx * tw * th + py * tw + px]
    }
}
