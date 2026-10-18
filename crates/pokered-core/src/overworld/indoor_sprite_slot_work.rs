//! Resumable raw-picture/VRAM-slot searches from map_sprites.asm.
//! Each search runs immediately before that object's transfers. The common
//! nextSpriteSlot tail runs only after both transfer children have returned.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use super::field_restore::SpriteSlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    PictureDe,
    ReadE,
    MaskE,
    SetB,
    ReadL,
    MaskL,
    CompareB,
    EndSearch,
    ReadPicture,
    ComparePicture,
    AlreadyLoaded,
    NextE,
    AddStride,
    StoreE,
    Repeat,
    PlayerDe,
    SetMax,
    FindE,
    FindStride,
    FindStoreE,
    FindL,
    CompareE,
    FoundSlot,
    ReadImage,
    CompareEleven,
    SkipStill,
    CompareMax,
    SkipLower,
    StoreMax,
    FindRepeat,
    IncrementMax,
    ReadMax,
    PushNext,
    ReadCurrent,
    SetPicture,
    CompareStill,
    Regular,
    PopStill,
    ReadFourTile,
    AddEleven,
    StoreJump,
    PopRegular,
    StoreSlot,
    StoreHramSlot,
    DuplicateIncrement,
    DuplicateRead,
    DuplicateStore,
    SearchDone,
    TailReadL,
    TailStride,
    TailStoreL,
    TailDecrement,
    TailJump,
    TailDone,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndoorSpriteSlotWork {
    step: Step,
    pictures: [u8; 16],
    images: [u8; 16],
    index: u8,
    remaining: u8,
    a: u8,
    b: u8,
    e: u8,
    l: u8,
    zero: bool,
    carry: bool,
    saved_next: u8,
    four_tile_count: u8,
    vram_slot: u8,
}
impl IndoorSpriteSlotWork {
    pub fn new(pictures: &[u8], prior: &[SpriteSlot]) -> Option<Self> {
        if pictures.len() > 15
            || prior.len() >= pictures.len()
            || prior.iter().zip(pictures).any(|(s, p)| s.picture != *p)
        {
            return None;
        }
        let mut raw = [0; 16];
        raw[0] = 1;
        raw[1..pictures.len() + 1].copy_from_slice(pictures);
        let mut images = raw;
        for (i, slot) in prior.iter().enumerate() {
            images[i + 1] = slot.vram_slot;
        }
        Some(Self {
            step: Step::PictureDe,
            pictures: raw,
            images,
            index: prior.len() as u8 + 1,
            remaining: (pictures.len() - prior.len()) as u8,
            a: 0,
            b: 0,
            e: 0,
            l: 0x1e + (prior.len() as u8) * 16,
            zero: false,
            carry: false,
            saved_next: 0,
            four_tile_count: u8::from(prior.iter().any(|s| s.vram_slot >= 11)),
            vram_slot: 0,
        })
    }
    pub fn slot(&self) -> Option<SpriteSlot> {
        (self.step == Step::SearchDone).then(|| SpriteSlot {
            picture: self.pictures[usize::from(self.index)],
            vram_slot: self.images[usize::from(self.index)],
        })
    }
    pub fn begin_tail(&mut self) -> bool {
        if self.step != Step::SearchDone {
            return false;
        }
        self.step = Step::TailReadL;
        true
    }
    fn compare(&mut self, other: u8) {
        self.zero = self.a == other;
        self.carry = self.a < other;
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        let tcycles = match self.step {
            SearchDone | TailDone => return Ok(false),
            PictureDe | PlayerDe | StoreHramSlot | ReadFourTile => 12,
            PushNext => 16,
            AlreadyLoaded => {
                if self.zero {
                    16
                } else {
                    12
                }
            }
            TailJump => {
                if self.remaining != 0 {
                    16
                } else {
                    12
                }
            }
            EndSearch | FoundSlot => {
                if self.zero {
                    12
                } else {
                    8
                }
            }
            SkipStill => {
                if !self.carry {
                    12
                } else {
                    8
                }
            }
            SkipLower | Regular => {
                if self.carry {
                    12
                } else {
                    8
                }
            }
            Repeat | FindRepeat | StoreJump | PopStill | PopRegular => 12,
            MaskE | MaskL | ReadPicture | ComparePicture | AddStride | SetMax | FindStride
            | ReadImage | CompareEleven | ReadCurrent | CompareStill | AddEleven | StoreSlot
            | DuplicateRead | DuplicateStore | TailStride | DuplicateIncrement => 8,
            _ => 4,
        };
        bus.execute(DisplayInstruction {
            tcycles,
            access: None,
        })?;
        self.step = match self.step {
            PictureDe => {
                self.e = 0x1d;
                ReadE
            }
            ReadE => {
                self.a = self.e;
                MaskE
            }
            MaskE => {
                self.a &= 0xf0;
                SetB
            }
            SetB => {
                self.b = self.a;
                ReadL
            }
            ReadL => {
                self.a = self.l;
                MaskL
            }
            MaskL => {
                self.a &= 0xf0;
                CompareB
            }
            CompareB => {
                self.compare(self.b);
                EndSearch
            }
            EndSearch => {
                if self.zero {
                    PlayerDe
                } else {
                    ReadPicture
                }
            }
            ReadPicture => {
                self.a = self.pictures[usize::from(self.e >> 4)];
                ComparePicture
            }
            ComparePicture => {
                self.compare(self.images[usize::from(self.index)]);
                AlreadyLoaded
            }
            AlreadyLoaded => {
                if self.zero {
                    DuplicateIncrement
                } else {
                    NextE
                }
            }
            NextE => {
                self.a = self.e;
                AddStride
            }
            AddStride => {
                self.a = self.a.wrapping_add(16);
                StoreE
            }
            StoreE => {
                self.e = self.a;
                Repeat
            }
            Repeat => ReadE,
            PlayerDe => {
                self.e = 0x0e;
                SetMax
            }
            SetMax => {
                self.b = 1;
                FindE
            }
            FindE => {
                self.a = self.e;
                FindStride
            }
            FindStride => {
                self.a = self.a.wrapping_add(16);
                FindStoreE
            }
            FindStoreE => {
                self.e = self.a;
                FindL
            }
            FindL => {
                self.a = self.l;
                CompareE
            }
            CompareE => {
                self.compare(self.e);
                FoundSlot
            }
            FoundSlot => {
                if self.zero {
                    IncrementMax
                } else {
                    ReadImage
                }
            }
            ReadImage => {
                self.a = self.images[usize::from(self.e >> 4)];
                CompareEleven
            }
            CompareEleven => {
                self.compare(11);
                SkipStill
            }
            SkipStill => {
                if !self.carry {
                    FindE
                } else {
                    CompareMax
                }
            }
            CompareMax => {
                self.compare(self.b);
                SkipLower
            }
            SkipLower => {
                if self.carry {
                    FindE
                } else {
                    StoreMax
                }
            }
            StoreMax => {
                self.b = self.a;
                FindRepeat
            }
            FindRepeat => FindE,
            IncrementMax => {
                self.b = self.b.wrapping_add(1);
                ReadMax
            }
            ReadMax => {
                self.a = self.b;
                PushNext
            }
            PushNext => {
                self.saved_next = self.a;
                ReadCurrent
            }
            ReadCurrent => {
                self.a = self.images[usize::from(self.index)];
                SetPicture
            }
            SetPicture => {
                self.b = self.a;
                CompareStill
            }
            CompareStill => {
                self.compare(0x3d);
                Regular
            }
            Regular => {
                if self.carry {
                    PopRegular
                } else {
                    PopStill
                }
            }
            PopStill => {
                self.a = self.saved_next;
                ReadFourTile
            }
            ReadFourTile => {
                self.a = self.four_tile_count;
                AddEleven
            }
            AddEleven => {
                self.a = self.a.wrapping_add(11);
                StoreJump
            }
            StoreJump => StoreSlot,
            PopRegular => {
                self.a = self.saved_next;
                StoreSlot
            }
            StoreSlot => {
                self.images[usize::from(self.index)] = self.a;
                StoreHramSlot
            }
            StoreHramSlot => {
                self.vram_slot = self.a;
                SearchDone
            }
            DuplicateIncrement => {
                self.e = self.e.wrapping_add(1);
                DuplicateRead
            }
            DuplicateRead => {
                self.a = self.images[usize::from(self.e >> 4)];
                DuplicateStore
            }
            DuplicateStore => {
                self.images[usize::from(self.index)] = self.a;
                SearchDone
            }
            TailReadL => {
                self.a = self.l;
                TailStride
            }
            TailStride => {
                self.a = self.a.wrapping_add(16);
                TailStoreL
            }
            TailStoreL => {
                self.l = self.a;
                TailDecrement
            }
            TailDecrement => {
                self.remaining = self.remaining.wrapping_sub(1);
                TailJump
            }
            TailJump => TailDone,
            SearchDone | TailDone => unreachable!(),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::Vec;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use crate::overworld::field_restore::SpritePatternPlan;
    #[test]
    fn searches_preserve_raw_aliases_duplicates_and_still_sprite_slots() {
        // Independent pinned-ROM opcode execution, audit1133; these times
        // are not derived from this work or from SpritePatternPlan.
        for (pictures, search_t) in [
            (
                &[14, 14, 14, 12, 6, 6, 6][..],
                &[200, 108, 108, 740, 920, 492, 492][..],
            ),
            (
                &[31, 32, 31, 61, 62, 63, 61, 14][..],
                &[200, 380, 108, 756, 912, 1068, 396, 1352][..],
            ),
        ] {
            let expected = SpritePatternPlan::new(pictures, false).slots;
            let mut prior = Vec::new();
            let mut bus = DisplayWorkTime::default();
            bus.begin_native_update();
            for i in 0..pictures.len() {
                let mut work = IndoorSpriteSlotWork::new(pictures, &prior).unwrap();
                let before = bus.elapsed();
                while work.step(&mut bus).unwrap() {}
                assert_eq!(bus.elapsed() - before, search_t[i]);
                assert_eq!(work.slot(), Some(expected[i]));
                prior.push(work.slot().unwrap());
                let start = bus.elapsed();
                assert!(work.begin_tail());
                while work.step(&mut bus).unwrap() {}
                assert_eq!(
                    bus.elapsed() - start,
                    if i + 1 == pictures.len() { 32 } else { 36 }
                );
                assert_eq!(work.remaining, (pictures.len() - i - 1) as u8);
            }
        }
    }
    #[test]
    fn budget_snapshot_resumes_inside_duplicate_search() {
        let pictures = [14, 12, 6, 7, 14];
        let prior = SpritePatternPlan::new(&pictures[..4], false).slots;
        let mut work = IndoorSpriteSlotWork::new(&pictures, &prior).unwrap();
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        while bus.remaining() > 80 {
            bus.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        loop {
            let old = work.clone();
            match work.step(&mut bus) {
                Ok(true) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(work, old);
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        let (mut restored, mut time): (IndoorSpriteSlotWork, DisplayWorkTime) =
            serde_json::from_str(&serde_json::to_string(&(&work, &bus)).unwrap()).unwrap();
        bus.begin_native_update();
        time.begin_native_update();
        while work.step(&mut bus).unwrap() {}
        while restored.step(&mut time).unwrap() {}
        assert_eq!(work, restored);
        assert_eq!(bus.elapsed(), time.elapsed());
        assert_eq!(work.slot(), Some(prior[0]));
    }
}
