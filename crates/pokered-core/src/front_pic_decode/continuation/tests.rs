use super::*;

#[derive(Default, Clone)]
struct Trace {
    instructions: usize,
    cycles: u64,
    hash: u64,
    refuse: bool,
}
impl DisplayInstructionBus for Trace {
    type Error = ();
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, ()> {
        if self.refuse {
            self.refuse = false;
            return Err(());
        }
        assert_eq!(
            instruction.access, None,
            "body scratch is private, wrapper ports are separate"
        );
        assert!(matches!(instruction.tcycles, 4 | 8 | 12 | 16 | 20 | 24));
        self.cycles += u64::from(instruction.tcycles);
        self.instructions += 1;
        self.hash = self.hash.wrapping_mul(0x100000001b3) ^ u64::from(instruction.tcycles);
        Ok(None)
    }
}
fn finish(work: &mut DecompressWork, bus: &mut Trace) {
    while work.step(bus).unwrap() {
        assert!(work.queue.len() <= MAX_BLOCK_INSTRUCTIONS);
        assert_eq!(work.foreground_tcycles(), bus.cycles);
    }
}
fn cases() -> impl Iterator<Item = (u8, &'static [u8])> {
    let bytes = include_bytes!("../../../../../docs/audits/hof-pc-load/front-codec-cases.bin");
    let mut at = 0;
    core::iter::from_fn(move || {
        if at == bytes.len() {
            return None;
        }
        let species = bytes[at];
        let length = u16::from_le_bytes(bytes[at + 1..at + 3].try_into().unwrap()) as usize;
        let input = &bytes[at + 3..at + 3 + length];
        at += 3 + length + 784 + 8;
        Some((species, input))
    })
}

#[test]
fn all_151_resumable_bodies_match_original_picture_and_foreground_receipts() {
    let mut count = 0;
    for (species, data) in cases() {
        let oracle = super::super::decode(data).unwrap();
        let mut work = DecompressWork::new(data).unwrap();
        let mut bus = Trace::default();
        assert!(work.output().is_none());
        finish(&mut work, &mut bus);
        assert_eq!(work.dimensions(), (oracle.width_tiles, oracle.height_tiles));
        assert_eq!(
            work.output().unwrap(),
            oracle.planes.concat(),
            "species {species}"
        );
        assert_eq!(
            bus.cycles + super::super::WRAPPER_T,
            oracle.work.foreground_tcycles,
            "species {species}"
        );
        let stats = work.completed_work().unwrap();
        assert_eq!(
            (
                stats.input_bits,
                stats.mode,
                stats.zero_groups,
                stats.literal_groups,
                stats.zero_packets,
                stats.run_prefix_ones
            ),
            (
                oracle.work.input_bits,
                oracle.work.mode,
                oracle.work.zero_groups,
                oracle.work.literal_groups,
                oracle.work.zero_packets,
                oracle.work.run_prefix_ones
            )
        );
        let end = bus.instructions;
        assert!(!work.step(&mut bus).unwrap());
        assert_eq!(end, bus.instructions);
        count += 1;
    }
    assert_eq!(count, 151);
}

#[test]
fn sparse_original_entry_points_match_the_actual_instruction_order() {
    let bytes = include_bytes!("../../../../../docs/audits/hof-pc-load/decompress-points.bin");
    let count = u16::from_le_bytes(bytes[..2].try_into().unwrap());
    assert_eq!(count, 6, "both natural modes at all three stock sizes");
    let mut at = 2;
    let mut total = 0;
    for _ in 0..count {
        let species = bytes[at];
        let expected_cycles = u32::from_le_bytes(bytes[at + 1..at + 5].try_into().unwrap());
        let points = u32::from_le_bytes(bytes[at + 5..at + 9].try_into().unwrap()) as usize;
        at += 9;
        let (_, input) = cases().find(|(id, _)| *id == species).unwrap();
        let mut work = DecompressWork::new(input).unwrap();
        let mut bus = Trace::default();
        finish(&mut work, &mut bus);
        assert_eq!(bus.cycles, u64::from(expected_cycles));
        assert_eq!(work.points.len(), points, "species {species}");
        for (index, actual) in work.points.iter().enumerate() {
            let expected = (
                bytes[at],
                u64::from(u32::from_le_bytes(
                    bytes[at + 1..at + 5].try_into().unwrap(),
                )),
            );
            assert_eq!(
                *actual, expected,
                "species {species}, original entry point {index}"
            );
            at += 5;
        }
        total += points;
    }
    assert_eq!(total, 54512);
    assert_eq!(at, bytes.len());
}

#[test]
fn refusal_and_serialized_resume_keep_exact_instruction_and_private_output() {
    let mut seen = [[false; 8]; 3];
    for (_, data) in cases().chain(core::iter::once((0, &[0x11, 0x3c, 0x13, 0xc1][..]))) {
        let oracle = super::super::decode(data).unwrap();
        let (mode, width) = (
            usize::from(oracle.work.mode),
            usize::from(oracle.width_tiles),
        );
        if seen[mode][width] {
            continue;
        }
        seen[mode][width] = true;
        let mut baseline = DecompressWork::new(data).unwrap();
        let mut expected = Trace::default();
        finish(&mut baseline, &mut expected);
        let mut work = DecompressWork::new(data).unwrap();
        let mut actual = Trace::default();
        while !work.finished() {
            assert!(work.output().is_none());
            if actual.instructions % 997 == 0 {
                actual.refuse = true;
                let before = actual.cycles;
                assert_eq!(work.step(&mut actual), Err(StepError::Bus(())));
                assert_eq!(actual.cycles, before);
                assert_eq!(work.foreground_tcycles(), before);
                let state = serde_json::to_vec(&work).unwrap();
                work = serde_json::from_slice(&state).unwrap();
                assert_eq!(serde_json::to_vec(&work).unwrap(), state);
                assert!(work.output().is_none());
            }
            work.step(&mut actual).unwrap();
        }
        assert_eq!(
            (actual.instructions, actual.cycles, actual.hash),
            (expected.instructions, expected.cycles, expected.hash)
        );
        assert_eq!(work.output(), baseline.output());
    }
    assert!(seen[1][5..=7].iter().all(|seen| *seen));
    assert!(seen[2][5..=7].iter().all(|seen| *seen));
    assert!(seen[0][1]);
}

#[test]
fn malformed_streams_stop_without_publishing_or_advancing_after_error() {
    assert!(matches!(DecompressWork::new(&[]), Err(Error::Truncated)));
    assert!(matches!(DecompressWork::new(&[0]), Err(Error::Dimensions)));
    for data in [&[0x55][..], &[0x55, 0x3f, 0xff, 0xff][..]] {
        let mut work = DecompressWork::new(data).unwrap();
        let mut bus = Trace::default();
        let error = loop {
            match work.step(&mut bus) {
                Ok(true) => (),
                Err(StepError::Decode(error)) => break error,
                other => panic!("malformed stream unexpectedly completed: {other:?}"),
            }
        };
        assert!(matches!(error, Error::Truncated | Error::RunLength));
        let before = bus.instructions;
        assert_eq!(work.step(&mut bus), Err(StepError::Decode(error)));
        assert_eq!(bus.instructions, before);
        assert!(work.output().is_none());
        assert!(!work.finished());
    }
}

#[test]
fn exact_plain_prefixes_preserve_every_instruction_across_owner_boundaries() {
    // Controlled instruction-budget seam, not actual frame/IRQ elapsed time.
    struct Budget {
        trace: Trace,
        available: u64,
        bulk: usize,
    }
    impl DisplayInstructionBus for Budget {
        type Error = ();
        fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, ()> {
            if u64::from(instruction.tcycles) > self.available {
                return Err(());
            }
            self.available -= u64::from(instruction.tcycles);
            self.trace.execute(instruction)
        }
        fn execute_plain_run<I: Iterator<Item = DisplayInstruction>>(
            &mut self,
            instructions: I,
        ) -> Result<usize, ()> {
            let mut count = 0;
            for instruction in instructions.take(37) {
                if u64::from(instruction.tcycles) > self.available {
                    break;
                }
                self.execute(instruction)?;
                count += 1;
            }
            self.bulk += count;
            Ok(count)
        }
    }
    for (_, data) in cases() {
        let mut baseline = DecompressWork::new(data).unwrap();
        let mut expected = Trace::default();
        finish(&mut baseline, &mut expected);
        let mut work = DecompressWork::new(data).unwrap();
        let mut owner = Budget {
            trace: Trace::default(),
            available: 0,
            bulk: 0,
        };
        let mut boundaries = 0;
        while !work.finished() {
            // Deliberately uneven grants force small and mid-block prefixes.
            owner.available += [1, 7, 29, 4003, 70224][boundaries % 5];
            match work.run_until_boundary(&mut owner) {
                Ok(()) => assert!(work.finished()),
                Err(StepError::Bus(())) => {
                    assert!(!work.finished());
                    assert!(work.output().is_none());
                    if boundaries % 13 == 0 {
                        let saved = serde_json::to_vec(&work).unwrap();
                        work = serde_json::from_slice(&saved).unwrap();
                    }
                }
                other => panic!("valid asset: {other:?}"),
            }
            assert_eq!(work.foreground_tcycles(), owner.trace.cycles);
            boundaries += 1;
        }
        assert!(owner.bulk > 1000);
        assert_eq!(
            (
                owner.trace.instructions,
                owner.trace.cycles,
                owner.trace.hash
            ),
            (expected.instructions, expected.cycles, expected.hash)
        );
        assert_eq!(work.output(), baseline.output());
    }
}
