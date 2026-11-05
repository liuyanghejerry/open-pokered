//! Indoor InitMapSprites entry through LoadMapSpriteTilePatterns' first slot.
//! This executes both sixteen-slot copy loops, including unused/hidden slots.
//! Slot search, transfers and the zero-picture tail are separate parent work.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    CallOutside,
    ReadMap,
    CompareMap,
    ReturnOutside,
    ReturnCarry,
    PictureHl,
    PictureDe,
    ReadPicture,
    WritePicture,
    PictureStrideE,
    AddE,
    StoreE,
    PictureStrideL,
    AddL,
    StoreL,
    PictureLoop,
    ReadCount,
    TestCount,
    CountBranch,
    EmptyReturn,
    SetC,
    SetB,
    ImageHl,
    ClearA,
    ClearFourTile,
    ReadImage,
    WriteImage,
    ReadL,
    ImageStride,
    ImageStoreL,
    DecB,
    ImageLoop,
    FirstSlot,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndoorSpritePrefixWork {
    step: Step,
    map: u8,
    count: u8,
    a: u8,
    b: u8,
    c: u8,
    l: u8,
    e: u8,
    pictures1: [u8; 16],
    pictures2: [u8; 16],
    images: [u8; 16],
    four_tile_count: u8,
}
impl IndoorSpritePrefixWork {
    pub fn new(map: u8, pictures: &[u8]) -> Option<Self> {
        if map < 0x25 || pictures.len() > 15 {
            return None;
        }
        let mut pictures1 = [0; 16];
        pictures1[0] = 1; // The native field caller's player picture is Red.
        pictures1[1..pictures.len() + 1].copy_from_slice(pictures);
        Some(Self {
            step: Step::CallOutside,
            map,
            count: pictures.len() as u8,
            a: 0,
            b: 0,
            c: 0,
            l: 0,
            e: 0,
            pictures1,
            pictures2: [0; 16],
            images: [0; 16],
            four_tile_count: 0,
        })
    }
    /// Consumed by the subsequent raw-picture slot search, not a precomputed
    /// replacement for the sixteen-slot initialization above.
    pub fn initialized_pictures(&self) -> Option<&[u8]> {
        (self.step == Step::Done).then(|| &self.images[1..usize::from(self.c) + 1])
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        let tcycles = match self.step {
            Done => return Ok(false),
            CallOutside => 24,
            ReadMap | ReadCount | EmptyReturn => 16,
            ReturnOutside => 20, // ret nc, taken for an indoor map
            ReturnCarry => 8,    // ret c, not taken
            PictureHl | PictureDe | ImageHl | ClearFourTile | FirstSlot => 12,
            PictureLoop => {
                if self.l != 0 {
                    12
                } else {
                    8
                }
            }
            CountBranch => {
                if self.a != 0 {
                    12
                } else {
                    8
                }
            }
            ImageLoop => {
                if self.b != 0 {
                    12
                } else {
                    8
                }
            }
            CompareMap | ReadPicture | WritePicture | PictureStrideE | PictureStrideL | SetB
            | ReadImage | WriteImage | ImageStride => 8,
            _ => 4,
        };
        // Owned sprite RAM is changed only after this instruction succeeds.
        // The common bus owns all elapsed device time; this child has no clock.
        bus.execute(DisplayInstruction {
            tcycles,
            access: None,
        })?;
        self.step = match self.step {
            CallOutside => ReadMap,
            ReadMap => {
                self.a = self.map;
                CompareMap
            }
            CompareMap => ReturnOutside,
            ReturnOutside => ReturnCarry,
            ReturnCarry => PictureHl,
            PictureHl => {
                self.l = 0;
                PictureDe
            }
            PictureDe => {
                self.e = 0x0d;
                ReadPicture
            }
            ReadPicture => {
                self.a = self.pictures1[usize::from(self.l >> 4)];
                WritePicture
            }
            WritePicture => {
                self.pictures2[usize::from(self.e >> 4)] = self.a;
                PictureStrideE
            }
            PictureStrideE => {
                self.a = 16;
                AddE
            }
            AddE => {
                self.a = self.a.wrapping_add(self.e);
                StoreE
            }
            StoreE => {
                self.e = self.a;
                PictureStrideL
            }
            PictureStrideL => {
                self.a = 16;
                AddL
            }
            AddL => {
                self.a = self.a.wrapping_add(self.l);
                StoreL
            }
            StoreL => {
                self.l = self.a;
                PictureLoop
            }
            PictureLoop => {
                if self.l != 0 {
                    ReadPicture
                } else {
                    ReadCount
                }
            }
            ReadCount => {
                self.a = self.count;
                TestCount
            }
            TestCount => CountBranch,
            CountBranch => {
                if self.a != 0 {
                    SetC
                } else {
                    EmptyReturn
                }
            }
            EmptyReturn => Done,
            SetC => {
                self.c = self.a;
                SetB
            }
            SetB => {
                self.b = 16;
                ImageHl
            }
            ImageHl => {
                self.l = 0x0d;
                ClearA
            }
            ClearA => {
                self.a = 0;
                ClearFourTile
            }
            ClearFourTile => {
                self.four_tile_count = self.a;
                ReadImage
            }
            ReadImage => {
                self.a = self.pictures2[usize::from(self.l >> 4)];
                self.l = self.l.wrapping_add(1);
                WriteImage
            }
            WriteImage => {
                self.images[usize::from(self.l >> 4)] = self.a;
                self.l = self.l.wrapping_sub(1);
                ReadL
            }
            ReadL => {
                self.a = self.l;
                ImageStride
            }
            ImageStride => {
                self.a = self.a.wrapping_add(16);
                ImageStoreL
            }
            ImageStoreL => {
                self.l = self.a;
                DecB
            }
            DecB => {
                self.b = self.b.wrapping_sub(1);
                ImageLoop
            }
            ImageLoop => {
                if self.b != 0 {
                    ReadImage
                } else {
                    FirstSlot
                }
            }
            FirstSlot => {
                self.l = 0x1e;
                Done
            }
            Done => Done,
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};
    #[test]
    fn duplicate_and_hidden_raw_pictures_survive_both_full_sixteen_slot_loops() {
        let pictures = [14, 14, 14, 12, 6, 6, 6];
        let mut work = IndoorSpritePrefixWork::new(0x52, &pictures).unwrap();
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        let start = bus.elapsed();
        while work.step(&mut bus).unwrap() {}
        assert_eq!(bus.elapsed() - start, 1904);
        assert_eq!(work.initialized_pictures(), Some(pictures.as_slice()));
        assert_eq!(work.pictures2, work.pictures1);
        assert_eq!(work.images, work.pictures1);
        assert_eq!(work.four_tile_count, 0);
        assert_eq!(work.l, 0x1e);
    }
    #[test]
    fn empty_map_returns_after_picture_copy_without_initializing_image_slots() {
        let mut work = IndoorSpritePrefixWork::new(0x25, &[]).unwrap();
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        let start = bus.elapsed();
        while work.step(&mut bus).unwrap() {}
        assert_eq!(bus.elapsed() - start, 1100);
        assert_eq!(work.pictures2[0], 1);
        assert_eq!(work.images, [0; 16]);
        assert_eq!(work.initialized_pictures(), Some([].as_slice()));
        assert!(IndoorSpritePrefixWork::new(0x24, &[]).is_none());
        assert!(IndoorSpritePrefixWork::new(0x25, &[1; 16]).is_none());
    }
    #[test]
    fn mid_loop_budget_and_snapshot_do_not_repeat_or_skip_writes() {
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        while bus.remaining() > 300 {
            bus.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let mut work = IndoorSpritePrefixWork::new(0x52, &[14, 12, 6]).unwrap();
        loop {
            let before = work.clone();
            match work.step(&mut bus) {
                Ok(true) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(work, before);
                    break;
                }
                other => panic!("unexpected result {other:?}"),
            }
        }
        let (mut resumed, mut time): (IndoorSpritePrefixWork, DisplayWorkTime) =
            serde_json::from_str(&serde_json::to_string(&(&work, &bus)).unwrap()).unwrap();
        bus.begin_native_update();
        time.begin_native_update();
        while work.step(&mut bus).unwrap() {}
        while resumed.step(&mut time).unwrap() {}
        assert_eq!(work, resumed);
        assert_eq!(bus.elapsed(), time.elapsed());
    }
}
