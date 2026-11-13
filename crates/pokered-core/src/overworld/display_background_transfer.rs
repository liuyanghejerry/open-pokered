//! Publication of a stationary native screen replaced through AutoBG.
//!
//! The two native rasters may have unrelated tile-handle namespaces. Track
//! accepted destination writes instead of reinterpreting old handles through
//! the new pool. A row becomes visible only when the LCD scans it; frontends
//! consume the last completed frame, independently of the foreground caller.
use super::{DisplayDevice, DisplayDeviceError};
use crate::alloc_prelude::*;

const FULL_ROW: u32 = (1 << 20) - 1;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackgroundTransferRaster {
    destination: u16,
    written: Vec<u32>,
    drawing: Vec<u32>,
    completed: Vec<u32>,
    revision: u64,
}

impl BackgroundTransferRaster {
    fn new(destination: u16) -> Self {
        Self {
            destination,
            written: vec![0; 18],
            drawing: vec![0; 144],
            completed: vec![0; 144],
            revision: 0,
        }
    }

    /// One bit per eight-pixel cell in a completed native display row.
    pub fn rows(&self) -> &[u32] {
        &self.completed
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn finished(&self) -> bool {
        self.completed.iter().all(|&row| row == FULL_ROW)
    }

    fn write(&mut self, address: u16) {
        let Some(offset) = address.checked_sub(self.destination) else {
            return;
        };
        let (row, column) = (usize::from(offset / 32), offset % 32);
        if row < 18 && column < 20 {
            self.written[row] |= 1 << column;
        }
    }

    pub(super) fn scan_line(&mut self, line: u8) {
        if line < 144 {
            self.drawing[usize::from(line)] = self.written[usize::from(line / 8)];
        }
    }

    pub(super) fn complete_frame(&mut self) {
        if self.completed != self.drawing {
            self.completed.copy_from_slice(&self.drawing);
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

impl DisplayDevice {
    /// Bind a stationary native projection before enabling its first new
    /// AutoBG transfer. This does not write VRAM or advance any display time.
    pub fn begin_native_background_transfer(
        &mut self,
        destination: u16,
    ) -> Result<(), DisplayDeviceError> {
        if !matches!(destination, 0x9800 | 0x9c00) {
            return Err(DisplayDeviceError::UnsupportedPort(destination));
        }
        self.background_transfer = Some(Box::new(BackgroundTransferRaster::new(destination)));
        Ok(())
    }

    pub fn background_transfer_raster(&self) -> Option<&BackgroundTransferRaster> {
        self.background_transfer.as_deref()
    }

    pub fn end_native_background_transfer(&mut self) {
        self.background_transfer = None;
    }

    /// Called only by the shared bus after a VRAM write was accepted.
    pub(in crate::overworld) fn record_native_background_write(&mut self, destination: u16) {
        if let Some(raster) = self.background_transfer.as_mut() {
            raster.write(destination);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_scanline_batch_preserves_background_publication() {
        let shadow = [0; 160];
        let period = 154 * 456;
        let mut batched = DisplayDevice::powered_off(0);
        batched.write_port(0xff40, 0x91, 0xc3).unwrap();
        batched.observe_time(2 * period, &shadow).unwrap();
        batched.begin_native_background_transfer(0x9800).unwrap();
        for row in 6..12 {
            for column in 0..20 {
                batched.record_native_background_write(0x9800 + row * 32 + column);
            }
        }
        let mut stepped = batched.clone();
        for target in (2 * period + 4..=3 * period).step_by(4) {
            stepped.observe_time(target, &shadow).unwrap();
        }
        batched.observe_time(3 * period, &shadow).unwrap();
        let rows = stepped.background_transfer_raster().unwrap().rows();
        for (line, &mask) in rows.iter().enumerate() {
            assert_eq!(
                mask,
                if (48..96).contains(&line) {
                    FULL_ROW
                } else {
                    0
                }
            );
        }
        assert_eq!(
            batched, stepped,
            "idle batching must publish the same scanned rows"
        );
    }

    #[test]
    fn only_bus_accepted_vram_writes_reach_completed_pixels() {
        use crate::overworld::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
        use crate::overworld::display_work_time::DisplayWorkTime;
        use crate::overworld::field_restore::{PatternAssets, PatternMemory, PatternSource};
        use crate::overworld::pattern_copy_work::{
            PatternAccess, PatternInstruction, PatternInstructionBus,
        };
        struct NoAssets;
        impl PatternAssets for NoAssets {
            fn read_pattern_byte(&self, _: PatternSource, _: u16) -> Option<u8> {
                None
            }
        }
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        time.device_mut()
            .begin_native_background_transfer(0x9800)
            .unwrap();
        let mut memory = PatternMemory::default();
        let write = PatternInstruction {
            tcycles: 8,
            access: Some(PatternAccess::Write {
                destination: 0x9800,
                value: 17,
                at_tcycle: 4,
            }),
        };
        while time.device().mode() != 3 {
            time.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        PatternInstructionBus::execute(&mut time.pattern_bus(&mut memory, &NoAssets), write)
            .unwrap();
        assert_eq!(memory.read(0x9800), Some(0));
        while time.device().mode() == 3 {
            time.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        PatternInstructionBus::execute(&mut time.pattern_bus(&mut memory, &NoAssets), write)
            .unwrap();
        assert_eq!(memory.read(0x9800), Some(17));
        assert!(time
            .device()
            .background_transfer_raster()
            .unwrap()
            .rows()
            .iter()
            .all(|&row| row == 0));
        time.idle_to_target().unwrap();
        let raster = time.device().background_transfer_raster().unwrap();
        assert_eq!(
            raster.rows()[0],
            0,
            "the first visible line was already scanned"
        );
        assert!(raster.rows()[1..8].iter().all(|&row| row == 1));
        assert!(raster.rows()[8..].iter().all(|&row| row == 0));
    }

    #[test]
    fn completed_scanout_retains_unwritten_cells_and_honors_map_stride() {
        let mut raster = BackgroundTransferRaster::new(0x9c00);
        raster.write(0x9800); // Other map is not the native screen.
        raster.write(0x9c00 + 20); // Off-screen tilemap padding.
        for row in 6..12 {
            for column in 0..20 {
                raster.write(0x9c00 + row * 32 + column);
            }
        }
        assert!(raster.rows().iter().all(|&row| row == 0));
        for line in 0..144 {
            raster.scan_line(line);
        }
        raster.complete_frame();
        for (line, &mask) in raster.rows().iter().enumerate() {
            assert_eq!(
                mask,
                if (48..96).contains(&line) {
                    FULL_ROW
                } else {
                    0
                }
            );
        }
        assert_eq!(raster.revision(), 1);
        raster.complete_frame();
        assert_eq!(raster.revision(), 1);
        assert!(!raster.finished());
    }

    #[test]
    fn a_late_write_cannot_replace_pixels_already_scanned_this_frame() {
        let mut raster = BackgroundTransferRaster::new(0x9800);
        for line in 0..4 {
            raster.scan_line(line);
        }
        raster.write(0x9800);
        for line in 4..144 {
            raster.scan_line(line);
        }
        raster.complete_frame();
        assert_eq!(&raster.rows()[..8], &[0, 0, 0, 0, 1, 1, 1, 1]);
        for row in 0..18 {
            for column in 0..20 {
                raster.write(0x9800 + row * 32 + column);
            }
        }
        for line in 0..144 {
            raster.scan_line(line);
        }
        assert!(!raster.finished());
        raster.complete_frame();
        assert!(raster.finished());
    }
}
