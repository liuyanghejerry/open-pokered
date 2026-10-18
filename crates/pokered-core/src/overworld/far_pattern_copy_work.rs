//! CALL FarCopyData2 plus its real bank save/switch/restore instructions.
//! The CopyData child and both bus types share one time owner. Returning
//! from CopyData does not skip the parent's bank restoration or RET.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::field_restore::{CopyFormat, PatternCopy, PatternSource};
use super::pattern_copy_work::{PatternCopyDataWork, PatternInstructionBus};

pub const ROM_BANK_TEMP: u16 = 0xff8b;
pub const LOADED_ROM_BANK: u16 = 0xffb8;
pub const ROM_BANK_SELECT: u16 = 0x2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    CallerCall,
    StoreTemp,
    ReadLoaded,
    PushBank,
    ReadTemp,
    SetLoaded,
    SelectBank,
    CallCopy,
    Copy,
    PopBank,
    RestoreLoaded,
    RestoreBank,
    Return,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FarPatternCopyCall {
    step: Step,
    a: u8,
    saved_bank: u8,
    copy: PatternCopyDataWork,
}

impl FarPatternCopyCall {
    pub fn from_2bpp(copy: PatternCopy) -> Option<Self> {
        if copy.format != CopyFormat::TwoBpp {
            return None;
        }
        let PatternSource::Sprite(raw) = copy.source else {
            return None;
        };
        let bank = super::sprite_sheet_metadata::sprite_sheet_metadata(raw)?.bank;
        Some(Self {
            step: Step::CallerCall,
            a: bank,
            saved_bank: 0,
            copy: PatternCopyDataWork::from_2bpp(copy)?,
        })
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }

    /// True means one source instruction executed. False means the complete
    /// CALL/body/RET returned. Errors preserve the pending instruction.
    pub fn step<B>(&mut self, bus: &mut B) -> Result<bool, <B as DisplayInstructionBus>::Error>
    where
        B: DisplayInstructionBus
            + PatternInstructionBus<Error = <B as DisplayInstructionBus>::Error>,
    {
        use Step::*;
        let effective_step = if self.step == Copy {
            if self.copy.step(bus)?.is_some() {
                return Ok(true);
            }
            PopBank
        } else {
            self.step
        };
        let (tcycles, access) = match effective_step {
            CallerCall | CallCopy => (24, None),
            StoreTemp => (
                12,
                Some(DisplayAccess::Write {
                    port: ROM_BANK_TEMP,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            ReadLoaded => (
                12,
                Some(DisplayAccess::Read {
                    port: LOADED_ROM_BANK,
                    at_tcycle: 8,
                }),
            ),
            PushBank | Return => (16, None),
            ReadTemp => (
                12,
                Some(DisplayAccess::Read {
                    port: ROM_BANK_TEMP,
                    at_tcycle: 8,
                }),
            ),
            SetLoaded | RestoreLoaded => (
                12,
                Some(DisplayAccess::Write {
                    port: LOADED_ROM_BANK,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            SelectBank | RestoreBank => (
                16,
                Some(DisplayAccess::Write {
                    port: ROM_BANK_SELECT,
                    value: self.a,
                    at_tcycle: 12,
                }),
            ),
            PopBank => (12, None),
            Done => return Ok(false),
            Copy => unreachable!(),
        };
        let value = DisplayInstructionBus::execute(bus, DisplayInstruction { tcycles, access })?;
        self.step = match effective_step {
            CallerCall => StoreTemp,
            StoreTemp => ReadLoaded,
            ReadLoaded => {
                self.a = value.expect("loaded bank read");
                PushBank
            }
            PushBank => {
                self.saved_bank = self.a;
                ReadTemp
            }
            ReadTemp => {
                self.a = value.expect("temporary bank read");
                SetLoaded
            }
            SetLoaded => SelectBank,
            SelectBank => CallCopy,
            CallCopy => Copy,
            PopBank => {
                self.a = self.saved_bank;
                RestoreLoaded
            }
            RestoreLoaded => RestoreBank,
            RestoreBank => Return,
            Return | Done => Done,
            Copy => unreachable!(),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use crate::overworld::field_restore::{PatternAssets, PatternMemory};
    use crate::overworld::pattern_copy_work::PatternInstruction;
    struct Bytes;
    impl PatternAssets for Bytes {
        fn read_pattern_byte(&self, _: PatternSource, n: u16) -> Option<u8> {
            Some(n as u8 ^ 0xa7)
        }
    }
    struct Bus {
        time: DisplayWorkTime,
        memory: PatternMemory,
    }
    impl DisplayInstructionBus for Bus {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
            self.time.execute(i)
        }
    }
    impl PatternInstructionBus for Bus {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: PatternInstruction) -> Result<Option<u16>, Self::Error> {
            PatternInstructionBus::execute(&mut self.time.pattern_bus(&mut self.memory, &Bytes), i)
        }
    }
    #[test]
    fn caller_switches_real_bank_before_copy_and_restores_after_child_return() {
        let mut bus = Bus {
            time: DisplayWorkTime::default(),
            memory: PatternMemory::default(),
        };
        bus.time.begin_native_update();
        bus.time
            .execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff40,
                    value: 0x11,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
        for port in [LOADED_ROM_BANK, ROM_BANK_SELECT] {
            bus.time
                .execute(DisplayInstruction {
                    tcycles: 16,
                    access: Some(DisplayAccess::Write {
                        port,
                        value: 37,
                        at_tcycle: 12,
                    }),
                })
                .unwrap();
        }
        let start = bus.time.elapsed();
        let mut call = FarPatternCopyCall::from_2bpp(PatternCopy {
            source: PatternSource::Sprite(14),
            source_offset: 0,
            destination: 0x80c0,
            tiles: 12,
            format: CopyFormat::TwoBpp,
        })
        .unwrap();
        for _ in 0..8 {
            assert!(call.step(&mut bus).unwrap());
        }
        assert_eq!(bus.time.elapsed() - start, 128);
        assert_eq!(bus.time.selected_rom_bank(), 5);
        assert_eq!(bus.time.source_loaded_rom_bank(), 5);
        assert_eq!(bus.memory.read(0x80c0), Some(0));
        while call.step(&mut bus).unwrap() {}
        assert!(call.finished());
        assert_eq!(bus.time.elapsed() - start, 52 * 192 + 12 + 184);
        assert_eq!(bus.time.source_loaded_rom_bank(), 37);
        assert_eq!(bus.time.selected_rom_bank(), 37);
        for n in 0..192 {
            assert_eq!(bus.memory.read(0x80c0 + n), Some(n as u8 ^ 0xa7));
        }
    }
    #[test]
    fn exhausted_grant_preserves_switched_bank_and_mid_copy_snapshot() {
        let mut bus = Bus {
            time: DisplayWorkTime::default(),
            memory: PatternMemory::default(),
        };
        bus.time.begin_native_update();
        bus.time
            .execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xff40,
                    value: 0x11,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
        while bus.time.remaining() > 140 {
            bus.time
                .execute(DisplayInstruction {
                    tcycles: 4,
                    access: None,
                })
                .unwrap();
        }
        let start = bus.time.elapsed();
        let mut call = FarPatternCopyCall::from_2bpp(PatternCopy {
            source: PatternSource::Sprite(31),
            source_offset: 0,
            destination: 0x8000,
            tiles: 12,
            format: CopyFormat::TwoBpp,
        })
        .unwrap();
        loop {
            let before = call;
            match call.step(&mut bus) {
                Ok(true) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(call, before);
                    break;
                }
                other => panic!("unexpected banked-copy result {other:?}"),
            }
        }
        assert_eq!(bus.time.selected_rom_bank(), 4);
        let saved = serde_json::to_string(&(call, &bus.time, &bus.memory)).unwrap();
        let (mut copy, time, memory): (FarPatternCopyCall, DisplayWorkTime, PatternMemory) =
            serde_json::from_str(&saved).unwrap();
        let mut other = Bus { time, memory };
        bus.time.begin_native_update();
        other.time.begin_native_update();
        while call.step(&mut bus).unwrap() {}
        while copy.step(&mut other).unwrap() {}
        assert_eq!(call, copy);
        assert_eq!(bus.memory, other.memory);
        assert_eq!(bus.time.elapsed(), other.time.elapsed());
        assert_eq!(bus.time.elapsed() - start, 52 * 192 + 12 + 184);
        assert_eq!(bus.time.source_loaded_rom_bank(), 1);
        assert_eq!(bus.time.selected_rom_bank(), 1);
    }
}
