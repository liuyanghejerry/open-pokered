//! Tile decoding and tileset management.
//!
//! Game Boy tiles are 8×8 pixels, stored in 2bpp (2 bits per pixel) format.
//! Each row is 2 bytes: the low bit-plane and the high bit-plane.
//! Pixel colors are indices 0–3 into a palette.

use crate::palette::{GbColor, Palette};
use dotzuki_engine::render::Rgba;

/// Number of bytes per tile row in 2bpp format (low byte + high byte).
pub const BYTES_PER_TILE_ROW: usize = 2;
/// Total bytes per 8×8 tile in 2bpp format.
pub const BYTES_PER_TILE: usize = 16;
/// Number of pixels per tile side.
pub const TILE_PIXELS: usize = 8;

/// Tile data format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileFormat {
    /// Game Boy 2bpp: 2 bits per pixel, 4 colors (0–3), 16 bytes per tile.
    Gb2bpp,
    /// GBA-style 4bpp: 4 bits per pixel, 16 colors (0–15), 32 bytes per tile.
    /// Uses 2 bitplanes × 16 bytes each.
    Gba4bpp,
    /// Direct RGBA: each pixel stored as 4 bytes (R, G, B, A), 256 bytes per tile.
    FullColor,
}

/// A decoded 8×8 tile. Each element is a color index (0–3).
#[derive(Debug, Clone)]
pub struct Tile {
    /// 8 rows × 8 columns of palette indices (0–3).
    /// Indexed as `pixels[row][col]`.
    pub pixels: [[u8; TILE_PIXELS]; TILE_PIXELS],
}

impl Tile {
    /// Decode a tile from 16 bytes of 2bpp data.
    ///
    /// Each row is 2 bytes: `low_byte` then `high_byte`.
    /// Bit 7 = leftmost pixel. The color index for pixel `x` is:
    ///   `((high_byte >> (7-x)) & 1) << 1 | ((low_byte >> (7-x)) & 1)`
    pub fn from_2bpp(data: &[u8]) -> Self {
        assert!(
            data.len() >= BYTES_PER_TILE,
            "Need {} bytes for a tile, got {}",
            BYTES_PER_TILE,
            data.len()
        );
        let mut pixels = [[0u8; TILE_PIXELS]; TILE_PIXELS];
        for row in 0..TILE_PIXELS {
            let lo = data[row * 2];
            let hi = data[row * 2 + 1];
            for col in 0..TILE_PIXELS {
                let bit = 7 - col;
                let color_index = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);
                pixels[row][col] = color_index;
            }
        }
        Self { pixels }
    }

    /// Create a blank (all color 0) tile.
    pub fn blank() -> Self {
        Self {
            pixels: [[0; TILE_PIXELS]; TILE_PIXELS],
        }
    }

    /// Get the color index at (row, col).
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> u8 {
        self.pixels[row][col]
    }

    /// Render this tile's row into RGBA pixels using a palette.
    /// Returns 8 RGBA values for the given tile row.
    pub fn render_row(&self, row: usize, palette: &Palette) -> [Rgba; TILE_PIXELS] {
        let mut out = [Rgba::TRANSPARENT; TILE_PIXELS];
        for col in 0..TILE_PIXELS {
            let color_idx = GbColor::from_u8(self.pixels[row][col]);
            out[col] = palette.color(color_idx);
        }
        out
    }

    /// Check if this tile is vertically flipped.
    pub fn flip_y(&self) -> Tile {
        let mut pixels = [[0u8; TILE_PIXELS]; TILE_PIXELS];
        for row in 0..TILE_PIXELS {
            pixels[row] = self.pixels[TILE_PIXELS - 1 - row];
        }
        Tile { pixels }
    }

    /// Check if this tile is horizontally flipped.
    pub fn flip_x(&self) -> Tile {
        let mut pixels = [[0u8; TILE_PIXELS]; TILE_PIXELS];
        for row in 0..TILE_PIXELS {
            for col in 0..TILE_PIXELS {
                pixels[row][col] = self.pixels[row][TILE_PIXELS - 1 - col];
            }
        }
        Tile { pixels }
    }
}

/// A set of decoded tiles, indexed by tile number.
#[derive(Debug, Clone)]
pub struct TileSet {
    tiles: Vec<Tile>,
    format: TileFormat,
    rgba_tiles: Vec<RgbaTile>,
}

impl TileSet {
    /// Create a tileset by decoding 2bpp tile data.
    /// The data length must be a multiple of 16 (bytes per tile).
    pub fn from_2bpp(data: &[u8]) -> Self {
        assert!(
            data.len() % BYTES_PER_TILE == 0,
            "Tile data length {} is not a multiple of {}",
            data.len(),
            BYTES_PER_TILE,
        );
        let count = data.len() / BYTES_PER_TILE;
        let mut tiles = Vec::with_capacity(count);
        for i in 0..count {
            let start = i * BYTES_PER_TILE;
            tiles.push(Tile::from_2bpp(&data[start..start + BYTES_PER_TILE]));
        }
        Self {
            tiles,
            format: TileFormat::Gb2bpp,
            rgba_tiles: Vec::new(),
        }
    }

    /// Create an empty tileset with `count` blank tiles.
    pub fn blank(count: usize) -> Self {
        Self {
            tiles: vec![Tile::blank(); count],
            format: TileFormat::Gb2bpp,
            rgba_tiles: Vec::new(),
        }
    }

    /// Get a tile by index. Returns blank tile if out of bounds.
    pub fn get(&self, index: usize) -> &Tile {
        if index < self.tiles.len() {
            &self.tiles[index]
        } else {
            // Return a static blank tile for out-of-bounds access
            // This matches GB behavior where VRAM reads wrap
            static BLANK: Tile = Tile {
                pixels: [[0; TILE_PIXELS]; TILE_PIXELS],
            };
            &BLANK
        }
    }

    /// Number of tiles in this set.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Whether this tileset is empty.
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Replace a tile at the given index.
    pub fn set(&mut self, index: usize, tile: Tile) {
        if index < self.tiles.len() {
            self.tiles[index] = tile;
        }
    }

    /// Load raw 2bpp data into the tileset starting at tile index `start_tile`.
    /// Overwrites existing tiles.
    pub fn load_2bpp_at(&mut self, start_tile: usize, data: &[u8]) {
        let count = data.len() / BYTES_PER_TILE;
        for i in 0..count {
            let tile_idx = start_tile + i;
            if tile_idx >= self.tiles.len() {
                break;
            }
            let start = i * BYTES_PER_TILE;
            self.tiles[tile_idx] = Tile::from_2bpp(&data[start..start + BYTES_PER_TILE]);
        }
    }

    /// Decode 1bpp tile data (8 bytes per tile, 1 bit per pixel).
    /// Color 0 → palette index 0, color 1 → palette index 3 (black).
    pub fn from_1bpp(data: &[u8]) -> Self {
        let bytes_per_tile_1bpp = 8;
        assert!(
            data.len() % bytes_per_tile_1bpp == 0,
            "1bpp tile data length {} is not a multiple of {}",
            data.len(),
            bytes_per_tile_1bpp,
        );
        let count = data.len() / bytes_per_tile_1bpp;
        let mut tiles = Vec::with_capacity(count);
        for i in 0..count {
            let mut pixels = [[0u8; TILE_PIXELS]; TILE_PIXELS];
            for row in 0..TILE_PIXELS {
                let byte = data[i * bytes_per_tile_1bpp + row];
                for col in 0..TILE_PIXELS {
                    let bit = 7 - col;
                    // 1bpp: bit=1 → color 3 (black), bit=0 → color 0 (white)
                    pixels[row][col] = if (byte >> bit) & 1 == 1 { 3 } else { 0 };
                }
            }
            tiles.push(Tile { pixels });
        }
        Self {
            tiles,
            format: TileFormat::Gb2bpp,
            rgba_tiles: Vec::new(),
        }
    }

    /// Decode GBA-style 4bpp tile data (32 bytes per tile, 2 bitplanes × 16 bytes each).
    ///
    /// Each tile requires 32 bytes. The first 16 bytes are bitplane 0, the
    /// next 16 bytes are bitplane 1. Each bitplane uses standard 2bpp
    /// encoding (2 bytes per row, low byte then high byte), contributing 2
    /// bits per pixel. Combined they give 4 bits per pixel (0–15), stored
    /// in [`Tile::pixels`] as u8.
    pub fn from_4bpp(data: &[u8]) -> Self {
        const BYTES_PER_TILE_4BPP: usize = 32;
        assert!(
            data.len() % BYTES_PER_TILE_4BPP == 0,
            "4bpp tile data length {} is not a multiple of {}",
            data.len(),
            BYTES_PER_TILE_4BPP,
        );
        let count = data.len() / BYTES_PER_TILE_4BPP;
        let mut tiles = Vec::with_capacity(count);
        for ti in 0..count {
            let base = ti * BYTES_PER_TILE_4BPP;
            let mut pixels = [[0u8; TILE_PIXELS]; TILE_PIXELS];
            for row in 0..TILE_PIXELS {
                let p0_lo = data[base + row * 2];
                let p0_hi = data[base + row * 2 + 1];
                let p1_lo = data[base + 16 + row * 2];
                let p1_hi = data[base + 16 + row * 2 + 1];
                for col in 0..TILE_PIXELS {
                    let bit = 7 - col;
                    // Plane 0 contributes 2 bits (standard 2bpp)
                    let p0_color = ((p0_hi >> bit) & 1) << 1 | ((p0_lo >> bit) & 1);
                    // Plane 1 contributes 2 bits (standard 2bpp)
                    let p1_color = ((p1_hi >> bit) & 1) << 1 | ((p1_lo >> bit) & 1);
                    // Combined: 4-bit color index (0–15)
                    pixels[row][col] = (p1_color << 2) | p0_color;
                }
            }
            tiles.push(Tile { pixels });
        }
        Self {
            tiles,
            format: TileFormat::Gba4bpp,
            rgba_tiles: Vec::new(),
        }
    }

    /// Create a tileset from flat RGBA pixel data.
    ///
    /// `pixels` should contain `tile_count * 64` RGBA values (8×8 pixels per tile).
    /// Tiles are laid out sequentially: pixels[0..64] = tile 0, pixels[64..128] = tile 1, etc.
    /// Each tile is stored as an [`RgbaTile`] in the internal `rgba_tiles` buffer for
    /// direct rendering. The `tiles` field contains blank dummy tiles for backward
    /// compatibility.
    pub fn from_rgba(pixels: &[Rgba], tile_count: usize) -> Self {
        assert!(
            pixels.len() >= tile_count * TILE_PIXELS * TILE_PIXELS,
            "RGBA pixel data length {} is insufficient for {} tiles (need {})",
            pixels.len(),
            tile_count,
            tile_count * TILE_PIXELS * TILE_PIXELS,
        );
        let mut rgba_tiles = Vec::with_capacity(tile_count);
        for ti in 0..tile_count {
            let base = ti * TILE_PIXELS * TILE_PIXELS;
            let mut rgba_pixels = [[Rgba::TRANSPARENT; TILE_PIXELS]; TILE_PIXELS];
            for row in 0..TILE_PIXELS {
                for col in 0..TILE_PIXELS {
                    rgba_pixels[row][col] = pixels[base + row * TILE_PIXELS + col];
                }
            }
            rgba_tiles.push(RgbaTile {
                pixels: rgba_pixels,
            });
        }
        Self {
            tiles: vec![Tile::blank(); tile_count],
            format: TileFormat::FullColor,
            rgba_tiles,
        }
    }

    /// Return the tile format of this tileset.
    pub fn tile_format(&self) -> TileFormat {
        self.format
    }

    /// Get an RGBA tile by index. Returns `None` if the tileset is not in
    /// [`TileFormat::FullColor`] mode or the index is out of bounds.
    pub fn get_rgba(&self, index: usize) -> Option<&RgbaTile> {
        if self.format != TileFormat::FullColor {
            return None;
        }
        self.rgba_tiles.get(index)
    }
}

/// Decode a single 2bpp tile row (2 bytes) into 8 color indices.
pub fn decode_2bpp_row(lo: u8, hi: u8) -> [u8; TILE_PIXELS] {
    let mut out = [0u8; TILE_PIXELS];
    for col in 0..TILE_PIXELS {
        let bit = 7 - col;
        out[col] = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);
    }
    out
}

// ---------------------------------------------------------------------------
// RGBA tiles (no palette remapping)
// ---------------------------------------------------------------------------

/// An 8×8 tile with direct RGBA pixel data (no palette remapping).
///
/// Each pixel is stored as 4 bytes (R, G, B, A), for a total of 256 bytes per tile.
/// These tiles are rendered without looking up palette indices — the RGBA values
/// are used directly.
#[derive(Debug, Clone)]
pub struct RgbaTile {
    /// 8 rows × 8 columns of RGBA pixels.
    /// Indexed as `pixels[row][col]`.
    pub pixels: [[Rgba; TILE_PIXELS]; TILE_PIXELS],
}

impl RgbaTile {
    /// Create a blank (transparent) RGBA tile.
    pub fn blank() -> Self {
        Self {
            pixels: [[Rgba::TRANSPARENT; TILE_PIXELS]; TILE_PIXELS],
        }
    }

    /// Get the RGBA pixel value at (row, col).
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> Rgba {
        self.pixels[row][col]
    }

    /// Return the RGBA values for the given row directly (no palette lookup).
    #[inline]
    pub fn render_row(&self, row: usize) -> [Rgba; TILE_PIXELS] {
        self.pixels[row]
    }
}

/// A set of RGBA tiles indexed by tile number.
///
/// Unlike [`TileSet`], these tiles store direct RGBA pixel data and are
/// rendered without any palette remapping.
#[derive(Debug, Clone)]
pub struct RgbaTileSet {
    tiles: Vec<RgbaTile>,
}

/// Blank RGBA tile for out-of-bounds access.
static BLANK_RGBA_TILE: RgbaTile = RgbaTile {
    pixels: [[Rgba::TRANSPARENT; TILE_PIXELS]; TILE_PIXELS],
};

impl RgbaTileSet {
    /// Load an RGBA tileset from PNG data.
    ///
    /// The PNG is cut into 8×8 tiles in row-major order.
    /// Each tile stores its pixels as RGBA data directly (no 2bpp bitplanes,
    /// no palette remapping).
    ///
    /// The PNG dimensions must be multiples of 8.
    #[cfg(feature = "gpu")]
    pub fn from_rgba_png(png_data: &[u8]) -> Result<Self, String> {
        use image::GenericImageView;

        let img = image::load_from_memory(png_data)
            .map_err(|e| format!("Failed to decode PNG: {}", e))?;
        let (w, h) = img.dimensions();
        if w % TILE_PIXELS as u32 != 0 || h % TILE_PIXELS as u32 != 0 {
            return Err(format!(
                "PNG dimensions {}×{} are not multiples of {}",
                w, h, TILE_PIXELS
            ));
        }

        let rgba = img.to_rgba8();
        let tiles_x = (w / TILE_PIXELS as u32) as usize;
        let tiles_y = (h / TILE_PIXELS as u32) as usize;
        let mut tiles = Vec::with_capacity(tiles_x * tiles_y);

        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let mut pixels = [[Rgba::TRANSPARENT; TILE_PIXELS]; TILE_PIXELS];
                let base_x = (tx * TILE_PIXELS) as u32;
                let base_y = (ty * TILE_PIXELS) as u32;
                for row in 0..TILE_PIXELS {
                    for col in 0..TILE_PIXELS {
                        let px = rgba.get_pixel(base_x + col as u32, base_y + row as u32);
                        pixels[row][col] = Rgba::from([px[0], px[1], px[2], px[3]]);
                    }
                }
                tiles.push(RgbaTile { pixels });
            }
        }

        Ok(Self { tiles })
    }

    /// Get a tile by index. Returns a transparent blank tile if out of bounds.
    pub fn get(&self, index: usize) -> &RgbaTile {
        if index < self.tiles.len() {
            &self.tiles[index]
        } else {
            &BLANK_RGBA_TILE
        }
    }

    /// Number of tiles in this set.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Whether this tileset is empty.
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}
