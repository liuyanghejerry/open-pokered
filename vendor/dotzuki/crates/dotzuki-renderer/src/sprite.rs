//! Sprite rendering — draws OAM sprites onto the framebuffer.
//!
//! Game Boy sprites (OBJ) are 8×8 or 8×16 tiles drawn from OAM entries.
//! Each OAM entry has: Y position, X position, tile number, and attributes.
//!
//! Attribute byte bits:
//!   bit 7: BG priority (0=above BG, 1=behind non-zero BG pixels)
//!   bit 6: Y flip
//!   bit 5: X flip
//!   bit 4: Palette (0=OBP0, 1=OBP1)
//!   bits 3-0: unused on DMG
//!
//! The classic Game Boy games use 8×8 sprites assembled into 16×16 characters
//! (4 OAM entries per sprite). Color 0 in sprite palettes is transparent.

use crate::palette::{ColorIndex, GbColor, GbaColor, Palette};
use crate::tile::{TileFormat, TileSet, TILE_PIXELS};
use crate::FbSurface;

/// OAM attribute flags.
pub const OAM_PRIORITY: u8 = 0x80; // bit 7: behind BG
pub const OAM_Y_FLIP: u8 = 0x40; // bit 6: vertical flip
pub const OAM_X_FLIP: u8 = 0x20; // bit 5: horizontal flip
pub const OAM_PALETTE: u8 = 0x10; // bit 4: OBP1 select

/// Maximum number of OAM entries on the Game Boy.
pub const MAX_OAM_ENTRIES: usize = 40;

/// An OAM entry for rendering purposes.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpriteOamEntry {
    /// Y position (screen Y + 16 on real hardware, we store screen Y directly)
    pub y: i32,
    /// X position (screen X + 8 on real hardware, we store screen X directly)
    pub x: i32,
    /// Tile index into sprite tileset
    pub tile_id: u8,
    /// Attribute byte (priority, flip, palette)
    pub attributes: u8,
}

impl SpriteOamEntry {
    pub fn new(y: i32, x: i32, tile_id: u8, attributes: u8) -> Self {
        Self {
            y,
            x,
            tile_id,
            attributes,
        }
    }

    /// Create from raw OAM bytes (hardware format: Y+16, X+8)
    pub fn from_raw(raw_y: u8, raw_x: u8, tile_id: u8, attributes: u8) -> Self {
        Self {
            y: raw_y as i32 - 16,
            x: raw_x as i32 - 8,
            tile_id,
            attributes,
        }
    }

    #[inline]
    pub fn bg_priority(&self) -> bool {
        self.attributes & OAM_PRIORITY != 0
    }

    #[inline]
    pub fn y_flip(&self) -> bool {
        self.attributes & OAM_Y_FLIP != 0
    }

    #[inline]
    pub fn x_flip(&self) -> bool {
        self.attributes & OAM_X_FLIP != 0
    }

    #[inline]
    pub fn uses_obp1(&self) -> bool {
        self.attributes & OAM_PALETTE != 0
    }

    /// Check if the sprite is visible on screen.
    pub fn is_on_screen(&self, screen_width: u32, screen_height: u32) -> bool {
        let x_end = self.x + TILE_PIXELS as i32;
        let y_end = self.y + TILE_PIXELS as i32;
        x_end > 0 && self.x < screen_width as i32 && y_end > 0 && self.y < screen_height as i32
    }
}

/// Sprite layer: holds a list of OAM entries to render.
#[derive(Debug, Clone)]
pub struct SpriteLayer {
    pub entries: Vec<SpriteOamEntry>,
}

impl SpriteLayer {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(MAX_OAM_ENTRIES),
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn add(&mut self, entry: SpriteOamEntry) {
        self.entries.push(entry);
    }

    /// Render all sprites onto the framebuffer.
    ///
    /// `tileset` - sprite tile data
    /// `obp0` / `obp1` - the two sprite palettes
    /// `bg_color_buffer` - optional: if provided, used for BG priority check.
    ///   When a sprite has bg_priority set, it only draws over color-0 BG pixels.
    ///   This buffer stores the BG color index (0-3) for each pixel.
    pub fn render(
        &self,
        fb: &mut impl FbSurface,
        tileset: &TileSet,
        obp0: &Palette,
        obp1: &Palette,
        bg_color_buffer: Option<&[u8]>,
    ) {
        // Render in reverse order (lower index = higher priority on DMG)
        for entry in self.entries.iter().rev() {
            render_sprite(fb, tileset, obp0, obp1, entry, bg_color_buffer);
        }
    }
}

impl Default for SpriteLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a single 8×8 sprite tile.
///
/// Rendering path is chosen based on [`TileSet::tile_format`]:
/// - [`TileFormat::FullColor`]: uses [`TileSet::get_rgba`] directly — no palette.
/// - [`TileFormat::Gb2bpp`]: existing GB path with [`GbColor`] palette lookup (unchanged).
/// - [`TileFormat::Gba4bpp`]: uses [`GbaColor`] palette lookup (accesses `palette.colors` directly).
fn render_sprite(
    fb: &mut impl FbSurface,
    tileset: &TileSet,
    obp0: &Palette,
    obp1: &Palette,
    entry: &SpriteOamEntry,
    bg_color_buffer: Option<&[u8]>,
) {
    if !entry.is_on_screen(fb.width(), fb.height()) {
        return;
    }

    let format = tileset.tile_format();
    let fb_w = fb.width() as usize;
    let fb_h = fb.height() as i32;

    // FullColor path: render directly from RGBA tiles, no palette lookup.
    if format == TileFormat::FullColor {
        if let Some(rgba_tile) = tileset.get_rgba(entry.tile_id as usize) {
            for row in 0..TILE_PIXELS {
                let screen_y = entry.y + row as i32;
                if screen_y < 0 || screen_y >= fb_h {
                    continue;
                }
                let tile_row = if entry.y_flip() {
                    TILE_PIXELS - 1 - row
                } else {
                    row
                };
                for col in 0..TILE_PIXELS {
                    let screen_x = entry.x + col as i32;
                    if screen_x < 0 || screen_x >= fb.width() as i32 {
                        continue;
                    }
                    let tile_col = if entry.x_flip() {
                        TILE_PIXELS - 1 - col
                    } else {
                        col
                    };
                    let rgba = rgba_tile.pixels[tile_row][tile_col];
                    // Transparent pixel
                    if rgba.a == 0 {
                        continue;
                    }
                    // BG priority check
                    if entry.bg_priority() {
                        if let Some(bg_buf) = bg_color_buffer {
                            let pixel_offset = screen_y as usize * fb_w + screen_x as usize;
                            if pixel_offset < bg_buf.len() && bg_buf[pixel_offset] != 0 {
                                continue;
                            }
                        }
                    }
                    fb.set_pixel(screen_x as u32, screen_y as u32, rgba);
                }
            }
        }
        return;
    }

    // Palette-based path (Gb2bpp and Gba4bpp).
    let tile = tileset.get(entry.tile_id as usize);
    let palette = if entry.uses_obp1() { obp1 } else { obp0 };
    let is_gba = format == TileFormat::Gba4bpp;

    for row in 0..TILE_PIXELS {
        let screen_y = entry.y + row as i32;
        if screen_y < 0 || screen_y >= fb_h {
            continue;
        }

        let tile_row = if entry.y_flip() {
            TILE_PIXELS - 1 - row
        } else {
            row
        };

        for col in 0..TILE_PIXELS {
            let screen_x = entry.x + col as i32;
            if screen_x < 0 || screen_x >= fb.width() as i32 {
                continue;
            }

            let tile_col = if entry.x_flip() {
                TILE_PIXELS - 1 - col
            } else {
                col
            };

            let color_idx = tile.get(tile_row, tile_col);

            // Color 0 is always transparent for sprites
            if color_idx == 0 {
                continue;
            }

            // BG priority check: if set, only draw over BG color 0 pixels
            if entry.bg_priority() {
                if let Some(bg_buf) = bg_color_buffer {
                    let pixel_offset = screen_y as usize * fb_w + screen_x as usize;
                    if pixel_offset < bg_buf.len() && bg_buf[pixel_offset] != 0 {
                        continue;
                    }
                }
            }

            let rgba = if is_gba {
                palette.colors[GbaColor::from_u8(color_idx).to_index()]
            } else {
                palette.color(GbColor::from_u8(color_idx))
            };
            fb.set_pixel(screen_x as u32, screen_y as u32, rgba);
        }
    }
}
