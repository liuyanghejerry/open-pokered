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

use dotzuki_renderer::battle_transition::TransitionFb;
use dotzuki_renderer::palette::GbColor;
use dotzuki_renderer::TILE_SIZE;

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
        for (width, height) in [(160, 144), (100, 50), (160, 30)] {
            for kind in KINDS {
                let snapshot = patterned(width, height);
                let mut fast = patterned(width, height);
                let mut reference = patterned(width, height);
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
}

