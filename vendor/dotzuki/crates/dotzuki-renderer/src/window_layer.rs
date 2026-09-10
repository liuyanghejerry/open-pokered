//! Window layer — the Game Boy's WINDOW overlay.
//!
//! The Game Boy WINDOW is an overlay that can cover part of the screen.
//! It has its own tile map and is positioned via WX (window X + 7) and WY.
//! When enabled, it replaces the BG for all pixels at and below WY,
//! starting from screen column (WX - 7).
//!
//! Unlike BG, the window does NOT wrap — it renders from top-left of its
//! own tile map, with no scroll offset.

use crate::palette::Palette;
use crate::tile::TileSet;
use crate::tilemap::BG_MAP_PIXEL_HEIGHT;
use crate::tilemap::BG_MAP_PIXEL_WIDTH;
use crate::{FbSurface, TILE_SIZE};
use dotzuki_engine::tilemap::Tilemap;

/// Window layer state.
#[derive(Debug, Clone)]
pub struct WindowLayer {
    /// The window's tile map (same format as BG: 32×32 entries).
    pub tilemap: Tilemap,
    /// Window X position register (WX). The window starts at screen column (wx - 7).
    /// Valid range: 0–166. A value of 7 means the window starts at screen column 0.
    pub wx: u32,
    /// Window Y position register (WY). The window starts at this screen row.
    /// Valid range: 0–143.
    pub wy: u32,
    /// Whether the window layer is enabled.
    pub enabled: bool,
}

impl WindowLayer {
    pub fn new() -> Self {
        Self {
            tilemap: Tilemap::new(32, 32),
            wx: 7, // Default: window at left edge
            wy: 0, // Default: window at top
            enabled: false,
        }
    }

    /// The actual screen X where the window starts.
    /// WX=7 means column 0. WX<7 is clamped to 0.
    #[inline]
    pub fn screen_x(&self) -> u32 {
        if self.wx < 7 {
            0
        } else {
            self.wx - 7
        }
    }

    /// Render the window layer on top of the existing framebuffer content.
    /// Only renders pixels where the window is visible (at and below WY,
    /// at and right of WX-7).
    pub fn render(&self, fb: &mut impl FbSurface, tileset: &TileSet, palette: &Palette) {
        if !self.enabled {
            return;
        }

        let win_start_x = self.screen_x();
        if win_start_x >= fb.width() || self.wy >= fb.height() {
            return;
        }

        for screen_y in self.wy..fb.height() {
            self.render_scanline(fb, tileset, palette, screen_y);
        }
    }

    /// Render a single scanline of the window layer.
    pub fn render_scanline(
        &self,
        fb: &mut impl FbSurface,
        tileset: &TileSet,
        palette: &Palette,
        screen_y: u32,
    ) {
        if !self.enabled || screen_y < self.wy {
            return;
        }

        let win_start_x = self.screen_x();
        if win_start_x >= fb.width() {
            return;
        }

        // Window-space coordinates: the window renders from its own (0,0)
        let win_y = screen_y - self.wy;
        if win_y >= BG_MAP_PIXEL_HEIGHT {
            return;
        }

        let tile_row = win_y / TILE_SIZE;
        let pixel_row = (win_y % TILE_SIZE) as usize;

        for screen_x in win_start_x..fb.width() {
            let win_x = screen_x - win_start_x;
            if win_x >= BG_MAP_PIXEL_WIDTH {
                break;
            }

            let tile_col = win_x / TILE_SIZE;
            let pixel_col = (win_x % TILE_SIZE) as usize;

            let tile_index = self
                .tilemap
                .get(tile_col as u16, tile_row as u16)
                .map(|e| e.tile_id as usize)
                .unwrap_or(0);
            let tile = tileset.get(tile_index);
            let color_idx = tile.get(pixel_row, pixel_col);

            let rgba = palette.color(crate::palette::GbColor::from_u8(color_idx));
            fb.set_pixel(screen_x, screen_y, rgba);
        }
    }
}

impl Default for WindowLayer {
    fn default() -> Self {
        Self::new()
    }
}
