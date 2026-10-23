//! Retained supported Audio1/3_PlayNextNote body, including its actual final RET.
//! Live APU instructions are offered to the external owner; no native frame
//! commit, independent clock, IRQ epilogue or Game hookup occurs here.
use super::apu_bus::{ApuInstruction, BusStepError, NoteBusInput};
use super::channel_end_instructions::{ChannelEndInput, ChannelEndInstructions, ChannelEndResult};
use super::note_pitch_instructions::{NotePitchInput, NotePitchInstructions, NotePitchResult};
use super::note_preparation::{
    NotePreparation, PreparationBoundary, PreparationInput, PreparationResult,
};
use super::note_stream_instructions::{StreamInputError, StreamStepError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct MusicNoteInput {
    pub preparation: PreparationInput,
    pub bank: u8,
    pub paired_sfx_active: bool,
    pub source_sfx_id: u8,
    pub frequency_modifier: u8,
    pub previous_frequency_low: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct MusicNoteResult {
    pub preparation: PreparationResult,
    pub saved_frequency_low: u8,
    pub wave_return_frequency: Option<u16>,
    pub cycles: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct MusicEndResult {
    pub stream: super::note_stream_instructions::StreamResult,
    pub ending: ChannelEndResult,
    pub cycles: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum MusicNoteOutcome {
    Note(MusicNoteResult),
    End(MusicEndResult),
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Preparation(NotePreparation),
    Pitch {
        preparation: PreparationResult,
        work: NotePitchInstructions,
    },
    Return {
        preparation: PreparationResult,
        pitch: NotePitchResult,
    },
    Ending {
        stream: super::note_stream_instructions::StreamResult,
        work: ChannelEndInstructions,
    },
    EndReturn {
        stream: super::note_stream_instructions::StreamResult,
        ending: ChannelEndResult,
    },
    EndDone(MusicEndResult),
    Done(MusicNoteResult),
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct MusicNoteInstructions {
    input: MusicNoteInput,
    stage: Stage,
    cycles: u32,
}
impl MusicNoteInstructions {
    pub fn new(input: MusicNoteInput) -> Option<Self> {
        if !matches!(input.bank, 2 | 31)
            || (input.preparation.stream.channel == 0
                && input.paired_sfx_active != (input.source_sfx_id != 0))
        {
            return None;
        }
        Some(Self {
            input,
            stage: Stage::Preparation(NotePreparation::new(input.preparation)?),
            cycles: 0,
        })
    }
    pub fn result(&self) -> Option<MusicNoteOutcome> {
        if let Stage::Done(result) = self.stage {
            Some(MusicNoteOutcome::Note(result))
        } else if let Stage::EndDone(result) = self.stage {
            Some(MusicNoteOutcome::End(result))
        } else {
            None
        }
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn try_step<E>(
        &mut self,
        data: &[u8],
        execute: impl FnOnce(u8) -> Result<(), E>,
    ) -> Result<bool, StreamStepError<E>> {
        self.try_step_inner(data, None, |instruction| {
            execute(instruction.tcycles).map(|()| None)
        })
    }
    /// Advance the same retained note body while offering live APU accesses.
    /// `panning` and the initial wave instrument belong to the native owner;
    /// stream commands replace that instrument before pitch/output when due.
    /// Wave patterns are caller-owned data, required only for an uncovered wave
    /// note. A missing pattern rejects before accepting its pitch instruction.
    pub fn try_step_with_bus<E>(
        &mut self,
        data: &[u8],
        panning: u8,
        initial_wave_instrument: u8,
        wave_patterns: &[[u8; 16]],
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, StreamStepError<E>> {
        self.try_step_inner(
            data,
            Some((panning, initial_wave_instrument, wave_patterns)),
            execute,
        )
    }
    fn try_step_inner<E>(
        &mut self,
        data: &[u8],
        bus: Option<(u8, u8, &[[u8; 16]])>,
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, StreamStepError<E>> {
        let mut accepted = 0;
        let clock = |instruction: ApuInstruction| {
            let value = execute(instruction)?;
            accepted = instruction.tcycles;
            Ok(value)
        };
        let transition = match &mut self.stage {
            Stage::Preparation(work) => {
                if work.try_step_with_bus(data, clock)? {
                    let preparation = work.result().expect("preparation boundary");
                    if preparation.boundary == PreparationBoundary::NoiseReturned {
                        Some(Stage::Return {
                            preparation,
                            pitch: NotePitchResult {
                                saved_frequency_low: self.input.previous_frequency_low,
                                wave_return_frequency: None,
                            },
                        })
                    } else {
                        let note = preparation.stream.note;
                        let pitch=NotePitchInstructions::new(NotePitchInput {bank:self.input.bank,channel:self.input.preparation.stream.channel,note,
                            octave:preparation.stream.state.octave,flags1:preparation.stream.state.flags1,paired_sfx_active:self.input.paired_sfx_active,
                            source_sfx_id:self.input.source_sfx_id,frequency_modifier:self.input.frequency_modifier,
                            pitch_base:if note<0xc0 {crate::NOTE_FREQUENCIES.get((note>>4) as usize).copied()} else {None},
                            previous_frequency_low:self.input.previous_frequency_low}).expect("supported stream preserves validated music mode and source pitch range");
                        Some(Stage::Pitch {
                            preparation,
                            work: pitch,
                        })
                    }
                } else if let Some((channel, stream)) = work.channel_end_context() {
                    // A noise ending under non-cry SFX reads no missing pan,
                    // saved-volume, disable-output or source pointer word.
                    // Broader endings retain the explicit unsupported boundary.
                    if channel == 3 && !(20..134).contains(&self.input.source_sfx_id) {
                        Some(Stage::Ending {
                            stream,
                            work: ChannelEndInstructions::new(ChannelEndInput {
                                bank: self.input.bank,
                                channel,
                                source_sfx_id: self.input.source_sfx_id,
                                flags1: stream.state.flags1,
                                flags2: self.input.preparation.flags2,
                                disable_output: None,
                                saved_volume: None,
                                panning: None,
                                command_ptr: None,
                            })
                            .expect("noise/non-cry tail reads no optional state"),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Stage::Pitch { preparation, work } => {
                let done = if let Some((panning, initial_instrument, patterns)) = bus {
                    let instrument = preparation
                        .stream
                        .wave_instrument
                        .unwrap_or(initial_instrument);
                    work.try_step_with_bus(
                        NoteBusInput {
                            note_delay: preparation.delay,
                            volume: preparation.stream.state.volume,
                            duty: preparation.stream.state.duty,
                            panning,
                            wave: patterns.get(usize::from(instrument)).copied(),
                        },
                        clock,
                    )
                    .map_err(|error| match error {
                        BusStepError::MissingWave => {
                            StreamStepError::Input(StreamInputError::MissingWave)
                        }
                        BusStepError::Execute(error) => StreamStepError::Clock(error),
                    })?
                } else {
                    work.try_step(|t| clock(ApuInstruction::plain(t)).map(|_| ()))
                        .map_err(StreamStepError::Clock)?
                };
                if done {
                    Some(Stage::Return {
                        preparation: *preparation,
                        pitch: work.result().expect("completed note pitch"),
                    })
                } else {
                    None
                }
            }
            Stage::Return { preparation, pitch } => {
                clock(ApuInstruction::plain(16)).map_err(StreamStepError::Clock)?;
                Some(Stage::Done(MusicNoteResult {
                    preparation: *preparation,
                    saved_frequency_low: pitch.saved_frequency_low,
                    wave_return_frequency: pitch.wave_return_frequency,
                    cycles: self.cycles + 16,
                }))
            }
            Stage::Ending { stream, work } => {
                if work
                    .try_step(|t| clock(ApuInstruction::plain(t)).map(|_| ()))
                    .map_err(StreamStepError::Clock)?
                {
                    Some(Stage::EndReturn {
                        stream: *stream,
                        ending: work.result().expect("ending tail RET"),
                    })
                } else {
                    None
                }
            }
            Stage::EndReturn { stream, ending } => {
                clock(ApuInstruction::plain(16)).map_err(StreamStepError::Clock)?;
                Some(Stage::EndDone(MusicEndResult {
                    stream: *stream,
                    ending: *ending,
                    cycles: self.cycles + 16,
                }))
            }
            Stage::Done(_) | Stage::EndDone(_) => return Ok(true),
        };
        self.cycles += u32::from(accepted);
        if let Some(stage) = transition {
            self.stage = stage;
        }
        Ok(matches!(self.stage, Stage::Done(_) | Stage::EndDone(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        input: MusicNoteInput,
        data: alloc::vec::Vec<u8>,
        result: MusicNoteResult,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn whole_note_calls_match_original_through_final_ret_without_a_second_native_owner() {
        let f: Fixture =
            serde_json::from_str(include_str!("music_note_instructions_golden.json")).unwrap();
        assert_eq!(f.cases.len(), 538);
        for (index, c) in f.cases.into_iter().enumerate() {
            // The stream is caller-owned. CPU cursors retain no replacement
            // data buffer, native channels, or independently ticking sequencer.
            let data = c.data;
            let mut w = MusicNoteInstructions::new(c.input).unwrap();
            let mut cycles = 0;
            for (pc, t) in c.trace {
                assert_eq!(w.result(), None);
                let frozen = w.clone();
                assert_eq!(
                    w.try_step(&data, |_| Err("suspend")),
                    Err(StreamStepError::Clock("suspend"))
                );
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                w.try_step(&data, |actual| {
                    assert_eq!(actual, t, "case {index} source PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                cycles += u32::from(t);
                assert_eq!(w.cycles(), cycles);
            }
            assert_eq!(
                w.result(),
                Some(MusicNoteOutcome::Note(c.result)),
                "case {index}"
            );
            assert!(w
                .try_step(&data, |_| -> Result<(), ()> {
                    panic!("completed body cannot replay RET or a native update")
                })
                .unwrap());
        }
    }
}

#[cfg(test)]
mod ending_tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        input: MusicNoteInput,
        data: alloc::vec::Vec<u8>,
        result: MusicEndResult,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn noise_end_handoff_matches_whole_source_without_zero_clock_transitions_or_fake_length() {
        let f: Fixture =
            serde_json::from_str(include_str!("music_channel_end_instructions_golden.json"))
                .unwrap();
        assert_eq!(f.cases.len(), 88);
        for (index, c) in f.cases.into_iter().enumerate() {
            let mut w = MusicNoteInstructions::new(c.input).unwrap();
            let mut elapsed = 0;
            for (pc, t) in c.trace {
                assert_eq!(w.result(), None);
                let frozen = w.clone();
                assert_eq!(
                    w.try_step(&c.data, |_| Err("yield")),
                    Err(StreamStepError::Clock("yield"))
                );
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                let mut calls = 0;
                w.try_step(&c.data, |actual| {
                    calls += 1;
                    assert_eq!(actual, t, "case {index} source PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                assert_eq!(calls, 1);
                elapsed += u32::from(t);
                assert_eq!(w.cycles(), elapsed);
            }
            assert_eq!(
                w.result(),
                Some(MusicNoteOutcome::End(c.result)),
                "case {index}"
            );
            assert!(w
                .try_step(&c.data, |_| -> Result<(), ()> {
                    panic!("completed ending")
                })
                .unwrap());
        }
    }
}

#[cfg(test)]
#[path = "music_note_bus_tests.rs"]
mod bus_tests;
