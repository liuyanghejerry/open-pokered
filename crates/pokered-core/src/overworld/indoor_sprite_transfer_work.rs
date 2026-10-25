//! Source transfer preparation/stack restoration between an indoor object's
//! slot search and nextSpriteSlot. Copy children remain explicit boundaries.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use super::field_restore::{CopyFormat, PatternCopy, PatternSource, SpriteSlot};
use super::sprite_sheet_metadata::{sprite_sheet_metadata, SpriteSheetMetadata};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Picture,
    DecrementPicture,
    Double1,
    Double2,
    PushParentBc,
    PushParentHl,
    Table,
    TableCarry1,
    TableHigh1,
    TableAddLow,
    TableStoreLow,
    TableCarry2,
    TableHigh2,
    PushTable,
    CallSheet1,
    SheetLow,
    SheetE,
    SheetHigh,
    SheetD,
    SheetSize,
    SheetC,
    SheetZero,
    SheetB,
    SheetBank,
    SheetReturn,
    PushBank1,
    PushSource1,
    PushLength,
    VramBase,
    SlotBytes,
    ReadSlot1,
    CompareSlot1,
    StillBranch,
    SlotD,
    SlotDecrement,
    VramAdd,
    VramDecrement,
    VramLoop,
    VramJump,
    StillBase,
    ReadFour,
    TestFour,
    FourBranch,
    FirstStillBase,
    IncrementFour,
    StoreFour,
    PopLength,
    PopSource,
    PopBank,
    PushDest1,
    PushDest2,
    SourceH,
    SourceL,
    PopDest,
    BankB,
    ReadFont1,
    TestFont1,
    FontBranch1,
    BankA,
    ClearB,
    LowerCopy,
    BetweenPopDest,
    BetweenPopTable,
    ReadSlot2,
    CompareSlot2,
    SecondBranch,
    PushDest3,
    CallSheet2,
    PushBank2,
    OffsetA,
    AddSourceE,
    SourceE,
    SourceCarry,
    SourceHigh,
    ReadFont2,
    TestFont2,
    FontBranch2,
    ColdPopBank,
    ColdPopDest,
    ColdUpperDest,
    ColdPushDest,
    ColdSourceH,
    ColdSourceL,
    ColdPopTarget,
    UpperBulkCopy,
    UpperJump,
    WarmPopBank,
    WarmPopDest,
    WarmUpperDest,
    WarmBankB,
    WarmTiles,
    CallVideo,
    UpperVideoCopy,
    EndPopHl,
    EndPopBc,
    EndJump,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndoorSpriteTransferWork {
    step: Step,
    sheet_return: Step,
    picture: u8,
    slot: u8,
    font: u8,
    four_tile_count: u8,
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    carry: bool,
    zero: bool,
    stack: [u16; 8],
    sp: u8,
}
impl IndoorSpriteTransferWork {
    pub fn new(
        index: u8,
        remaining: u8,
        slot: SpriteSlot,
        font: u8,
        four_tile_count: u8,
    ) -> Option<Self> {
        sprite_sheet_metadata(slot.picture)?;
        if index >= 15
            || remaining == 0
            || remaining > 15 - index
            || slot.vram_slot < 2
            || four_tile_count > 1
        {
            return None;
        }
        Some(Self {
            step: Step::Picture,
            sheet_return: Step::Done,
            picture: slot.picture,
            slot: slot.vram_slot,
            font,
            four_tile_count,
            a: slot.vram_slot,
            b: slot.picture,
            c: remaining,
            d: 0,
            e: 0,
            h: 0xc2,
            l: 0x1e + index * 16,
            carry: false,
            zero: false,
            stack: [0; 8],
            sp: 0,
        })
    }
    fn metadata(&self) -> SpriteSheetMetadata {
        sprite_sheet_metadata(self.picture).expect("validated sprite picture")
    }
    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    fn set_hl(&mut self, n: u16) {
        [self.h, self.l] = n.to_be_bytes();
    }
    fn set_de(&mut self, n: u16) {
        [self.d, self.e] = n.to_be_bytes();
    }
    fn set_bc(&mut self, n: u16) {
        [self.b, self.c] = n.to_be_bytes();
    }
    fn push(&mut self, n: u16) {
        self.stack[usize::from(self.sp)] = n;
        self.sp += 1;
    }
    fn pop(&mut self) -> u16 {
        self.sp -= 1;
        self.stack[usize::from(self.sp)]
    }
    fn push_af(&mut self) {
        self.push(u16::from_be_bytes([
            self.a,
            u8::from(self.zero) * 0x80 + u8::from(self.carry) * 0x10,
        ]));
    }
    fn pop_af(&mut self) {
        let [a, f] = self.pop().to_be_bytes();
        self.a = a;
        self.zero = f & 0x80 != 0;
        self.carry = f & 0x10 != 0;
    }
    fn add_a(&mut self, n: u8) {
        let sum = u16::from(self.a) + u16::from(n);
        self.a = sum as u8;
        self.carry = sum > 255;
        self.zero = self.a == 0;
    }
    fn compare(&mut self, n: u8) {
        self.zero = self.a == n;
        self.carry = self.a < n;
    }
    fn sheet_byte(&mut self) -> u8 {
        let m = self.metadata();
        let start = 0x7b27 + u16::from(self.picture - 1) * 4;
        let bytes = [m.address as u8, (m.address >> 8) as u8, m.bytes, m.bank];
        let byte = bytes[usize::from(self.hl().checked_sub(start).expect("source table pointer"))];
        self.set_hl(self.hl().wrapping_add(1));
        byte
    }
    pub fn copy(&self) -> Option<PatternCopy> {
        use Step::*;
        let (source, destination, tiles) = match self.step {
            LowerCopy | UpperBulkCopy => (self.hl(), self.de(), (self.bc() / 16) as u8),
            UpperVideoCopy => (self.de(), self.hl(), self.c),
            _ => return None,
        };
        Some(PatternCopy {
            source: PatternSource::Sprite(self.picture),
            source_offset: source
                .checked_sub(self.metadata().address)
                .expect("source sprite address"),
            destination,
            tiles,
            format: CopyFormat::TwoBpp,
        })
    }
    pub fn copy_returned(&mut self) -> bool {
        use Step::*;
        self.step = match self.step {
            LowerCopy => BetweenPopDest,
            UpperBulkCopy => UpperJump,
            UpperVideoCopy => EndPopHl,
            _ => return false,
        };
        true
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    pub fn four_tile_count(&self) -> u8 {
        self.four_tile_count
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        let tcycles = match self.step {
            LowerCopy | UpperBulkCopy | UpperVideoCopy | Done => return Ok(false),
            PushParentBc | PushParentHl | PushTable | PushBank1 | PushSource1 | PushLength
            | PushDest1 | PushDest2 | PushDest3 | PushBank2 | ColdPushDest | ReadFont1
            | ReadFont2 => 16,
            CallSheet1 | CallSheet2 | CallVideo => 24,
            SheetReturn => 16,
            Table | VramBase | SlotBytes | ReadSlot1 | StillBase | ReadFour | FirstStillBase
            | StoreFour | PopLength | PopSource | PopBank | PopDest | BetweenPopDest
            | BetweenPopTable | ReadSlot2 | ColdPopBank | ColdPopDest | ColdPopTarget
            | WarmPopBank | WarmPopDest | EndPopHl | EndPopBc | EndJump | VramJump | UpperJump => {
                12
            }
            TableCarry1 | TableCarry2 | SourceCarry => {
                if !self.carry {
                    12
                } else {
                    8
                }
            }
            StillBranch | SecondBranch => {
                if !self.carry {
                    12
                } else {
                    8
                }
            }
            VramLoop | FourBranch | FontBranch1 | FontBranch2 => {
                if !self.zero {
                    12
                } else {
                    8
                }
            }
            SheetLow | SheetHigh | SheetSize | SheetBank | CompareSlot1 | CompareSlot2
            | TestFont1 | TestFont2 | ClearB | OffsetA | ColdUpperDest | WarmUpperDest
            | WarmTiles => 8,
            VramAdd => 8,
            _ => 4,
        };
        bus.execute(DisplayInstruction {
            tcycles,
            access: None,
        })?;
        self.step = match self.step {
            Picture => {
                self.a = self.b;
                DecrementPicture
            }
            DecrementPicture => {
                self.a = self.a.wrapping_sub(1);
                Double1
            }
            Double1 => {
                self.add_a(self.a);
                Double2
            }
            Double2 => {
                self.add_a(self.a);
                PushParentBc
            }
            PushParentBc => {
                self.push(self.bc());
                PushParentHl
            }
            PushParentHl => {
                self.push(self.hl());
                Table
            }
            Table => {
                self.set_hl(0x7b27);
                TableCarry1
            }
            TableCarry1 => {
                if !self.carry {
                    TableAddLow
                } else {
                    TableHigh1
                }
            }
            TableHigh1 => {
                self.h = self.h.wrapping_add(1);
                TableAddLow
            }
            TableAddLow => {
                self.add_a(self.l);
                TableStoreLow
            }
            TableStoreLow => {
                self.l = self.a;
                TableCarry2
            }
            TableCarry2 => {
                if !self.carry {
                    PushTable
                } else {
                    TableHigh2
                }
            }
            TableHigh2 => {
                self.h = self.h.wrapping_add(1);
                PushTable
            }
            PushTable => {
                self.push(self.hl());
                CallSheet1
            }
            CallSheet1 => {
                self.sheet_return = PushBank1;
                SheetLow
            }
            SheetLow => {
                self.a = self.sheet_byte();
                SheetE
            }
            SheetE => {
                self.e = self.a;
                SheetHigh
            }
            SheetHigh => {
                self.a = self.sheet_byte();
                SheetD
            }
            SheetD => {
                self.d = self.a;
                SheetSize
            }
            SheetSize => {
                self.a = self.sheet_byte();
                SheetC
            }
            SheetC => {
                self.c = self.a;
                SheetZero
            }
            SheetZero => {
                self.a = 0;
                self.zero = true;
                self.carry = false;
                SheetB
            }
            SheetB => {
                self.b = self.a;
                SheetBank
            }
            SheetBank => {
                self.a = self.sheet_byte();
                SheetReturn
            }
            SheetReturn => self.sheet_return,
            PushBank1 => {
                self.push_af();
                PushSource1
            }
            PushSource1 => {
                self.push(self.de());
                PushLength
            }
            PushLength => {
                self.push(self.bc());
                VramBase
            }
            VramBase => {
                self.set_hl(0x8000);
                SlotBytes
            }
            SlotBytes => {
                self.set_bc(192);
                ReadSlot1
            }
            ReadSlot1 => {
                self.a = self.slot;
                CompareSlot1
            }
            CompareSlot1 => {
                self.compare(11);
                StillBranch
            }
            StillBranch => {
                if !self.carry {
                    StillBase
                } else {
                    SlotD
                }
            }
            SlotD => {
                self.d = self.a;
                SlotDecrement
            }
            SlotDecrement => {
                self.d = self.d.wrapping_sub(1);
                VramAdd
            }
            VramAdd => {
                self.set_hl(self.hl().wrapping_add(self.bc()));
                VramDecrement
            }
            VramDecrement => {
                self.d = self.d.wrapping_sub(1);
                self.zero = self.d == 0;
                VramLoop
            }
            VramLoop => {
                if !self.zero {
                    VramAdd
                } else {
                    VramJump
                }
            }
            VramJump => PopLength,
            StillBase => {
                self.set_hl(0x87c0);
                ReadFour
            }
            ReadFour => {
                self.a = self.four_tile_count;
                TestFour
            }
            TestFour => {
                self.zero = self.a == 0;
                self.carry = false;
                FourBranch
            }
            FourBranch => {
                if !self.zero {
                    PopLength
                } else {
                    FirstStillBase
                }
            }
            FirstStillBase => {
                self.set_hl(0x8780);
                IncrementFour
            }
            IncrementFour => {
                self.a = self.a.wrapping_add(1);
                StoreFour
            }
            StoreFour => {
                self.four_tile_count = self.a;
                PopLength
            }
            PopLength => {
                let v = self.pop();
                self.set_bc(v);
                PopSource
            }
            PopSource => {
                let v = self.pop();
                self.set_de(v);
                PopBank
            }
            PopBank => {
                self.pop_af();
                PushDest1
            }
            PushDest1 => {
                self.push(self.hl());
                PushDest2
            }
            PushDest2 => {
                self.push(self.hl());
                SourceH
            }
            SourceH => {
                self.h = self.d;
                SourceL
            }
            SourceL => {
                self.l = self.e;
                PopDest
            }
            PopDest => {
                let v = self.pop();
                self.set_de(v);
                BankB
            }
            BankB => {
                self.b = self.a;
                ReadFont1
            }
            ReadFont1 => {
                self.a = self.font;
                TestFont1
            }
            TestFont1 => {
                self.zero = self.a & 1 == 0;
                FontBranch1
            }
            FontBranch1 => {
                if !self.zero {
                    BetweenPopDest
                } else {
                    BankA
                }
            }
            BankA => {
                self.a = self.b;
                ClearB
            }
            ClearB => {
                self.b = 0;
                LowerCopy
            }
            BetweenPopDest => {
                let v = self.pop();
                self.set_de(v);
                BetweenPopTable
            }
            BetweenPopTable => {
                let v = self.pop();
                self.set_hl(v);
                ReadSlot2
            }
            ReadSlot2 => {
                self.a = self.slot;
                CompareSlot2
            }
            CompareSlot2 => {
                self.compare(11);
                SecondBranch
            }
            SecondBranch => {
                if !self.carry {
                    EndPopHl
                } else {
                    PushDest3
                }
            }
            PushDest3 => {
                self.push(self.de());
                CallSheet2
            }
            CallSheet2 => {
                self.sheet_return = PushBank2;
                SheetLow
            }
            PushBank2 => {
                self.push_af();
                OffsetA
            }
            OffsetA => {
                self.a = 192;
                AddSourceE
            }
            AddSourceE => {
                self.add_a(self.e);
                SourceE
            }
            SourceE => {
                self.e = self.a;
                SourceCarry
            }
            SourceCarry => {
                if !self.carry {
                    ReadFont2
                } else {
                    SourceHigh
                }
            }
            SourceHigh => {
                self.d = self.d.wrapping_add(1);
                ReadFont2
            }
            ReadFont2 => {
                self.a = self.font;
                TestFont2
            }
            TestFont2 => {
                self.zero = self.a & 1 == 0;
                FontBranch2
            }
            FontBranch2 => {
                if !self.zero {
                    WarmPopBank
                } else {
                    ColdPopBank
                }
            }
            ColdPopBank => {
                self.pop_af();
                ColdPopDest
            }
            ColdPopDest => {
                let v = self.pop();
                self.set_hl(v);
                ColdUpperDest
            }
            ColdUpperDest => {
                self.h |= 8;
                ColdPushDest
            }
            ColdPushDest => {
                self.push(self.hl());
                ColdSourceH
            }
            ColdSourceH => {
                self.h = self.d;
                ColdSourceL
            }
            ColdSourceL => {
                self.l = self.e;
                ColdPopTarget
            }
            ColdPopTarget => {
                let v = self.pop();
                self.set_de(v);
                UpperBulkCopy
            }
            UpperJump => EndPopHl,
            WarmPopBank => {
                self.pop_af();
                WarmPopDest
            }
            WarmPopDest => {
                let v = self.pop();
                self.set_hl(v);
                WarmUpperDest
            }
            WarmUpperDest => {
                self.h |= 8;
                WarmBankB
            }
            WarmBankB => {
                self.b = self.a;
                WarmTiles
            }
            WarmTiles => {
                self.c = self.c.rotate_left(4);
                CallVideo
            }
            CallVideo => UpperVideoCopy,
            EndPopHl => {
                let v = self.pop();
                self.set_hl(v);
                EndPopBc
            }
            EndPopBc => {
                let v = self.pop();
                self.set_bc(v);
                EndJump
            }
            EndJump => Done,
            LowerCopy | UpperBulkCopy | UpperVideoCopy | Done => unreachable!(),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::Vec;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};
    fn drive(mut w: IndoorSpriteTransferWork) -> (Vec<PatternCopy>, u64, IndoorSpriteTransferWork) {
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        let mut copies = Vec::new();
        loop {
            while w.step(&mut bus).unwrap() {}
            if let Some(copy) = w.copy() {
                copies.push(copy);
                assert!(w.copy_returned());
            } else {
                assert!(w.finished());
                break;
            }
        }
        (copies, bus.elapsed(), w)
    }
    #[test]
    fn actual_prepared_addresses_select_cold_and_font_loaded_transfers() {
        for (picture, slot, four, base, tiles, cold_t, warm_t) in [
            (14, 2, 0, 0x80c0, 12, 828, 812),
            (12, 3, 0, 0x8180, 12, 852, 836),
            (6, 4, 0, 0x8240, 12, 876, 860),
            (72, 11, 0, 0x8780, 4, 596, 588),
            (61, 12, 1, 0x87c0, 4, 572, 564),
            (64, 12, 1, 0x87c0, 4, 572, 564),
            (65, 12, 1, 0x87c0, 4, 572, 564),
        ] {
            for font in [0, 1] {
                let (copies, elapsed, w) = drive(
                    IndoorSpriteTransferWork::new(
                        3,
                        4,
                        SpriteSlot {
                            picture,
                            vram_slot: slot,
                        },
                        font,
                        four,
                    )
                    .unwrap(),
                );
                let lower = PatternCopy {
                    source: PatternSource::Sprite(picture),
                    source_offset: 0,
                    destination: base,
                    tiles,
                    format: CopyFormat::TwoBpp,
                };
                let upper = PatternCopy {
                    source: PatternSource::Sprite(picture),
                    source_offset: 192,
                    destination: base + 0x800,
                    tiles: 12,
                    format: CopyFormat::TwoBpp,
                };
                let mut expected = Vec::new();
                if font == 0 {
                    expected.push(lower);
                }
                if slot < 11 {
                    expected.push(upper);
                }
                assert_eq!(elapsed, if font == 0 { cold_t } else { warm_t });
                assert_eq!(copies, expected);
                assert_eq!(w.sp, 0);
                assert_eq!(w.hl(), 0xc24e);
                assert_eq!(w.bc(), u16::from_be_bytes([picture, 4]));
            }
        }
    }
    #[test]
    fn pending_copy_cannot_finish_or_resume_twice_and_snapshot_keeps_stack() {
        let mut w = IndoorSpriteTransferWork::new(
            0,
            7,
            SpriteSlot {
                picture: 14,
                vram_slot: 2,
            },
            0,
            0,
        )
        .unwrap();
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        while bus.remaining() > 100 {
            bus.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        loop {
            let before = w.clone();
            match w.step(&mut bus) {
                Ok(true) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(w, before);
                    break;
                }
                other => panic!("unexpected{other:?}"),
            }
        }
        let saved = serde_json::to_string(&w).unwrap();
        let resumed: IndoorSpriteTransferWork = serde_json::from_str(&saved).unwrap();
        let (a, _, wa) = drive(w);
        let (b, _, wb) = drive(resumed);
        assert_eq!(a, b);
        assert_eq!(wa, wb);
        let mut w = IndoorSpriteTransferWork::new(
            0,
            7,
            SpriteSlot {
                picture: 14,
                vram_slot: 2,
            },
            0,
            0,
        )
        .unwrap();
        bus.begin_native_update();
        while w.step(&mut bus).unwrap() {}
        let start = bus.elapsed();
        let copy = w.copy();
        assert!(!w.step(&mut bus).unwrap());
        assert_eq!(start, bus.elapsed());
        assert_eq!(copy, w.copy());
        assert!(w.copy_returned());
        assert!(!w.copy_returned());
    }
}
