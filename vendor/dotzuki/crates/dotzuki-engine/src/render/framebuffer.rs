//! Pixel framebuffer with incremental dirty-region tracking.
//!
//! This module provides [`FrameBuffer`], a configurable-resolution RGBA pixel
//! buffer, and [`DirtyRegion`], which tracks which screen areas need
//! re-rendering for incremental update.

use crate::render::Rgba;
use crate::render_config::RenderConfig;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Game Boy tile size in pixels (8×8).
///
/// This is a tile-format constant — not a screen-resolution value.
/// All positions and dimensions in this module are in pixel units,
/// derived from [`RenderConfig`].
pub const TILE_SIZE: u32 = 8;

/// Bytes per pixel in the RGBA framebuffer.
pub const BYTES_PER_PIXEL: usize = 4;

// ---------------------------------------------------------------------------
// DirtyRegion
// ---------------------------------------------------------------------------

/// A rectangular region of the screen that needs redrawing.
///
/// Dirty regions are used to implement incremental rendering: only pixels
/// within dirty regions are re-rendered each frame, skipping unchanged areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirtyRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Whether this region contains any dirty area. When `present` is false,
    /// the entire screen is considered clean (no redraw needed).
    pub present: bool,
}

impl DirtyRegion {
    /// An empty dirty region (nothing to redraw).
    pub fn empty() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            present: false,
        }
    }

    /// A dirty region covering the entire screen.
    pub fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
            present: true,
        }
    }

    /// Create a dirty region at (x, y) with the given dimensions.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            present: true,
        }
    }

    /// Union this region with another, producing the bounding box of both.
    pub fn union(&self, other: &DirtyRegion) -> DirtyRegion {
        if !self.present {
            return *other;
        }
        if !other.present {
            return *self;
        }
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width as i32).max(other.x + other.width as i32);
        let y2 = (self.y + self.height as i32).max(other.y + other.height as i32);
        DirtyRegion::new(x1, y1, (x2 - x1).max(0) as u32, (y2 - y1).max(0) as u32)
    }

    /// Check whether a pixel at (px, py) is within this dirty region.
    /// Returns `false` when the region is not present (nothing to redraw).
    #[inline]
    pub fn contains_pixel(&self, px: u32, py: u32) -> bool {
        if !self.present {
            return false;
        }
        px as i32 >= self.x
            && (px as i32) < self.x + self.width as i32
            && py as i32 >= self.y
            && (py as i32) < self.y + self.height as i32
    }

    /// Convert the dirty region to tile-space coordinates, rounding outward.
    /// Returns (tile_x, tile_y, tile_width, tile_height).
    pub fn to_tile_rect(&self, tile_size: u32) -> (i32, i32, u32, u32) {
        if !self.present {
            return (0, 0, 0, 0);
        }
        let ts = tile_size as i32;
        let tx = if self.x >= 0 {
            self.x / ts
        } else {
            (self.x - ts + 1) / ts
        };
        let ty = if self.y >= 0 {
            self.y / ts
        } else {
            (self.y - ts + 1) / ts
        };
        let right = self.x + self.width as i32;
        let bottom = self.y + self.height as i32;
        let tw = ((right + ts - 1) / ts - tx).max(0) as u32;
        let th = ((bottom + ts - 1) / ts - ty).max(0) as u32;
        (tx, ty, tw, th)
    }
}

impl Default for DirtyRegion {
    fn default() -> Self {
        Self::empty()
    }
}

// ---------------------------------------------------------------------------
// FrameBuffer
// ---------------------------------------------------------------------------

/// A pixel RGBA framebuffer with configurable dimensions.
///
/// The internal buffer is a flat array of RGBA bytes in row-major order.
/// Pixel (x, y) starts at byte offset `(y * width + x) * 4`.
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    /// Raw RGBA pixel data, `width * height * 4` bytes.
    pub data: Vec<u8>,
    /// Screen width in pixels.
    pub width: u32,
    /// Screen height in pixels.
    pub height: u32,
    /// Accumulated dirty region for incremental rendering.
    /// Callers mark areas that need redrawing; render functions
    /// may skip pixels outside this region.
    pub dirty_region: DirtyRegion,
}

impl FrameBuffer {
    /// Create a new framebuffer with the given render config, cleared to the given color.
    pub fn new(config: RenderConfig, clear_color: Rgba) -> Self {
        let fb_size =
            (config.screen_width as usize) * (config.screen_height as usize) * BYTES_PER_PIXEL;
        let mut fb = Self {
            data: vec![0; fb_size],
            width: config.screen_width,
            height: config.screen_height,
            dirty_region: DirtyRegion::full(config.screen_width, config.screen_height),
        };
        fb.clear(clear_color);
        fb
    }

    /// Mark the entire framebuffer as dirty (force full redraw).
    pub fn mark_all_dirty(&mut self) {
        self.dirty_region = DirtyRegion::full(self.width, self.height);
    }

    /// Mark a rectangular region as dirty.
    pub fn mark_dirty_rect(&mut self, x: i32, y: i32, w: u32, h: u32) {
        let r = DirtyRegion::new(x, y, w, h);
        self.dirty_region = self.dirty_region.union(&r);
    }

    /// Mark a tile as dirty given its tile coordinates (tx, ty).
    pub fn mark_dirty_tile(&mut self, tx: i32, ty: i32) {
        let tile_size = TILE_SIZE as i32;
        self.mark_dirty_rect(tx * tile_size, ty * tile_size, TILE_SIZE, TILE_SIZE);
    }

    /// Clear all dirty regions (nothing to redraw).
    pub fn clear_dirty(&mut self) {
        self.dirty_region = DirtyRegion::empty();
    }

    /// Check whether a pixel is in a dirty region (needs redrawing).
    /// If no dirty region is present, all pixels are considered dirty.
    #[inline]
    pub fn is_dirty_pixel(&self, x: u32, y: u32) -> bool {
        self.dirty_region.contains_pixel(x, y)
    }

    /// Returns the width of this framebuffer in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of this framebuffer in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Clear the entire framebuffer to a single color.
    pub fn clear(&mut self, color: Rgba) {
        let rgba = color.to_array();
        for pixel in self.data.chunks_exact_mut(BYTES_PER_PIXEL) {
            pixel.copy_from_slice(&rgba);
        }
    }

    /// Set a single pixel. Returns false if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Rgba) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let offset = ((y as usize) * (self.width as usize) + (x as usize)) * BYTES_PER_PIXEL;
        self.data[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&color.to_array());
        true
    }

    /// Get the color of a single pixel. Returns None if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = ((y as usize) * (self.width as usize) + (x as usize)) * BYTES_PER_PIXEL;
        let mut c = [0u8; 4];
        c.copy_from_slice(&self.data[offset..offset + BYTES_PER_PIXEL]);
        Some(Rgba::from(c))
    }

    /// Fill a rectangular region with a color. Coordinates are clamped to screen bounds.
    pub fn fill_rect(&mut self, x: u32, y: u32, rect_width: u32, rect_height: u32, color: Rgba) {
        let x_start = x.min(self.width);
        let y_start = y.min(self.height);
        let x_end = (x + rect_width).min(self.width);
        let y_end = (y + rect_height).min(self.height);
        let rgba = color.to_array();

        for row in y_start..y_end {
            let row_offset = (row as usize) * (self.width as usize) * BYTES_PER_PIXEL;
            for col in x_start..x_end {
                let offset = row_offset + (col as usize) * BYTES_PER_PIXEL;
                self.data[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&rgba);
            }
        }
    }

    /// Get a slice of one pixel row's RGBA data. Returns None if y is out of bounds.
    pub fn row_slice(&self, y: u32) -> Option<&[u8]> {
        if y >= self.height {
            return None;
        }
        let start = (y as usize) * (self.width as usize) * BYTES_PER_PIXEL;
        let end = start + (self.width as usize) * BYTES_PER_PIXEL;
        Some(&self.data[start..end])
    }

    /// Get a mutable slice of one pixel row's RGBA data. Returns None if y is out of bounds.
    pub fn row_slice_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        if y >= self.height {
            return None;
        }
        let start = (y as usize) * (self.width as usize) * BYTES_PER_PIXEL;
        let end = start + (self.width as usize) * BYTES_PER_PIXEL;
        Some(&mut self.data[start..end])
    }

    /// Copy a horizontal line of RGBA data into the framebuffer.
    /// `src` must be exactly `count * 4` bytes.
    /// Returns false if the line goes out of bounds.
    pub fn blit_row(&mut self, x: u32, y: u32, src: &[u8], count: u32) -> bool {
        if y >= self.height || x >= self.width {
            return false;
        }
        let actual_count = count.min(self.width - x) as usize;
        let src_bytes = actual_count * BYTES_PER_PIXEL;
        if src.len() < src_bytes {
            return false;
        }
        let offset = ((y as usize) * (self.width as usize) + (x as usize)) * BYTES_PER_PIXEL;
        self.data[offset..offset + src_bytes].copy_from_slice(&src[..src_bytes]);
        true
    }

    /// Save the framebuffer as a PNG file.
    ///
    /// Uses the `image` crate to encode the raw RGBA data.
    ///
    /// Host-only (behind the `image` feature): bare-metal targets have no
    /// filesystem and no std::io; a GBA build never enables this.
    #[cfg(feature = "image")]
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        use image::{ImageBuffer, Rgba as ImgRgba};
        let img: ImageBuffer<ImgRgba<u8>, _> =
            ImageBuffer::from_raw(self.width, self.height, self.data.clone())
                .expect("FrameBuffer data size mismatch");
        img.save(path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}
