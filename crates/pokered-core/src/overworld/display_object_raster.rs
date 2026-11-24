//! Stationary native objects published through the live OAM/DMA device.
//! Each scanline observes hardware OAM, never the logical image or shadow page.
//! Pattern bytes remain owned by the caller and must stay fixed while scanning.
use super::{DisplayDevice, DisplayDeviceError};
use crate::alloc_prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObjectRasterSpan {
    pub start_line: u8,
    pub oam: Vec<u8>,
    pub lcdc: u8,
    pub palettes: [u8; 2],
    #[serde(default)]
    pub oam_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObjectRaster {
    drawing: Vec<ObjectRasterSpan>,
    completed: Vec<ObjectRasterSpan>,
    previous_control: (u8, u8, u8),
    #[serde(default)]
    revision: u64,
}

impl ObjectRaster {
    pub fn spans(&self) -> &[ObjectRasterSpan] {
        &self.completed
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn scan_line(
        &mut self,
        line: u8,
        oam: &[u8],
        oam_revision: u64,
        lcdc: u8,
        palettes: [u8; 2],
    ) {
        if line == 0 {
            self.drawing.clear();
        }
        if self.drawing.last().is_none_or(|last| {
            (last.oam_revision != oam_revision && last.oam != oam)
                || last.lcdc != lcdc
                || last.palettes != palettes
        }) {
            self.drawing.push(ObjectRasterSpan {
                start_line: line,
                oam: oam.to_vec(),
                lcdc,
                palettes,
                oam_revision,
            });
        }
    }

    pub(super) fn restart_frame(&mut self) {
        let mut span = self.drawing.last().expect("native object span").clone();
        span.start_line = 0;
        self.drawing.clear();
        self.drawing.push(span);
    }

    pub(super) fn complete_frame(&mut self) {
        if self.completed != self.drawing {
            self.completed.clone_from(&self.drawing);
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

impl DisplayDevice {
    /// Bind the existing native object's image at the handoff to timed work.
    /// This is a native projection, not an executed DMA or recovered GB boot.
    pub fn begin_native_object_scanout(
        &mut self,
        oam: &[u8],
        palettes: [u8; 2],
    ) -> Result<(), DisplayDeviceError> {
        if oam.len() != 160 {
            return Err(DisplayDeviceError::InvalidOam);
        }
        let previous_control = (self.lcdc, self.obp0, self.obp1);
        if self.oam != oam {
            self.oam_revision = self.oam_revision.wrapping_add(1);
            self.oam.copy_from_slice(oam);
        }
        self.lcdc = (self.lcdc | 2) & !4; // Native field objects use 8×8 tiles.
        [self.obp0, self.obp1] = palettes;
        let initial = ObjectRasterSpan {
            start_line: 0,
            oam: oam.to_vec(),
            lcdc: self.lcdc,
            palettes,
            oam_revision: self.oam_revision,
        };
        self.object_raster = Some(Box::new(ObjectRaster {
            drawing: vec![initial.clone()],
            completed: vec![initial],
            previous_control,
            revision: 0,
        }));
        Ok(())
    }

    pub fn native_object_raster(&self) -> Option<&ObjectRaster> {
        self.object_raster.as_deref()
    }

    /// The synchronous native renderer resumes ownership of its objects.
    pub fn end_native_object_scanout(&mut self) {
        if let Some(raster) = self.object_raster.take() {
            (self.lcdc, self.obp0, self.obp1) = raster.previous_control;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_object_raster_publishes_dma_only_after_the_affected_scan() {
        let mut device = DisplayDevice::powered_off(0);
        device.write_port(0xff40, 0x91, 0xc3).unwrap();
        let mut shadow = [0; 160];
        device.observe_time(2 * 70224, &shadow).unwrap();
        device
            .begin_native_object_scanout(&shadow, [0xd0, 0xe0])
            .unwrap();
        let revision = device.native_object_raster().unwrap().revision();
        shadow[..4].copy_from_slice(&[76, 72, 0, 0]);
        // Changing shadow OAM alone cannot expose the player.
        device.observe_time(3 * 70224, &shadow).unwrap();
        assert_eq!(device.native_object_raster().unwrap().revision(), revision);
        assert!(device
            .native_object_raster()
            .unwrap()
            .spans()
            .iter()
            .all(|s| s.oam.iter().all(|&b| b == 0)));
        let at = 3 * 70224;
        device.write_port(0xff46, 0xc3, 0xc3).unwrap();
        device.observe_time(at + 648, &shadow).unwrap();
        assert_eq!(device.native_object_raster().unwrap().revision(), revision);
        assert_eq!(&device.oam()[..4], &[76, 72, 0, 0]);
        assert!(device
            .native_object_raster()
            .unwrap()
            .spans()
            .iter()
            .all(|s| s.oam.iter().all(|&b| b == 0)));
        device.observe_time(4 * 70224, &shadow).unwrap();
        assert_ne!(device.native_object_raster().unwrap().revision(), revision);
        assert_eq!(
            &device
                .native_object_raster()
                .unwrap()
                .spans()
                .last()
                .unwrap()
                .oam[..4],
            &[76, 72, 0, 0]
        );
        let restored: DisplayDevice =
            serde_json::from_slice(&serde_json::to_vec(&device).unwrap()).unwrap();
        assert_eq!(restored, device);
        device.end_native_object_scanout();
        assert_eq!(device.read_port(0xff40).unwrap(), 0x91);
    }

    #[test]
    fn revision_changes_still_compare_content_before_publishing_a_span() {
        let oam = [0; 160];
        let initial = ObjectRasterSpan {
            start_line: 0,
            oam: oam.to_vec(),
            lcdc: 0x93,
            palettes: [0xd0, 0xe0],
            oam_revision: 0,
        };
        let mut raster = ObjectRaster {
            drawing: vec![initial.clone()],
            completed: vec![initial],
            previous_control: (0, 0, 0),
            revision: 0,
        };
        raster.scan_line(1, &oam, 2, 0x93, [0xd0, 0xe0]);
        assert_eq!(raster.drawing.len(), 1);
        let mut changed = oam;
        changed[0] = 76;
        raster.scan_line(2, &changed, 3, 0x93, [0xd0, 0xe0]);
        assert_eq!(raster.drawing.len(), 2);
        assert_eq!(raster.drawing[1].oam_revision, 3);
    }
}
