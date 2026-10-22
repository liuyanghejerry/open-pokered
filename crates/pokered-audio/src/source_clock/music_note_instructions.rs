//! Retained supported Audio1/3_PlayNextNote body, including its actual final RET.
//! No native frame commit, APU bus scheduling, IRQ epilogue or Game hookup occurs.
use super::note_pitch_instructions::{NotePitchInput, NotePitchInstructions, NotePitchResult};
use super::note_preparation::{
    NotePreparation, PreparationBoundary, PreparationInput, PreparationResult,
};
use super::note_stream_instructions::StreamStepError;

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
    pub fn result(&self) -> Option<MusicNoteResult> {
        if let Stage::Done(result) = self.stage {
            Some(result)
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
        let mut accepted = 0;
        let clock = |t| {
            execute(t)?;
            accepted = t;
            Ok(())
        };
        let transition = match &mut self.stage {
            Stage::Preparation(work) => {
                if work.try_step(data, clock)? {
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
                } else {
                    None
                }
            }
            Stage::Pitch { preparation, work } => {
                if work.try_step(clock).map_err(StreamStepError::Clock)? {
                    Some(Stage::Return {
                        preparation: *preparation,
                        pitch: work.result().expect("completed note pitch"),
                    })
                } else {
                    None
                }
            }
            Stage::Return { preparation, pitch } => {
                clock(16).map_err(StreamStepError::Clock)?;
                Some(Stage::Done(MusicNoteResult {
                    preparation: *preparation,
                    saved_frequency_low: pitch.saved_frequency_low,
                    wave_return_frequency: pitch.wave_return_frequency,
                    cycles: self.cycles + 16,
                }))
            }
            Stage::Done(_) => return Ok(true),
        };
        self.cycles += u32::from(accepted);
        if let Some(stage) = transition {
            self.stage = stage;
        }
        Ok(matches!(self.stage, Stage::Done(_)))
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
            assert_eq!(w.result(), Some(c.result), "case {index}");
            assert!(w
                .try_step(&data, |_| -> Result<(), ()> {
                    panic!("completed body cannot replay RET or a native update")
                })
                .unwrap());
        }
    }
}
