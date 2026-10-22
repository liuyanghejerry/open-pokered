//! Source music note-pitch continuation through its own RET, excluding the
//! enclosing PlayNextNote RET. Physical APU writes/native updates are not owned.
use super::channel_output_instructions::{ChannelOutputInstructions, OutputInput, OutputRoutine};
use super::note_math_instructions::FrequencyInstructions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct NotePitchInput {
    pub bank: u8,
    pub channel: u8,
    pub note: u8,
    pub octave: u8,
    pub flags1: u8,
    pub paired_sfx_active: bool,
    pub source_sfx_id: u8,
    pub frequency_modifier: u8,
    pub pitch_base: Option<u16>,
    pub previous_frequency_low: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct NotePitchResult {
    pub saved_frequency_low: u8,
    /// DE returned by ApplyWavePatternAndFrequency, not a full APU snapshot.
    pub wave_return_frequency: Option<u16>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum After {
    RestRestart,
    Envelope,
    Duty,
    Enable,
    Wave,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Classify(u8),
    RestGate(u8),
    RestChannel(u8),
    RestSfx(u8),
    RestWave(u8),
    RestCall(u8),
    RestRestart(u8),
    FrequencyCall(u8),
    Frequency(FrequencyInstructions),
    PitchCheck(u8),
    PairGate(u8),
    PairPop,
    PairJump,
    EnvelopeCall(u8),
    EnvelopeStore(u8),
    EnableCall,
    RestorePerfect(u8),
    PerfectInc(u8),
    SaveFrequency(u8),
    Output {
        work: ChannelOutputInstructions,
        after: After,
    },
    Return,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct NotePitchInstructions {
    input: NotePitchInput,
    stage: Stage,
    frequency: u16,
    result: NotePitchResult,
    cycles: u32,
}
impl NotePitchInstructions {
    pub fn new(input: NotePitchInput) -> Option<Self> {
        if !matches!(input.bank, 2 | 31)
            || input.channel >= 3
            || input.note >= 0xd0
            || input.flags1 & 16 != 0
            || (input.channel == 0 && input.paired_sfx_active != (input.source_sfx_id != 0))
            || (input.note < 0xc0 && input.pitch_base.is_none())
        {
            return None;
        }
        Some(Self {
            input,
            stage: Stage::Classify(0),
            frequency: 0,
            result: NotePitchResult {
                saved_frequency_low: input.previous_frequency_low,
                wave_return_frequency: None,
            },
            cycles: 0,
        })
    }
    pub fn finished(&self) -> bool {
        matches!(self.stage, Stage::Done)
    }
    pub fn result(&self) -> Option<NotePitchResult> {
        self.finished().then_some(self.result)
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    fn output(&self, routine: OutputRoutine, after: After) -> Stage {
        Stage::Output {
            work: ChannelOutputInstructions::new(
                routine,
                OutputInput {
                    bank: self.input.bank,
                    channel: self.input.channel,
                    paired_sfx_active: self.input.paired_sfx_active,
                    source_sfx_id: self.input.source_sfx_id,
                    frequency_modifier: self.input.frequency_modifier,
                    frequency: self.frequency,
                },
            )
            .expect("validated music channel/bank"),
            after,
        }
    }
    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        if let Stage::Frequency(work) = &mut self.stage {
            let t = work.next_cycles().expect("pending frequency instruction");
            execute(t)?;
            work.next();
            self.cycles += u32::from(t);
            if let Some(frequency) = work.result() {
                self.frequency = frequency;
                self.stage = Stage::PitchCheck(0);
            }
            return Ok(false);
        }
        if let Stage::Output { work, after } = &mut self.stage {
            let mut accepted = 0;
            let done = work.try_step(|t| {
                execute(t)?;
                accepted = t;
                Ok(())
            })?;
            self.cycles += u32::from(accepted);
            if done {
                self.stage = match *after {
                    After::RestRestart => Stage::RestRestart(0),
                    After::Envelope => Stage::EnvelopeStore(0),
                    After::Duty => Stage::EnableCall,
                    After::Enable => Stage::RestorePerfect(0),
                    After::Wave => {
                        self.result.wave_return_frequency = work.frequency();
                        Stage::Return
                    }
                };
            }
            return Ok(false);
        }
        let Some(t) = self.next_cycles() else {
            return Ok(true);
        };
        execute(t)?;
        self.advance();
        self.cycles += u32::from(t);
        Ok(self.finished())
    }
    fn next_cycles(&self) -> Option<u8> {
        use Stage::*;
        let input = self.input;
        Some(match self.stage {
            Classify(n) => {
                if n < 3 {
                    [12, 8, 8][n as usize]
                } else if input.note & 0xf0 == 0xc0 {
                    8
                } else {
                    12
                }
            }
            RestGate(n) => {
                if n < 7 {
                    [4, 8, 8, 12, 8, 8, 4][n as usize]
                } else if input.paired_sfx_active {
                    12
                } else {
                    8
                }
            }
            RestChannel(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if input.channel == 2 {
                    12
                } else {
                    8
                }
            }
            RestSfx(n) => [8, 12][n as usize],
            RestWave(n) => [8, 12, 8, 12, 8, 12, 12][n as usize],
            RestCall(n) => [8, 24][n as usize],
            RestRestart(n) => [8, 8, 8, 8, 8][n as usize],
            FrequencyCall(n) => [8, 8, 12, 8, 8, 24][n as usize],
            PitchCheck(n) => [8, 12, 8, 12, 12][n as usize],
            PairGate(n) => {
                if n < 10 {
                    [16, 4, 8, 8, 12, 8, 4, 8, 8, 4][n as usize]
                } else if input.paired_sfx_active {
                    12
                } else {
                    8
                }
            }
            PairPop => 12,
            PairJump => 12,
            EnvelopeCall(n) => [8, 12, 8, 8, 8, 24][n as usize],
            EnvelopeStore(n) => [8, 24][n as usize],
            EnableCall => 24,
            RestorePerfect(n) => {
                if n < 5 {
                    [12, 8, 12, 8, 12][n as usize]
                } else if input.flags1 & 1 == 0 {
                    12
                } else {
                    8
                }
            }
            // INC E preserves carry. EnableChannelOutput's final OR clears it,
            // so the source JR nc skips INC D even when low wraps $ff -> $00.
            PerfectInc(n) => [4, 12][n as usize],
            SaveFrequency(n) => [12, 8, 8, 24][n as usize],
            Return => 16,
            Done => return None,
            Frequency(_) | Output { .. } => unreachable!("child owns its instruction"),
        })
    }
    fn advance(&mut self) {
        use Stage::*;
        self.stage = match self.stage {
            Classify(n) => {
                if n == 3 {
                    if self.input.note & 0xf0 == 0xc0 {
                        RestGate(0)
                    } else {
                        FrequencyCall(0)
                    }
                } else {
                    Classify(n + 1)
                }
            }
            RestGate(n) => {
                if n == 7 {
                    if self.input.paired_sfx_active {
                        Return
                    } else {
                        RestChannel(0)
                    }
                } else {
                    RestGate(n + 1)
                }
            }
            RestChannel(n) => {
                if n == 2 {
                    if self.input.channel == 2 {
                        RestWave(0)
                    } else {
                        RestSfx(0)
                    }
                } else {
                    RestChannel(n + 1)
                }
            }
            RestSfx(n) => {
                if n == 1 {
                    RestCall(0)
                } else {
                    RestSfx(1)
                }
            }
            RestWave(n) => {
                if n == 6 {
                    Return
                } else {
                    RestWave(n + 1)
                }
            }
            RestCall(n) => {
                if n == 1 {
                    self.output(OutputRoutine::RegisterPointer, After::RestRestart)
                } else {
                    RestCall(1)
                }
            }
            RestRestart(n) => {
                if n == 4 {
                    Return
                } else {
                    RestRestart(n + 1)
                }
            }
            FrequencyCall(n) => {
                if n == 5 {
                    Frequency(FrequencyInstructions::new(
                        self.input.pitch_base.expect("validated pitch table entry"),
                        self.input.octave,
                    ))
                } else {
                    FrequencyCall(n + 1)
                }
            }
            PitchCheck(n) => {
                if n == 4 {
                    PairGate(0)
                } else {
                    PitchCheck(n + 1)
                }
            }
            PairGate(n) => {
                if n == 10 {
                    if self.input.paired_sfx_active {
                        PairPop
                    } else {
                        PairJump
                    }
                } else {
                    PairGate(n + 1)
                }
            }
            PairPop => Return,
            PairJump => EnvelopeCall(0),
            EnvelopeCall(n) => {
                if n == 5 {
                    self.output(OutputRoutine::RegisterPointer, After::Envelope)
                } else {
                    EnvelopeCall(n + 1)
                }
            }
            EnvelopeStore(n) => {
                if n == 1 {
                    self.output(OutputRoutine::DutyAndLength, After::Duty)
                } else {
                    EnvelopeStore(1)
                }
            }
            EnableCall => self.output(OutputRoutine::EnableChannel, After::Enable),
            RestorePerfect(n) => {
                if n == 5 {
                    if self.input.flags1 & 1 == 0 {
                        SaveFrequency(0)
                    } else {
                        PerfectInc(0)
                    }
                } else {
                    RestorePerfect(n + 1)
                }
            }
            PerfectInc(n) => {
                if n == 0 {
                    self.frequency = (self.frequency & 0xff00)
                        | u16::from((self.frequency as u8).wrapping_add(1));
                    PerfectInc(1)
                } else {
                    SaveFrequency(0)
                }
            }
            SaveFrequency(n) => {
                if n == 2 {
                    self.result.saved_frequency_low = self.frequency as u8;
                }
                if n == 3 {
                    self.output(OutputRoutine::WaveFrequency, After::Wave)
                } else {
                    SaveFrequency(n + 1)
                }
            }
            Return => Done,
            Done => Done,
            Frequency(_) | Output { .. } => unreachable!("child owns its progress"),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        input: NotePitchInput,
        result: NotePitchResult,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn complete_pitch_continuations_match_original_at_every_resumable_boundary() {
        let f: Fixture =
            serde_json::from_str(include_str!("note_pitch_instructions_golden.json")).unwrap();
        assert_eq!(f.cases.len(), 700);
        for (index, c) in f.cases.into_iter().enumerate() {
            let mut w = NotePitchInstructions::new(c.input).unwrap();
            let mut cycles = 0;
            for (pc, t) in c.trace {
                assert_eq!(w.result(), None);
                let frozen = w.clone();
                assert_eq!(w.try_step(|_| Err("yield")), Err("yield"));
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
            assert_eq!(w.result(), Some(c.result), "case {index}");
            assert!(w
                .try_step(|_| -> Result<(), ()> {
                    panic!("own RET is complete; outer RET remains with caller")
                })
                .unwrap());
        }
    }
    #[test]
    fn unknown_pitch_slide_missing_pitch_and_inconsistent_sfx_do_not_receive_a_fake_return() {
        let input = NotePitchInput {
            bank: 31,
            channel: 0,
            note: 0x40,
            octave: 6,
            flags1: 0,
            paired_sfx_active: false,
            source_sfx_id: 0,
            frequency_modifier: 0,
            pitch_base: Some(0xf800),
            previous_frequency_low: 77,
        };
        assert!(NotePitchInstructions::new(NotePitchInput { bank: 8, ..input }).is_none());
        assert!(NotePitchInstructions::new(NotePitchInput {
            channel: 3,
            ..input
        })
        .is_none());
        assert!(NotePitchInstructions::new(NotePitchInput {
            flags1: 16,
            ..input
        })
        .is_none());
        assert!(NotePitchInstructions::new(NotePitchInput {
            pitch_base: None,
            ..input
        })
        .is_none());
        assert!(NotePitchInstructions::new(NotePitchInput {
            source_sfx_id: 144,
            ..input
        })
        .is_none());
        assert!(NotePitchInstructions::new(NotePitchInput {
            paired_sfx_active: true,
            ..input
        })
        .is_none());
    }
}
