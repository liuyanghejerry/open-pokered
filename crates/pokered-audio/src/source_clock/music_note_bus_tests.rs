use super::*;
use crate::source_clock::apu_bus::ApuAccess;
use alloc::{collections::BTreeMap, string::String, vec::Vec};

#[derive(serde::Deserialize)]
struct SourceCase {
    input: MusicNoteInput,
    data: Vec<u8>,
    result: MusicNoteResult,
    trace: Vec<(u16, u8)>,
}
#[derive(serde::Deserialize)]
struct SourceFixture {
    cases: Vec<SourceCase>,
}
#[derive(serde::Deserialize)]
struct BusCase {
    source_index: Option<usize>,
    input: Option<MusicNoteInput>,
    data: Option<Vec<u8>>,
    trace: Option<Vec<(u16, u8)>>,
    panning: u8,
    initial_wave_instrument: u8,
    nr51: u8,
    events: Vec<(usize, String, u16, u8)>,
}
#[derive(serde::Deserialize)]
struct BusFixture {
    waves: BTreeMap<String, Vec<[u8; 16]>>,
    cases: Vec<BusCase>,
}

fn source_fixture() -> SourceFixture {
    serde_json::from_str(include_str!("music_note_instructions_golden.json")).unwrap()
}

#[test]
fn live_music_calls_match_original_bus_values_order_and_every_instruction_yield() {
    let source = source_fixture();
    let fixture: BusFixture =
        serde_json::from_str(include_str!("music_note_bus_golden.json")).unwrap();
    assert_eq!(fixture.cases.len(), 548);
    assert_eq!(
        fixture.cases.iter().map(|c| c.events.len()).sum::<usize>(),
        5074
    );
    for (case_index, case) in fixture.cases.iter().enumerate() {
        let original = case.source_index.map(|i| &source.cases[i]);
        let input = original.map(|c| c.input).or(case.input).unwrap();
        let data = original.map(|c| &c.data).or(case.data.as_ref()).unwrap();
        let trace = original.map(|c| &c.trace).or(case.trace.as_ref()).unwrap();
        let waves = &fixture.waves[if input.bank == 2 { "2" } else { "31" }];
        let mut work = MusicNoteInstructions::new(input).unwrap();
        let mut ports = BTreeMap::from([(0xff25, case.nr51)]);
        let mut event_index = 0;
        let mut elapsed = 0;
        for (instruction_index, &(pc, tcycles)) in trace.iter().enumerate() {
            assert_eq!(work.result(), None);
            let frozen = work.clone();
            assert_eq!(
                work.try_step_with_bus(
                    data,
                    case.panning,
                    case.initial_wave_instrument,
                    waves,
                    |_| Err("pause"),
                ),
                Err(StreamStepError::Clock("pause")),
                "case {case_index}, instruction {instruction_index}, pc {pc:04x}",
            );
            assert_eq!(work, frozen);
            work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
            let expected = case
                .events
                .get(event_index)
                .filter(|e| e.0 == instruction_index);
            let mut calls = 0;
            work.try_step_with_bus(
                data,
                case.panning,
                case.initial_wave_instrument,
                waves,
                |actual| {
                    calls += 1;
                    assert_eq!(actual.tcycles, tcycles, "case {case_index}, pc {pc:04x}");
                    let value = match (actual.access, expected) {
                        (None, None) => None,
                        (Some(ApuAccess::Read { address, at_tcycle }), Some((_, kind, want_address, value))) => {
                            assert_eq!(kind, "read");
                            assert_eq!(address, *want_address, "case {case_index}, pc {pc:04x}");
                            assert_eq!(at_tcycle, tcycles - 4);
                            assert_eq!(ports.get(&address), Some(value));
                            Some(*value)
                        }
                        (Some(ApuAccess::Write { address, value, at_tcycle }), Some((_, kind, want_address, want_value))) => {
                            assert_eq!(kind, "write");
                            assert_eq!((address, value), (*want_address, *want_value), "case {case_index}, pc {pc:04x}");
                            assert_eq!(at_tcycle, tcycles - 4);
                            ports.insert(address, value);
                            None
                        }
                        pair => panic!("case {case_index}, instruction {instruction_index}, pc {pc:04x}: {pair:?}"),
                    };
                    Ok::<_, ()>(value)
                },
            ).unwrap();
            assert_eq!(calls, 1);
            event_index += usize::from(expected.is_some());
            elapsed += u32::from(tcycles);
            assert_eq!(work.cycles(), elapsed);
        }
        assert_eq!(event_index, case.events.len());
        assert!(work.result().is_some());
        if let Some(original) = original {
            assert_eq!(work.result(), Some(MusicNoteOutcome::Note(original.result)));
        } else {
            let volumes: Vec<_> = case
                .events
                .iter()
                .filter(|(_, kind, address, _)| kind == "write" && *address == 0xff24)
                .map(|(_, _, _, value)| *value)
                .collect();
            assert_eq!(
                volumes,
                if data.last() == Some(&0xff) {
                    [0x21, 0x43]
                } else {
                    [0x12, 0x65]
                }
            );
        }
        assert_eq!(
            work.try_step_with_bus::<()>(
                data,
                case.panning,
                case.initial_wave_instrument,
                waves,
                |_| panic!("finished call replayed")
            ),
            Ok(true),
        );
    }
}

fn wave_input() -> MusicNoteInput {
    let mut input = source_fixture()
        .cases
        .into_iter()
        .find(|c| c.input.preparation.stream.channel == 2)
        .unwrap()
        .input;
    input.preparation.stream.ptr = 0;
    input.preparation.stream.return_ptr = 0;
    input.preparation.stream.loop_counter = 0;
    input.preparation.stream.state.flags1 = 0;
    input.paired_sfx_active = false;
    input.source_sfx_id = 0;
    input
}

fn finish_with_bus(
    work: &mut MusicNoteInstructions,
    data: &[u8],
    instrument: u8,
    waves: &[[u8; 16]],
) -> Vec<(u16, u8)> {
    let mut writes = Vec::new();
    let mut ports = BTreeMap::from([(0xff25, 0xa5)]);
    for _ in 0..3000 {
        if work
            .try_step_with_bus(data, 0x5a, instrument, waves, |instruction| {
                Ok::<_, ()>(match instruction.access {
                    Some(ApuAccess::Read { address, .. }) => Some(ports[&address]),
                    Some(ApuAccess::Write { address, value, .. }) => {
                        ports.insert(address, value);
                        writes.push((address, value));
                        None
                    }
                    None => None,
                })
            })
            .unwrap()
        {
            return writes;
        }
    }
    panic!("music call did not finish");
}

#[test]
fn missing_wave_rejects_before_accepting_pitch_and_can_resume_with_real_table() {
    let mut work = MusicNoteInstructions::new(wave_input()).unwrap();
    let data = [0x07];
    let mut missing = false;
    for _ in 0..3000 {
        let frozen = work.clone();
        let mut called = false;
        let result = work.try_step_with_bus(&data, 0x5a, 0, &[], |_| {
            called = true;
            Ok::<_, ()>(None)
        });
        if result == Err(StreamStepError::Input(StreamInputError::MissingWave)) {
            assert!(!called);
            assert_eq!(work, frozen);
            work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
            assert_eq!(
                work.try_step_with_bus::<()>(&data, 0x5a, 255, &[], |_| panic!(
                    "missing wave accepted"
                )),
                result
            );
            assert_eq!(work, frozen);
            missing = true;
            break;
        }
        assert_eq!(result, Ok(false));
    }
    assert!(missing);
    let waves = [[0x31; 16]];
    let writes = finish_with_bus(&mut work, &data, 0, &waves);
    let wave_writes: Vec<_> = writes
        .into_iter()
        .filter(|(a, _)| (0xff30..=0xff3f).contains(a))
        .collect();
    assert_eq!(
        wave_writes,
        (0xff30..=0xff3f)
            .map(|address| (address, 0x31))
            .collect::<Vec<_>>()
    );
}

#[test]
fn stream_instrument_overrides_initial_value_and_non_wave_paths_need_no_table() {
    let mut work = MusicNoteInstructions::new(wave_input()).unwrap();
    let waves = [[0x10; 16], [0x21; 16], [0x32; 16], [0x43; 16]];
    let writes = finish_with_bus(&mut work, &[0xd1, 0x23, 0x07], 255, &waves);
    assert_eq!(
        writes
            .iter()
            .filter(|(a, _)| (0xff30..=0xff3f).contains(a))
            .count(),
        16
    );
    assert!(writes
        .iter()
        .filter(|(a, _)| (0xff30..=0xff3f).contains(a))
        .all(|(_, v)| *v == 0x43));
    for (note, covered) in [(0xc1, false), (0x07, true)] {
        let mut input = wave_input();
        input.paired_sfx_active = covered;
        let mut work = MusicNoteInstructions::new(input).unwrap();
        let writes = finish_with_bus(&mut work, &[note], 255, &[]);
        assert!(!writes.iter().any(|(a, _)| (0xff30..=0xff3f).contains(a)));
    }
    let mut input = wave_input();
    input.preparation.stream.channel = 3;
    input.preparation.stream.state.flags1 = 4;
    let mut work = MusicNoteInstructions::new(input).unwrap();
    assert_eq!(
        finish_with_bus(&mut work, &[0xf0, 0x21, 0xf0, 0x43, 0xff], 255, &[]),
        [(0xff24, 0x21), (0xff24, 0x43)]
    );
    assert!(matches!(work.result(), Some(MusicNoteOutcome::End(_))));
}
