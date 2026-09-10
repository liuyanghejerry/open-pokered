//! Indexed (palette-based) framebuffer with packed bitplane storage.
//!
//! [`IndexedFrameBuffer`] stores one palette index per pixel instead of RGBA
//! bytes. It is an additive alternative to the engine's RGBA
//! [`dotzuki_engine::render::FrameBuffer`] for fixed-palette games (the GB-port
//! plan, `docs/low-end-hardware-optimization.md` §5.4, and NES/SNES-style projects):
//! a Game Boy 160×144 screen costs 5,760 bytes instead of 92,160.
//!
//! # Storage format (packed 2bpp, planar bitplanes)
//!
//! Two candidate storages were considered: one byte per pixel (`W*H` bytes)
//! or packed 2bpp (`W*H/4` bytes). Packed 2bpp was chosen:
//!
//! - **VRAM isomorphism.** The packing is the *same planar bitplane layout
//!   used by Game Boy VRAM tiles* and by this crate's tile pipeline
//!   ([`crate::tile::Tile::from_2bpp`], [`crate::resource::png_to_2bpp`]):
//!   each row of 8 pixels is stored as one byte per bitplane, bit 7 =
//!   leftmost pixel, bitplane 0 first. A [`GbColor`] (2-bit) buffer's raw
//!   bytes are literally GB 2bpp tile data — tile blits and buffer contents
//!   are interchangeable, and rendering code translates almost 1:1 to real
//!   GB VRAM (`docs/low-end-hardware-optimization.md` §5.4).
//! - **4× smaller**: 5.7 KiB vs 23 KiB for a 160×144 screen.
//! - The cost — bit twiddling in `set_pixel`/`get_pixel` — is negligible
//!   for a fixed 5.7 KiB buffer.
//!
//! The bit width is derived from `C::MAX` (2 bits for [`GbColor`], 4 bits
//! for [`GbaColor`]), so the same type serves both the 4-color DMG and
//! 16-color GBA palettes. RGBA conversion is deferred to display time via
//! [`IndexedFrameBuffer::to_rgba`], which is what makes palette swaps
//! (fades, flashes) nearly free — the way real GB hardware does them.
//!
//! # Memory
//!
//! Dimensions are fixed at construction (runtime values, like the engine's
//! [`dotzuki_engine::render::FrameBuffer`]); [`Default`] is the 160×144 Game
//! Boy screen. Storage is a `Vec` allocated exactly once at construction
//! (`packed_len` bytes) and never resized. A compile-time-sized fixed-array
//! variant is not possible on stable Rust today — array lengths cannot be
//! computed from generic parameters (`generic_const_exprs` is unstable) —
//! so the eventual no_std/GB step can swap the `Vec` for a static buffer
//! without touching any other code.

use core::marker::PhantomData;

use crate::palette::{ColorIndex, GbColor, GbaColor, Palette, GRAYSCALE_PALETTE};
use dotzuki_engine::render::Rgba;
use dotzuki_engine::render_config::RenderConfig;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default screen width in pixels (Game Boy DMG resolution).
pub const SCREEN_WIDTH: usize = 160;
/// Default screen height in pixels (Game Boy DMG resolution).
pub const SCREEN_HEIGHT: usize = 144;

/// Number of bits needed to store one palette index of type `C`.
///
/// 2 for [`GbColor`] (4 colors), 4 for [`GbaColor`] (16 colors), at least 1
/// for any index type.
pub const fn index_bits<C: ColorIndex>() -> usize {
    let bits = C::MAX.ilog2();
    if bits < 1 {
        1
    } else {
        bits as usize
    }
}

/// Number of 8-pixel groups per row (rounded up).
///
/// A full Game Boy row is 20 groups; rows not divisible by 8 pack a partial
/// final group whose unused bits stay zero and are never read.
const fn groups_per_row(width: usize) -> usize {
    (width + 7) / 8
}

/// Packed storage length in bytes for a `width × height` buffer of `C`
/// indices: `height * groups_per_row(width) * index_bits::<C>()`.
///
/// 5,760 for a 160×144 [`GbColor`] buffer, 11,520 for [`GbaColor`].
pub const fn packed_len<C: ColorIndex>(width: usize, height: usize) -> usize {
    height * groups_per_row(width) * index_bits::<C>()
}

// ---------------------------------------------------------------------------
// IndexedFrameBuffer
// ---------------------------------------------------------------------------

/// A fixed-size framebuffer storing palette indices instead of RGBA pixels.
///
/// `C` is the palette index type ([`GbColor`] for 4-color DMG, [`GbaColor`]
/// for 16-color GBA). Dimensions are chosen at construction (see
/// [`Default`] for the 160×144 Game Boy screen). Storage is a packed planar
/// bitplane array (see the [module docs](self)); index values are implicitly
/// masked to the storage width on write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedFrameBuffer<C: ColorIndex = GbColor> {
    /// Packed planar bitplane storage, `packed_len::<C>(width, height)` bytes.
    data: Vec<u8>,
    /// Screen width in pixels.
    width: usize,
    /// Screen height in pixels.
    height: usize,
    #[doc(hidden)]
    _phantom: PhantomData<C>,
}

impl<C: ColorIndex> IndexedFrameBuffer<C> {
    /// Create a new `width × height` buffer, cleared to `clear`.
    pub fn new(width: usize, height: usize, clear: C) -> Self {
        let mut fb = Self {
            data: vec![0; packed_len::<C>(width, height)],
            width,
            height,
            _phantom: PhantomData,
        };
        fb.clear(clear);
        fb
    }

    /// Screen width in pixels.
    #[inline]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Screen height in pixels.
    #[inline]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Total number of pixels.
    #[inline]
    pub fn len(&self) -> usize {
        self.width * self.height
    }

    /// Whether the buffer holds no pixels.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Clear the entire buffer to a single index.
    pub fn clear(&mut self, color: C) {
        let value = color.to_index();
        let bits = index_bits::<C>();
        // In planar layout, byte `i` holds bitplane `i % bits` of a row
        // group, so filling every byte of a plane with 0xFF/0x00 paints
        // that plane across all 8 pixels of each group.
        for (i, byte) in self.data.iter_mut().enumerate() {
            let plane = i % bits;
            *byte = if (value >> plane) & 1 == 1 {
                0xFF
            } else {
                0x00
            };
        }
    }

    /// Set a single pixel. Returns false if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: C) -> bool {
        if x >= self.width as u32 || y >= self.height as u32 {
            return false;
        }
        let value = color.to_index();
        let bits = index_bits::<C>();
        let group = (x as usize) / 8;
        let bit = 7 - ((x as usize) % 8);
        let base = ((y as usize) * groups_per_row(self.width) + group) * bits;
        for plane in 0..bits {
            let plane_bit = ((value >> plane) & 1) as u8;
            let byte = &mut self.data[base + plane];
            *byte = (*byte & !(1 << bit)) | (plane_bit << bit);
        }
        true
    }

    /// Get the index of a single pixel. Returns None if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<C> {
        if x >= self.width as u32 || y >= self.height as u32 {
            return None;
        }
        let bits = index_bits::<C>();
        let group = (x as usize) / 8;
        let bit = 7 - ((x as usize) % 8);
        let base = ((y as usize) * groups_per_row(self.width) + group) * bits;
        let mut value = 0usize;
        for plane in 0..bits {
            if (self.data[base + plane] >> bit) & 1 == 1 {
                value |= 1 << plane;
            }
        }
        Some(C::from_u8(value as u8))
    }

    /// Fill a rectangular region with an index. Coordinates are clamped to
    /// buffer bounds.
    pub fn fill_rect(&mut self, x: u32, y: u32, rect_width: u32, rect_height: u32, color: C) {
        let x_start = (x as usize).min(self.width);
        let y_start = (y as usize).min(self.height);
        let x_end = (x.saturating_add(rect_width) as usize).min(self.width);
        let y_end = (y.saturating_add(rect_height) as usize).min(self.height);
        for row in y_start..y_end {
            for col in x_start..x_end {
                self.set_pixel(col as u32, row as u32, color);
            }
        }
    }

    /// Expand the indexed buffer into RGBA using `palette`.
    ///
    /// Writes `width * height * 4` bytes of row-major `[r, g, b, a]` pixel
    /// data into `out`. Returns false (and writes nothing) if `out` is too
    /// small. The palette should define an entry for every index in the
    /// buffer.
    pub fn to_rgba(&self, palette: &Palette<C>, out: &mut [u8]) -> bool {
        let need = self.width * self.height * 4;
        if out.len() < need {
            return false;
        }
        let mut base = 0;
        for y in 0..self.height {
            for x in 0..self.width {
                let index = self.get_pixel(x as u32, y as u32).expect("pixel in bounds");
                out[base..base + 4].copy_from_slice(&palette.color(index).to_array());
                base += 4;
            }
        }
        true
    }

    /// Raw packed storage, in the planar bitplane format described in the
    /// [module docs](self). For [`GbColor`] this is GB 2bpp tile data, so
    /// any 8×8-aligned region can be fed directly to
    /// [`crate::tile::Tile::from_2bpp`].
    #[inline]
    pub fn packed(&self) -> &[u8] {
        &self.data
    }

    /// Mutable raw packed storage (e.g. for tile blits into 8×8-aligned
    /// regions, or for serialization). The caller must preserve the
    /// planar bitplane layout.
    #[inline]
    pub fn packed_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

/// The default [`IndexedFrameBuffer`] is a 160×144 Game Boy screen,
/// cleared to index 0.
impl<C: ColorIndex> Default for IndexedFrameBuffer<C> {
    fn default() -> Self {
        Self::new(SCREEN_WIDTH, SCREEN_HEIGHT, C::from_u8(0))
    }
}

// ---------------------------------------------------------------------------
// Quantization
// ---------------------------------------------------------------------------

/// Find the palette entry closest to `color`.
///
/// Distance is summed squared channel difference over all four channels
/// (r, g, b, a), so transparent palette entries only win for transparent
/// input colors. Only the first `palette.count` entries are considered;
/// ties prefer the lower index.
pub fn quantize<C: ColorIndex>(palette: &Palette<C>, color: Rgba) -> C {
    let mut best = C::from_u8(0);
    let mut best_dist = u32::MAX;
    for i in 0..palette.count as usize {
        let entry = palette.colors[i];
        let dr = entry.r as i32 - color.r as i32;
        let dg = entry.g as i32 - color.g as i32;
        let db = entry.b as i32 - color.b as i32;
        let da = entry.a as i32 - color.a as i32;
        let dist = (dr * dr + dg * dg + db * db + da * da) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = C::from_u8(i as u8);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{GbaColor, GRAYSCALE_PALETTE, GRAYSCALE_SPRITE_PALETTE};
    use crate::tile::Tile;

    #[test]
    fn storage_sizes() {
        // The flagship case from the port plan: 160×144, 2bpp → 5,760 B.
        assert_eq!(packed_len::<GbColor>(SCREEN_WIDTH, SCREEN_HEIGHT), 5760);
        assert_eq!(packed_len::<GbColor>(160, 144), 5760);
        // 4-bit indices double the footprint.
        assert_eq!(packed_len::<GbaColor>(160, 144), 11520);
        assert_eq!(index_bits::<GbColor>(), 2);
        assert_eq!(index_bits::<GbaColor>(), 4);
        // The buffer itself is exactly that many bytes, no slack.
        let fb = IndexedFrameBuffer::<GbColor>::new(160, 144, GbColor::White);
        assert_eq!(fb.packed().len(), 5760);
        let gba = IndexedFrameBuffer::<GbaColor>::new(160, 144, GbaColor(0));
        assert_eq!(gba.packed().len(), 11520);
    }

    #[test]
    fn default_is_screen_sized_cleared() {
        let fb = IndexedFrameBuffer::<GbColor>::default();
        assert_eq!(fb.width(), SCREEN_WIDTH);
        assert_eq!(fb.height(), SCREEN_HEIGHT);
        assert_eq!(fb.len(), 160 * 144);
        assert_eq!(fb.get_pixel(0, 0), Some(GbColor::White));
        assert_eq!(fb.get_pixel(159, 143), Some(GbColor::White));
        let gba = IndexedFrameBuffer::<GbaColor>::default();
        assert_eq!(gba.get_pixel(159, 143), Some(GbaColor(0)));
    }

    #[test]
    fn packing_round_trip_gb() {
        let mut fb = IndexedFrameBuffer::<GbColor>::new(16, 8, GbColor::White);
        let pattern = [
            GbColor::White,
            GbColor::LightGray,
            GbColor::DarkGray,
            GbColor::Black,
        ];
        for y in 0..8u32 {
            for x in 0..16u32 {
                fb.set_pixel(x, y, pattern[((x + y) as usize) % 4]);
            }
        }
        for y in 0..8u32 {
            for x in 0..16u32 {
                assert_eq!(
                    fb.get_pixel(x, y),
                    Some(pattern[((x + y) as usize) % 4]),
                    "mismatch at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn packing_round_trip_gba() {
        let mut fb = IndexedFrameBuffer::<GbaColor>::new(8, 4, GbaColor(0));
        for y in 0..4u32 {
            for x in 0..8u32 {
                fb.set_pixel(x, y, GbaColor(((x * 3 + y * 5) % 16) as u8));
            }
        }
        for y in 0..4u32 {
            for x in 0..8u32 {
                assert_eq!(
                    fb.get_pixel(x, y),
                    Some(GbaColor(((x * 3 + y * 5) % 16) as u8))
                );
            }
        }
    }

    #[test]
    fn packing_round_trip_non_multiple_of_8() {
        // 10×7 is not divisible by 8; the partial row group must stay
        // readable and never alias into the next row.
        let mut fb = IndexedFrameBuffer::<GbColor>::new(10, 7, GbColor::White);
        for y in 0..7u32 {
            for x in 0..10u32 {
                fb.set_pixel(x, y, GbColor::from_u8(((x + y) % 4) as u8));
            }
        }
        for y in 0..7u32 {
            for x in 0..10u32 {
                assert_eq!(
                    fb.get_pixel(x, y),
                    Some(GbColor::from_u8(((x + y) % 4) as u8))
                );
            }
        }
        // The unused bits of the partial group must read back as nothing:
        // those pixel positions are out of bounds.
        assert_eq!(fb.get_pixel(10, 0), None);
        assert_eq!(fb.get_pixel(0, 7), None);
    }

    #[test]
    fn packed_layout_is_gb_vram_bitplanes() {
        // Pin the exact byte layout: row of [1,0,3,0,2,0,1,0] packs to
        // plane 0 = 0b10100010 (bits 7,5,1), plane 1 = 0b00101000 (bits 5,3).
        let mut fb = IndexedFrameBuffer::<GbColor>::new(8, 1, GbColor::White);
        let row = [1u8, 0, 3, 0, 2, 0, 1, 0];
        for (x, &v) in row.iter().enumerate() {
            fb.set_pixel(x as u32, 0, GbColor::from_u8(v));
        }
        assert_eq!(fb.packed(), &[0xA2, 0x28]);
    }

    #[test]
    fn packed_data_feeds_tile_decoder() {
        // VRAM isomorphism: a GbColor 8×8 buffer's packed bytes are GB 2bpp
        // tile data, so Tile::from_2bpp decodes them 1:1.
        let mut fb = IndexedFrameBuffer::<GbColor>::new(8, 8, GbColor::White);
        for y in 0..8u32 {
            for x in 0..8u32 {
                fb.set_pixel(x, y, GbColor::from_u8(((x * y) % 4) as u8));
            }
        }
        let tile = Tile::from_2bpp(fb.packed());
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(tile.pixels[y][x], ((x * y) % 4) as u8);
            }
        }
    }

    #[test]
    fn bounds_are_checked() {
        let mut fb = IndexedFrameBuffer::<GbColor>::new(8, 4, GbColor::White);
        assert!(fb.set_pixel(7, 3, GbColor::Black));
        assert!(!fb.set_pixel(8, 0, GbColor::Black));
        assert!(!fb.set_pixel(0, 4, GbColor::Black));
        assert!(!fb.set_pixel(u32::MAX, 0, GbColor::Black));
        assert_eq!(fb.get_pixel(8, 0), None);
        assert_eq!(fb.get_pixel(0, 4), None);
        assert_eq!(fb.get_pixel(7, 3), Some(GbColor::Black));
    }

    #[test]
    fn clear_fills_every_pixel() {
        let mut fb = IndexedFrameBuffer::<GbColor>::new(10, 7, GbColor::Black);
        // Deface it.
        fb.fill_rect(0, 0, 10, 7, GbColor::LightGray);
        assert_eq!(fb.get_pixel(5, 3), Some(GbColor::LightGray));
        fb.clear(GbColor::Black);
        for y in 0..7u32 {
            for x in 0..10u32 {
                assert_eq!(fb.get_pixel(x, y), Some(GbColor::Black));
            }
        }
        assert_eq!(fb.packed(), &[0xFF; packed_len::<GbColor>(10, 7)]);
    }

    #[test]
    fn fill_rect_clamps_to_bounds() {
        let mut fb = IndexedFrameBuffer::<GbColor>::new(8, 8, GbColor::White);
        // Start beyond the edge and overflow the opposite edge.
        fb.fill_rect(4, 4, 100, 100, GbColor::Black);
        assert_eq!(fb.get_pixel(3, 3), Some(GbColor::White));
        assert_eq!(fb.get_pixel(4, 3), Some(GbColor::White));
        assert_eq!(fb.get_pixel(3, 4), Some(GbColor::White));
        assert_eq!(fb.get_pixel(4, 4), Some(GbColor::Black));
        assert_eq!(fb.get_pixel(7, 7), Some(GbColor::Black));
        // Fully out of bounds: no-op.
        fb.fill_rect(8, 8, 4, 4, GbColor::DarkGray);
        assert_eq!(fb.get_pixel(7, 7), Some(GbColor::Black));
    }

    #[test]
    fn to_rgba_applies_palette() {
        let mut fb = IndexedFrameBuffer::<GbColor>::new(4, 2, GbColor::White);
        fb.set_pixel(0, 0, GbColor::Black);
        fb.set_pixel(3, 1, GbColor::DarkGray);
        let pal = GRAYSCALE_PALETTE;
        let mut out = [0u8; 4 * 2 * 4];
        assert!(fb.to_rgba(&pal, &mut out));
        assert_eq!(&out[0..4], &Rgba::rgb(0x00, 0x00, 0x00).to_array());
        assert_eq!(&out[1 * 4..2 * 4], &Rgba::rgb(0xFF, 0xFF, 0xFF).to_array());
        assert_eq!(
            &out[(3 + 1 * 4) * 4..(3 + 1 * 4) * 4 + 4],
            &Rgba::rgb(0x55, 0x55, 0x55).to_array()
        );
    }

    #[test]
    fn to_rgba_rejects_short_slice() {
        let fb = IndexedFrameBuffer::<GbColor>::new(4, 2, GbColor::White);
        let mut out = [0u8; 4 * 2 * 4 - 1];
        assert!(!fb.to_rgba(&GRAYSCALE_PALETTE, &mut out));
        assert_eq!(out, [0u8; 4 * 2 * 4 - 1]); // untouched
    }

    #[test]
    fn quantize_exact_match() {
        let pal = GRAYSCALE_PALETTE;
        assert_eq!(quantize(&pal, Rgba::rgb(0xFF, 0xFF, 0xFF)), GbColor::White);
        assert_eq!(
            quantize(&pal, Rgba::rgb(0xAA, 0xAA, 0xAA)),
            GbColor::LightGray
        );
        assert_eq!(
            quantize(&pal, Rgba::rgb(0x55, 0x55, 0x55)),
            GbColor::DarkGray
        );
        assert_eq!(quantize(&pal, Rgba::rgb(0x00, 0x00, 0x00)), GbColor::Black);
    }

    #[test]
    fn quantize_picks_nearest() {
        // Midway between white (255) and light gray (170) → light gray.
        let pal = GRAYSCALE_PALETTE;
        assert_eq!(quantize(&pal, Rgba::rgb(200, 200, 200)), GbColor::LightGray);
        // Midway between light gray (170) and dark gray (85) → dark gray.
        assert_eq!(
            quantize(&pal, Rgba::rgb(0x7F, 0x7F, 0x7F)),
            GbColor::DarkGray
        );
        // Darkest possible input → black.
        assert_eq!(quantize(&pal, Rgba::rgb(30, 30, 30)), GbColor::Black);
    }

    #[test]
    fn quantize_alpha_aware() {
        // Sprite palette entry 0 is transparent: an opaque dark pixel must
        // not quantize to it.
        let pal = GRAYSCALE_SPRITE_PALETTE;
        assert_eq!(pal.colors[0], Rgba::TRANSPARENT);
        assert_eq!(quantize(&pal, Rgba::rgb(0x00, 0x00, 0x00)), GbColor::Black);
        // A fully transparent pixel prefers the transparent entry.
        assert_eq!(quantize(&pal, Rgba::TRANSPARENT), GbColor::White);
    }

    #[test]
    fn quantize_gba_palette() {
        let mut colors = [Rgba::BLACK; 16];
        colors[0] = Rgba::rgb(0xFF, 0x00, 0x00);
        colors[1] = Rgba::rgb(0x00, 0xFF, 0x00);
        colors[2] = Rgba::rgb(0x00, 0x00, 0xFF);
        let pal = Palette::<GbaColor>::from_gba_palette(colors);
        assert_eq!(quantize(&pal, Rgba::rgb(0xFF, 0x00, 0x00)), GbaColor(0));
        assert_eq!(quantize(&pal, Rgba::rgb(0x00, 0xFF, 0x00)), GbaColor(1));
        assert_eq!(quantize(&pal, Rgba::rgb(0x00, 0x00, 0xFF)), GbaColor(2));
        // Midway between red and green, the input lands on the closer one.
        assert_eq!(quantize(&pal, Rgba::rgb(0xC0, 0x40, 0x00)), GbaColor(0));
    }
}

// ---------------------------------------------------------------------------
// DefaultPalette — per-index-type construction default
// ---------------------------------------------------------------------------

/// The palette [`RgbaIndexedFrameBuffer`] quantizes against when constructed
/// without an explicit palette.
pub trait DefaultPalette: ColorIndex {
    /// Default quantization/display palette for this index type.
    fn default_palette() -> Palette<Self>;
}

impl DefaultPalette for GbColor {
    fn default_palette() -> Palette<Self> {
        // The pokered render chain draws in GRAYSCALE shades, so quantizing
        // against the grayscale palette is exact for every current draw call.
        GRAYSCALE_PALETTE
    }
}

impl DefaultPalette for GbaColor {
    fn default_palette() -> Palette<Self> {
        let mut colors = [Rgba::BLACK; 16];
        for i in 0..16 {
            let v = (255 - i * 17) as u8;
            colors[i] = Rgba::rgb(v, v, v);
        }
        Palette::<GbaColor>::from_gba_palette(colors)
    }
}

// ---------------------------------------------------------------------------
// FbSurface — shared draw/present surface trait
// ---------------------------------------------------------------------------

/// A framebuffer draw surface shared by the engine's RGBA [`FrameBuffer`]
/// and the indexed [`RgbaIndexedFrameBuffer`].
///
/// Draw code written against RGBA (`set_pixel` / `fill_rect` / `clear`)
/// compiles and behaves identically on both: the indexed surface quantizes
/// every RGBA write through its base palette. `present_into` dumps the
/// final RGBA pixels (applying the display palette for indexed surfaces),
/// and `pixel_rgba` reads a single pixel (used by terminal halfblock
/// presenters).
pub trait FbSurface: Sized {
    /// Create a new `width × height` surface, cleared to black.
    fn new_screen(width: u32, height: u32) -> Self;
    /// Screen width in pixels.
    fn width(&self) -> u32;
    /// Screen height in pixels.
    fn height(&self) -> u32;
    /// Set a single pixel. Returns false if out of bounds.
    fn set_pixel(&mut self, x: u32, y: u32, color: Rgba) -> bool;
    /// Get the current color of a single pixel. Returns None if out of bounds.
    fn get_pixel(&self, x: u32, y: u32) -> Option<Rgba>;
    /// Clear the entire surface to a single color.
    fn clear(&mut self, color: Rgba);
    /// Fill a rectangular region with a color (clamped to bounds).
    fn fill_rect(&mut self, x: u32, y: u32, rect_width: u32, rect_height: u32, color: Rgba);
    /// Read a single pixel as RGBA; out-of-bounds reads return transparent.
    fn pixel_rgba(&self, x: u32, y: u32) -> Rgba {
        self.get_pixel(x, y).unwrap_or(Rgba::TRANSPARENT)
    }
    /// Dump the whole surface as row-major RGBA into `out`, which must hold
    /// at least `width * height * 4` bytes.
    fn present_into(&self, out: &mut [u8]);
}

impl FbSurface for dotzuki_engine::render::FrameBuffer {
    fn new_screen(width: u32, height: u32) -> Self {
        Self::new(RenderConfig::new(width, height), Rgba::BLACK)
    }
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn set_pixel(&mut self, x: u32, y: u32, color: Rgba) -> bool {
        Self::set_pixel(self, x, y, color)
    }
    fn get_pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        Self::get_pixel(self, x, y)
    }
    fn clear(&mut self, color: Rgba) {
        Self::clear(self, color)
    }
    fn fill_rect(&mut self, x: u32, y: u32, rect_width: u32, rect_height: u32, color: Rgba) {
        Self::fill_rect(self, x, y, rect_width, rect_height, color)
    }
    fn present_into(&self, out: &mut [u8]) {
        assert!(out.len() >= self.data.len(), "present buffer too small");
        out[..self.data.len()].copy_from_slice(&self.data);
    }
}

// ---------------------------------------------------------------------------
// RgbaIndexedFrameBuffer — RGBA facade over IndexedFrameBuffer
// ---------------------------------------------------------------------------

/// An [`IndexedFrameBuffer`] with an RGBA-facing facade: RGBA writes are
/// quantized through a fixed *base* palette (so drawing is stable no matter
/// what display effect is active), while a separate *display* palette is
/// applied at present time.
///
/// Fades and flashes become palette operations, the way real GB hardware
/// does them: swap the display palette ([`Self::set_palette`],
/// [`Self::remap_shades`], [`Self::scale_shades`], [`Self::apply_bgp`])
/// instead of touching every pixel. The buffer itself stays packed 2bpp —
/// 5,760 bytes for a 160×144 screen instead of 92,160.
///
/// `base` doubles as the initial display palette; [`Self::reset_palette`]
/// restores it. The indexed API remains reachable via [`Self::indexed`] /
/// [`Self::indexed_mut`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaIndexedFrameBuffer<C: ColorIndex = GbColor> {
    /// The indexed pixel storage.
    buffer: IndexedFrameBuffer<C>,
    /// Quantization palette: RGBA writes map to the nearest entry's index.
    base: Palette<C>,
    /// Display palette applied at present time; fades/flashes remap this.
    pub palette: Palette<C>,
}

impl<C: ColorIndex> RgbaIndexedFrameBuffer<C> {
    /// Create a `config`-sized buffer with an explicit base palette, cleared
    /// to `clear` (quantized through `base`).
    pub fn with_palette(config: RenderConfig, clear: Rgba, base: Palette<C>) -> Self {
        let mut fb = Self {
            buffer: IndexedFrameBuffer::new(
                config.screen_width as usize,
                config.screen_height as usize,
                C::from_u8(0),
            ),
            palette: base,
            base,
        };
        fb.clear(clear);
        fb
    }

    /// The current display palette.
    #[inline]
    pub fn display_palette(&self) -> &Palette<C> {
        &self.palette
    }

    /// Replace the display palette (fade/flash effect).
    pub fn set_palette(&mut self, palette: Palette<C>) {
        self.palette = palette;
    }

    /// Restore the display palette to the base palette.
    pub fn reset_palette(&mut self) {
        self.palette = self.base;
    }

    /// Remap every display shade through `map`: display color `i` becomes the
    /// base palette entry `map[i]`. This is the indexed-buffer equivalent of
    /// the per-pixel `remap_shades` loops (rBGP-style register writes).
    pub fn remap_shades(&mut self, map: &[u8]) {
        let count = self.palette.count as usize;
        for i in 0..count {
            let mapped = map.get(i).copied().unwrap_or(i as u8) as usize % count;
            self.palette.colors[i] = self.base.colors[mapped];
        }
        self.palette.count = self.base.count;
    }

    /// Scale the display colors toward black by `scale` (0.0 = black,
    /// 1.0 = base palette). Alpha is preserved. Mirrors the per-pixel
    /// "brighten/darken" loops (e.g. the Ghost Marowak reveal).
    pub fn scale_shades(&mut self, scale: f32) {
        let scale = scale.clamp(0.0, 1.0);
        for i in 0..self.palette.count as usize {
            let c = self.base.colors[i];
            self.palette.colors[i] = Rgba::new(
                (c.r as f32 * scale) as u8,
                (c.g as f32 * scale) as u8,
                (c.b as f32 * scale) as u8,
                c.a,
            );
        }
    }

    /// Read-only access to the underlying indexed buffer.
    #[inline]
    pub fn indexed(&self) -> &IndexedFrameBuffer<C> {
        &self.buffer
    }

    /// Mutable access to the underlying indexed buffer (C-index API).
    #[inline]
    pub fn indexed_mut(&mut self) -> &mut IndexedFrameBuffer<C> {
        &mut self.buffer
    }

    /// Raw packed 2bpp storage (see [`IndexedFrameBuffer::packed`]).
    #[inline]
    pub fn packed(&self) -> &[u8] {
        self.buffer.packed()
    }

    /// Mutable raw packed storage.
    #[inline]
    pub fn packed_mut(&mut self) -> &mut [u8] {
        self.buffer.packed_mut()
    }

    /// Expand the buffer into RGBA using the *display* palette.
    /// Writes `width * height * 4` bytes into `out`; returns false (and
    /// writes nothing) if `out` is too small.
    pub fn to_rgba(&self, out: &mut [u8]) -> bool {
        self.buffer.to_rgba(&self.palette, out)
    }

    /// Copy the pixels and display palette of `other` into this buffer.
    /// Both buffers must have the same dimensions.
    pub fn copy_from(&mut self, other: &Self) {
        self.buffer
            .packed_mut()
            .copy_from_slice(other.buffer.packed());
        self.palette = other.palette;
        self.base = other.base;
    }

    /// Set a single pixel by palette index. Returns false if out of bounds.
    pub fn set_pixel_index(&mut self, x: u32, y: u32, color: C) -> bool {
        self.buffer.set_pixel(x, y, color)
    }

    /// Get the palette index of a single pixel. Returns None if out of bounds.
    pub fn get_index(&self, x: u32, y: u32) -> Option<C> {
        self.buffer.get_pixel(x, y)
    }

    /// Clear the entire buffer to a single palette index.
    pub fn clear_index(&mut self, color: C) {
        self.buffer.clear(color);
    }

    /// Total number of pixels.
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Screen width in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.buffer.width() as u32
    }

    /// Screen height in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.buffer.height() as u32
    }

    /// Whether the buffer holds no pixels.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Set a single pixel, quantized through the base palette.
    /// Returns false if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Rgba) -> bool {
        let index = quantize(&self.base, color);
        self.buffer.set_pixel(x, y, index)
    }

    /// Get the current display color of a single pixel (through the display
    /// palette). Returns None if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        self.buffer.get_pixel(x, y).map(|i| self.palette.color(i))
    }

    /// Clear the entire buffer to a single color (quantized through the base
    /// palette).
    ///
    /// Also restores the display palette to the base palette. This mirrors
    /// the RGBA buffer's contract — after a clear the framebuffer is in a
    /// pristine state — and is what prevents fade/flash palettes from
    /// leaking into the next frame: every frame starts with a clear, so the
    /// display mapping always begins from the base and effects re-apply
    /// their palette at the end of the frame.
    pub fn clear(&mut self, color: Rgba) {
        let index = quantize(&self.base, color);
        self.buffer.clear(index);
        self.palette = self.base;
    }

    /// Fill a rectangular region with a color (quantized through the base
    /// palette). Coordinates are clamped to buffer bounds.
    pub fn fill_rect(&mut self, x: u32, y: u32, rect_width: u32, rect_height: u32, color: Rgba) {
        let index = quantize(&self.base, color);
        self.buffer.fill_rect(x, y, rect_width, rect_height, index);
    }

    /// Copy a horizontal line of RGBA data into the buffer (each pixel
    /// quantized through the base palette). `src` must be exactly
    /// `count * 4` bytes. Returns false if the line goes out of bounds.
    pub fn blit_row(&mut self, x: u32, y: u32, src: &[u8], count: u32) -> bool {
        if y >= self.height() || x >= self.width() {
            return false;
        }
        let actual_count = count.min(self.width() - x) as usize;
        let src_bytes = actual_count * 4;
        if src.len() < src_bytes {
            return false;
        }
        for i in 0..actual_count {
            let off = i * 4;
            let c = Rgba::new(src[off], src[off + 1], src[off + 2], src[off + 3]);
            self.buffer
                .set_pixel(x + i as u32, y, quantize(&self.base, c));
        }
        true
    }

    /// Save the framebuffer as a PNG file (display palette applied).
    #[cfg(any(feature = "gpu", feature = "image-assets"))]
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        use image::{ImageBuffer, Rgba as ImgRgba};
        let w = self.width() as u32;
        let h = self.height() as u32;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        self.to_rgba(&mut rgba);
        let img: ImageBuffer<ImgRgba<u8>, _> =
            ImageBuffer::from_raw(w, h, rgba).expect("framebuffer size mismatch");
        img.save(path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

impl<C: ColorIndex + DefaultPalette> RgbaIndexedFrameBuffer<C> {
    /// Create a `config`-sized buffer using the type's default base palette,
    /// cleared to `clear`.
    pub fn new(config: RenderConfig, clear: Rgba) -> Self {
        Self::with_palette(config, clear, C::default_palette())
    }
}

impl RgbaIndexedFrameBuffer<GbColor> {
    /// Apply a DMG BGP register byte: display color `i` becomes the base
    /// palette's shade `(bgp >> (2 * i)) & 3`. This is exactly how the
    /// original hardware performs fades (by writing the BGP register).
    pub fn apply_bgp(&mut self, bgp: u8) {
        let mut remapped = [0u8; 4];
        for i in 0..4 {
            remapped[i] = (bgp >> (2 * i)) & 3;
        }
        self.remap_shades(&remapped);
    }
}

impl<C: ColorIndex + DefaultPalette> FbSurface for RgbaIndexedFrameBuffer<C> {
    fn new_screen(width: u32, height: u32) -> Self {
        Self::new(RenderConfig::new(width, height), Rgba::BLACK)
    }
    fn width(&self) -> u32 {
        self.buffer.width() as u32
    }
    fn height(&self) -> u32 {
        self.buffer.height() as u32
    }
    fn set_pixel(&mut self, x: u32, y: u32, color: Rgba) -> bool {
        self.set_pixel(x, y, color)
    }
    fn get_pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        self.get_pixel(x, y)
    }
    fn clear(&mut self, color: Rgba) {
        self.clear(color)
    }
    fn fill_rect(&mut self, x: u32, y: u32, rect_width: u32, rect_height: u32, color: Rgba) {
        self.fill_rect(x, y, rect_width, rect_height, color)
    }
    fn present_into(&self, out: &mut [u8]) {
        assert!(out.len() >= self.len() * 4, "present buffer too small");
        self.to_rgba(out);
    }
}

#[cfg(test)]
mod facade_tests {
    use super::*;
    use crate::palette::GRAYSCALE_SPRITE_PALETTE;

    fn fb() -> RgbaIndexedFrameBuffer<GbColor> {
        RgbaIndexedFrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE)
    }

    #[test]
    fn storage_is_packed() {
        let fb = fb();
        assert_eq!(fb.len(), 160 * 144);
        assert_eq!(fb.packed().len(), 5760);
        assert_eq!(fb.packed().len(), packed_len::<GbColor>(160, 144));
    }

    #[test]
    fn grayscale_round_trips_exactly() {
        let mut fb = fb();
        let colors = [
            Rgba::WHITE,
            Rgba::rgb(0xAA, 0xAA, 0xAA),
            Rgba::rgb(0x55, 0x55, 0x55),
            Rgba::BLACK,
        ];
        for (i, &c) in colors.iter().enumerate() {
            assert!(fb.set_pixel(i as u32, 0, c));
        }
        for (i, &c) in colors.iter().enumerate() {
            assert_eq!(fb.get_pixel(i as u32, 0), Some(c));
            assert_eq!(fb.get_index(i as u32, 0), Some(GbColor::from_u8(i as u8)));
        }
    }

    #[test]
    fn near_grays_quantize_to_nearest_shade() {
        let mut fb = fb();
        // 0xC0 → light gray (170), 0x80 → light gray (170), 0x40 → dark gray.
        fb.set_pixel(0, 0, Rgba::rgb(0xC0, 0xC0, 0xC0));
        fb.set_pixel(1, 0, Rgba::rgb(0x80, 0x80, 0x80));
        fb.set_pixel(2, 0, Rgba::rgb(0x40, 0x40, 0x40));
        assert_eq!(fb.get_index(0, 0), Some(GbColor::LightGray));
        assert_eq!(fb.get_index(1, 0), Some(GbColor::LightGray));
        assert_eq!(fb.get_index(2, 0), Some(GbColor::DarkGray));
    }

    #[test]
    fn transparent_writes_pick_nearest_opaque_shade() {
        // GRAYSCALE_PALETTE has no transparent entry, so a transparent write
        // quantizes to the nearest opaque color: black. Presenters ignore
        // alpha (native/web textures, TUI halfblocks), so this matches what
        // the old RGBA buffer displayed for such pixels.
        let mut fb = fb();
        fb.set_pixel(3, 3, Rgba::TRANSPARENT);
        assert_eq!(fb.get_index(3, 3), Some(GbColor::Black));
    }

    #[test]
    fn bounds_checked_rgba_facade() {
        let mut fb = fb();
        assert!(fb.set_pixel(159, 143, Rgba::BLACK));
        assert!(!fb.set_pixel(160, 0, Rgba::BLACK));
        assert!(!fb.set_pixel(0, 144, Rgba::BLACK));
        assert_eq!(fb.get_pixel(160, 0), None);
        assert_eq!(fb.pixel_rgba(160, 0), Rgba::TRANSPARENT);
    }

    #[test]
    fn clear_and_fill_quantize() {
        let mut fb = fb();
        fb.fill_rect(0, 0, 100, 100, Rgba::rgb(0x55, 0x55, 0x55));
        assert_eq!(fb.get_index(50, 50), Some(GbColor::DarkGray));
        fb.clear(Rgba::BLACK);
        assert_eq!(fb.get_index(0, 0), Some(GbColor::Black));
        assert_eq!(fb.get_index(159, 143), Some(GbColor::Black));
    }

    #[test]
    fn blit_row_quantizes_each_pixel() {
        let mut fb = fb();
        let row = [255u8, 255, 255, 255, 0, 0, 0, 0];
        assert!(fb.blit_row(0, 0, &row, 2));
        assert_eq!(fb.get_index(0, 0), Some(GbColor::White));
        assert_eq!(fb.get_index(1, 0), Some(GbColor::Black));
        assert!(!fb.blit_row(160, 0, &row, 2));
        assert!(!fb.blit_row(0, 0, &row, 3)); // src too short
    }

    #[test]
    fn remap_shades_inverts() {
        let mut fb = fb();
        fb.fill_rect(0, 0, 8, 8, Rgba::BLACK);
        // Invert: 0→3, 1→2, 2→1, 3→0.
        fb.remap_shades(&[3, 2, 1, 0]);
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE));
        // Display palette changed; the index underneath is untouched.
        assert_eq!(fb.get_index(0, 0), Some(GbColor::Black));
        fb.reset_palette();
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::BLACK));
    }

    #[test]
    fn apply_bgp_fade_to_black() {
        let mut fb = fb();
        fb.set_pixel(0, 0, Rgba::WHITE);
        // rBGP = dc 3,3,3,3 → every shade maps to black.
        fb.apply_bgp(0b11111111);
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::BLACK));
        assert_eq!(fb.get_index(0, 0), Some(GbColor::White));
    }

    #[test]
    fn scale_shades_dims_display() {
        let mut fb = fb();
        fb.fill_rect(0, 0, 8, 8, Rgba::WHITE);
        fb.scale_shades(0.5);
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::rgb(127, 127, 127)));
        fb.scale_shades(0.0);
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::BLACK));
    }

    #[test]
    fn palette_swap_does_not_touch_draws() {
        let mut fb = fb();
        fb.set_pixel(4, 4, Rgba::rgb(0x55, 0x55, 0x55));
        fb.apply_bgp(0b11100100); // identity-ish fade state
                                  // Drawing while a display effect is active still quantizes via base.
        fb.set_pixel(5, 4, Rgba::rgb(0xAA, 0xAA, 0xAA));
        fb.reset_palette();
        assert_eq!(fb.get_pixel(5, 4), Some(Rgba::rgb(0xAA, 0xAA, 0xAA)));
    }

    #[test]
    fn copy_from_copies_pixels_and_palette() {
        let mut src = fb();
        src.fill_rect(0, 0, 16, 16, Rgba::BLACK);
        src.apply_bgp(0b00000000); // all white
        let mut dst = fb();
        dst.copy_from(&src);
        assert_eq!(dst.get_index(8, 8), Some(GbColor::Black));
        assert_eq!(dst.get_pixel(8, 8), Some(Rgba::WHITE));
        assert_eq!(dst.packed(), src.packed());
    }

    #[test]
    fn clear_resets_display_palette() {
        let mut fb = fb();
        fb.apply_bgp(0b00000000); // white-out display palette
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::WHITE));
        fb.clear(Rgba::BLACK);
        // Clear restores the base display mapping: black stays black.
        assert_eq!(fb.get_pixel(0, 0), Some(Rgba::BLACK));
        fb.set_pixel(1, 0, Rgba::WHITE);
        assert_eq!(fb.get_pixel(1, 0), Some(Rgba::WHITE));
    }

    #[test]
    fn to_rgba_uses_display_palette() {
        let mut fb = RgbaIndexedFrameBuffer::<GbColor>::new(RenderConfig::new(2, 1), Rgba::WHITE);
        fb.set_pixel(0, 0, Rgba::WHITE);
        fb.apply_bgp(0b00000000); // white-out
        let mut out = [0u8; 8];
        assert!(fb.to_rgba(&mut out));
        assert_eq!(&out[0..4], &[0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn fb_surface_present_and_pixels() {
        let mut fb = RgbaIndexedFrameBuffer::<GbColor>::new_screen(4, 2);
        fb.set_pixel(1, 1, Rgba::WHITE);
        assert_eq!(fb.width(), 4);
        assert_eq!(fb.height(), 2);
        assert_eq!(fb.pixel_rgba(1, 1), Rgba::WHITE);
        assert_eq!(fb.pixel_rgba(0, 0), Rgba::BLACK);
        let mut out = [0u8; 4 * 2 * 4];
        fb.present_into(&mut out);
        assert_eq!(&out[5 * 4..6 * 4], &[0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn sprite_palette_quantization_matches_draw_palette() {
        // The pokered sprite path draws with GRAYSCALE_SPRITE_PALETTE colors;
        // quantizing those through the facade base must recover the original
        // indices. Color 0 is transparent and quantizes to black (nearest
        // opaque shade in the grayscale base palette) — the same visual the
        // RGBA buffer produced, since presenters ignore alpha.
        let mut fb = fb();
        for (i, &c) in GRAYSCALE_SPRITE_PALETTE.colors[..4].iter().enumerate() {
            fb.set_pixel(i as u32, 0, c);
            let expected = if i == 0 {
                GbColor::Black
            } else {
                GbColor::from_u8(i as u8)
            };
            assert_eq!(fb.get_index(i as u32, 0), Some(expected));
        }
    }
}
