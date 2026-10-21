//! Source UncompressSpriteData bank/SRAM wrapper around the resumable body.
//! Reads and writes use the same owner's bus as every body instruction. This
//! does not invent an initial loaded bank, restore SRAM controls the original
//! leaves enabled, or expose scratch before the wrapper's final RET.
use super::{
    continuation::{DecompressWork, StepError},
    Error,
};
use crate::source_video::{
    DisplayAccess, DisplayInstruction, DisplayInstructionBus, LOADED_ROM_BANK, ROM_BANK_SELECT,
};

pub const SRAM_ENABLE: u16 = 0;
pub const SRAM_BANK: u16 = 0x4000;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DecompressCallerWork {
    body: DecompressWork,
    source_bank: u8,
    saved_bank: Option<u8>,
    phase: u8,
    foreground_tcycles: u64,
}

impl DecompressCallerWork {
    pub fn new(source_bank: u8, data: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            body: DecompressWork::new(data)?,
            source_bank,
            saved_bank: None,
            phase: 0,
            foreground_tcycles: 0,
        })
    }
    pub fn finished(&self) -> bool {
        self.phase == 16
    }
    pub fn foreground_tcycles(&self) -> u64 {
        self.foreground_tcycles
    }
    pub fn output(&self) -> Option<&[u8]> {
        if self.finished() {
            self.body.output()
        } else {
            None
        }
    }
    pub fn dimensions(&self) -> (u8, u8) {
        self.body.dimensions()
    }

    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<bool, StepError<B::Error>> {
        if self.finished() {
            return Ok(false);
        }
        if self.phase == 11 {
            if !self.body.finished() {
                let before = self.body.foreground_tcycles();
                let result = self.body.step(bus);
                self.foreground_tcycles += self.body.foreground_tcycles() - before;
                return result;
            }
            self.phase = 12;
        }
        let write = |port, value, tcycles| DisplayInstruction {
            tcycles,
            access: Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle: tcycles - 4,
            }),
        };
        let plain = |tcycles| DisplayInstruction {
            tcycles,
            access: None,
        };
        let instruction = match self.phase {
            0 => plain(4), // LD B,A
            1 => DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Read {
                    port: LOADED_ROM_BANK,
                    at_tcycle: 8,
                }),
            },
            2 => plain(16), // PUSH AF
            3 => plain(4),  // LD A,B
            4 => write(LOADED_ROM_BANK, self.source_bank, 12),
            5 => write(ROM_BANK_SELECT, self.source_bank, 16),
            6 => plain(8), // LD A,RAMG_SRAM_ENABLE
            7 => write(SRAM_ENABLE, 0x0a, 16),
            8 => plain(4), // XOR A
            9 => write(SRAM_BANK, 0, 16),
            10 => plain(24), // CALL _UncompressSpriteData
            12 => plain(12), // POP AF
            13 => write(
                LOADED_ROM_BANK,
                self.saved_bank.expect("executed bank read"),
                12,
            ),
            14 => write(
                ROM_BANK_SELECT,
                self.saved_bank.expect("executed bank read"),
                16,
            ),
            15 => plain(16), // RET
            _ => unreachable!("source wrapper phase"),
        };
        let receipt = bus.execute(instruction).map_err(StepError::Bus)?;
        if self.phase == 1 {
            self.saved_bank = Some(receipt.expect("source bus must return its actual bank read"));
        }
        self.foreground_tcycles += u64::from(instruction.tcycles);
        self.phase += 1;
        Ok(true)
    }

    pub fn run_until_boundary<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<(), StepError<B::Error>> {
        while !self.finished() {
            if self.phase == 11 {
                let before = self.body.foreground_tcycles();
                let result = self.body.run_until_boundary(bus);
                self.foreground_tcycles += self.body.foreground_tcycles() - before;
                result?;
                self.phase = 12;
            } else {
                self.step(bus)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::*;

    struct Bus {
        loaded: u8,
        selected: u8,
        ram_enable: u8,
        ram_bank: u8,
        cycles: u64,
        accesses: Vec<(u64, u16, u8)>,
        refuse: bool,
    }
    impl Bus {
        fn new(loaded: u8) -> Self {
            Self {
                loaded,
                selected: loaded,
                ram_enable: 0,
                ram_bank: 3,
                cycles: 0,
                accesses: Vec::new(),
                refuse: false,
            }
        }
    }
    impl DisplayInstructionBus for Bus {
        type Error = ();
        fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, ()> {
            if self.refuse {
                self.refuse = false;
                return Err(());
            }
            let receipt = match instruction.access {
                Some(DisplayAccess::Read {
                    port: LOADED_ROM_BANK,
                    at_tcycle,
                }) => {
                    self.accesses.push((
                        self.cycles + u64::from(at_tcycle),
                        LOADED_ROM_BANK,
                        self.loaded,
                    ));
                    Some(self.loaded)
                }
                Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle,
                }) => {
                    self.accesses
                        .push((self.cycles + u64::from(at_tcycle), port, value));
                    match port {
                        LOADED_ROM_BANK => self.loaded = value,
                        ROM_BANK_SELECT => self.selected = value,
                        SRAM_ENABLE => self.ram_enable = value,
                        SRAM_BANK => self.ram_bank = value,
                        _ => panic!("unexpected port"),
                    }
                    None
                }
                None => None,
                other => panic!("unexpected read {other:?}"),
            };
            self.cycles += u64::from(instruction.tcycles);
            Ok(receipt)
        }
    }
    #[test]
    fn wrapper_uses_actual_bank_receipt_and_keeps_source_sram_effects() {
        let input = &[0x11, 0x3c, 0x13, 0xc1]; // controlled mode0 zero asset
        for bank in [1, 9, 29, 255] {
            let mut bus = Bus::new(bank);
            let mut work = DecompressCallerWork::new(30, input).unwrap();
            work.run_until_boundary(&mut bus).unwrap();
            let body = bus.cycles - super::super::WRAPPER_T;
            assert_eq!(
                work.foreground_tcycles(),
                super::super::decode(input).unwrap().work.foreground_tcycles
            );
            assert_eq!(
                (bus.loaded, bus.selected, bus.ram_enable, bus.ram_bank),
                (bank, bank, 0x0a, 0)
            );
            assert_eq!(
                bus.accesses,
                vec![
                    (12, LOADED_ROM_BANK, bank),
                    (44, LOADED_ROM_BANK, 30),
                    (60, ROM_BANK_SELECT, 30),
                    (84, SRAM_ENABLE, 0x0a),
                    (104, SRAM_BANK, 0),
                    (body + 152, LOADED_ROM_BANK, bank),
                    (body + 168, ROM_BANK_SELECT, bank)
                ]
            );
            assert!(work.finished());
            assert_eq!(work.output().unwrap(), &[0; 784]);
        }
    }
    #[test]
    fn refused_wrapper_access_retries_same_store_after_serialized_resume() {
        let input = &[0x11, 0x3c, 0x13, 0xc1];
        for phase in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15] {
            let mut bus = Bus::new(7);
            let mut work = DecompressCallerWork::new(28, input).unwrap();
            while work.phase != phase && !(phase == 12 && work.phase == 11 && work.body.finished())
            {
                assert!(
                    work.step(&mut bus).unwrap(),
                    "requested wrapper boundary must be reachable"
                );
            }
            assert!(work.output().is_none());
            let before = (
                bus.cycles,
                bus.accesses.clone(),
                bus.loaded,
                bus.selected,
                bus.ram_enable,
                bus.ram_bank,
            );
            bus.refuse = true;
            assert_eq!(work.step(&mut bus), Err(StepError::Bus(())));
            assert_eq!(
                before,
                (
                    bus.cycles,
                    bus.accesses.clone(),
                    bus.loaded,
                    bus.selected,
                    bus.ram_enable,
                    bus.ram_bank
                )
            );
            work = serde_json::from_slice(&serde_json::to_vec(&work).unwrap()).unwrap();
            work.run_until_boundary(&mut bus).unwrap();
            assert_eq!(
                (bus.loaded, bus.selected, bus.ram_enable, bus.ram_bank),
                (7, 7, 0x0a, 0)
            );
            assert_eq!(bus.accesses.len(), 7);
            assert_eq!(
                bus.cycles,
                super::super::decode(input).unwrap().work.foreground_tcycles
            );
        }
    }

    #[test]
    fn partial_body_grants_keep_wrapper_receipts_and_bank_restore_once() {
        struct Budget {
            bus: Bus,
            remaining: u64,
        }
        impl DisplayInstructionBus for Budget {
            type Error = ();
            fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, ()> {
                if u64::from(instruction.tcycles) > self.remaining {
                    return Err(());
                }
                self.remaining -= u64::from(instruction.tcycles);
                self.bus.execute(instruction)
            }
        }
        // Controlled bus grants, not real frame/IRQ receipts.
        let input = &[0x11, 0x3c, 0x13, 0xc1];
        let mut owner = Budget {
            bus: Bus::new(9),
            remaining: 0,
        };
        let mut work = DecompressCallerWork::new(29, input).unwrap();
        let mut partial = 0;
        while !work.finished() {
            owner.remaining += 997;
            match work.run_until_boundary(&mut owner) {
                Err(StepError::Bus(())) => {
                    partial += 1;
                    assert!(work.output().is_none());
                    work = serde_json::from_slice(&serde_json::to_vec(&work).unwrap()).unwrap();
                }
                Ok(()) => assert!(work.finished()),
                other => panic!("valid controlled asset: {other:?}"),
            }
            assert_eq!(work.foreground_tcycles(), owner.bus.cycles);
        }
        assert!(partial > 10);
        assert_eq!(owner.bus.accesses.len(), 7);
        assert_eq!((owner.bus.loaded, owner.bus.selected), (9, 9));
        assert_eq!(
            owner.bus.cycles,
            super::super::decode(input).unwrap().work.foreground_tcycles
        );
    }
}
