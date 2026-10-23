//! Retained Audio1/3 hardware-channel setup CPU work. This component does not
//! perform physical WRAM/APU writes or tick the native sequencer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum OutputRoutine {
    RegisterPointer,
    DutyAndLength,
    EnableChannel,
    WaveFrequency,
}

#[path = "channel_output_bus.rs"]
pub mod bus;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct OutputInput {
    pub bank: u8,
    pub channel: u8,
    pub paired_sfx_active: bool,
    pub source_sfx_id: u8,
    pub frequency_modifier: u8,
    pub frequency: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum RegisterReturn {
    Finish,
    DutyStore,
    WaveStore,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Register(RegisterReturn, u8),
    DutyPrefix(u8),
    DutySfx(u8),
    DutyInclude(u8),
    DutyCall(u8),
    DutyStore(u8),
    EnablePrefix(u8),
    EnableSfx(u8),
    EnablePair(u8),
    Panning(u8),
    EnableStore(u8),
    WaveKind(u8),
    WaveSfx(u8),
    WaveSetup(u8),
    SfxInstrument,
    WaveData(u8),
    WaveCopy(u8, u8),
    WaveEnd(u8),
    FrequencyHeader(u8),
    WaveStore(u8),
    ModifierCall,
    CryPrefix(u8),
    CryEnd(u8),
    CryYes,
    CryNo(u8),
    ModifierBranch,
    Modifier(u8),
    ModifierCarry,
    ModifierStore(u8),
    ModifierReturn,
    WaveReturn,
    Done,
}
/// Each step accepts one actual source instruction, or retains it untouched.
/// Bank/mute/IRQ/native-owner validation remains with the enclosing audio owner.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ChannelOutputInstructions {
    input: OutputInput,
    routine: OutputRoutine,
    stage: Stage,
    frequency: u16,
    carry: bool,
    low: u8,
    cycles: u32,
    /// Most recent source NR51 read, retained across later ALU/store instructions.
    #[cfg_attr(any(test, feature = "source-clock-serde"), serde(default))]
    nr51: Option<u8>,
}
impl ChannelOutputInstructions {
    pub fn new(routine: OutputRoutine, input: OutputInput) -> Option<Self> {
        if !matches!(input.bank, 2 | 31) || input.channel >= 8 {
            return None;
        }
        let stage = match routine {
            OutputRoutine::RegisterPointer => Stage::Register(RegisterReturn::Finish, 0),
            OutputRoutine::DutyAndLength => Stage::DutyPrefix(0),
            OutputRoutine::EnableChannel => Stage::EnablePrefix(0),
            OutputRoutine::WaveFrequency => Stage::WaveKind(0),
        };
        Some(Self {
            input,
            routine,
            stage,
            frequency: input.frequency,
            carry: false,
            low: 0,
            cycles: 0,
            nr51: None,
        })
    }
    pub fn finished(&self) -> bool {
        self.stage == Stage::Done
    }
    pub fn frequency(&self) -> Option<u16> {
        (self.finished() && self.routine == OutputRoutine::WaveFrequency).then_some(self.frequency)
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        let Some(cycles) = self.next_cycles() else {
            return Ok(true);
        };
        execute(cycles)?;
        self.advance();
        self.cycles += u32::from(cycles);
        Ok(self.finished())
    }
    fn next_cycles(&self) -> Option<u8> {
        use Stage::*;
        let ch = self.input.channel;
        let id = self.input.source_sfx_id;
        Some(match self.stage {
            // Both pinned bank tables' low bytes plus channels 0..7 cannot
            // carry. JR nc is taken; INC H is not fabricated as part of it.
            Register(_, n) => [4, 12, 4, 12, 4, 8, 4, 4, 8, 16][n as usize],
            DutyPrefix(n) => {
                if n < 6 {
                    [8, 12, 8, 8, 4, 8][n as usize]
                } else if ch == 2 {
                    12
                } else {
                    8
                }
            }
            DutySfx(n) => {
                if n == 0 {
                    8
                } else if ch == 6 {
                    12
                } else {
                    8
                }
            }
            DutyInclude(n) => [4, 8, 4, 12, 8, 8, 4, 4][n as usize],
            DutyCall(n) => [8, 24][n as usize],
            DutyStore(n) => [8, 16][n as usize],
            EnablePrefix(n) => {
                if n < 8 {
                    [8, 12, 8, 12, 8, 4, 4, 8][n as usize]
                } else if ch == 7 {
                    12
                } else {
                    8
                }
            }
            EnableSfx(n) => {
                if n == 0 {
                    8
                } else if ch >= 4 {
                    12
                } else {
                    8
                }
            }
            EnablePair(n) => {
                if n < 4 {
                    [12, 8, 8, 4][n as usize]
                } else if self.input.paired_sfx_active {
                    12
                } else {
                    8
                }
            }
            Panning(n) => [16, 12, 8, 8, 4, 12, 12, 8, 8, 4, 4][n as usize],
            EnableStore(n) => [4, 12, 16][n as usize],
            WaveKind(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if ch == 2 {
                    12
                } else {
                    8
                }
            }
            WaveSfx(n) => {
                if n == 0 {
                    8
                } else if ch == 6 {
                    8
                } else {
                    12
                }
            }
            WaveSetup(n) => {
                if n < 3 {
                    [16, 12, 8][n as usize]
                } else if ch == 2 {
                    12
                } else {
                    8
                }
            }
            SfxInstrument => 12,
            WaveData(n) => [8, 4, 8, 4, 12, 8, 8, 8, 8, 12, 8, 8, 12][n as usize],
            WaveCopy(byte, n) => {
                if n < 6 {
                    [8, 8, 8, 4, 4, 4][n as usize]
                } else if byte == 15 {
                    8
                } else {
                    12
                }
            }
            WaveEnd(n) => [8, 12, 12][n as usize],
            FrequencyHeader(n) => [4, 8, 8, 4, 8, 24][n as usize],
            WaveStore(n) => [8, 8, 8, 24][n as usize],
            ModifierCall => 24,
            CryPrefix(n) => {
                if n < 2 {
                    [16, 8][n as usize]
                } else if id >= 20 {
                    12
                } else {
                    8
                }
            }
            CryEnd(n) => {
                if n == 0 {
                    8
                } else if n == 1 {
                    if id == 134 {
                        12
                    } else {
                        8
                    }
                } else if id < 134 {
                    12
                } else {
                    8
                }
            }
            CryYes => 4,
            CryNo(n) => {
                if n == 2 {
                    16
                } else {
                    4
                }
            }
            ModifierBranch => {
                if (20..134).contains(&id) {
                    8
                } else {
                    12
                }
            }
            Modifier(n) => {
                if n < 2 {
                    [16, 4][n as usize]
                } else if self.carry {
                    8
                } else {
                    12
                }
            }
            ModifierCarry => 4,
            ModifierStore(n) => [8, 4, 8, 8, 8][n as usize],
            ModifierReturn | WaveReturn => 16,
            Done => return None,
        })
    }
    fn advance(&mut self) {
        use Stage::*;
        let ch = self.input.channel;
        let id = self.input.source_sfx_id;
        self.stage = match self.stage {
            Register(ret, n) => {
                if n == 9 {
                    match ret {
                        RegisterReturn::Finish => Done,
                        RegisterReturn::DutyStore => DutyStore(0),
                        RegisterReturn::WaveStore => WaveStore(0),
                    }
                } else {
                    Register(ret, n + 1)
                }
            }
            DutyPrefix(n) => {
                if n == 6 {
                    if ch == 2 {
                        DutyCall(0)
                    } else {
                        DutySfx(0)
                    }
                } else {
                    DutyPrefix(n + 1)
                }
            }
            DutySfx(n) => {
                if n == 1 {
                    if ch == 6 {
                        DutyCall(0)
                    } else {
                        DutyInclude(0)
                    }
                } else {
                    DutySfx(1)
                }
            }
            DutyInclude(n) => {
                if n == 7 {
                    DutyCall(0)
                } else {
                    DutyInclude(n + 1)
                }
            }
            DutyCall(n) => {
                if n == 1 {
                    Register(RegisterReturn::DutyStore, 0)
                } else {
                    DutyCall(1)
                }
            }
            DutyStore(n) => {
                if n == 1 {
                    Done
                } else {
                    DutyStore(1)
                }
            }
            EnablePrefix(n) => {
                if n == 8 {
                    if ch == 7 {
                        Panning(0)
                    } else {
                        EnableSfx(0)
                    }
                } else {
                    EnablePrefix(n + 1)
                }
            }
            EnableSfx(n) => {
                if n == 1 {
                    if ch >= 4 {
                        EnableStore(0)
                    } else {
                        EnablePair(0)
                    }
                } else {
                    EnableSfx(1)
                }
            }
            EnablePair(n) => {
                if n == 4 {
                    if self.input.paired_sfx_active {
                        EnableStore(0)
                    } else {
                        Panning(0)
                    }
                } else {
                    EnablePair(n + 1)
                }
            }
            Panning(n) => {
                if n == 10 {
                    EnableStore(0)
                } else {
                    Panning(n + 1)
                }
            }
            EnableStore(n) => {
                if n == 2 {
                    Done
                } else {
                    EnableStore(n + 1)
                }
            }
            WaveKind(n) => {
                if n == 2 {
                    if ch == 2 {
                        WaveSetup(0)
                    } else {
                        WaveSfx(0)
                    }
                } else {
                    WaveKind(n + 1)
                }
            }
            WaveSfx(n) => {
                if n == 1 {
                    if ch == 6 {
                        WaveSetup(0)
                    } else {
                        FrequencyHeader(0)
                    }
                } else {
                    WaveSfx(1)
                }
            }
            WaveSetup(n) => {
                if n == 3 {
                    if ch == 2 {
                        WaveData(0)
                    } else {
                        SfxInstrument
                    }
                } else {
                    WaveSetup(n + 1)
                }
            }
            SfxInstrument => WaveData(0),
            WaveData(n) => {
                if n == 12 {
                    WaveCopy(0, 0)
                } else {
                    WaveData(n + 1)
                }
            }
            WaveCopy(byte, n) => {
                if n == 6 {
                    if byte == 15 {
                        WaveEnd(0)
                    } else {
                        WaveCopy(byte + 1, 0)
                    }
                } else {
                    WaveCopy(byte, n + 1)
                }
            }
            WaveEnd(n) => {
                if n == 2 {
                    FrequencyHeader(0)
                } else {
                    WaveEnd(n + 1)
                }
            }
            FrequencyHeader(n) => {
                if n == 3 {
                    self.frequency = (self.frequency & 255)
                        | u16::from(((self.frequency >> 8) as u8 | 0x80) & 0xc7) << 8;
                }
                if n == 5 {
                    Register(RegisterReturn::WaveStore, 0)
                } else {
                    FrequencyHeader(n + 1)
                }
            }
            WaveStore(n) => {
                if n == 3 {
                    ModifierCall
                } else {
                    WaveStore(n + 1)
                }
            }
            ModifierCall => CryPrefix(0),
            CryPrefix(n) => {
                if n == 2 {
                    if id >= 20 {
                        CryEnd(0)
                    } else {
                        CryEnd(3)
                    }
                } else {
                    CryPrefix(n + 1)
                }
            }
            CryEnd(3) => CryNo(0), // Unconditional JR .no on IDs below CRY_SFX_START.
            CryEnd(n) => {
                if n == 1 && id == 134 {
                    CryNo(0)
                } else if n == 2 {
                    if id < 134 {
                        CryYes
                    } else {
                        CryNo(0)
                    }
                } else {
                    CryEnd(n + 1)
                }
            }
            CryYes => CryNo(2), // Shared RET, with the source carry result preserved.
            CryNo(n) => {
                if n == 2 {
                    ModifierBranch
                } else if n == 1 {
                    CryNo(2)
                } else {
                    CryNo(1)
                }
            }
            ModifierBranch => {
                if (20..134).contains(&id) {
                    Modifier(0)
                } else {
                    ModifierReturn
                }
            }
            Modifier(n) => {
                if n == 1 {
                    let sum =
                        u16::from(self.input.frequency_modifier) + u16::from(self.frequency as u8);
                    self.low = sum as u8;
                    self.carry = sum > 255;
                }
                if n == 2 {
                    if self.carry {
                        ModifierCarry
                    } else {
                        ModifierStore(0)
                    }
                } else {
                    Modifier(n + 1)
                }
            }
            ModifierCarry => {
                self.frequency = self.frequency.wrapping_add(0x100);
                ModifierStore(0)
            }
            ModifierStore(n) => {
                if n == 1 {
                    self.frequency = (self.frequency & 0xff00) | u16::from(self.low);
                }
                if n == 4 {
                    ModifierReturn
                } else {
                    ModifierStore(n + 1)
                }
            }
            ModifierReturn => WaveReturn,
            WaveReturn => Done,
            Done => Done,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        routine: OutputRoutine,
        input: OutputInput,
        frequency: Option<u16>,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn every_channel_setup_instruction_matches_original_including_cry_carry_edges() {
        let f: Fixture =
            serde_json::from_str(include_str!("channel_output_instructions_golden.json")).unwrap();
        assert_eq!(f.cases.len(), 310);
        for (index, c) in f.cases.into_iter().enumerate() {
            let mut w = ChannelOutputInstructions::new(c.routine, c.input).unwrap();
            let mut cycles = 0;
            for (pc, t) in c.trace {
                assert!(!w.finished());
                let frozen = w.clone();
                assert_eq!(w.try_step(|_| Err("blocked")), Err("blocked"));
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                w.try_step(|actual| {
                    assert_eq!(actual, t, "case {index} source PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                cycles += u32::from(t);
                assert_eq!(w.cycles(), cycles);
            }
            assert!(w.finished(), "case {index}");
            assert_eq!(w.frequency(), c.frequency, "case {index}");
            assert!(w
                .try_step(|_| -> Result<(), ()> { panic!("finished child repeats no instruction") })
                .unwrap());
        }
    }
    #[test]
    fn unsupported_bank_and_channel_do_not_default_to_audio3() {
        let input = OutputInput {
            bank: 31,
            channel: 0,
            paired_sfx_active: false,
            source_sfx_id: 0,
            frequency_modifier: 0,
            frequency: 0,
        };
        for bank in [0, 8, 255] {
            assert!(ChannelOutputInstructions::new(
                OutputRoutine::WaveFrequency,
                OutputInput { bank, ..input }
            )
            .is_none());
        }
        for channel in 8..=255 {
            assert!(ChannelOutputInstructions::new(
                OutputRoutine::RegisterPointer,
                OutputInput { channel, ..input }
            )
            .is_none());
        }
    }
}
