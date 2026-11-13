//! Retained Audio1/3 slide initialization and continuation calls. These own
//! scalar source work only; the enclosing frame owns the APU and native commit.
use super::apu_bus::ApuInstruction;
use super::channel_output_instructions::bus::ApuWrite;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct PitchSlideState {
    pub length_modifier: u8,
    /// Native snapshots retain the full value; source admission requires a byte.
    pub step: u16,
    pub step_fraction: u8,
    pub current_fraction: u8,
    pub current: u16,
    pub target: u16,
}
impl PitchSlideState {
    pub(crate) fn from_native(state: &crate::PitchSlideState) -> Self {
        Self {
            length_modifier: state.length_modifier,
            step: state.freq_step,
            step_fraction: state.step_frac,
            current_fraction: state.freq_frac,
            current: state.current_freq,
            target: state.target_freq,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct PitchSlideResult {
    pub state: PitchSlideState,
    pub flags1: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct InitPitchSlideInput {
    pub channel: u8,
    pub current_frequency: u16,
    pub target_frequency: u16,
    pub note_delay: u8,
    pub length_modifier: u8,
    pub flags1: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct InitPitchSlideResult {
    pub state: PitchSlideState,
    pub flags1: u8,
    /// Division temporaries returned in DE, distinct from state.current.
    pub returned_frequency: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum InitStage {
    Entry(u8),
    Underflow,
    Direction(u8),
    Increasing(u8),
    Decreasing(u8),
    DivideSetup(u8),
    Divide(u8),
    HighTest(u8),
    HighDecrement(u8),
    Finish(u8),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct InitPitchSlideInstructions {
    input: InitPitchSlideInput,
    stage: InitStage,
    frames: u8,
    decreasing: bool,
    division: u16,
    quotient: u8,
    borrow: bool,
    cycles: u32,
}

impl InitPitchSlideInstructions {
    pub fn new(input: InitPitchSlideInput) -> Option<Self> {
        let frames = input
            .note_delay
            .checked_sub(input.length_modifier)
            .unwrap_or(1);
        // A zero source divisor loops forever. Do not invent a source RET.
        if input.channel >= 3 || frames == 0 || input.flags1 & 16 == 0 {
            return None;
        }
        let current = input.current_frequency;
        let target = input.target_frequency;
        let (low, borrow) = (current as u8).overflowing_sub(target as u8);
        let high = ((current >> 8) as u8).wrapping_sub(u8::from(borrow));
        let (high, increasing) = high.overflowing_sub((target >> 8) as u8);
        let division = if increasing {
            let (low, borrow) = (target as u8).overflowing_sub(current as u8);
            // Source borrows from the current high byte before subtracting it.
            let high = ((target >> 8) as u8)
                .wrapping_sub(((current >> 8) as u8).wrapping_sub(u8::from(borrow)));
            u16::from(high) << 8 | u16::from(low)
        } else {
            u16::from(high) << 8 | u16::from(low)
        };
        Some(Self {
            input,
            stage: InitStage::Entry(0),
            frames,
            decreasing: !increasing,
            division,
            quotient: 0,
            borrow: false,
            cycles: 0,
        })
    }

    pub fn result(&self) -> Option<InitPitchSlideResult> {
        matches!(self.stage, InitStage::Done).then(|| {
            let remainder = (self.division as u8).wrapping_add(self.frames);
            InitPitchSlideResult {
                state: PitchSlideState {
                    length_modifier: self.frames,
                    step: u16::from(self.quotient),
                    step_fraction: remainder,
                    current_fraction: remainder,
                    current: self.input.current_frequency,
                    target: self.input.target_frequency,
                },
                flags1: (self.input.flags1 & !32) | if self.decreasing { 32 } else { 0 },
                returned_frequency: u16::from(self.quotient) << 8 | (self.division & 255),
            }
        })
    }

    pub fn cycles(&self) -> u32 {
        self.cycles
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        use InitStage::*;
        Some(match self.stage {
            Entry(12) => {
                if self.input.note_delay < self.input.length_modifier {
                    8
                } else {
                    12
                }
            }
            Entry(n) => [12, 8, 8, 12, 8, 8, 12, 8, 8, 12, 8, 8][n as usize],
            Underflow => 8,
            Direction(11) => {
                if self.decreasing {
                    8
                } else {
                    12
                }
            }
            Direction(n) => [8, 12, 8, 4, 8, 4, 4, 4, 12, 8, 8][n as usize],
            Decreasing(n) => [4, 8, 12, 8, 16, 12][n as usize],
            Increasing(n) => [
                12, 8, 8, 12, 8, 8, 12, 8, 8, 4, 4, 4, 4, 4, 12, 8, 8, 4, 4, 8, 12, 8, 16,
            ][n as usize],
            DivideSetup(n) => [12, 8][n as usize],
            Divide(4) => {
                if self.borrow {
                    8
                } else {
                    12
                }
            }
            Divide(n) => [4, 4, 8, 4][n as usize],
            HighTest(2) => {
                if self.division >> 8 == 0 {
                    12
                } else {
                    8
                }
            }
            HighTest(_) => 4,
            HighDecrement(n) => [4, 4, 12][n as usize],
            Finish(n) => [4, 8, 4, 8, 12, 8, 8, 12, 8, 8, 12, 8, 8, 16][n as usize],
            Done => return None,
        })
    }

    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        let Some(t) = self.peek() else {
            return Ok(true);
        };
        execute(t)?;
        self.accept();
        Ok(self.result().is_some())
    }

    pub(crate) fn accept(&mut self) {
        use InitStage::*;
        let t = self.peek().expect("unfinished slide initialization");
        self.stage = match self.stage {
            Entry(12) => {
                if self.input.note_delay < self.input.length_modifier {
                    Underflow
                } else {
                    Direction(0)
                }
            }
            Entry(n) => Entry(n + 1),
            Underflow => Direction(0),
            Direction(11) => {
                if self.decreasing {
                    Decreasing(0)
                } else {
                    Increasing(0)
                }
            }
            Direction(n) => Direction(n + 1),
            Decreasing(5) | Increasing(22) => DivideSetup(0),
            Decreasing(n) => Decreasing(n + 1),
            Increasing(n) => Increasing(n + 1),
            DivideSetup(1) => Divide(0),
            DivideSetup(n) => DivideSetup(n + 1),
            Divide(0) => {
                self.quotient = self.quotient.wrapping_add(1);
                Divide(1)
            }
            Divide(2) => {
                let (low, borrow) = (self.division as u8).overflowing_sub(self.frames);
                self.division = (self.division & 0xff00) | u16::from(low);
                self.borrow = borrow;
                Divide(3)
            }
            Divide(4) => {
                if self.borrow {
                    HighTest(0)
                } else {
                    Divide(0)
                }
            }
            Divide(n) => Divide(n + 1),
            HighTest(2) => {
                if self.division >> 8 == 0 {
                    Finish(0)
                } else {
                    HighDecrement(0)
                }
            }
            HighTest(n) => HighTest(n + 1),
            HighDecrement(1) => {
                self.division -= 0x100;
                HighDecrement(2)
            }
            HighDecrement(2) => Divide(0),
            HighDecrement(n) => HighDecrement(n + 1),
            Finish(13) => Done,
            Finish(n) => Finish(n + 1),
            Done => unreachable!(),
        };
        self.cycles += u32::from(t);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum SlideStage {
    Entry(u8),
    Increasing(u8),
    IncreasingHigh(u8),
    IncreasingLow(u8),
    Decreasing(u8),
    DecreasingHigh(u8),
    DecreasingLow(u8),
    Store(u8),
    Register(u8),
    Write(u8),
    Stop(u8),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct PitchSlideInstructions {
    channel: u8,
    result: PitchSlideResult,
    decreasing: bool,
    next: u16,
    fraction: u8,
    stage: SlideStage,
    cycles: u32,
}

impl PitchSlideInstructions {
    pub fn new(channel: u8, state: PitchSlideState, flags1: u8) -> Option<Self> {
        if channel >= 3 || flags1 & 16 == 0 || state.step > 255 {
            return None;
        }
        let decreasing = flags1 & 32 != 0;
        let (fraction, carry) = if decreasing {
            state.step_fraction.overflowing_mul(2)
        } else {
            state.current_fraction.overflowing_add(state.step_fraction)
        };
        let step = state.step + u16::from(carry);
        let next = if decreasing {
            state.current.wrapping_sub(step)
        } else {
            state.current.wrapping_add(step)
        };
        Some(Self {
            channel,
            result: PitchSlideResult { state, flags1 },
            decreasing,
            next,
            fraction,
            stage: SlideStage::Entry(0),
            cycles: 0,
        })
    }
    pub fn result(&self) -> Option<PitchSlideResult> {
        matches!(self.stage, SlideStage::Done).then_some(self.result)
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    fn crossed_high(&self) -> bool {
        if self.decreasing {
            self.next >> 8 < self.result.state.target >> 8
        } else {
            self.next >> 8 > self.result.state.target >> 8
        }
    }
    fn different_high(&self) -> bool {
        self.next >> 8 != self.result.state.target >> 8
    }
    fn crossed_low(&self) -> bool {
        if self.decreasing {
            (self.next as u8) < self.result.state.target as u8
        } else {
            (self.next as u8) > self.result.state.target as u8
        }
    }
    pub(crate) fn peek(&self) -> Option<u8> {
        use SlideStage::*;
        Some(match self.stage {
            Entry(3) => {
                if self.decreasing {
                    16
                } else {
                    12
                }
            }
            Entry(n) => [12, 8, 12][n as usize],
            Increasing(n) => [
                12, 8, 8, 12, 8, 8, 12, 8, 8, 4, 8, 4, 4, 12, 8, 16, 12, 8, 8, 12, 8, 8, 8, 4, 4,
                8, 4, 4, 12, 8, 8, 4,
            ][n as usize],
            IncreasingHigh(0) => {
                if self.crossed_high() {
                    16
                } else {
                    12
                }
            }
            IncreasingHigh(_) | DecreasingHigh(1) => {
                if self.different_high() {
                    12
                } else {
                    8
                }
            }
            IncreasingLow(4) => {
                if self.crossed_low() {
                    16
                } else {
                    12
                }
            }
            IncreasingLow(n) => [12, 8, 8, 4, 0, 12][n as usize],
            Decreasing(n) => [
                12, 8, 8, 12, 8, 8, 12, 8, 8, 4, 4, 4, 4, 4, 12, 8, 8, 4, 8, 4, 4, 4, 4, 4, 4, 12,
                8, 4, 8,
            ][n as usize],
            DecreasingHigh(_) => {
                if self.crossed_high() {
                    12
                } else {
                    8
                }
            }
            DecreasingLow(4) => {
                if self.crossed_low() {
                    12
                } else {
                    8
                }
            }
            DecreasingLow(n) => [12, 8, 4, 8][n as usize],
            Store(n) => [12, 8, 8, 12, 8, 8, 8, 24][n as usize],
            // Both source banks' hardware-base tables have no carry for 0..2.
            Register(n) => [4, 12, 4, 12, 4, 8, 4, 4, 8, 16][n as usize],
            Write(n) => [4, 8, 8, 16][n as usize],
            Stop(n) => [12, 8, 16, 16, 16][n as usize],
            Done => return None,
        })
    }
    pub(crate) fn pending_apu_write(&self) -> Option<ApuWrite> {
        let base = [0xff13, 0xff18, 0xff1d][usize::from(self.channel)];
        match self.stage {
            SlideStage::Write(1) => Some(ApuWrite {
                address: base,
                value: self.next as u8,
            }),
            SlideStage::Write(2) => Some(ApuWrite {
                address: base + 1,
                value: (self.next >> 8) as u8,
            }),
            _ => None,
        }
    }
    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        self.try_step_with_bus(|instruction| execute(instruction.tcycles))
    }
    pub fn try_step_with_bus<E>(
        &mut self,
        execute: impl FnOnce(ApuInstruction) -> Result<(), E>,
    ) -> Result<bool, E> {
        let Some(t) = self.peek() else {
            return Ok(true);
        };
        let instruction = if let Some(write) = self.pending_apu_write() {
            ApuInstruction::write(t, write.address, write.value)
        } else {
            ApuInstruction::plain(t)
        };
        execute(instruction)?;
        self.accept();
        Ok(self.result().is_some())
    }
    pub(crate) fn accept(&mut self) {
        use SlideStage::*;
        let t = self.peek().expect("unfinished pitch slide");
        self.stage = match self.stage {
            Entry(3) => {
                if self.decreasing {
                    Decreasing(0)
                } else {
                    Increasing(0)
                }
            }
            Entry(n) => Entry(n + 1),
            Increasing(21) => {
                self.result.state.current_fraction = self.fraction;
                Increasing(22)
            }
            Increasing(31) => IncreasingHigh(0),
            Increasing(n) => Increasing(n + 1),
            IncreasingHigh(0) => {
                if self.crossed_high() {
                    Stop(0)
                } else {
                    IncreasingHigh(1)
                }
            }
            IncreasingHigh(1) => {
                if self.different_high() {
                    Store(0)
                } else {
                    IncreasingLow(0)
                }
            }
            IncreasingHigh(_) => unreachable!(),
            IncreasingLow(4) => {
                if self.crossed_low() {
                    Stop(0)
                } else {
                    IncreasingLow(5)
                }
            }
            IncreasingLow(5) => Store(0),
            IncreasingLow(n) => IncreasingLow(n + 1),
            Decreasing(18) => {
                self.result.state.step_fraction = self.fraction;
                Decreasing(19)
            }
            Decreasing(28) => DecreasingHigh(0),
            Decreasing(n) => Decreasing(n + 1),
            DecreasingHigh(0) => {
                if self.crossed_high() {
                    Stop(0)
                } else {
                    DecreasingHigh(1)
                }
            }
            DecreasingHigh(1) => {
                if self.different_high() {
                    Store(0)
                } else {
                    DecreasingLow(0)
                }
            }
            DecreasingHigh(_) => unreachable!(),
            DecreasingLow(4) => {
                if self.crossed_low() {
                    Stop(0)
                } else {
                    Store(0)
                }
            }
            DecreasingLow(n) => DecreasingLow(n + 1),
            Store(2) => {
                self.result.state.current =
                    (self.result.state.current & 0xff00) | (self.next & 255);
                Store(3)
            }
            Store(5) => {
                self.result.state.current = self.next;
                Store(6)
            }
            Store(7) => Register(0),
            Store(n) => Store(n + 1),
            Register(9) => Write(0),
            Register(n) => Register(n + 1),
            Write(3) => Done,
            Write(n) => Write(n + 1),
            Stop(2) => {
                self.result.flags1 &= !16;
                Stop(3)
            }
            Stop(3) => {
                self.result.flags1 &= !32;
                Stop(4)
            }
            Stop(4) => Done,
            Stop(n) => Stop(n + 1),
            Done => unreachable!(),
        };
        self.cycles += u32::from(t);
    }
}

#[cfg(test)]
#[path = "pitch_slide_instruction_tests.rs"]
mod tests;
