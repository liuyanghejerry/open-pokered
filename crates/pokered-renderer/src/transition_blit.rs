//! Tile-level blits for the battle-transition wipe.
//!
//! [`BattleTransitionState::render`] walks the 20×18 tile grid and paints every
//! tile through [`TransitionFb`]. Its generic implementation reads and writes
//! one pixel at a time, bounds-checked twice per pixel: on bare metal the whole
//! 160×144 screen then costs a large multiple of a video frame per wipe frame,
//! which is exactly the pause the player sees entering a battle.
//!
//! `render` takes `&F` and `&mut F` of the *same* type, so the source and the
//! destination share this wrapper. The linear (bare-metal) layout writes the
//! index plane eight bytes at a time; the packed layout keeps the generic
//! per-pixel blit, which is also the reference the linear path is tested
//! against ([`tests`]).

use alloc::{boxed::Box, vec};
use dotzuki_renderer::battle_transition::{BattleTransitionState, TransitionFb};
use dotzuki_renderer::palette::GbColor;
use dotzuki_renderer::palette::Palette;
use dotzuki_renderer::TILE_SIZE;

// Expand four 2-bit indices together. GBA uses the otherwise lightly-used
// IWRAM for this 1 KiB table and the conversion loops, avoiding slow ROM
// instruction fetches without consuming the constrained EWRAM heap.
#[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
static UNPACK: [u32; 256] = {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = u32::from_ne_bytes([
            i as u8 & 3,
            (i >> 2) as u8 & 3,
            (i >> 4) as u8 & 3,
            (i >> 6) as u8,
        ]);
        i += 1;
    }
    table
};

/// A four-colour transition source costs 2 bits/pixel even when the hardware
/// staging buffer uses a byte/pixel. Capture directly: constructing a second
/// linear framebuffer first would retain the very allocation peak we avoid.
#[derive(Debug, Clone)]
pub struct CompactSnapshot {
    pixels: Box<[u8]>,
    width: usize,
    height: usize,
    palette: Palette<GbColor>,
}

impl CompactSnapshot {
    #[inline(never)]
    #[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
    pub fn capture(fb: &TransitionFb0<true>) -> Self {
        let mut pixels = vec![0; fb.indices().len().div_ceil(4)].into_boxed_slice();
        let words = fb.indices().len() / 4;
        // LinearRgbaIndexedFrameBuffer backs its indices with aligned u32
        // storage. Read only complete words, then handle the partial tail.
        let source = fb.indices().as_ptr().cast::<u32>();
        for (i, packed) in pixels[..words].iter_mut().enumerate() {
            let indices = unsafe { source.add(i).read() }.to_le();
            *packed = ((indices & 3)
                | ((indices >> 6) & 12)
                | ((indices >> 12) & 48)
                | ((indices >> 18) & 192)) as u8;
        }
        if fb.indices().len() % 4 != 0 {
            for (shift, &index) in fb.indices()[words * 4..].iter().enumerate() {
                pixels[words] |= (index & 3) << (shift * 2);
            }
        }
        Self {
            pixels,
            width: fb.width() as usize,
            height: fb.height() as usize,
            palette: *fb.display_palette(),
        }
    }

    #[inline(never)]
    #[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
    fn restore_indices(&self, fb: &mut TransitionFb0<true>) {
        assert_eq!(
            (fb.width() as usize, fb.height() as usize),
            (self.width, self.height)
        );
        let indices = fb.indices_mut();
        let words = indices.len() / 4;
        let destination = indices.as_mut_ptr().cast::<u32>();
        for (i, &packed) in self.pixels[..words].iter().enumerate() {
            // Same aligned storage contract as capture; never write the
            // partial last word past the exposed index slice.
            unsafe {
                destination.add(i).write(UNPACK[packed as usize]);
            }
        }
        if indices.len() % 4 != 0 {
            let tail = &mut indices[words * 4..];
            tail.copy_from_slice(&UNPACK[self.pixels[words] as usize].to_ne_bytes()[..tail.len()]);
        }
    }

    pub fn restore(&self, fb: &mut TransitionFb0<true>) {
        self.restore_indices(fb);
        fb.set_palette(self.palette);
    }

    /// Expand once in a linear pass, then skip identity tile copies. Most
    /// wipes only black tiles; decoding 360 individual tiles adds substantial
    /// call/bounds-check overhead on ARM7. Shifted Shrink tiles still read the
    /// immutable snapshot, so earlier destination writes cannot affect them.
    pub fn render(&self, transition: &BattleTransitionState, fb: &mut TransitionFb0<true>) -> bool {
        self.restore_indices(fb);
        transition.render(
            &CompactTransitionTarget::Source(self),
            &mut CompactTransitionTarget::Dest(fb),
        )
    }

    fn index(&self, x: usize, y: usize) -> u8 {
        let offset = y * self.width + x;
        (self.pixels[offset / 4] >> ((offset % 4) * 2)) & 3
    }
}

/// The engine's transition walker requires identical source/destination
/// types. This view lets it read a compact snapshot and write linear indices.
enum CompactTransitionTarget<'a> {
    Source(&'a CompactSnapshot),
    Dest(&'a mut TransitionFb0<true>),
}

impl TransitionFb for CompactTransitionTarget<'_> {
    fn size(&self) -> (usize, usize) {
        match self {
            Self::Source(snapshot) => (snapshot.width, snapshot.height),
            Self::Dest(fb) => (fb.width() as usize, fb.height() as usize),
        }
    }

    fn tile_black(&mut self, tx: usize, ty: usize) {
        if let Self::Dest(fb) = self {
            TransitionTarget::dest(fb).tile_black(tx, ty);
        }
    }

    #[inline(never)]
    #[cfg_attr(all(target_os = "none", target_arch = "arm"), link_section = ".iwram")]
    fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize) {
        // Only constructed by CompactSnapshot::render, after restoring the
        // source indices. The walker visits every destination tile once.
        if tx == stx && ty == sty {
            return;
        }
        let (Self::Dest(fb), Self::Source(snapshot)) = (self, src) else {
            return;
        };
        let (px, py) = tile_origin(tx, ty);
        let (sx, sy) = tile_origin(stx, sty);
        let width = fb.width() as usize;
        let columns = visible_span(px, width).min(visible_span(sx, snapshot.width));
        let rows = visible_span(py, fb.height() as usize).min(visible_span(sy, snapshot.height));
        if columns == 0 || rows == 0 {
            return;
        }
        let pixels = fb.indices_mut();
        for row in 0..rows {
            let start = (py + row) * width + px;
            if columns == 8 && snapshot.width % 4 == 0 && width % 4 == 0 {
                let packed_start = ((sy + row) * snapshot.width + sx) / 4;
                // Tile x is a multiple of 8 and row width is a multiple of
                // 4. The clipped row contains both aligned output words.
                let to = unsafe { pixels.as_mut_ptr().add(start).cast::<u32>() };
                unsafe {
                    to.write(UNPACK[snapshot.pixels[packed_start] as usize]);
                    to.add(1)
                        .write(UNPACK[snapshot.pixels[packed_start + 1] as usize]);
                }
                continue;
            }
            for (column, pixel) in pixels[start..start + columns].iter_mut().enumerate() {
                *pixel = snapshot.index(sx + column, sy + row);
            }
        }
    }
}

/// The index the wipe paints black tiles with (`C::from_u8(3)`).
const BLACK_INDEX: u8 = 3;

/// A transition surface: the indexed framebuffer in either storage layout.
type TransitionFb0<const LINEAR: bool> = dotzuki_renderer::RgbaIndexedFrameBuffer<GbColor, LINEAR>;

/// Read-only or writable view of a framebuffer handed to the transition walker.
pub enum TransitionTarget<'a, const LINEAR: bool> {
    Source(&'a TransitionFb0<LINEAR>),
    Dest(&'a mut TransitionFb0<LINEAR>),
}

impl<'a, const LINEAR: bool> TransitionTarget<'a, LINEAR> {
    pub fn source(fb: &'a TransitionFb0<LINEAR>) -> Self {
        Self::Source(fb)
    }

    pub fn dest(fb: &'a mut TransitionFb0<LINEAR>) -> Self {
        Self::Dest(fb)
    }

    fn view(&self) -> &TransitionFb0<LINEAR> {
        match self {
            Self::Source(fb) => fb,
            Self::Dest(fb) => fb,
        }
    }
}

/// One palette index per pixel, row-major: every tile is eight short row
/// copies, and the clip is resolved once per tile instead of once per pixel.
impl TransitionFb for TransitionTarget<'_, true> {
    fn size(&self) -> (usize, usize) {
        let fb = self.view();
        (fb.width() as usize, fb.height() as usize)
    }

    fn tile_black(&mut self, tx: usize, ty: usize) {
        let Self::Dest(fb) = self else {
            return;
        };
        let (px, py) = tile_origin(tx, ty);
        let width = fb.width() as usize;
        let columns = visible_span(px, width);
        let rows = visible_span(py, fb.height() as usize);
        if columns == 0 || rows == 0 {
            return;
        }
        let destination = fb.indices_mut().as_mut_ptr();
        for row in 0..rows {
            let to = unsafe { destination.add((py + row) * width + px) };
            // A constant count inlines to a store; a runtime count calls
            // memset once per row, which costs more than the eight bytes.
            if columns == TILE_SIZE as usize {
                unsafe { core::ptr::write_bytes(to, BLACK_INDEX, TILE_SIZE as usize) };
            } else {
                unsafe { core::ptr::write_bytes(to, BLACK_INDEX, columns) };
            }
        }
    }

    fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize) {
        let Self::Dest(fb) = self else {
            return;
        };
        let (px, py) = tile_origin(tx, ty);
        let (sx, sy) = tile_origin(stx, sty);
        let width = fb.width() as usize;
        let height = fb.height() as usize;
        let columns = visible_span(px, width).min(visible_span(sx, width));
        let rows = visible_span(py, height).min(visible_span(sy, height));
        if columns == 0 || rows == 0 {
            return;
        }
        let source = src.view().indices().as_ptr();
        let destination = fb.indices_mut().as_mut_ptr();
        for row in 0..rows {
            let from = unsafe { source.add((sy + row) * width + sx) };
            let to = unsafe { destination.add((py + row) * width + px) };
            if columns == TILE_SIZE as usize {
                unsafe { core::ptr::copy_nonoverlapping(from, to, TILE_SIZE as usize) };
            } else {
                unsafe { core::ptr::copy_nonoverlapping(from, to, columns) };
            }
        }
    }
}

/// Packed 2bpp storage: the per-pixel blit is already the cheap shape there,
/// and it stays the reference implementation for the linear path.
impl TransitionFb for TransitionTarget<'_, false> {
    fn size(&self) -> (usize, usize) {
        let fb = self.view();
        (fb.width() as usize, fb.height() as usize)
    }

    fn tile_black(&mut self, tx: usize, ty: usize) {
        let Self::Dest(fb) = self else {
            return;
        };
        fb.indexed_mut().tile_black(tx, ty);
    }

    fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize) {
        let Self::Dest(fb) = self else {
            return;
        };
        fb.indexed_mut()
            .tile_copy(tx, ty, src.view().indexed(), stx, sty);
    }
}

/// Top-left pixel of tile (`tx`, `ty`).
fn tile_origin(tx: usize, ty: usize) -> (usize, usize) {
    (tx * TILE_SIZE as usize, ty * TILE_SIZE as usize)
}

/// Pixels of an 8-pixel tile row that fit before the `limit` edge.
fn visible_span(start: usize, limit: usize) -> usize {
    (TILE_SIZE as usize).min(limit.saturating_sub(start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_renderer::battle_transition::{BattleTransitionKind, BattleTransitionState};
    use dotzuki_renderer::{RenderConfig, Rgba};

    const KINDS: [BattleTransitionKind; 8] = [
        BattleTransitionKind::Circle,
        BattleTransitionKind::DoubleCircle,
        BattleTransitionKind::Spiral { outward: false },
        BattleTransitionKind::Spiral { outward: true },
        BattleTransitionKind::SpiralTrainerStronger,
        BattleTransitionKind::HorizontalStripes,
        BattleTransitionKind::VerticalStripes,
        BattleTransitionKind::Shrink,
    ];

    /// The pre-change blit, kept as the reference: one pixel at a time through
    /// the generic `IndexedFrameBuffer` implementation.
    struct PixelByPixel<'a> {
        source: Option<&'a TransitionFb0<true>>,
        dest: Option<&'a mut TransitionFb0<true>>,
    }

    impl<'a> PixelByPixel<'a> {
        fn source(fb: &'a TransitionFb0<true>) -> Self {
            Self {
                source: Some(fb),
                dest: None,
            }
        }

        fn dest(fb: &'a mut TransitionFb0<true>) -> Self {
            Self {
                source: None,
                dest: Some(fb),
            }
        }
    }

    impl TransitionFb for PixelByPixel<'_> {
        fn size(&self) -> (usize, usize) {
            let fb = self.source.or(self.dest.as_deref()).expect("one side set");
            (fb.width() as usize, fb.height() as usize)
        }

        fn tile_black(&mut self, tx: usize, ty: usize) {
            if let Some(fb) = self.dest.as_mut() {
                fb.indexed_mut().tile_black(tx, ty);
            }
        }

        fn tile_copy(&mut self, tx: usize, ty: usize, src: &Self, stx: usize, sty: usize) {
            let Some(source) = src.source else {
                return;
            };
            if let Some(fb) = self.dest.as_mut() {
                fb.indexed_mut()
                    .tile_copy(tx, ty, source.indexed(), stx, sty);
            }
        }
    }

    /// A framebuffer whose four shades are all present in a fixed pattern, so a
    /// shifted or partially written tile cannot pass unnoticed.
    fn patterned(width: u32, height: u32) -> TransitionFb0<true> {
        let mut fb = TransitionFb0::<true>::new(RenderConfig::new(width, height), Rgba::WHITE);
        for (index, pixel) in fb.indices_mut().iter_mut().enumerate() {
            *pixel = ((index * 5 + index / width as usize) % 4) as u8;
        }
        fb
    }

    /// Every transition frame must leave the linear index plane byte-identical
    /// to the per-pixel reference, including tiles clipped by a buffer that is
    /// not a whole number of tiles wide.
    #[test]
    fn linear_transition_matches_the_per_pixel_blit() {
        for (width, height) in [(160, 144), (100, 50), (160, 30), (13, 11), (1, 1)] {
            for kind in KINDS {
                let snapshot = patterned(width, height);
                let mut fast = patterned(width, height);
                let mut reference = patterned(width, height);
                let compact_snapshot = CompactSnapshot::capture(&snapshot);
                let mut compact = patterned(width, height);
                let mut state = BattleTransitionState::new(kind, 20, 18);
                state.tick();

                for frame in 0..80 {
                    state.render(
                        &TransitionTarget::source(&snapshot),
                        &mut TransitionTarget::dest(&mut fast),
                    );
                    state.render(
                        &PixelByPixel::source(&snapshot),
                        &mut PixelByPixel::dest(&mut reference),
                    );
                    compact_snapshot.render(&state, &mut compact);
                    assert_eq!(
                        compact.indices(),
                        reference.indices(),
                        "compact {kind:?} diverged at frame {frame} on {width}x{height}"
                    );
                    assert_eq!(
                        fast.indices(),
                        reference.indices(),
                        "{kind:?} diverged at frame {frame} on a {width}x{height} buffer"
                    );
                    if state.is_done() {
                        break;
                    }
                    state.tick();
                }
            }
        }
    }

    #[test]
    fn compact_snapshot_restores_indices_and_display_palette() {
        for (width, height) in [(160, 144), (13, 11), (1, 1)] {
            let mut source = patterned(width, height);
            source.set_palette(Palette::from_bgp_register(
                0x1B,
                &dotzuki_renderer::palette::GRAYSCALE_PALETTE,
            ));
            let snapshot = CompactSnapshot::capture(&source);
            assert_eq!(
                snapshot.pixels.len(),
                (width as usize * height as usize).div_ceil(4)
            );
            let mut dest =
                TransitionFb0::<true>::new(RenderConfig::new(width, height), Rgba::BLACK);
            snapshot.restore(&mut dest);
            assert_eq!(dest.indices(), source.indices());
            for i in 0..4 {
                assert_eq!(
                    dest.display_palette().color(GbColor::from_u8(i)),
                    source.display_palette().color(GbColor::from_u8(i))
                );
            }
            // Wipes use the caller's reset display palette, whereas flash
            // frames restore the snapshot palette before applying a strobe.
            dest.reset_palette();
            let palette = *dest.display_palette();
            snapshot.render(
                &BattleTransitionState::new(BattleTransitionKind::Circle, 20, 18),
                &mut dest,
            );
            assert_eq!(*dest.display_palette(), palette);
        }
    }

    #[test]
    fn compact_snapshot_roundtrips_every_four_pixel_combination() {
        let mut source = TransitionFb0::<true>::new(RenderConfig::new(64, 16), Rgba::WHITE);
        for (packed, indices) in source.indices_mut().chunks_exact_mut(4).enumerate() {
            for (shift, index) in indices.iter_mut().enumerate() {
                *index = ((packed >> (shift * 2)) & 3) as u8;
            }
        }
        let snapshot = CompactSnapshot::capture(&source);
        for (expected, &packed) in snapshot.pixels.iter().enumerate() {
            assert_eq!(packed, expected as u8);
        }
        let mut restored = TransitionFb0::<true>::new(RenderConfig::new(64, 16), Rgba::BLACK);
        snapshot.restore(&mut restored);
        assert_eq!(source.indices(), restored.indices());
    }
}
