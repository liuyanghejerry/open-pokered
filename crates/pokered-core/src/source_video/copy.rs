//! CopyVideoData[Double] foreground body from home/copy2.asm.
//! Inputs are the caller's live B/C/DE/HL, not captured scene totals. Request
//! ports share the owner's bus with VBlank. Only DelayFrame's real interrupt
//! return receipt can wake this caller; clearing size or an LCD edge cannot.
//! Construction starts at function entry (the outer CALL belongs to its parent).
use super::delay::{DelayFrameBus, DelayFrameWork, DelayProgress};
use super::CopyFormat;
use super::{DisplayAccess, DisplayInstruction};
use super::{LOADED_ROM_BANK, ROM_BANK_SELECT, ROM_BANK_TEMP};

pub const AUTO_BG_ENABLED: u16 = 0xffba;
pub const COPY_SIZE: u16 = 0xffc6;
pub const DOUBLE_SIZE: u16 = 0xffcb;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    ReadBg,
    PushBg,
    Zero,
    DisableBg,
    ReadBank,
    SaveBank,
    LoadBank,
    SetBank,
    SelectBank,
    SourceLow,
    StoreSourceLow,
    SourceHigh,
    StoreSourceHigh,
    DestLow,
    StoreDestLow,
    DestHigh,
    StoreDestHigh,
    Count,
    Compare,
    Branch,
    Eight,
    StoreSize,
    CallDelay,
    Delay,
    Remaining,
    Subtract,
    StoreRemaining,
    Loop,
    ReadSavedBank,
    RestoreBank,
    SelectSavedBank,
    PopBg,
    RestoreBg,
    Return,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VideoCopyCallerWork {
    step: Step,
    bank: u8,
    remaining: u8,
    source: u16,
    destination: u16,
    size_port: u16,
    a: u8,
    saved_bg: u8,
    full_batch: bool,
    delay: DelayFrameWork,
}
impl VideoCopyCallerWork {
    pub fn new(bank: u8, tiles: u8, source: u16, destination: u16, format: CopyFormat) -> Self {
        Self {
            step: Step::ReadBg,
            bank,
            remaining: tiles,
            source,
            destination,
            size_port: if format == CopyFormat::TwoBpp {
                COPY_SIZE
            } else {
                DOUBLE_SIZE
            },
            a: 0,
            saved_bg: 0,
            full_batch: false,
            delay: Default::default(),
        }
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    pub fn sleeping(&self) -> bool {
        self.step == Step::Delay && self.delay.sleeping()
    }
    pub fn size_port(&self) -> u16 {
        self.size_port
    }
    pub fn step<B: DelayFrameBus>(&mut self, bus: &mut B) -> Result<DelayProgress, B::Error> {
        use Step::*;
        if self.step == Done {
            return Ok(DelayProgress::Returned);
        }
        if self.step == Delay {
            return match self.delay.step(bus)? {
                DelayProgress::Returned => {
                    self.step = if self.full_batch {
                        Remaining
                    } else {
                        ReadSavedBank
                    };
                    Ok(DelayProgress::Instruction)
                }
                progress => Ok(progress),
            };
        }
        let read = |port| Some(DisplayAccess::Read { port, at_tcycle: 8 });
        let write = |port, value| {
            Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle: 8,
            })
        };
        let (tcycles, access) = match self.step {
            ReadBg => (12, read(AUTO_BG_ENABLED)),
            PushBg => (16, None),
            DisableBg | RestoreBg => (12, write(AUTO_BG_ENABLED, self.a)),
            ReadBank | ReadSavedBank => (
                12,
                read(if self.step == ReadBank {
                    LOADED_ROM_BANK
                } else {
                    ROM_BANK_TEMP
                }),
            ),
            SaveBank => (12, write(ROM_BANK_TEMP, self.a)),
            SetBank | RestoreBank => (12, write(LOADED_ROM_BANK, self.a)),
            SelectBank | SelectSavedBank => (
                16,
                Some(DisplayAccess::Write {
                    port: ROM_BANK_SELECT,
                    value: self.a,
                    at_tcycle: 12,
                }),
            ),
            StoreSourceLow => (12, write(self.size_port + 1, self.a)),
            StoreSourceHigh => (12, write(self.size_port + 2, self.a)),
            StoreDestLow => (12, write(self.size_port + 3, self.a)),
            StoreDestHigh => (12, write(self.size_port + 4, self.a)),
            Compare | Eight | Subtract => (8, None),
            Branch => (if self.full_batch { 12 } else { 8 }, None),
            StoreSize => (12, write(self.size_port, self.a)),
            CallDelay => (24, None),
            Loop | PopBg => (12, None),
            Return => (16, None),
            _ => (4, None),
        };
        let value = bus.execute(DisplayInstruction { tcycles, access })?;
        self.step = match self.step {
            ReadBg => {
                self.a = value.expect("BG control read");
                PushBg
            }
            PushBg => {
                self.saved_bg = self.a;
                Zero
            }
            Zero => {
                self.a = 0;
                DisableBg
            }
            DisableBg => ReadBank,
            ReadBank => {
                self.a = value.expect("loaded bank read");
                SaveBank
            }
            SaveBank => LoadBank,
            LoadBank => {
                self.a = self.bank;
                SetBank
            }
            SetBank => SelectBank,
            SelectBank => SourceLow,
            SourceLow => {
                self.a = self.source as u8;
                StoreSourceLow
            }
            StoreSourceLow => SourceHigh,
            SourceHigh => {
                self.a = (self.source >> 8) as u8;
                StoreSourceHigh
            }
            StoreSourceHigh => DestLow,
            DestLow => {
                self.a = self.destination as u8;
                StoreDestLow
            }
            StoreDestLow => DestHigh,
            DestHigh => {
                self.a = (self.destination >> 8) as u8;
                StoreDestHigh
            }
            StoreDestHigh => Count,
            Count => {
                self.a = self.remaining;
                Compare
            }
            Compare => {
                self.full_batch = self.a >= 8;
                Branch
            }
            Branch => {
                if self.full_batch {
                    Eight
                } else {
                    StoreSize
                }
            }
            Eight => {
                self.a = 8;
                StoreSize
            }
            StoreSize => CallDelay,
            CallDelay => {
                self.delay = Default::default();
                Delay
            }
            Remaining => {
                self.a = self.remaining;
                Subtract
            }
            Subtract => {
                self.a = self.a.wrapping_sub(8);
                StoreRemaining
            }
            StoreRemaining => {
                self.remaining = self.a;
                Loop
            }
            Loop => Count,
            ReadSavedBank => {
                self.a = value.expect("saved bank read");
                RestoreBank
            }
            RestoreBank => SelectSavedBank,
            SelectSavedBank => PopBg,
            PopBg => {
                self.a = self.saved_bg;
                RestoreBg
            }
            RestoreBg => Return,
            Return => Done,
            Delay | Done => unreachable!(),
        };
        Ok(if self.finished() {
            DelayProgress::Returned
        } else {
            DelayProgress::Instruction
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::*;
    use crate::source_video::delay::VBLANK_FLAG_PORT;
    use crate::source_video::tests::{Bus, Error};
    #[test]
    fn original_hof_49_tile_caller_matches_each_observed_loop_and_pointer() {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/audits/hof-pc-load/copy-caller-49.json"
        ))
        .unwrap();
        let case = &proof["case"];
        let byte = |name: &str| case[name].as_u64().unwrap() as u8;
        let word = |name: &str| case[name].as_u64().unwrap() as u16;
        let mut bus = Bus::new(
            byte("bg_before"),
            byte("loaded_before"),
            word("source"),
            word("destination"),
        );
        let mut work = VideoCopyCallerWork::new(
            byte("bank"),
            byte("tiles"),
            word("source"),
            word("destination"),
            CopyFormat::TwoBpp,
        );
        let mut observed = Vec::new();
        let mut wakes = 0;
        for _ in 0..10000 {
            if work.finished() {
                break;
            }
            if work.step == Step::Count {
                observed.push(serde_json::json!({"remaining":work.remaining,"source":bus.pair(COPY_SIZE+1),"destination":bus.pair(COPY_SIZE+3),"bg":bus.ports[&AUTO_BG_ENABLED],"loaded_bank":bus.ports[&LOADED_ROM_BANK]}));
            }
            if work.step(&mut bus).unwrap() == DelayProgress::Sleeping {
                let size = bus.ports[&COPY_SIZE];
                bus.ports.insert(COPY_SIZE, 0);
                bus.ports.insert(VBLANK_FLAG_PORT, 0);
                assert_eq!(work.step(&mut bus).unwrap(), DelayProgress::Sleeping);
                bus.ports.insert(COPY_SIZE, size);
                bus.service_copy(CopyFormat::TwoBpp);
                wakes += 1;
            }
        }
        assert!(work.finished());
        assert_eq!(wakes, 7);
        assert_eq!(serde_json::Value::Array(observed), case["loops"]);
        assert_eq!(&bus.destination[..784], &bus.source[..784]);
        assert_eq!(bus.ports[&AUTO_BG_ENABLED], byte("bg_before"));
        assert_eq!(bus.ports[&LOADED_ROM_BANK], byte("loaded_before"));
        assert_eq!(bus.ports[&ROM_BANK_SELECT], byte("loaded_before"));
    }
    #[test]
    fn zero_and_eight_multiples_still_require_the_terminal_interrupt_return() {
        for format in [CopyFormat::TwoBpp, CopyFormat::OneBppDouble] {
            for tiles in [0, 1, 8, 9, 16, 49] {
                let mut bus = Bus::new(0xa5, 7, 0xa188, 0x9000);
                let mut work = VideoCopyCallerWork::new(4, tiles, 0xa188, 0x9000, format);
                let mut waits = 0;
                let mut sizes = Vec::new();
                for _ in 0..10000 {
                    if work.finished() {
                        break;
                    }
                    if work.step(&mut bus).unwrap() == DelayProgress::Sleeping {
                        let size = bus.ports[&work.size_port()];
                        sizes.push(size);
                        assert_eq!(bus.ports[&AUTO_BG_ENABLED], 0);
                        bus.service_copy(format);
                        waits += 1;
                    }
                }
                assert!(work.finished());
                assert_eq!(waits, u64::from(tiles) / 8 + 1);
                assert_eq!(sizes.last().copied(), Some(tiles % 8));
                assert_eq!(bus.ports[&AUTO_BG_ENABLED], 0xa5);
                assert_eq!(bus.ports[&LOADED_ROM_BANK], 7);
                if format == CopyFormat::TwoBpp {
                    assert_eq!(
                        &bus.destination[..usize::from(tiles) * 16],
                        &bus.source[..usize::from(tiles) * 16]
                    );
                } else {
                    for i in 0..usize::from(tiles) * 8 {
                        assert_eq!(bus.destination[i * 2], bus.source[i]);
                        assert_eq!(bus.destination[i * 2 + 1], bus.source[i]);
                    }
                }
            }
        }
    }
    #[test]
    fn refused_instruction_preserves_saved_bg_and_pending_bank_write() {
        let mut work = VideoCopyCallerWork::new(4, 12, 0xa188, 0x9000, CopyFormat::TwoBpp);
        let mut bus = Bus::new(0xa5, 7, 0xa188, 0x9000);
        while work.step != Step::SetBank {
            work.step(&mut bus).unwrap();
        }
        let before = work.clone();
        let ports = bus.ports.clone();
        let length = bus.body.len();
        bus.reject_next = true;
        assert_eq!(work.step(&mut bus), Err(Error::Refused));
        assert_eq!(work, before);
        assert_eq!(bus.ports, ports);
        assert_eq!(bus.body.len(), length);
        let mut restored: VideoCopyCallerWork =
            serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
        restored.step(&mut bus).unwrap();
        assert_eq!(bus.ports[&LOADED_ROM_BANK], 4);
        assert_eq!(bus.ports[&ROM_BANK_TEMP], 7);
        assert_eq!(work.saved_bg, 0xa5);
    }
}
