//! Resumable, unflipped `_UncompressSpriteData` body.
//!
//! The bus executes individual source instruction lengths. No frame clock or
//! wrapper bank/MMIO effects live here. Scratch bytes and their controlling
//! branches are prepared one bounded block ahead, kept private across refusals,
//! and exposed only after the final RET has executed. An attached IRQ must not
//! read these private scratch bytes. The caller owns the source bank wrapper,
//! resource handoff and actual shared device/interrupt scheduling.
use super::{Error, Work, PLANE_BYTES};
use crate::alloc_prelude::*;
use crate::source_video::{DisplayInstruction, DisplayInstructionBus};

const STORE: &[u8] = &[4, 16, 16, 4, 16, 16, 16];
const READ_BYTE: &[u8] = &[16, 4, 16, 4, 8, 4, 4, 16, 4, 16, 4, 16];
const RESET_RETURN: &[u8] = &[4, 16, 4, 16, 4, 16, 4, 16, 16];
const MAX_BLOCK_INSTRUCTIONS: usize = 256;
const CODES: [[u8; 16]; 2] = [
    [0, 1, 3, 2, 7, 6, 4, 5, 15, 14, 12, 13, 8, 9, 11, 10],
    [15, 14, 12, 13, 8, 9, 11, 10, 0, 1, 3, 2, 7, 6, 4, 5],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Start,
    Clear(usize),
    Header,
    PlanePrefix,
    Mode,
    PlaneStart,
    Literal,
    RunPrefix,
    RunNumber,
    Zero,
    Unpack,
    DiffHead,
    DiffByte,
    DiffReturn,
    XorEntry,
    Xor,
    Done,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StepError<E> {
    Decode(Error),
    Bus(E),
}

/// Owned compressed asset plus 784 private scratch bytes and a bounded queue.
/// This is a source-body continuation, not a complete front-picture loader.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DecompressWork {
    input: Vec<u8>,
    planes: Vec<u8>,
    width: usize,
    height: usize,
    phase: Phase,
    queue: Vec<u8>,
    instruction: usize,
    bits: usize,
    order: usize,
    last_plane: bool,
    cursor: usize,
    ones: usize,
    number_bits: usize,
    number: usize,
    run: usize,
    diff_plane: usize,
    diff_stage: u8,
    row: usize,
    column: usize,
    last_nibble: u8,
    work: Work,
    error: Option<Error>,
    // Passive unit-oracle markers; never part of runtime state or timing.
    #[cfg(test)]
    #[serde(skip)]
    queue_points: Vec<(usize, u8)>,
    #[cfg(test)]
    #[serde(skip)]
    points: Vec<(u8, u64)>,
}

impl DecompressWork {
    pub fn new(data: &[u8]) -> Result<Self, Error> {
        let dimensions = *data.first().ok_or(Error::Truncated)?;
        let (width, height) = (dimensions >> 4, dimensions & 15);
        if !(1..=7).contains(&width) || !(1..=7).contains(&height) {
            return Err(Error::Dimensions);
        }
        Ok(Self {
            input: data.to_vec(),
            planes: vec![0; PLANE_BYTES * 2],
            width: width.into(),
            height: usize::from(height) * 8,
            phase: Phase::Start,
            queue: Vec::with_capacity(MAX_BLOCK_INSTRUCTIONS),
            instruction: 0,
            bits: 8,
            order: 0,
            last_plane: false,
            cursor: 0,
            ones: 0,
            number_bits: 0,
            number: 0,
            run: 0,
            diff_plane: 0,
            diff_stage: 0,
            row: 0,
            column: 0,
            last_nibble: 0,
            work: Work::default(),
            error: None,
            #[cfg(test)]
            queue_points: Vec::new(),
            #[cfg(test)]
            points: Vec::new(),
        })
    }

    pub fn finished(&self) -> bool {
        self.error.is_none() && self.phase == Phase::Done && self.instruction == self.queue.len()
    }
    /// Only instructions actually accepted by the bus count here. No wrapper,
    /// interrupt or device time is included in this foreground receipt.
    pub fn foreground_tcycles(&self) -> u64 {
        self.work.foreground_tcycles
    }
    pub fn dimensions(&self) -> (u8, u8) {
        (self.width as u8, (self.height / 8) as u8)
    }
    /// Plane 0 followed by plane 1. Prepared future/private bytes never escape.
    pub fn output(&self) -> Option<&[u8]> {
        self.finished().then_some(&self.planes)
    }
    pub fn completed_work(&self) -> Option<&Work> {
        self.finished().then_some(&self.work)
    }

    /// Execute one source instruction, or return false after the final RET.
    /// A bus refusal keeps the exact pending instruction for retry/snapshot.
    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<bool, StepError<B::Error>> {
        if let Some(error) = &self.error {
            return Err(StepError::Decode(error.clone()));
        }
        if self.finished() {
            return Ok(false);
        }
        if self.instruction == self.queue.len() {
            self.queue.clear();
            self.instruction = 0;
            #[cfg(test)]
            self.queue_points.clear();
            if let Err(error) = self.prepare() {
                self.error = Some(error.clone());
                return Err(StepError::Decode(error));
            }
        }
        let tcycles = self.queue[self.instruction];
        bus.execute(DisplayInstruction {
            tcycles,
            access: None,
        })
        .map_err(StepError::Bus)?;
        #[cfg(test)]
        for &(index, tag) in &self.queue_points {
            if index == self.instruction {
                self.points.push((tag, self.work.foreground_tcycles));
            }
        }
        self.instruction += 1;
        self.work.foreground_tcycles += u64::from(tcycles);
        Ok(true)
    }

    /// Run until completion or the owner's next budget/IRQ boundary. A bus may
    /// accept an exact plain-instruction prefix only when it has proved that
    /// no IRQ interleaves it. Otherwise this falls back to individual steps.
    pub fn run_until_boundary<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<(), StepError<B::Error>> {
        loop {
            if let Some(error) = &self.error {
                return Err(StepError::Decode(error.clone()));
            }
            if self.finished() {
                return Ok(());
            }
            if self.instruction == self.queue.len() {
                self.step(bus)?;
                continue;
            }
            let accepted = bus
                .execute_plain_run(self.queue[self.instruction..].iter().map(|&tcycles| {
                    DisplayInstruction {
                        tcycles,
                        access: None,
                    }
                }))
                .map_err(StepError::Bus)?;
            assert!(accepted <= self.queue.len() - self.instruction);
            if accepted == 0 {
                self.step(bus)?;
                continue;
            }
            for _ in 0..accepted {
                #[cfg(test)]
                for &(index, tag) in &self.queue_points {
                    if index == self.instruction {
                        self.points.push((tag, self.work.foreground_tcycles));
                    }
                }
                self.work.foreground_tcycles += u64::from(self.queue[self.instruction]);
                self.instruction += 1;
            }
        }
    }

    fn emit(&mut self, instructions: &[u8]) {
        assert!(self.queue.len() + instructions.len() <= MAX_BLOCK_INSTRUCTIONS);
        self.queue.extend_from_slice(instructions);
    }
    fn mark(&mut self, _tag: u8) {
        #[cfg(test)]
        self.queue_points.push((self.queue.len(), _tag));
    }
    fn bit(&mut self) -> Result<u8, Error> {
        let byte = *self.input.get(self.bits / 8).ok_or(Error::Truncated)?;
        self.emit(&[24]); // CALL ReadNextInputBit
        self.mark(0);
        self.emit(&[16, 4]); // LD A,[bit counter]; DEC A
        if self.bits % 8 == 0 {
            self.emit(&[8, 24]);
            self.emit(READ_BYTE);
            self.emit(&[16, 8]);
        } else {
            self.emit(&[12]);
        }
        self.emit(&[16, 16, 4, 16, 8, 16]);
        let value = (byte >> (7 - self.bits % 8)) & 1;
        self.bits += 1;
        self.work.input_bits = self.bits;
        Ok(value)
    }
    fn reset_pointers(&mut self) {
        self.emit(&[16, 8, if self.order ^ 1 != 0 { 12 } else { 8 }, 12, 12]);
        if self.order ^ 1 == 0 {
            self.emit(&[12]);
        }
        self.emit(RESET_RETURN);
    }
    fn write_move(&mut self, group: u8, zero: bool) {
        let row = self.cursor % self.height;
        let pair = self.cursor / self.height % 4;
        let column = self.cursor / (self.height * 4);
        let offset = 3 - pair;
        let plane = self.order ^ usize::from(self.last_plane);
        if zero {
            self.emit(&[4, 4]);
        } // LD B,E; XOR A
        self.emit(&[24]);
        self.mark(1); // WriteSpriteBitsToBuffer entry
        self.emit(&[4, 16, 4]);
        match offset {
            0 => self.emit(&[12]),
            1 => self.emit(&[8, 8, 12, 8, 8, 12]),
            2 => self.emit(&[8, 8, 8, 12, 8]),
            3 => self.emit(&[8, 8, 8, 8, 8, 8, 12]),
            _ => unreachable!(),
        }
        self.emit(&[16, 4, 16, 4, 8, 4, 8, 16]);
        let index = PLANE_BYTES * plane + column * self.height + row;
        self.planes[index] |= group << (offset * 2);
        if zero {
            self.emit(&[4]);
        } // restore E before MoveToNextBufferPosition
        self.emit(&[24]);
        self.mark(2); // MoveToNextBufferPosition entry
        self.emit(&[16, 4, 16, 4, 4, if row + 1 == self.height { 12 } else { 8 }]);
        if row + 1 != self.height {
            let base = if plane == 0 { 0xa188 } else { 0xa310 };
            let carry = (base + column * self.height + row + 1) & 255 == 0;
            self.emit(&[16, 16, 4, 16, if carry { 8 } else { 20 }]);
            if carry {
                self.emit(&[16, 4, 16, 16]);
            }
        } else {
            self.emit(&[4, 16, 16, 4, if offset == 0 { 12 } else { 8 }]);
            if offset != 0 {
                self.emit(&[4, 16, 12, 8, 16, 8, 16, 16]);
            } else {
                self.emit(&[
                    8,
                    16,
                    16,
                    8,
                    16,
                    4,
                    16,
                    4,
                    if column + 1 == self.width { 12 } else { 8 },
                ]);
                if column + 1 != self.width {
                    self.emit(&[16, 4, 16, 4, 8, 16]);
                    self.emit(STORE);
                } else {
                    self.emit(&[12, 4, 16, 16, 8, if self.last_plane { 12 } else { 8 }]);
                    if self.last_plane {
                        self.emit(&[16]);
                    } else {
                        self.emit(&[8, 8, 16, 16]);
                    }
                }
            }
        }
        self.cursor += 1;
    }
    fn after_group(&mut self, zero: bool) {
        if self.cursor == self.width * self.height * 4 {
            if self.last_plane {
                self.phase = Phase::Unpack;
            } else {
                self.last_plane = true;
                self.cursor = 0;
                self.phase = Phase::PlanePrefix;
            }
            return;
        }
        if zero {
            self.emit(&[8, 4, 4, if (self.run - 1) >> 8 != 0 { 12 } else { 8 }]);
            if (self.run - 1) >> 8 == 0 {
                self.emit(&[4, 4]);
            }
            self.run -= 1;
            self.emit(&[if self.run != 0 { 12 } else { 8 }]);
            if self.run == 0 {
                self.emit(&[12]);
                self.phase = Phase::Literal;
            }
        } else {
            self.emit(&[12]);
        }
    }
    fn nibble(&mut self, value: u8, last: u8) -> u8 {
        let odd = value & 1 != 0;
        let previous = last & 1 != 0;
        self.emit(&[24]);
        self.mark(4); // DifferentialDecodeNybble entry
        self.emit(&[8, 8, if odd { 8 } else { 12 }]);
        if odd {
            self.emit(&[8]);
        }
        self.emit(&[4, 16, 4, 12, 8, 4, if previous { 12 } else { 8 }, 16, 4, 16]);
        if !previous {
            self.emit(&[12]);
        }
        // Both pinned lookup tables stay in-page for all eight entry offsets.
        self.emit(&[4, 4, 4, 4, 12, 8, 8, if odd { 12 } else { 8 }]);
        if !odd {
            self.emit(&[8]);
        }
        self.emit(&[8, 4, 16]);
        CODES[usize::from(previous)][usize::from(value)]
    }
    fn begin_diff(&mut self, plane: usize, stage: u8) {
        self.diff_plane = plane;
        self.diff_stage = stage;
        self.row = 0;
        self.column = 0;
        self.last_nibble = 0;
        self.phase = Phase::DiffHead;
    }
    fn prepare(&mut self) -> Result<(), Error> {
        match self.phase {
            Phase::Start => {
                self.emit(&[12, 8, 8, 4, 24, 16, 4]);
                self.phase = Phase::Clear(0);
            }
            Phase::Clear(index) => {
                let done = index + 1 == PLANE_BYTES * 2;
                self.emit(&[4, 8, 8, 4, 4, if done { 8 } else { 12 }]);
                if done {
                    self.emit(&[12, 16]);
                    self.phase = Phase::Header;
                } else {
                    self.phase = Phase::Clear(index + 1);
                }
            }
            Phase::Header => {
                self.emit(&[8, 16, 8, 16, 4, 16, 16, 16, 24]);
                self.emit(READ_BYTE);
                self.emit(&[4, 8, 4, 4, 4, 16, 4, 8, 8, 4, 4, 4, 16]);
                self.order = usize::from(self.bit()?);
                self.emit(&[16]);
                self.phase = Phase::PlanePrefix;
            }
            Phase::PlanePrefix => {
                let plane = self.order ^ usize::from(self.last_plane);
                self.emit(&[12, 16, 8, if plane == 0 { 12 } else { 8 }]);
                if plane != 0 {
                    self.emit(&[12]);
                }
                self.emit(&[24]);
                self.emit(STORE);
                self.emit(&[16, 8, if self.last_plane { 8 } else { 12 }]);
                self.phase = if self.last_plane {
                    Phase::Mode
                } else {
                    Phase::PlaneStart
                };
            }
            Phase::Mode => {
                let first = self.bit()?;
                self.emit(&[4, if first == 0 { 12 } else { 8 }]);
                self.work.mode = if first != 0 {
                    let value = 1 + self.bit()?;
                    self.emit(&[4]);
                    value
                } else {
                    0
                };
                self.emit(&[16]);
                self.phase = Phase::PlaneStart;
            }
            Phase::PlaneStart => {
                let literal = self.bit()? != 0;
                self.emit(&[4, if literal { 8 } else { 12 }]);
                if literal {
                    self.phase = Phase::Literal;
                } else {
                    self.emit(&[8]);
                    self.ones = 0;
                    self.phase = Phase::RunPrefix;
                }
            }
            Phase::Literal => {
                let high = self.bit()?;
                self.emit(&[4]);
                let low = self.bit()?;
                let group = high * 2 + low;
                self.emit(&[8, 4, 4, if group == 0 { 12 } else { 8 }]);
                if group == 0 {
                    self.emit(&[8]);
                    self.ones = 0;
                    self.phase = Phase::RunPrefix;
                } else {
                    self.write_move(group, false);
                    self.work.literal_groups += 1;
                    self.after_group(false);
                }
            }
            Phase::RunPrefix => {
                let bit = self.bit()?;
                self.emit(&[4, if bit == 0 { 12 } else { 8 }]);
                if bit != 0 {
                    self.ones += 1;
                    self.emit(&[4, 12]);
                    if self.ones >= 16 {
                        return Err(Error::RunLength);
                    }
                } else {
                    self.emit(&[4, 4, 12, 4, 4, 12, 8, 4, 8, 16, 4, 8, 4]);
                    self.number = 0;
                    self.number_bits = self.ones + 1;
                    self.phase = Phase::RunNumber;
                }
            }
            Phase::RunNumber => {
                self.number = (self.number << 1) | usize::from(self.bit()?);
                self.number_bits -= 1;
                self.emit(&[4, 4, 4, if self.number_bits == 0 { 12 } else { 8 }]);
                if self.number_bits != 0 {
                    self.emit(&[8, 8, 12]);
                } else {
                    self.emit(&[12, 8, 4, 4]);
                    self.run = (1 << (self.ones + 1)) - 1 + self.number;
                    self.work.zero_packets += 1;
                    self.work.run_prefix_ones += self.ones;
                    self.phase = Phase::Zero;
                }
            }
            Phase::Zero => {
                self.write_move(0, true);
                self.work.zero_groups += 1;
                self.after_group(true);
            }
            Phase::Unpack => {
                let mode = self.work.mode;
                self.emit(&[16, 8, if mode == 2 { 16 } else { 12 }]);
                if mode == 0 {
                    self.emit(&[4, 12, 12, 24]);
                    self.begin_diff(0, 0);
                } else if mode == 2 {
                    self.emit(&[24]);
                    self.reset_pointers();
                    self.emit(&[16, 16, 4, 16, 16, 4, 16, 4, 24]);
                    self.begin_diff(self.order ^ 1, 2);
                } else {
                    self.emit(&[4, 16]);
                    self.phase = Phase::XorEntry;
                }
            }
            Phase::DiffHead => {
                self.emit(&[4, 16, 16, 24]);
                self.emit(STORE);
                self.emit(&[16, 4, 12, 12, 12, 4, 16, 4, 16, 4, 16, 4, 16, 8]);
                self.phase = Phase::DiffByte;
            }
            Phase::DiffByte => {
                self.mark(3); // SpriteDifferentialDecode.decodeNextByteLoop
                let index = self.diff_plane * PLANE_BYTES + self.column * self.height + self.row;
                let byte = self.planes[index];
                self.emit(&[16, 4, 16, 4, 8, 4, 8, 8]);
                let high = self.nibble(byte >> 4, self.last_nibble);
                self.emit(&[8, 4, 4, 8]);
                let low = self.nibble(byte & 15, high);
                self.planes[index] = high << 4 | low;
                self.last_nibble = low;
                self.emit(&[4, 4, 16, 4, 16, 4, 4, 8, 16, 4]);
                let base = if self.diff_plane == 0 { 0xa188 } else { 0xa310 };
                let carry = (base + self.column * self.height + self.row) & 255;
                if carry + self.height > 255 {
                    self.emit(&[8, 4]);
                } else {
                    self.emit(&[12]);
                }
                self.column += 1;
                self.emit(&[
                    16,
                    4,
                    16,
                    16,
                    8,
                    16,
                    4,
                    16,
                    4,
                    if self.column != self.width { 12 } else { 8 },
                ]);
                if self.column == self.width {
                    self.column = 0;
                    self.row += 1;
                    self.last_nibble = 0;
                    self.emit(&[
                        4,
                        4,
                        16,
                        16,
                        4,
                        16,
                        4,
                        16,
                        4,
                        if self.row == self.height { 12 } else { 8 },
                    ]);
                    if self.row != self.height {
                        self.emit(&[16, 4, 16, 4, 8, 24]);
                        self.emit(STORE);
                        self.emit(&[12]);
                    } else {
                        self.emit(&[4, 16, 16]);
                        self.phase = Phase::DiffReturn;
                    }
                }
            }
            Phase::DiffReturn => match self.diff_stage {
                0 => {
                    self.emit(&[12]);
                    self.begin_diff(1, 1);
                }
                1 => unreachable!("second mode-zero decode finishes at its final RET"),
                2 => {
                    self.emit(&[24]);
                    self.reset_pointers();
                    self.emit(&[12, 16, 16]);
                    self.phase = Phase::XorEntry;
                }
                3 => {
                    self.emit(&[24]);
                    self.reset_pointers();
                    self.emit(&[16, 4, 16, 4, 16, 4, 16, 4]);
                    self.column = 0;
                    self.row = 0;
                    self.phase = Phase::Xor;
                }
                _ => unreachable!(),
            },
            Phase::XorEntry => {
                self.emit(&[4, 16, 16, 24]);
                self.reset_pointers();
                self.emit(&[16, 4, 16, 4, 24]);
                self.begin_diff(self.order, 3);
            }
            Phase::Xor => {
                self.mark(5); // XorSpriteChunks.xorChunksLoop
                self.emit(&[
                    16,
                    4,
                    12,
                    8,
                    4,
                    8,
                    4,
                    8,
                    8,
                    16,
                    4,
                    16,
                    4,
                    16,
                    4,
                    if self.row + 1 != self.height { 12 } else { 8 },
                ]);
                let index = self.column * self.height + self.row;
                self.planes[(self.order ^ 1) * PLANE_BYTES + index] ^=
                    self.planes[self.order * PLANE_BYTES + index];
                self.row += 1;
                if self.row == self.height {
                    self.row = 0;
                    self.column += 1;
                    self.emit(&[
                        4,
                        16,
                        16,
                        8,
                        16,
                        4,
                        16,
                        4,
                        if self.column != self.width { 12 } else { 8 },
                    ]);
                    if self.column == self.width {
                        self.emit(&[4, 16, 16]);
                        self.phase = Phase::Done;
                    }
                }
            }
            Phase::Done => unreachable!("completed continuation is not prepared"),
        }
        if self.phase == Phase::DiffReturn && self.diff_stage == 1 {
            self.phase = Phase::Done;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
