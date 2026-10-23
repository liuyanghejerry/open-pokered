use super::*;
use crate::source_clock::note_pitch_instructions::{
    NotePitchInput, NotePitchInstructions, NotePitchResult,
};
use alloc::vec::Vec;

#[derive(serde::Deserialize)]
struct HelperSource {
    input: OutputInput,
    routine: OutputRoutine,
    trace: Vec<(u16, u8)>,
    frequency: Option<u16>,
}
#[derive(serde::Deserialize)]
struct PitchSource {
    input: NotePitchInput,
    trace: Vec<(u16, u8)>,
    result: NotePitchResult,
}
#[derive(serde::Deserialize)]
struct Sources<T> {
    cases: Vec<T>,
}
#[derive(serde::Deserialize)]
struct Case {
    prior: usize,
    bus: NoteBusInput,
    accesses: Vec<(usize, u16, alloc::string::String, u16, u8, u8)>,
}
#[derive(serde::Deserialize)]
struct Fixture {
    helpers: Vec<Case>,
    pitches: Vec<Case>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("output_live_bus_golden.json")).unwrap()
}
fn helpers() -> Sources<HelperSource> {
    serde_json::from_str(include_str!("channel_output_instructions_golden.json")).unwrap()
}
fn pitches() -> Sources<PitchSource> {
    serde_json::from_str(include_str!("note_pitch_instructions_golden.json")).unwrap()
}
trait LiveCursor:
    Clone + core::fmt::Debug + PartialEq + serde::Serialize + serde::de::DeserializeOwned
{
    fn step<E>(
        &mut self,
        input: NoteBusInput,
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, BusStepError<E>>;
    fn cycles(&self) -> u32;
}
impl LiveCursor for ChannelOutputInstructions {
    fn step<E>(
        &mut self,
        input: NoteBusInput,
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, BusStepError<E>> {
        self.try_step_with_bus(input, execute)
    }
    fn cycles(&self) -> u32 {
        self.cycles()
    }
}
impl LiveCursor for NotePitchInstructions {
    fn step<E>(
        &mut self,
        input: NoteBusInput,
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, BusStepError<E>> {
        self.try_step_with_bus(input, execute)
    }
    fn cycles(&self) -> u32 {
        self.cycles()
    }
}
fn verify<W: LiveCursor>(mut work: W, case: &Case, trace: &[(u16, u8)]) -> W {
    let mut cycles = 0;
    let mut accesses = Vec::new();
    for (index, &(pc, tcycles)) in trace.iter().enumerate() {
        let expected = case.accesses.iter().find(|access| access.0 == index);
        if let Some(access) = expected {
            assert_eq!(access.1, pc);
        }
        let access = expected.map(|access| match access.2.as_str() {
            "Read" => ApuAccess::Read {
                address: access.3,
                at_tcycle: access.5,
            },
            "Write" => ApuAccess::Write {
                address: access.3,
                value: access.4,
                at_tcycle: access.5,
            },
            _ => panic!("unknown source access"),
        });
        let instruction = ApuInstruction { tcycles, access };
        let frozen = work.clone();
        for _ in 0..2 {
            assert_eq!(
                work.step(case.bus, |actual| {
                    assert_eq!(actual, instruction, "source PC {pc:04x}");
                    Err("owner refused before clock/access")
                }),
                Err(BusStepError::Execute("owner refused before clock/access"))
            );
            assert_eq!(work, frozen);
            assert_eq!(work.cycles(), cycles);
        }
        work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
        let done = work
            .step(case.bus, |actual| {
                assert_eq!(actual, instruction, "source PC {pc:04x}");
                cycles += u32::from(actual.tcycles);
                if let Some(access) = expected {
                    accesses.push(access.clone());
                }
                Ok::<_, ()>(expected.and_then(|access| (access.2 == "Read").then_some(access.4)))
            })
            .unwrap();
        assert_eq!(done, index + 1 == trace.len());
        assert_eq!(work.cycles(), cycles);
    }
    assert_eq!(accesses, case.accesses);
    assert!(work
        .step(case.bus, |_| -> Result<Option<u8>, ()> {
            panic!("completed cursor repeats neither clocks nor accesses")
        })
        .unwrap());
    work
}

#[test]
fn live_helpers_match_original_access_values_offsets_and_order_with_changed_read_samples() {
    let sources = helpers();
    let fixture = fixture();
    assert_eq!(fixture.helpers.len(), 930);
    let mut differing_reads = 0;
    for case in fixture.helpers {
        let source = &sources.cases[case.prior];
        let reads: Vec<_> = case
            .accesses
            .iter()
            .filter(|access| access.2 == "Read")
            .collect();
        if reads.len() == 2 && reads[0].4 != reads[1].4 {
            differing_reads += 1;
        }
        let work = verify(
            ChannelOutputInstructions::new(source.routine, source.input).unwrap(),
            &case,
            &source.trace,
        );
        assert_eq!(work.frequency(), source.frequency);
    }
    assert!(differing_reads > 0);
}

#[test]
fn live_pitch_matches_original_envelopes_rests_duty_and_wave_accesses_after_resume() {
    let sources = pitches();
    let fixture = fixture();
    assert_eq!(fixture.pitches.len(), 2100);
    for case in fixture.pitches {
        let source = &sources.cases[case.prior];
        let work = verify(
            NotePitchInstructions::new(source.input).unwrap(),
            &case,
            &source.trace,
        );
        assert_eq!(work.result(), Some(source.result));
    }
}

fn rejects_missing_wave<W: LiveCursor>(mut work: W, mut input: NoteBusInput) {
    let complete = input;
    input.wave = None;
    for resumed in [false, true] {
        if resumed {
            work.step(complete, |_| Ok::<_, ()>(None)).unwrap();
            work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
        }
        let frozen = work.clone();
        assert_eq!(
            work.step(input, |_| -> Result<Option<u8>, ()> {
                panic!("missing wave must reject before any clock/access")
            }),
            Err(BusStepError::MissingWave)
        );
        assert_eq!(work, frozen);
    }
}

#[test]
fn required_wave_is_rejected_before_entry_or_resumed_child_progress() {
    let sources = helpers();
    let fixture = fixture();
    for case in fixture.helpers {
        let source = &sources.cases[case.prior];
        let work = ChannelOutputInstructions::new(source.routine, source.input).unwrap();
        if work.requires_wave() {
            rejects_missing_wave(work, case.bus);
        }
    }
    let sources = pitches();
    for case in fixture.pitches {
        let source = &sources.cases[case.prior];
        let work = NotePitchInstructions::new(source.input).unwrap();
        if work.requires_wave() {
            rejects_missing_wave(work, case.bus);
        }
    }
}
