use super::super::{
    field_restore::{CopyFormat, PatternCopy, PatternSource},
    vblank_pattern_work::VBlankPatternWork,
};
use super::*;
use core::cell::RefCell;

struct Bytes(RefCell<Vec<(PatternSource, u16)>>);
impl PatternAssets for Bytes {
    fn read_pattern_byte(&self, source: PatternSource, offset: u16) -> Option<u8> {
        self.0.borrow_mut().push((source, offset));
        Some((offset as u8).wrapping_mul(3))
    }
}

fn seed(at: Option<u64>) -> DisplayWorkTime {
    let mut time = DisplayWorkTime::default();
    if at.is_none() {
        time.device.write_port(0xff40, 0x11, 0xc3).unwrap();
    }
    time.elapse(at.unwrap_or(0)).unwrap();
    time.active_vblank = true;
    time.deadline = time.elapsed + 128;
    time
}

fn same_time(actual: &DisplayWorkTime, expected: &DisplayWorkTime) {
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(actual.idle_period_cached, expected.idle_period_cached);
}

// Obtain the oracle instructions from the original single-step cursor rather
// than restating the write/increment sequence implemented by the optimized bus.
fn pair_cursor<const DOUBLE: bool>(destination: u16, last: bool) -> VBlankPatternWork {
    struct Untimed;
    impl PatternInstructionBus for Untimed {
        type Error = ();
        fn execute(&mut self, i: PatternInstruction) -> Result<Option<u16>, ()> {
            Ok(matches!(i.access, Some(PatternAccess::ReadPair { .. })).then_some(0x3412))
        }
    }
    let preceding = if last {
        if DOUBLE {
            12
        } else {
            14
        }
    } else {
        0
    };
    let initial = (destination & 0xff00) | u16::from((destination as u8).wrapping_sub(preceding));
    let mut cursor = VBlankPatternWork::new(PatternCopy {
        source: PatternSource::WalkingPlayer,
        source_offset: 0,
        destination: initial,
        tiles: 1,
        format: if DOUBLE {
            CopyFormat::OneBppDouble
        } else {
            CopyFormat::TwoBpp
        },
    });
    let offset = if last {
        if DOUBLE {
            8
        } else {
            16
        }
    } else {
        2
    };
    loop {
        if cursor.source_offset() == Some(offset)
            && matches!(
                cursor.instruction().unwrap().access,
                Some(PatternAccess::Write { .. })
            )
        {
            assert_eq!(cursor.destination(), Some(destination));
            return cursor;
        }
        cursor.step(&mut Untimed).unwrap().unwrap();
    }
}

fn check_pairs<const DOUBLE: bool>() {
    let mut accepted = 0;
    let mut blocked_vram = 0;
    let source = Bytes(RefCell::new(Vec::new()));
    for last in [false, true] {
        for destination in [0x8000, 0x80fc, 0x88ff, 0x9ffe] {
            let cursor = pair_cursor::<DOUBLE>(destination, last);
            for at in [
                None,
                Some(0),
                Some(4),
                Some(80),
                Some(84),
                Some(252),
                Some(260),
                Some(450),
                Some(456),
                Some(65500),
                Some(65660),
                Some(69600),
                Some(70220),
            ] {
                for grant in 0..=56 {
                    let mut time = seed(at);
                    time.deadline = time.elapsed + grant;
                    let mut expected = time.clone();
                    let mut memory = PatternMemory::default();
                    let mut expected_memory = memory.clone();
                    let result = time
                        .pattern_bus(&mut memory, &source)
                        .try_vblank_pair_writes::<DOUBLE>(destination, 0x3412, last)
                        .unwrap();
                    if let Some(final_destination) = result {
                        accepted += 1;
                        blocked_vram += usize::from(!expected.device.vram_accessible());
                        let mut reference = cursor;
                        let mut bus = expected
                            .pattern_bus(&mut expected_memory, &source)
                            .checking_each_instruction();
                        for _ in 0..if DOUBLE { 8 } else { 4 } {
                            reference.step(&mut bus).unwrap().unwrap();
                        }
                        assert_eq!(Some(final_destination), reference.destination());
                    }
                    same_time(&time, &expected);
                    assert_eq!(memory.bytes(), expected_memory.bytes());
                }
            }
        }
    }
    assert!(accepted > 100);
    assert!(blocked_vram > 0);
    assert!(
        source.0.borrow().is_empty(),
        "the write run must not reread ROM"
    );
}

#[test]
fn vblank_pair_writes_match_single_instructions_at_budget_and_lcd_boundaries() {
    check_pairs::<false>();
    check_pairs::<true>();
}

#[test]
fn vblank_pair_refuses_observable_or_invalid_spans_without_side_effects() {
    let source = Bytes(RefCell::new(Vec::new()));
    for case in 0..7 {
        let mut time = seed(None);
        let mut memory = PatternMemory::default();
        let destination = if case == 2 {
            0x7fff
        } else if case == 3 {
            0xa000
        } else {
            0x8800
        };
        match case {
            0 => time.active_vblank = false,
            1 => time.device.write_port(0xff46, 0xc3, 0xc3).unwrap(),
            4 => time.shadow_oam.clear(),
            5 => time.deadline = time.elapsed,
            6 => memory = serde_json::from_value(serde_json::json!({"bytes": [0]})).unwrap(),
            _ => (),
        }
        let before = time.clone();
        let bytes = memory.bytes().to_vec();
        assert_eq!(
            time.pattern_bus(&mut memory, &source)
                .try_vblank_pair_writes::<true>(destination, 0x3412, true),
            Ok(None)
        );
        same_time(&time, &before);
        assert_eq!(memory.bytes(), bytes);
    }
    let mut time = seed(None);
    time.elapsed += 1; // a malformed imported clock must still be rejected
    let before = time.clone();
    let mut memory = PatternMemory::default();
    assert_eq!(
        time.pattern_bus(&mut memory, &source)
            .try_vblank_pair_writes::<false>(0x8800, 0x3412, false),
        Err(DisplayWorkError::Device(
            DisplayDeviceError::TimeNotSynchronized
        ))
    );
    same_time(&time, &before);
    assert!(memory.bytes().iter().all(|byte| *byte == 0));
}

#[test]
fn vblank_pair_runner_matches_refusals_resources_and_snapshot_resumption() {
    for format in [CopyFormat::TwoBpp, CopyFormat::OneBppDouble] {
        for destination in [0x8800, 0x88fc, 0x9ff0] {
            for tiles in [1, 8] {
                for at in [None, Some(248), Some(65660), Some(70220)] {
                    for grant in [1, 11, 28, 52, 128, 1024] {
                        let mut time = seed(at);
                        let mut expected = time.clone();
                        let mut memory = PatternMemory::default();
                        let mut expected_memory = memory.clone();
                        let source = Bytes(RefCell::new(Vec::new()));
                        let expected_source = Bytes(RefCell::new(Vec::new()));
                        let mut work = VBlankPatternWork::new(PatternCopy {
                            source: PatternSource::WalkingPlayer,
                            source_offset: 17,
                            destination,
                            tiles,
                            format,
                        });
                        let mut reference = work;
                        let mut restored = false;
                        loop {
                            time.deadline = time.elapsed + grant;
                            expected.deadline = expected.elapsed + grant;
                            let actual = work.run(&mut time.pattern_bus(&mut memory, &source));
                            let result = {
                                let mut bus = expected
                                    .pattern_bus(&mut expected_memory, &expected_source)
                                    .checking_each_instruction();
                                loop {
                                    match reference.step(&mut bus) {
                                        Ok(Some(_)) => (),
                                        Ok(None) => break Ok(()),
                                        Err(error) => break Err(error),
                                    }
                                }
                            };
                            assert_eq!(actual, result);
                            assert_eq!(work, reference);
                            same_time(&time, &expected);
                            assert_eq!(memory.bytes(), expected_memory.bytes());
                            assert_eq!(*source.0.borrow(), *expected_source.0.borrow());
                            source.0.borrow_mut().clear();
                            expected_source.0.borrow_mut().clear();
                            if actual != Err(DisplayWorkError::Budget) {
                                break;
                            }
                            if !restored {
                                work = serde_json::from_value(serde_json::to_value(work).unwrap())
                                    .unwrap();
                                time = serde_json::from_value(serde_json::to_value(&time).unwrap())
                                    .unwrap();
                                restored = true;
                            }
                        }
                    }
                }
            }
        }
    }
}
