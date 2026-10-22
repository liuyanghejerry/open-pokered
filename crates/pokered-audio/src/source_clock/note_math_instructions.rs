//! Retained source arithmetic for next-note handling. These cursors describe CPU
//! work and temporary registers only; they never update native channel/APU state.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum MultiplyStage {
    Init,
    Shift,
    BranchAdd,
    Add,
    ShiftLow,
    RotateHigh,
    Test,
    BranchDone,
    Loop,
    Return,
    Done,
}

/// Audio1/3_MultiplyAdd: HL = L + A * DE, with source wrapping arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct MultiplyAddInstructions {
    stage: MultiplyStage,
    factor: u8,
    value: u16,
    result: u16,
    carry: bool,
}
impl MultiplyAddInstructions {
    pub fn new(factor: u8, value: u16, low: u8) -> Self {
        Self {
            stage: MultiplyStage::Init,
            factor,
            value,
            result: u16::from(low),
            carry: false,
        }
    }
    pub fn result(&self) -> Option<u16> {
        (self.stage == MultiplyStage::Done).then_some(self.result)
    }
    pub fn next_cycles(&self) -> Option<u8> {
        use MultiplyStage::*;
        Some(match self.stage {
            Init | Shift | Add | ShiftLow | RotateHigh => 8,
            BranchAdd => {
                if self.carry {
                    8
                } else {
                    12
                }
            }
            Test => 4,
            BranchDone => {
                if self.factor == 0 {
                    12
                } else {
                    8
                }
            }
            Loop => 12,
            Return => 16,
            Done => return None,
        })
    }
}
impl Iterator for MultiplyAddInstructions {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        use MultiplyStage::*;
        let cycles = self.next_cycles()?;
        self.stage = match self.stage {
            Init => Shift,
            Shift => {
                self.carry = self.factor & 1 != 0;
                self.factor >>= 1;
                BranchAdd
            }
            BranchAdd => {
                if self.carry {
                    Add
                } else {
                    ShiftLow
                }
            }
            Add => {
                self.result = self.result.wrapping_add(self.value);
                ShiftLow
            }
            ShiftLow => {
                self.carry = self.value & 0x80 != 0;
                self.value = (self.value & 0xff00) | u16::from((self.value as u8) << 1);
                RotateHigh
            }
            RotateHigh => {
                self.value = (self.value & 255)
                    | u16::from(((self.value >> 8) as u8).wrapping_shl(1) | u8::from(self.carry))
                        << 8;
                Test
            }
            Test => BranchDone,
            BranchDone => {
                if self.factor == 0 {
                    Return
                } else {
                    Loop
                }
            }
            Loop => Shift,
            Return => Done,
            Done => Done,
        };
        Some(cycles)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum FrequencyStage {
    Prefix(u8),
    Compare,
    BranchDone,
    ShiftHigh,
    RotateLow,
    Increment,
    Loop,
    Suffix(u8),
    Done,
}

/// Audio1/3_CalculateFrequency. Pitch is the actual source table value, not a
/// captured frequency or duration. All octave wraparound branches are retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct FrequencyInstructions {
    stage: FrequencyStage,
    pitch: u16,
    octave: u8,
    carry: bool,
    high: u8,
}
impl FrequencyInstructions {
    pub fn new(pitch: u16, octave: u8) -> Self {
        Self {
            stage: FrequencyStage::Prefix(0),
            pitch,
            octave,
            carry: false,
            high: 0,
        }
    }
    pub fn result(&self) -> Option<u16> {
        (self.stage == FrequencyStage::Done).then_some(self.pitch)
    }
    pub fn carry(&self) -> Option<bool> {
        (self.stage == FrequencyStage::Done).then_some(self.carry)
    }
    pub fn next_cycles(&self) -> Option<u8> {
        use FrequencyStage::*;
        Some(match self.stage {
            Prefix(n) => [8, 4, 8, 4, 4, 12, 8, 8, 8, 8, 4][n as usize],
            Compare => 8,
            BranchDone => {
                if self.octave == 7 {
                    12
                } else {
                    8
                }
            }
            ShiftHigh | RotateLow => 8,
            Increment => 4,
            Loop => 12,
            Suffix(n) => [8, 4, 4, 16][n as usize],
            Done => return None,
        })
    }
}
impl Iterator for FrequencyInstructions {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        use FrequencyStage::*;
        let cycles = self.next_cycles()?;
        self.stage = match self.stage {
            Prefix(n) => {
                if n == 10 {
                    Compare
                } else {
                    Prefix(n + 1)
                }
            }
            Compare => BranchDone,
            BranchDone => {
                if self.octave == 7 {
                    Suffix(0)
                } else {
                    ShiftHigh
                }
            }
            ShiftHigh => {
                let h = (self.pitch >> 8) as u8;
                self.carry = h & 1 != 0;
                self.pitch = (self.pitch & 255) | u16::from(((h as i8) >> 1) as u8) << 8;
                RotateLow
            }
            RotateLow => {
                self.pitch = (self.pitch & 0xff00)
                    | u16::from((self.pitch as u8) >> 1 | if self.carry { 128 } else { 0 });
                Increment
            }
            Increment => {
                self.octave = self.octave.wrapping_add(1);
                Loop
            }
            Loop => Compare,
            Suffix(0) => Suffix(1),
            Suffix(1) => {
                let sum = 8 + (self.pitch >> 8);
                self.carry = sum > 255;
                self.high = sum as u8;
                Suffix(2)
            }
            Suffix(2) => {
                self.pitch = (self.pitch & 255) | u16::from(self.high) << 8;
                Suffix(3)
            }
            Suffix(_) => Done,
            Done => Done,
        };
        Some(cycles)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct NoteLengthInput {
    pub channel: u8,
    pub note: u8,
    pub speed: u8,
    pub fraction: u8,
    pub tempo: u16,
    pub flags1: u8,
    pub flags2: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum LengthStage {
    Prefix(u8),
    First(MultiplyAddInstructions),
    Middle(u8),
    Second(MultiplyAddInstructions),
    Suffix(u8),
    FlagOne(u8),
    NoiseReturn(u8),
    NotePitch,
    Returned,
}

/// Source music note-length body, ending at the next real control boundary:
/// note-pitch entry, or the noise music channel's early RET. It never substitutes
/// a complete note or an IRQ return for those boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct NoteLengthInstructions {
    input: NoteLengthInput,
    stage: LengthStage,
    length: u8,
    computed: u16,
    cycles: u32,
}
impl NoteLengthInstructions {
    pub fn new(input: NoteLengthInput) -> Option<Self> {
        if input.channel >= 4
            || (input.channel == 3 && (input.flags1 & 4 == 0 || input.flags2 & 1 != 0))
            || (input.channel != 3 && input.flags2 & 1 == 0 && input.flags1 & 4 != 0)
        {
            return None;
        }
        Some(Self {
            input,
            stage: LengthStage::Prefix(0),
            length: 0,
            computed: 0,
            cycles: 0,
        })
    }
    /// Fractional low byte and delay high byte, in source storage order.
    pub fn result(&self) -> Option<(u8, u8)> {
        self.finished()
            .then_some((self.computed as u8, (self.computed >> 8) as u8))
    }
    pub fn finished(&self) -> bool {
        matches!(self.stage, LengthStage::NotePitch | LengthStage::Returned)
    }
    pub fn returned(&self) -> bool {
        self.stage == LengthStage::Returned
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn next_cycles(&self) -> Option<u8> {
        use LengthStage::*;
        Some(match self.stage {
            Prefix(n) => [4, 16, 8, 4, 8, 4, 4, 12, 8, 8, 4, 24][n as usize],
            First(w) | Second(w) => w.next_cycles()?,
            Middle(n) => [4, 8, 8, 16, 4, 16, 4, 12, 4, 8, 12, 8, 8, 24][n as usize],
            Suffix(n) => {
                if n < 12 {
                    [4, 4, 12, 8, 8, 4, 12, 8, 8, 12, 8, 12][n as usize]
                } else if self.input.flags2 & 1 != 0 {
                    12
                } else {
                    8
                }
            }
            FlagOne(n) => {
                if n < 3 {
                    [12, 8, 12][n as usize]
                } else if self.input.flags1 & 4 == 0 {
                    12
                } else {
                    8
                }
            }
            NoiseReturn(n) => [12, 16][n as usize],
            NotePitch | Returned => return None,
        })
    }
    /// An execution refusal leaves the instruction and temporary registers intact.
    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        let Some(cycles) = self.next_cycles() else {
            return Ok(true);
        };
        execute(cycles)?;
        self.advance();
        self.cycles += u32::from(cycles);
        Ok(self.finished())
    }
    fn advance(&mut self) {
        use LengthStage::*;
        self.stage = match self.stage {
            Prefix(n) => {
                if n == 11 {
                    First(MultiplyAddInstructions::new(
                        self.input.speed,
                        u16::from((self.input.note & 15) + 1),
                        0,
                    ))
                } else {
                    Prefix(n + 1)
                }
            }
            First(mut w) => {
                w.next();
                if let Some(result) = w.result() {
                    self.length = result as u8;
                    Middle(0)
                } else {
                    First(w)
                }
            }
            Middle(n) => {
                if n == 13 {
                    Second(MultiplyAddInstructions::new(
                        self.length,
                        self.input.tempo,
                        self.input.fraction,
                    ))
                } else {
                    Middle(n + 1)
                }
            }
            Second(mut w) => {
                w.next();
                if let Some(result) = w.result() {
                    self.computed = result;
                    Suffix(0)
                } else {
                    Second(w)
                }
            }
            Suffix(n) => {
                if n == 12 {
                    if self.input.flags2 & 1 != 0 {
                        NotePitch
                    } else {
                        FlagOne(0)
                    }
                } else {
                    Suffix(n + 1)
                }
            }
            FlagOne(n) => {
                if n == 3 {
                    if self.input.flags1 & 4 == 0 {
                        NotePitch
                    } else {
                        NoiseReturn(0)
                    }
                } else {
                    FlagOne(n + 1)
                }
            }
            NoiseReturn(n) => {
                if n == 1 {
                    Returned
                } else {
                    NoiseReturn(1)
                }
            }
            NotePitch => NotePitch,
            Returned => Returned,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    #[derive(serde::Deserialize)]
    struct MultiplyCase {
        bank: u8,
        factor: u8,
        value: u16,
        low: u8,
        result: u16,
        trace: Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct FrequencyCase {
        bank: u8,
        pitch: u16,
        octave: u8,
        result: u16,
        carry: bool,
        trace: Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct LengthCase {
        bank: u8,
        input: NoteLengthInput,
        result: (u8, u8),
        returned: bool,
        trace: Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        multiply: Vec<MultiplyCase>,
        frequency: Vec<FrequencyCase>,
        length: Vec<LengthCase>,
    }
    fn fixture() -> Fixture {
        serde_json::from_str(include_str!("note_math_instructions_golden.json")).unwrap()
    }
    #[test]
    fn multiply_instructions_and_results_match_pinned_rom_in_both_banks() {
        let cases = fixture().multiply;
        assert_eq!(cases.len(), 1044);
        for (index, c) in cases.into_iter().enumerate() {
            assert!(matches!(c.bank, 2 | 31));
            let mut w = MultiplyAddInstructions::new(c.factor, c.value, c.low);
            assert_eq!(w.result(), None);
            for (pc, t) in c.trace {
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                assert_eq!(
                    w.next_cycles(),
                    Some(t),
                    "multiply case {index} PC {pc:04x}"
                );
                assert_eq!(w.next(), Some(t));
            }
            assert_eq!(w.next(), None);
            assert_eq!(w.result(), Some(c.result));
        }
    }
    #[test]
    fn frequency_shift_and_octave_wrap_instructions_match_pinned_rom() {
        let cases = fixture().frequency;
        assert_eq!(cases.len(), 264);
        for (index, c) in cases.into_iter().enumerate() {
            assert!(matches!(c.bank, 2 | 31));
            let mut w = FrequencyInstructions::new(c.pitch, c.octave);
            assert_eq!(w.result(), None);
            for (pc, t) in c.trace {
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                assert_eq!(
                    w.next_cycles(),
                    Some(t),
                    "frequency case {index} PC {pc:04x}"
                );
                assert_eq!(w.next(), Some(t));
            }
            assert_eq!(w.next(), None);
            assert_eq!(w.result(), Some(c.result));
            assert_eq!(w.carry(), Some(c.carry));
        }
    }
    #[test]
    fn note_length_refusals_and_resumptions_match_every_source_instruction() {
        let cases = fixture().length;
        assert_eq!(cases.len(), 778);
        for (index, c) in cases.into_iter().enumerate() {
            assert!(matches!(c.bank, 2 | 31));
            let mut w = NoteLengthInstructions::new(c.input).unwrap();
            let mut cycles = 0;
            for (pc, t) in c.trace {
                assert!(!w.finished());
                assert_eq!(w.result(), None);
                let frozen = w;
                assert_eq!(w.try_step(|_| Err("defer")), Err("defer"));
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                w.try_step(|actual| {
                    assert_eq!(actual, t, "length case {index} PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                cycles += u32::from(t);
                assert_eq!(w.cycles(), cycles);
            }
            assert!(w.finished());
            assert_eq!(w.returned(), c.returned);
            assert_eq!(w.result(), Some(c.result));
            assert!(w
                .try_step(|_| -> Result<(), ()> { panic!("completed source boundary") })
                .unwrap());
        }
    }
    #[test]
    fn unsupported_channel_modes_fail_before_accepting_work() {
        let input = NoteLengthInput {
            channel: 0,
            note: 0x40,
            speed: 6,
            fraction: 0,
            tempo: 0x100,
            flags1: 0,
            flags2: 0,
        };
        for channel in 4..=255 {
            assert!(NoteLengthInstructions::new(NoteLengthInput { channel, ..input }).is_none());
        }
        assert!(NoteLengthInstructions::new(NoteLengthInput {
            channel: 3,
            ..input
        })
        .is_none());
        assert!(NoteLengthInstructions::new(NoteLengthInput {
            channel: 3,
            flags1: 4,
            flags2: 1,
            ..input
        })
        .is_none());
        assert!(NoteLengthInstructions::new(NoteLengthInput { flags1: 4, ..input }).is_none());
    }
}
