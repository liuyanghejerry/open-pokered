//! Source RedrawRowOrColumn including its two .DrawHalf calls/returns.
//! Raw mode1 selects a column; any other nonzero mode selects two rows.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use super::pattern_copy_work::{PatternAccess, PatternInstruction, PatternInstructionBus};
use crate::alloc_prelude::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Mode,
    Test,
    ReturnZero,
    SetMode,
    Zero,
    Clear,
    DecrementMode,
    Branch,
    Source,
    DestLow,
    SetE,
    DestHigh,
    SetD,
    ColumnCount,
    PushDest,
    CallHalf,
    HalfCount,
    ReadFirst,
    WriteFirst,
    IncrementDest,
    ReadSecond,
    WriteSecond,
    ColumnStride,
    AddE,
    StoreE,
    ColumnCarry,
    IncD,
    ReadD,
    MaskD,
    MapBase,
    StoreD,
    DecColumn,
    ColumnLoop,
    ZeroAgain,
    ClearAgain,
    Return,
    RowReadE,
    RowIncA,
    RowMaskLow,
    RowB,
    RowReadHigh,
    RowMaskHigh,
    RowOr,
    RowStoreE,
    DecRow,
    RowLoop,
    ReturnHalf,
    PopDest,
    LowerStride,
    LowerAdd,
    LowerStore,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BgRedrawWork {
    step: Step,
    mode: u8,
    initial_dest: u16,
    tiles: Vec<u8>,
    at: usize,
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    carry: bool,
    column: bool,
    half: u8,
    saved_dest: u16,
}
impl BgRedrawWork {
    pub fn new(mode: u8, destination: u16, tiles: Vec<u8>) -> Option<Self> {
        if mode != 0 && tiles.len() != if mode == 1 { 36 } else { 40 } {
            return None;
        }
        Some(Self {
            step: Step::Mode,
            mode,
            initial_dest: destination,
            tiles,
            at: 0,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            carry: false,
            column: false,
            half: 0,
            saved_dest: 0,
        })
    }
    fn dest(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    fn set_dest(&mut self, v: u16) {
        [self.d, self.e] = v.to_be_bytes();
    }
    pub fn mode(&self) -> u8 {
        self.mode
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    fn add_e(&mut self) {
        let sum = u16::from(self.a) + u16::from(self.e);
        self.a = sum as u8;
        self.carry = sum > 255;
    }
    pub fn step<B>(&mut self, bus: &mut B) -> Result<bool, <B as DisplayInstructionBus>::Error>
    where
        B: DisplayInstructionBus
            + PatternInstructionBus<Error = <B as DisplayInstructionBus>::Error>,
    {
        use Step::*;
        let t = match self.step {
            Done => return Ok(false),
            Mode | Clear | DestLow | DestHigh | ClearAgain | PopDest => 12,
            ReturnZero => {
                if self.a == 0 {
                    20
                } else {
                    8
                }
            }
            Branch => {
                if self.b != 0 {
                    12
                } else {
                    8
                }
            }
            Source => 12,
            ColumnCount | HalfCount | ReadFirst | WriteFirst | IncrementDest | ReadSecond
            | WriteSecond | ColumnStride | MaskD | MapBase | RowMaskLow | RowMaskHigh
            | LowerStride => 8,
            PushDest | Return | ReturnHalf => 16,
            CallHalf => 24,
            ColumnCarry => {
                if !self.carry {
                    12
                } else {
                    8
                }
            }
            ColumnLoop | RowLoop => {
                if self.c != 0 {
                    12
                } else {
                    8
                }
            }
            _ => 4,
        };
        if matches!(self.step, WriteFirst | WriteSecond) {
            PatternInstructionBus::execute(
                bus,
                PatternInstruction {
                    tcycles: t,
                    access: Some(PatternAccess::Write {
                        destination: self.dest(),
                        value: self.a,
                        at_tcycle: 4,
                    }),
                },
            )?;
        } else {
            DisplayInstructionBus::execute(
                bus,
                DisplayInstruction {
                    tcycles: t,
                    access: None,
                },
            )?;
        }
        self.step = match self.step {
            Mode => {
                self.a = self.mode;
                Test
            }
            Test => ReturnZero,
            ReturnZero => {
                if self.a == 0 {
                    Done
                } else {
                    SetMode
                }
            }
            SetMode => {
                self.b = self.a;
                Zero
            }
            Zero => {
                self.a = 0;
                Clear
            }
            Clear => {
                self.mode = 0;
                DecrementMode
            }
            DecrementMode => {
                self.b = self.b.wrapping_sub(1);
                Branch
            }
            Branch => {
                self.column = self.b == 0;
                Source
            }
            Source => DestLow,
            DestLow => {
                self.a = self.initial_dest as u8;
                SetE
            }
            SetE => {
                self.e = self.a;
                DestHigh
            }
            DestHigh => {
                self.a = (self.initial_dest >> 8) as u8;
                SetD
            }
            SetD => {
                self.d = self.a;
                if self.column {
                    ColumnCount
                } else {
                    PushDest
                }
            }
            ColumnCount => {
                self.c = 18;
                ReadFirst
            }
            PushDest => {
                self.saved_dest = self.dest();
                CallHalf
            }
            CallHalf => HalfCount,
            HalfCount => {
                self.c = 10;
                ReadFirst
            }
            ReadFirst => {
                self.a = self.tiles[self.at];
                self.at += 1;
                WriteFirst
            }
            WriteFirst => IncrementDest,
            IncrementDest => {
                self.set_dest(self.dest().wrapping_add(1));
                ReadSecond
            }
            ReadSecond => {
                self.a = self.tiles[self.at];
                self.at += 1;
                WriteSecond
            }
            WriteSecond => {
                if self.column {
                    ColumnStride
                } else {
                    RowReadE
                }
            }
            ColumnStride => {
                self.a = 31;
                AddE
            }
            AddE => {
                self.add_e();
                StoreE
            }
            StoreE => {
                self.e = self.a;
                ColumnCarry
            }
            ColumnCarry => {
                if !self.carry {
                    ReadD
                } else {
                    IncD
                }
            }
            IncD => {
                self.d = self.d.wrapping_add(1);
                ReadD
            }
            ReadD => {
                self.a = self.d;
                MaskD
            }
            MaskD => {
                self.a &= 3;
                MapBase
            }
            MapBase => {
                self.a |= 0x98;
                StoreD
            }
            StoreD => {
                self.d = self.a;
                DecColumn
            }
            DecColumn => {
                self.c = self.c.wrapping_sub(1);
                ColumnLoop
            }
            ColumnLoop => {
                if self.c != 0 {
                    ReadFirst
                } else {
                    ZeroAgain
                }
            }
            ZeroAgain => {
                self.a = 0;
                ClearAgain
            }
            ClearAgain => {
                self.mode = 0;
                Return
            }
            Return => Done,
            RowReadE => {
                self.a = self.e;
                RowIncA
            }
            RowIncA => {
                self.a = self.a.wrapping_add(1);
                RowMaskLow
            }
            RowMaskLow => {
                self.a &= 31;
                RowB
            }
            RowB => {
                self.b = self.a;
                RowReadHigh
            }
            RowReadHigh => {
                self.a = self.e;
                RowMaskHigh
            }
            RowMaskHigh => {
                self.a &= 0xe0;
                RowOr
            }
            RowOr => {
                self.a |= self.b;
                RowStoreE
            }
            RowStoreE => {
                self.e = self.a;
                DecRow
            }
            DecRow => {
                self.c = self.c.wrapping_sub(1);
                RowLoop
            }
            RowLoop => {
                if self.c != 0 {
                    ReadFirst
                } else {
                    ReturnHalf
                }
            }
            ReturnHalf => {
                if self.half == 0 {
                    self.half = 1;
                    PopDest
                } else {
                    Done
                }
            }
            PopDest => {
                self.set_dest(self.saved_dest);
                LowerStride
            }
            LowerStride => {
                self.a = 32;
                LowerAdd
            }
            LowerAdd => {
                self.add_e();
                LowerStore
            }
            LowerStore => {
                self.e = self.a;
                HalfCount
            }
            Done => unreachable!(),
        };
        Ok(true)
    }
}
