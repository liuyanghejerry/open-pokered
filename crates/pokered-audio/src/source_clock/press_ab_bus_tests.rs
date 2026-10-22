use super::*;
use crate::source_clock::apu_bus::ApuAccess;
use alloc::{string::String, vec::Vec};
use std::collections::BTreeMap;

#[derive(serde::Deserialize)]
struct Fixture {
    rom_sha256: String,
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct Case {
    input: PressAbInput,
    data: Vec<u8>,
    nr51: u8,
    trace: Vec<(u16, u8)>,
    accesses: Vec<Access>,
    final_apu: BTreeMap<u16, u8>,
    result: PressAbOutcome,
}

#[derive(serde::Deserialize)]
struct Access {
    instruction: usize,
    pc: u16,
    kind: String,
    address: u16,
    value: u8,
    at_tcycle: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Bus {
    registers: BTreeMap<u16, u8>,
    cycles: u32,
    // Timestamp, address, value, is_write. Reads record the actual returned byte.
    events: Vec<(u32, u16, u8, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refused {
    Deadline,
    MissingRegister(u16),
}

impl Bus {
    fn new(nr51: u8) -> Self {
        Self {
            registers: BTreeMap::from([(0xff25, nr51)]),
            cycles: 0,
            events: Vec::new(),
        }
    }

    fn execute(&mut self, instruction: ApuInstruction) -> Result<Option<u8>, Refused> {
        let receipt = match instruction.access {
            Some(ApuAccess::Read { address, .. }) => Some(
                *self
                    .registers
                    .get(&address)
                    .ok_or(Refused::MissingRegister(address))?,
            ),
            _ => None,
        };
        match instruction.access {
            Some(ApuAccess::Read { address, at_tcycle }) => self.events.push((
                self.cycles + u32::from(at_tcycle),
                address,
                receipt.unwrap(),
                false,
            )),
            Some(ApuAccess::Write {
                address,
                value,
                at_tcycle,
            }) => {
                self.registers.insert(address, value);
                self.events
                    .push((self.cycles + u32::from(at_tcycle), address, value, true));
            }
            None => {}
        }
        self.cycles += u32::from(instruction.tcycles);
        Ok(receipt)
    }
}

fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
    serde_json::from_slice(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("press_ab_bus_golden.json")).unwrap()
}

#[test]
fn actual_apu_accesses_match_both_original_banks_at_every_resumable_instruction() {
    let fixture = fixture();
    assert_eq!(
        fixture.rom_sha256,
        "5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b"
    );
    assert_eq!(fixture.cases.len(), 200);
    let mut access_count = 0;
    for case in fixture.cases {
        let mut work = PressAbInstructions::new(case.input, &case.data).unwrap();
        let mut bus = Bus::new(case.nr51);
        let mut expected_events = Vec::new();
        for (index, &(pc, tcycles)) in case.trace.iter().enumerate() {
            let access = case
                .accesses
                .iter()
                .find(|access| access.instruction == index);
            let expected = ApuInstruction {
                tcycles,
                access: access.map(|access| {
                    assert_eq!(access.pc, pc);
                    match access.kind.as_str() {
                        "read" => ApuAccess::Read {
                            address: access.address,
                            at_tcycle: access.at_tcycle,
                        },
                        "write" => ApuAccess::Write {
                            address: access.address,
                            value: access.value,
                            at_tcycle: access.at_tcycle,
                        },
                        _ => panic!("unknown original access kind"),
                    }
                }),
            };
            let frozen_work = work.clone();
            let frozen_bus = bus.clone();
            for _ in 0..2 {
                assert_eq!(
                    work.try_step_with_bus(&case.data, |instruction| {
                        assert_eq!(
                            instruction, expected,
                            "bank {} ptr {} PC {pc:04x}",
                            case.input.bank, case.input.ptr
                        );
                        Err::<Option<u8>, _>(Refused::Deadline)
                    }),
                    Err(PressAbStepError::Clock(Refused::Deadline))
                );
                assert_eq!(work, frozen_work);
                assert_eq!(bus, frozen_bus);
            }
            work = round_trip(&work);
            bus = round_trip(&bus);
            if let Some(access) = access {
                expected_events.push((
                    bus.cycles + u32::from(access.at_tcycle),
                    access.address,
                    access.value,
                    access.kind == "write",
                ));
            }
            let done = work
                .try_step_with_bus(&case.data, |instruction| {
                    assert_eq!(
                        instruction, expected,
                        "bank {} ptr {} PC {pc:04x}",
                        case.input.bank, case.input.ptr
                    );
                    bus.execute(instruction)
                })
                .unwrap();
            assert_eq!(done, index + 1 == case.trace.len());
            assert_eq!(bus.events, expected_events);
            assert_eq!(work.cycles(), bus.cycles);
        }
        assert_eq!(work.result(), Some(case.result));
        assert_eq!(bus.registers, case.final_apu);
        assert_eq!(
            work.try_step_with_bus(&case.data, |_| -> Result<_, Refused> {
                panic!("completed cursor must not execute twice")
            }),
            Ok(true)
        );
        access_count += bus.events.len();
    }
    assert_eq!(access_count, 1120);
}

#[test]
fn accepted_nr51_read_is_retained_across_serialization_and_later_bus_changes() {
    let case = fixture()
        .cases
        .into_iter()
        .find(|case| case.input.bank == 31 && case.input.ptr == 6)
        .unwrap();
    let mut work = PressAbInstructions::new(case.input, &case.data).unwrap();
    let mut bus = Bus::new(0x20);
    let mut read = false;
    for _ in 0..case.trace.len() {
        work.try_step_with_bus(&case.data, |instruction| {
            read = matches!(
                instruction.access,
                Some(ApuAccess::Read {
                    address: 0xff25,
                    ..
                })
            );
            bus.execute(instruction)
        })
        .unwrap();
        if read {
            break;
        }
    }
    assert!(read);
    assert_eq!(
        bus.events.last().map(|event| (event.1, event.2, event.3)),
        Some((0xff25, 0x20, false))
    );
    work = round_trip(&work);
    let frozen = work.clone();
    assert_eq!(
        work.try_step_with_bus(&case.data, |_| Err::<Option<u8>, _>(Refused::Deadline)),
        Err(PressAbStepError::Clock(Refused::Deadline))
    );
    assert_eq!(work, frozen);
    bus.registers.insert(0xff25, 0xc0);
    while !work
        .try_step_with_bus(&case.data, |instruction| bus.execute(instruction))
        .unwrap()
    {}
    assert_eq!(bus.registers[&0xff25], 0x31);
    assert_eq!(bus.events.iter().filter(|event| !event.3).count(), 1);
}

#[test]
fn missing_actual_nr51_and_changed_program_refuse_without_replaying_prior_stores() {
    let case = fixture()
        .cases
        .into_iter()
        .find(|case| case.input.bank == 2 && case.input.ptr == 0)
        .unwrap();
    let mut work = PressAbInstructions::new(case.input, &case.data).unwrap();
    let mut bus = Bus::new(0xa5);
    bus.registers.remove(&0xff25);
    let read_index = case
        .accesses
        .iter()
        .find(|access| access.kind == "read")
        .unwrap()
        .instruction;
    for _ in 0..read_index {
        assert!(!work
            .try_step_with_bus(&case.data, |instruction| bus.execute(instruction))
            .unwrap());
    }
    let frozen_work = work.clone();
    let frozen_bus = bus.clone();
    for _ in 0..2 {
        assert_eq!(
            work.try_step_with_bus(&case.data, |instruction| bus.execute(instruction)),
            Err(PressAbStepError::Clock(Refused::MissingRegister(0xff25)))
        );
        assert_eq!(work, frozen_work);
        assert_eq!(bus, frozen_bus);
    }
    let mut changed = case.data.clone();
    changed[0] ^= 1;
    assert_eq!(
        work.try_step_with_bus(&changed, |_| -> Result<_, Refused> {
            panic!("changed program cannot execute")
        }),
        Err(PressAbStepError::ChangedProgram)
    );
    assert_eq!(work, frozen_work);
    bus.registers.insert(0xff25, 0xa5);
    work = round_trip(&work);
    while !work
        .try_step_with_bus(&case.data, |instruction| bus.execute(instruction))
        .unwrap()
    {}
    assert_eq!(bus.events.len(), 7);
    assert_eq!(bus.registers[&0xff25], 0xb5);
    assert_eq!(work.result(), Some(case.result));
}
