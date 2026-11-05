//! Passive background palette scanout. Stores take effect at the device's
//! actual scan position; the frontend consumes the last completed raster.
//! No host-frame countdown, scene-specific scanline, or extra clock lives here.
use super::{DisplayDevice, RasterPhase};
use crate::alloc_prelude::*;

const PIXELS: u16 = 160 * 144;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PaletteSpan {
    pub start: u16,
    pub palette: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackgroundPaletteRaster {
    drawing: Vec<PaletteSpan>,
    completed: Vec<PaletteSpan>,
    revision: u64,
}
impl BackgroundPaletteRaster {
    fn new(palette: u8) -> Self {
        let span = PaletteSpan { start: 0, palette };
        Self {
            drawing: vec![span],
            completed: vec![span],
            revision: 0,
        }
    }
    pub fn spans(&self) -> &[PaletteSpan] {
        &self.completed
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn is_white(&self) -> bool {
        self.completed.len() == 1 && self.completed[0].palette == 0
    }
    pub(super) fn begin_frame(&mut self, palette: u8) {
        self.drawing.clear();
        self.drawing.push(PaletteSpan { start: 0, palette });
    }
    pub(super) fn complete_frame(&mut self) {
        if self.completed != self.drawing {
            self.completed.clone_from(&self.drawing);
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub(super) fn blank_now(&mut self) {
        self.begin_frame(0);
        self.complete_frame();
    }
    fn write(&mut self, start: u16, palette: u8) {
        if start >= PIXELS {
            return;
        }
        // Multiple stores can precede the same pixel, including mode2. Only
        // the final value reaches that pixel, and equal spans coalesce.
        while self.drawing.last().is_some_and(|span| span.start >= start) {
            self.drawing.pop();
        }
        if self
            .drawing
            .last()
            .is_none_or(|span| span.palette != palette)
        {
            self.drawing.push(PaletteSpan { start, palette });
        }
    }
}

impl DisplayDevice {
    /// Explicit handoff of an already rendered native image. Its indices are
    /// canonical shades, so the caller supplies the identity BGP projection.
    /// This is not an original IO instruction or reconstructed boot history.
    pub fn begin_native_background_scanout(&mut self, palette: u8) {
        self.bgp = palette;
        self.palette_raster = Some(Box::new(BackgroundPaletteRaster::new(
            self.effective_background_palette(),
        )));
    }
    pub fn end_native_background_scanout(&mut self) {
        self.palette_raster = None;
    }
    pub fn background_palette_raster(&self) -> Option<&BackgroundPaletteRaster> {
        self.palette_raster.as_deref()
    }
    pub(super) fn effective_background_palette(&self) -> u8 {
        if self.blank_first_frame() || self.lcdc & 1 == 0 {
            0
        } else {
            self.bgp
        }
    }
    pub(super) fn record_background_palette(&mut self) {
        if self.palette_raster.is_none() {
            return;
        }
        let start = match self.phase {
            RasterPhase::Off | RasterPhase::VBlank => return,
            RasterPhase::EnableStore
            | RasterPhase::StartupMode0
            | RasterPhase::Mode2
            | RasterPhase::AdvanceLy => u16::from(self.ly) * 160,
            RasterPhase::Mode3 => u16::from(self.ly) * 160 + self.mode3_pixels_emitted(),
            RasterPhase::HBlank => (u16::from(self.ly) + 1) * 160,
        };
        let palette = self.effective_background_palette();
        self.palette_raster.as_mut().unwrap().write(start, palette);
    }

    /// Pixel progress under this device's bounded fetch model. The same
    /// window/OBJ penalties used by mode3_dots occur at their visible X,
    /// rather than being charged entirely before the first pixel. Midline
    /// OAM corruption and window-enable quirks remain outside this device.
    fn mode3_pixels_emitted(&self) -> u16 {
        let elapsed = self.mode3_length - self.phase_remaining;
        let mut time = 12 + u16::from(self.scx & 7);
        let window = self.lcdc & 0x21 == 0x21 && self.window_y_reached && self.wx <= 166;
        let window_x = i16::from(self.wx) - 7;
        let mut stalls = [(0u16, 0u16); 11];
        let mut len = 0;
        if window {
            stalls[len] = (window_x.max(0) as u16, 6);
            len += 1;
        }
        if self.lcdc & 2 != 0 {
            let height = if self.lcdc & 4 != 0 { 16 } else { 8 };
            let mut selected = [(0u8, 0usize); 10];
            let mut count = 0;
            for (index, object) in self.oam.chunks_exact(4).enumerate() {
                let top = i16::from(object[0]) - 16;
                if top <= i16::from(self.ly) && i16::from(self.ly) < top + height {
                    selected[count] = (object[1], index);
                    count += 1;
                    if count == 10 {
                        break;
                    }
                }
            }
            selected[..count].sort_unstable();
            let mut considered = [(false, 0i16); 10];
            let mut considered_len = 0;
            for &(x, _) in &selected[..count] {
                if x >= 168 {
                    continue;
                }
                let pixel = i16::from(x) - 8;
                let in_window = window && pixel >= window_x;
                let within = if in_window {
                    pixel - window_x
                } else {
                    pixel + i16::from(self.scx)
                };
                let tile = (in_window, within.div_euclid(8));
                let penalty = if x == 0 {
                    11
                } else {
                    let mut penalty = 6;
                    if !considered[..considered_len].contains(&tile) {
                        considered[considered_len] = tile;
                        considered_len += 1;
                        penalty += (5 - within.rem_euclid(8)).max(0) as u16;
                    }
                    penalty
                };
                stalls[len] = (pixel.max(0) as u16, penalty);
                len += 1;
            }
        }
        stalls[..len].sort_unstable();
        let mut pixel = 0;
        for &(x, penalty) in &stalls[..len] {
            if elapsed <= time + x - pixel {
                return pixel + elapsed.saturating_sub(time);
            }
            time += x - pixel + penalty;
            pixel = x;
            if elapsed <= time {
                return pixel;
            }
        }
        (pixel + elapsed.saturating_sub(time)).min(160)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device() -> DisplayDevice {
        let mut d = DisplayDevice::powered_off(0);
        d.write_port(0xff40, 0x91, 0xc3).unwrap();
        // Finish enable-store/startup and stop on the first settled mode2
        // edge, without treating an arbitrary host boundary as line zero.
        while d.first_frame {
            d.observe_time(d.observed_tcycles + u64::from(d.phase_remaining), &[0; 160])
                .unwrap();
        }
        assert_eq!((d.ly, d.phase), (0, RasterPhase::Mode2));
        d.begin_native_background_scanout(0xe4);
        d
    }
    fn finish(d: &mut DisplayDevice) {
        let epoch = d.vblank_epoch;
        while d.vblank_epoch == epoch {
            d.observe_time(d.observed_tcycles + u64::from(d.phase_remaining), &[0; 160])
                .unwrap();
        }
    }
    #[test]
    fn writes_affect_only_pixels_after_the_actual_bus_position() {
        for (dot, start) in [
            (0, 0),
            (79, 0),
            (80, 0),
            (91, 0),
            (92, 0),
            (93, 1),
            (171, 79),
            (251, 159),
            (252, 160),
            (455, 160),
        ] {
            for line in [0, 7, 20, 91, 143] {
                let mut d = device();
                d.observe_time(d.observed_tcycles + line * 456 + dot, &[0; 160])
                    .unwrap();
                d.write_port(0xff47, 0, 0xc3).unwrap();
                assert_eq!(
                    d.background_palette_raster().unwrap().spans(),
                    &[PaletteSpan {
                        start: 0,
                        palette: 0xe4
                    }]
                );
                finish(&mut d);
                let at = (line * 160 + start) as u16;
                let expected = if at == 0 {
                    vec![PaletteSpan {
                        start: 0,
                        palette: 0,
                    }]
                } else if at == PIXELS {
                    vec![PaletteSpan {
                        start: 0,
                        palette: 0xe4,
                    }]
                } else {
                    vec![
                        PaletteSpan {
                            start: 0,
                            palette: 0xe4,
                        },
                        PaletteSpan {
                            start: at,
                            palette: 0,
                        },
                    ]
                };
                assert_eq!(
                    d.background_palette_raster().unwrap().spans(),
                    expected,
                    "line={line} dot={dot}"
                );
            }
        }
    }
    #[test]
    fn vblank_store_changes_the_next_frame_and_stable_raster_reuses_revision() {
        let mut d = device();
        finish(&mut d);
        d.write_port(0xff47, 0, 0xc3).unwrap();
        assert!(!d.background_palette_raster().unwrap().is_white());
        finish(&mut d);
        assert!(d.background_palette_raster().unwrap().is_white());
        let revision = d.background_palette_raster().unwrap().revision();
        d.observe_time(d.observed_tcycles + 3 * 70224, &[0; 160])
            .unwrap();
        assert_eq!(d.background_palette_raster().unwrap().revision(), revision);
    }
    #[test]
    fn lcd_disable_and_first_enable_frame_are_blank_independent_of_bgp() {
        let mut d = device();
        finish(&mut d);
        d.write_port(0xff40, 0x11, 0xc3).unwrap();
        assert!(d.background_palette_raster().unwrap().is_white());
        d.write_port(0xff47, 0xe4, 0xc3).unwrap();
        d.write_port(0xff40, 0x91, 0xc3).unwrap();
        finish(&mut d);
        assert!(d.background_palette_raster().unwrap().is_white());
        finish(&mut d);
        assert_eq!(
            d.background_palette_raster().unwrap().spans(),
            &[PaletteSpan {
                start: 0,
                palette: 0xe4
            }]
        );
    }
    #[test]
    fn split_advances_and_snapshot_keep_pending_partial_raster() {
        let mut d = device();
        d.observe_time(d.observed_tcycles + 20 * 456, &[0; 160])
            .unwrap();
        d.write_port(0xff47, 0x90, 0xc3).unwrap();
        let mut other: DisplayDevice =
            serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        d.observe_time(4 * 70224 + 123, &[0; 160]).unwrap();
        while other.observed_tcycles < 4 * 70224 + 123 {
            let target = (other.observed_tcycles + 137).min(4 * 70224 + 123);
            other.observe_time(target, &[0; 160]).unwrap();
        }
        assert_eq!(d, other);
    }
    #[test]
    fn scroll_window_and_object_stalls_shift_pixel_progress() {
        for scx in 0..8 {
            let mut d = device();
            d.write_port(0xff43, scx, 0xc3).unwrap();
            d.observe_time(
                d.observed_tcycles + 80 + 12 + u64::from(scx) + 30,
                &[0; 160],
            )
            .unwrap();
            assert_eq!(d.mode3_pixels_emitted(), 30);
        }
        let mut d = device();
        let start = d.observed_tcycles;
        d.write_port(0xff40, 0xb3, 0xc3).unwrap();
        d.write_port(0xff4b, 47, 0xc3).unwrap();
        d.oam[..4].copy_from_slice(&[16, 28, 1, 0]);
        d.observe_time(start + 80 + 12 + 30 + 7, &[0; 160]).unwrap();
        assert_eq!(d.mode3_pixels_emitted(), 30, "OBJ at x20 stalls seven dots");
        d.observe_time(start + 80 + 12 + 60 + 7 + 6, &[0; 160])
            .unwrap();
        assert_eq!(d.mode3_pixels_emitted(), 60, "window at x40 adds six dots");
    }
}
