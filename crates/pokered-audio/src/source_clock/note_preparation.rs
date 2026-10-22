//! One retained source continuation from PlayNextNote entry through command
//! parsing and note-length arithmetic. Native channels/APU remain the caller's
//! sole owner; pitch/output and the enclosing return are not executed here.
use super::note_math_instructions::{NoteLengthInput, NoteLengthInstructions};
use super::note_stream_instructions::{
    StreamInput, StreamInstructions, StreamResult, StreamStepError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum PreparationBoundary {
    NotePitch,
    NoiseReturned,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct PreparationInput {
    pub stream: StreamInput,
    pub flags2: u8,
    pub fraction: u8,
    pub tempo: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct PreparationResult {
    pub stream: StreamResult,
    pub fraction: u8,
    pub delay: u8,
    pub boundary: PreparationBoundary,
    pub cycles: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Stream(StreamInstructions),
    Length {
        stream: StreamResult,
        work: NoteLengthInstructions,
    },
    Boundary(PreparationResult),
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct NotePreparation {
    input: PreparationInput,
    stage: Stage,
    cycles: u32,
}
impl NotePreparation {
    pub fn new(input: PreparationInput) -> Option<Self> {
        // Supported commands cannot change flags2 or the noise-mode bit. Reject
        // unsupported modes before accepting any of the caller's instructions.
        NoteLengthInstructions::new(NoteLengthInput {
            channel: input.stream.channel,
            note: 0,
            speed: input.stream.state.speed,
            fraction: input.fraction,
            tempo: input.tempo,
            flags1: input.stream.state.flags1,
            flags2: input.flags2,
        })?;
        let stream = StreamInstructions::new(input.stream).ok()?;
        Some(Self {
            input,
            stage: Stage::Stream(stream),
            cycles: 0,
        })
    }
    pub fn result(&self) -> Option<PreparationResult> {
        if let Stage::Boundary(result) = self.stage {
            Some(result)
        } else {
            None
        }
    }
    pub fn channel_end_context(&self) -> Option<(u8, StreamResult)> {
        if let Stage::Stream(work) = &self.stage {
            work.channel_end_context()
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
            Stage::Stream(work) => {
                if work.try_step(data, clock)? {
                    let stream = work.result().expect("stream at note-length entry");
                    let length = NoteLengthInstructions::new(NoteLengthInput {
                        channel: self.input.stream.channel,
                        note: stream.note,
                        speed: stream.state.speed,
                        fraction: if stream.tempo.is_some() {
                            0
                        } else {
                            self.input.fraction
                        },
                        tempo: stream.tempo.unwrap_or(self.input.tempo),
                        flags1: stream.state.flags1,
                        flags2: self.input.flags2,
                    })
                    .expect("supported stream commands preserve validated channel mode");
                    Some(Stage::Length {
                        stream,
                        work: length,
                    })
                } else {
                    None
                }
            }
            Stage::Length { stream, work } => {
                if work.try_step(clock).map_err(StreamStepError::Clock)? {
                    let (fraction, delay) = work.result().expect("completed note length");
                    Some(Stage::Boundary(PreparationResult {
                        stream: *stream,
                        fraction,
                        delay,
                        boundary: if work.returned() {
                            PreparationBoundary::NoiseReturned
                        } else {
                            PreparationBoundary::NotePitch
                        },
                        cycles: self.cycles + u32::from(accepted),
                    }))
                } else {
                    None
                }
            }
            Stage::Boundary(_) => return Ok(true),
        };
        self.cycles += u32::from(accepted);
        if let Some(stage) = transition {
            self.stage = stage;
        }
        Ok(matches!(self.stage, Stage::Boundary(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        bank: u8,
        input: PreparationInput,
        data: alloc::vec::Vec<u8>,
        result: PreparationResult,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn composed_commands_and_length_match_whole_source_execution_without_extra_calls() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("note_preparation_golden.json")).unwrap();
        assert_eq!(fixture.cases.len(), 478);
        for (index, c) in fixture.cases.into_iter().enumerate() {
            assert!(matches!(c.bank, 2 | 31));
            let mut w = NotePreparation::new(c.input).unwrap();
            let mut elapsed = 0;
            for (pc, t) in c.trace {
                assert_eq!(w.result(), None);
                let frozen = w.clone();
                assert_eq!(
                    w.try_step(&c.data, |_| Err("paused")),
                    Err(StreamStepError::Clock("paused"))
                );
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                w.try_step(&c.data, |actual| {
                    assert_eq!(actual, t, "case {index} source PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                elapsed += u32::from(t);
                assert_eq!(w.cycles(), elapsed);
            }
            assert_eq!(w.result(), Some(c.result), "case {index}");
            assert!(w
                .try_step(&c.data, |_| -> Result<(), ()> {
                    panic!("boundary does not imply pitch/output/outer RET")
                })
                .unwrap());
        }
    }
}
