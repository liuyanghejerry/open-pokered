//! Retained source pulse confirmation-tone body. Native sequencer/APU ownership
//! stays with the caller; this cursor accepts CPU instructions, not audio ticks.
use super::apu_bus::{ApuInstruction, BusStepError, NoteBusInput};
use super::channel_end_instructions::{ChannelEndInput, ChannelEndInstructions, ChannelEndResult};
use super::channel_output_instructions::{ChannelOutputInstructions, OutputInput, OutputRoutine};
use super::note_math_instructions::MultiplyAddInstructions;
use super::press_ab_work::SOURCE_PRESS_AB_ID;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct PressAbInput {
    pub bank: u8,
    pub source_sfx_id: u8,
    pub ptr: usize,
    pub fraction: u8,
    pub duty: u8,
    pub flags1: u8,
    pub flags2: u8,
    pub note_speed: u8,
    pub sfx_tempo: u16,
    pub vibrato_reload: u8,
}

#[cfg(test)]
#[path = "press_ab_bus_tests.rs"]
mod bus_tests;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum PressAbOutcome {
    Note {
        next_ptr: usize,
        fraction: u8,
        delay: u8,
        duty: u8,
        envelope: u8,
        frequency: u16,
        wave_return_frequency: u16,
        vibrato_delay: u8,
    },
    End {
        next_ptr: usize,
        fraction: u8,
        duty: u8,
        vibrato_delay: u8,
        ending: ChannelEndResult,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressAbStepError<E> {
    ChangedProgram,
    Truncated(usize),
    MissingWave,
    Clock(E),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Fetch {
    Command,
    Duty,
    Envelope,
    Low,
    High,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum After {
    DutyStore,
    EnvelopeStore,
    Enable,
    Wave,
    Return,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Entry(u8),
    FetchCall(Fetch),
    FetchByte(Fetch, u8),
    ReturnCompare(u8),
    ReturnFlags(u8),
    Dispatch(u8),
    CompareDuty(u8),
    SetDuty(u8),
    OtherCommands(u8),
    SfxDispatch(u8),
    LengthPrefix(u8),
    First(MultiplyAddInstructions),
    SelectTempo(u8),
    SetTempoCall,
    CryCheck(u8),
    SetTempo(u8),
    ReadTempo(u8),
    SecondPrefix(u8),
    Second(MultiplyAddInstructions),
    LengthStore(u8),
    LengthReturn(u8),
    DutySetup(u8),
    DutyStore,
    EnvelopeSetup(u8),
    EnvelopeStore,
    LowSetup(u8),
    HighSetup(u8),
    EnableCall,
    WaveCall(u8),
    Output {
        work: ChannelOutputInstructions,
        after: After,
    },
    Ending(ChannelEndInstructions),
    InnerReturn,
    OuterReturn,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct PressAbInstructions {
    input: PressAbInput,
    stage: Stage,
    ptr: usize,
    last: u8,
    note: u8,
    duty: u8,
    envelope: u8,
    low: u8,
    frequency: u16,
    wave_frequency: u16,
    computed: u16,
    length: u8,
    ended: Option<ChannelEndResult>,
    cycles: u32,
}
impl PressAbInstructions {
    pub fn new(input: PressAbInput, data: &[u8]) -> Option<Self> {
        if !matches!(input.bank, 2 | 31)
            || input.source_sfx_id != SOURCE_PRESS_AB_ID
            || input.flags1 != 4
            || input.flags2 != 0
            || input.note_speed != 1
            || input.sfx_tempo != 0x100
            || ![0, 6, 10, 14, 18].contains(&input.ptr)
            || data != crate::sfx_data::SFX_PRESS_AB_CH5
        {
            return None;
        }
        Some(Self {
            input,
            stage: Stage::Entry(0),
            ptr: input.ptr,
            last: 0,
            note: 0,
            duty: input.duty,
            envelope: 0,
            low: 0,
            frequency: 0,
            wave_frequency: 0,
            computed: 0,
            length: 0,
            ended: None,
            cycles: 0,
        })
    }
    pub fn result(&self) -> Option<PressAbOutcome> {
        if !matches!(self.stage, Stage::Done) {
            return None;
        }
        Some(if let Some(ending) = self.ended {
            PressAbOutcome::End {
                next_ptr: self.ptr,
                fraction: self.input.fraction,
                duty: self.duty,
                vibrato_delay: self.input.vibrato_reload,
                ending,
            }
        } else {
            PressAbOutcome::Note {
                next_ptr: self.ptr,
                fraction: self.computed as u8,
                delay: (self.computed >> 8) as u8,
                duty: self.duty,
                envelope: self.envelope,
                frequency: self.frequency,
                wave_return_frequency: self.wave_frequency,
                vibrato_delay: self.input.vibrato_reload,
            }
        })
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    /// Execute the same retained CPU instruction with its actual APU access.
    /// The owner must reject before clocking/writing and return the live value
    /// for a successful read. This finite pulse SFX never reads wave or stereo
    /// panning data; its EnableChannelOutput child does read live NR51.
    pub fn try_step_with_bus<E>(
        &mut self,
        data: &[u8],
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, PressAbStepError<E>> {
        if self.result().is_some() {
            return Ok(true);
        }
        if data != crate::sfx_data::SFX_PRESS_AB_CH5 {
            return Err(PressAbStepError::ChangedProgram);
        }
        if let Stage::Output { work, after } = &mut self.stage {
            let input = NoteBusInput {
                note_delay: (self.computed >> 8) as u8,
                volume: self.envelope,
                duty: self.duty,
                // Channel 4 skips the stereo-panning path. This unused scalar
                // does not stand in for a logical byte or a hardware read.
                panning: 0,
                wave: None,
            };
            let mut accepted = 0;
            let done = work
                .try_step_with_bus(input, |instruction| {
                    let receipt = execute(instruction)?;
                    accepted = instruction.tcycles;
                    Ok(receipt)
                })
                .map_err(|error| match error {
                    BusStepError::MissingWave => PressAbStepError::MissingWave,
                    BusStepError::Execute(error) => PressAbStepError::Clock(error),
                })?;
            self.cycles += u32::from(accepted);
            if done {
                self.stage = match *after {
                    After::DutyStore => Stage::DutyStore,
                    After::EnvelopeStore => Stage::EnvelopeStore,
                    After::Enable => Stage::EnableCall,
                    After::Wave => Stage::WaveCall(0),
                    After::Return => {
                        self.wave_frequency = work.frequency().expect("wave return");
                        Stage::InnerReturn
                    }
                };
            }
            return Ok(false);
        }
        let store = match self.stage {
            // Audio*_sfx_note does not mask the delay byte at this first store.
            // ApplyDutyCycleAndSoundLength performs its separate masked store.
            Stage::DutyStore => Some((0xff11, self.duty | (self.computed >> 8) as u8)),
            Stage::EnvelopeStore => Some((0xff12, self.envelope)),
            _ => None,
        };
        // All other child paths reuse the existing CPU progression, including
        // the non-cry channel-4 ending, whose complete source tail has no APU I/O.
        self.try_step(data, |cycles| {
            let instruction = if let Some((address, value)) = store {
                ApuInstruction::write(cycles, address, value)
            } else {
                ApuInstruction::plain(cycles)
            };
            execute(instruction).map(|_| ())
        })
    }
    fn output(&self, routine: OutputRoutine, after: After) -> Stage {
        Stage::Output {
            work: ChannelOutputInstructions::new(
                routine,
                OutputInput {
                    bank: self.input.bank,
                    channel: 4,
                    paired_sfx_active: false,
                    source_sfx_id: self.input.source_sfx_id,
                    frequency_modifier: 0,
                    frequency: self.frequency,
                },
            )
            .expect("validated PressAB channel/bank"),
            after,
        }
    }
    pub fn try_step<E>(
        &mut self,
        data: &[u8],
        execute: impl FnOnce(u8) -> Result<(), E>,
    ) -> Result<bool, PressAbStepError<E>> {
        if self.result().is_some() {
            return Ok(true);
        }
        if data != crate::sfx_data::SFX_PRESS_AB_CH5 {
            return Err(PressAbStepError::ChangedProgram);
        }
        match &mut self.stage {
            Stage::First(_) | Stage::Second(_) => {
                let first = matches!(self.stage, Stage::First(_));
                // The source multiply's registers advance only after admission.
                let work = match &mut self.stage {
                    Stage::First(w) | Stage::Second(w) => w,
                    _ => unreachable!(),
                };
                let t = work.next_cycles().expect("pending multiply");
                execute(t).map_err(PressAbStepError::Clock)?;
                work.next();
                self.cycles += u32::from(t);
                if let Some(value) = work.result() {
                    if first {
                        self.length = value as u8;
                        self.stage = Stage::SelectTempo(0);
                    } else {
                        self.computed = value;
                        self.stage = Stage::LengthStore(0);
                    }
                }
                return Ok(false);
            }
            Stage::Output { work, after } => {
                let mut t = 0;
                let done = work
                    .try_step(|cycles| {
                        execute(cycles)?;
                        t = cycles;
                        Ok(())
                    })
                    .map_err(PressAbStepError::Clock)?;
                self.cycles += u32::from(t);
                if done {
                    self.stage = match *after {
                        After::DutyStore => Stage::DutyStore,
                        After::EnvelopeStore => Stage::EnvelopeStore,
                        After::Enable => Stage::EnableCall,
                        After::Wave => Stage::WaveCall(0),
                        After::Return => {
                            self.wave_frequency = work.frequency().expect("wave return");
                            Stage::InnerReturn
                        }
                    };
                }
                return Ok(false);
            }
            Stage::Ending(work) => {
                let mut t = 0;
                let done = work
                    .try_step(|cycles| {
                        execute(cycles)?;
                        t = cycles;
                        Ok(())
                    })
                    .map_err(PressAbStepError::Clock)?;
                self.cycles += u32::from(t);
                if done {
                    self.ended = work.result();
                    self.stage = Stage::OuterReturn;
                }
                return Ok(false);
            }
            _ => (),
        }
        let byte = if matches!(self.stage, Stage::FetchByte(_, 10)) {
            Some(
                *data
                    .get(self.ptr)
                    .ok_or(PressAbStepError::Truncated(self.ptr))?,
            )
        } else {
            None
        };
        let t = self.next_cycles().expect("pending PressAB instruction");
        execute(t).map_err(PressAbStepError::Clock)?;
        self.advance(byte);
        self.cycles += u32::from(t);
        Ok(matches!(self.stage, Stage::Done))
    }
    fn next_cycles(&self) -> Option<u8> {
        use Stage::*;
        Some(match self.stage {
            Entry(n) => [12, 8, 8, 12, 8, 8, 12, 8, 16, 16, 24][n as usize],
            FetchCall(_) => 24,
            FetchByte(_, n) => [8, 4, 4, 4, 12, 8, 8, 4, 8, 4, 8, 8, 8, 8, 8, 16][n as usize],
            ReturnCompare(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if self.note == 0xff {
                    12
                } else {
                    16
                }
            }
            ReturnFlags(n) => [8, 12, 8, 12, 8][n as usize],
            Dispatch(n) => [8, 16, 8, 16, 8, 8, 16][n as usize],
            CompareDuty(n) => {
                if n < 8 {
                    [4, 8, 12, 8, 12, 8, 12, 8][n as usize]
                } else if self.note == 0xec {
                    8
                } else {
                    12
                }
            }
            SetDuty(n) => [4, 4, 8, 8, 12, 8, 8, 16][n as usize],
            OtherCommands(n) => {
                if n % 2 == 0 {
                    8
                } else {
                    12
                }
            }
            SfxDispatch(n) => [8, 8, 12, 8, 8, 4, 8, 8, 8, 12, 8, 12, 8, 24][n as usize],
            LengthPrefix(n) => [4, 16, 8, 4, 8, 4, 4, 12, 8, 8, 4, 24][n as usize],
            SelectTempo(n) => [4, 8, 12, 8, 8, 8, 8, 24][n as usize],
            SetTempoCall => 24,
            CryCheck(n) => [16, 8, 12, 8, 8, 8, 4, 4, 16][n as usize],
            SetTempo(n) => [12, 4, 16, 8, 16, 16][n as usize],
            ReadTempo(n) => [16, 4, 16, 4][n as usize],
            SecondPrefix(n) => [4, 8, 12, 8, 8, 24][n as usize],
            LengthStore(n) => [4, 4, 12, 8, 8, 4, 12, 8, 8, 12, 8, 12][n as usize],
            LengthReturn(n) => [8, 12, 8, 12, 8, 12, 16][n as usize],
            DutySetup(n) => [4, 8, 12, 8, 8, 4, 4, 8, 24][n as usize],
            DutyStore => 8,
            EnvelopeSetup(n) => [4, 8, 24][n as usize],
            EnvelopeStore => 8,
            LowSetup(n) => [4, 4, 8, 8, 8, 16][n as usize],
            HighSetup(n) => [12, 4, 16, 24][n as usize],
            EnableCall => 24,
            WaveCall(n) => [12, 24][n as usize],
            InnerReturn | OuterReturn => 16,
            Done => return None,
            First(_) | Second(_) | Output { .. } | Ending(_) => unreachable!("child instruction"),
        })
    }
    fn advance(&mut self, byte: Option<u8>) {
        use Stage::*;
        self.stage = match self.stage {
            Entry(n) => {
                if n == 10 {
                    FetchCall(Fetch::Command)
                } else {
                    Entry(n + 1)
                }
            }
            FetchCall(f) => FetchByte(f, 0),
            FetchByte(f, n) => {
                if n == 10 {
                    self.last = byte.expect("checked byte");
                }
                if n == 11 {
                    self.ptr += 1;
                }
                if n == 15 {
                    match f {
                        Fetch::Command => {
                            self.note = self.last;
                            ReturnCompare(0)
                        }
                        Fetch::Duty => SetDuty(0),
                        Fetch::Envelope => {
                            self.envelope = self.last;
                            EnvelopeSetup(0)
                        }
                        Fetch::Low => {
                            self.low = self.last;
                            LowSetup(0)
                        }
                        Fetch::High => {
                            self.frequency = u16::from(self.low) | u16::from(self.last) << 8;
                            HighSetup(0)
                        }
                    }
                } else {
                    FetchByte(f, n + 1)
                }
            }
            ReturnCompare(n) => {
                if n == 2 {
                    if self.note == 0xff {
                        ReturnFlags(0)
                    } else {
                        Dispatch(0)
                    }
                } else {
                    ReturnCompare(n + 1)
                }
            }
            ReturnFlags(n) => {
                if n == 4 {
                    Ending(
                        ChannelEndInstructions::new(ChannelEndInput {
                            bank: self.input.bank,
                            channel: 4,
                            source_sfx_id: self.input.source_sfx_id,
                            flags1: 4,
                            flags2: 0,
                            disable_output: None,
                            saved_volume: None,
                            panning: None,
                            command_ptr: None,
                        })
                        .expect("non-cry pulse ending reads no optional state"),
                    )
                } else {
                    ReturnFlags(n + 1)
                }
            }
            Dispatch(n) => {
                if n == 6 {
                    CompareDuty(0)
                } else {
                    Dispatch(n + 1)
                }
            }
            CompareDuty(n) => {
                if n == 8 {
                    if self.note == 0xec {
                        FetchCall(Fetch::Duty)
                    } else {
                        OtherCommands(0)
                    }
                } else {
                    CompareDuty(n + 1)
                }
            }
            SetDuty(n) => {
                if n == 6 {
                    self.duty = (self.last & 3) << 6;
                }
                if n == 7 {
                    FetchCall(Fetch::Command)
                } else {
                    SetDuty(n + 1)
                }
            }
            OtherCommands(n) => {
                if n == 11 {
                    SfxDispatch(0)
                } else {
                    OtherCommands(n + 1)
                }
            }
            SfxDispatch(n) => {
                if n == 13 {
                    LengthPrefix(0)
                } else {
                    SfxDispatch(n + 1)
                }
            }
            LengthPrefix(n) => {
                if n == 11 {
                    First(MultiplyAddInstructions::new(
                        1,
                        u16::from((self.note & 15) + 1),
                        0,
                    ))
                } else {
                    LengthPrefix(n + 1)
                }
            }
            SelectTempo(n) => {
                if n == 7 {
                    SetTempoCall
                } else {
                    SelectTempo(n + 1)
                }
            }
            SetTempoCall => CryCheck(0),
            CryCheck(n) => {
                if n == 8 {
                    SetTempo(0)
                } else {
                    CryCheck(n + 1)
                }
            }
            SetTempo(n) => {
                if n == 5 {
                    ReadTempo(0)
                } else {
                    SetTempo(n + 1)
                }
            }
            ReadTempo(n) => {
                if n == 3 {
                    SecondPrefix(0)
                } else {
                    ReadTempo(n + 1)
                }
            }
            SecondPrefix(n) => {
                if n == 5 {
                    Second(MultiplyAddInstructions::new(
                        self.length,
                        0x100,
                        self.input.fraction,
                    ))
                } else {
                    SecondPrefix(n + 1)
                }
            }
            LengthStore(n) => {
                if n == 11 {
                    LengthReturn(0)
                } else {
                    LengthStore(n + 1)
                }
            }
            LengthReturn(n) => {
                if n == 6 {
                    DutySetup(0)
                } else {
                    LengthReturn(n + 1)
                }
            }
            DutySetup(n) => {
                if n == 8 {
                    self.output(OutputRoutine::RegisterPointer, After::DutyStore)
                } else {
                    DutySetup(n + 1)
                }
            }
            DutyStore => FetchCall(Fetch::Envelope),
            EnvelopeSetup(n) => {
                if n == 2 {
                    self.output(OutputRoutine::RegisterPointer, After::EnvelopeStore)
                } else {
                    EnvelopeSetup(n + 1)
                }
            }
            EnvelopeStore => FetchCall(Fetch::Low),
            LowSetup(n) => {
                if n == 5 {
                    FetchCall(Fetch::High)
                } else {
                    LowSetup(n + 1)
                }
            }
            HighSetup(n) => {
                if n == 3 {
                    self.output(OutputRoutine::DutyAndLength, After::Enable)
                } else {
                    HighSetup(n + 1)
                }
            }
            EnableCall => self.output(OutputRoutine::EnableChannel, After::Wave),
            WaveCall(n) => {
                if n == 1 {
                    self.output(OutputRoutine::WaveFrequency, After::Return)
                } else {
                    WaveCall(1)
                }
            }
            InnerReturn => OuterReturn,
            OuterReturn => Done,
            Done => Done,
            First(_) | Second(_) | Output { .. } | Ending(_) => unreachable!("child progress"),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Input {
        ptr: usize,
        fraction: u8,
        duty: u8,
        data: alloc::vec::Vec<u8>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        input: Input,
        #[serde(rename = "T")]
        cycles: u32,
        next_ptr: usize,
        delay: u8,
        fraction: u8,
        duty: u8,
        flags1: u8,
        sound_id: u8,
        instructions: alloc::vec::Vec<(u16, u8, u8)>,
        registers: serde_json::Value,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn finite_press_ab_matches_each_original_recorded_instruction_and_register_result() {
        let f: Fixture = serde_json::from_str(include_str!("press_ab_work_golden.json")).unwrap();
        assert_eq!(f.cases.len(), 20);
        for (index, c) in f.cases.into_iter().enumerate() {
            let input = PressAbInput {
                bank: 31,
                source_sfx_id: SOURCE_PRESS_AB_ID,
                ptr: c.input.ptr,
                fraction: c.input.fraction,
                duty: c.input.duty,
                flags1: 4,
                flags2: 0,
                note_speed: 1,
                sfx_tempo: 0x100,
                vibrato_reload: 0,
            };
            let mut w = PressAbInstructions::new(input, &c.input.data).unwrap();
            let mut elapsed = 0;
            for (pc, _, t) in c.instructions {
                assert_eq!(w.result(), None);
                let frozen = w.clone();
                assert_eq!(
                    w.try_step(&c.input.data, |_| Err("yield")),
                    Err(PressAbStepError::Clock("yield"))
                );
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                let mut calls = 0;
                w.try_step(&c.input.data, |actual| {
                    calls += 1;
                    assert_eq!(actual, t, "case {index} original PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                assert_eq!(calls, 1);
                elapsed += u32::from(t);
                assert_eq!(w.cycles(), elapsed);
            }
            assert_eq!(w.cycles(), c.cycles);
            match w.result().unwrap() {
                PressAbOutcome::Note {
                    next_ptr,
                    fraction,
                    delay,
                    duty,
                    envelope,
                    wave_return_frequency,
                    ..
                } => {
                    assert_eq!(
                        (next_ptr, fraction, delay, duty),
                        (c.next_ptr, c.fraction, c.delay, c.duty)
                    );
                    assert_eq!(c.flags1, 4);
                    assert_eq!(c.sound_id, 144);
                    assert_eq!(u64::from(envelope), c.registers["0xff12"].as_u64().unwrap());
                    assert_eq!(
                        u64::from(wave_return_frequency & 255),
                        c.registers["0xff13"].as_u64().unwrap()
                    );
                    assert_eq!(
                        u64::from(wave_return_frequency >> 8),
                        c.registers["0xff14"].as_u64().unwrap()
                    );
                }
                PressAbOutcome::End {
                    next_ptr,
                    fraction,
                    duty,
                    ending,
                    ..
                } => {
                    assert_eq!((next_ptr, fraction, duty), (c.next_ptr, c.fraction, c.duty));
                    assert_eq!(c.flags1, 0);
                    assert_eq!(c.sound_id, 0);
                    assert!(ending.ended);
                }
            }
            assert!(w
                .try_step(&c.input.data, |_| -> Result<(), ()> {
                    panic!("completed clip repeats no instruction or native tick")
                })
                .unwrap());
        }
    }
}

#[cfg(test)]
mod bank_tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        input: PressAbInput,
        data: alloc::vec::Vec<u8>,
        result: PressAbOutcome,
        cycles: u32,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn both_banks_match_independent_pinned_source_header_data_instructions_and_results() {
        let f: Fixture =
            serde_json::from_str(include_str!("press_ab_instructions_golden.json")).unwrap();
        assert_eq!(f.cases.len(), 40);
        for (index, c) in f.cases.into_iter().enumerate() {
            let mut w = PressAbInstructions::new(c.input, &c.data).unwrap();
            let mut cycles = 0;
            for (pc, t) in c.trace {
                let frozen = w.clone();
                assert_eq!(
                    w.try_step(&c.data, |_| Err("busy")),
                    Err(PressAbStepError::Clock("busy"))
                );
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                w.try_step(&c.data, |actual| {
                    assert_eq!(actual, t, "case {index} source PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                cycles += u32::from(t);
                assert_eq!(w.cycles(), cycles);
            }
            assert_eq!(w.cycles(), c.cycles);
            assert_eq!(w.result(), Some(c.result), "case {index}");
        }
    }
    #[test]
    fn unsupported_context_and_changed_program_do_not_become_a_silent_voice() {
        let input = PressAbInput {
            bank: 31,
            source_sfx_id: 144,
            ptr: 0,
            fraction: 0,
            duty: 0,
            flags1: 4,
            flags2: 0,
            note_speed: 1,
            sfx_tempo: 0x100,
            vibrato_reload: 0,
        };
        let data = crate::sfx_data::SFX_PRESS_AB_CH5;
        for bad in [
            PressAbInput { bank: 8, ..input },
            PressAbInput {
                source_sfx_id: 0,
                ..input
            },
            PressAbInput { ptr: 2, ..input },
            PressAbInput { flags1: 0, ..input },
            PressAbInput { flags2: 1, ..input },
            PressAbInput {
                note_speed: 2,
                ..input
            },
            PressAbInput {
                sfx_tempo: 124,
                ..input
            },
        ] {
            assert!(PressAbInstructions::new(bad, data).is_none());
        }
        let mut w = PressAbInstructions::new(input, data).unwrap();
        w.try_step(data, |_| Ok::<(), ()>(())).unwrap();
        let frozen = w.clone();
        let mut changed = data.to_vec();
        changed[0] = 0xe4;
        assert!(PressAbInstructions::new(input, &changed).is_none());
        assert_eq!(
            w.try_step(&changed, |_| -> Result<(), ()> {
                panic!("changed stream cannot clock")
            }),
            Err(PressAbStepError::ChangedProgram)
        );
        assert_eq!(w, frozen);
        assert_eq!(w.result(), None);
    }
}
