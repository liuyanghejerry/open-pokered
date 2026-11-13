//! CopyVideoData[Double] foreground body from home/copy2.asm.
//! Inputs are the caller's live B/C/DE/HL, not captured scene totals. Request
//! ports share the owner's bus with VBlank. Only DelayFrame's real interrupt
//! return receipt can wake this caller; clearing size or an LCD edge cannot.
//! Construction starts at function entry (the outer CALL belongs to its parent).
use super::display_delay_work::{DelayFrameBus, DelayFrameWork, DelayProgress};
use super::display_lcd_work::{DisplayAccess, DisplayInstruction};
use super::far_pattern_copy_work::{LOADED_ROM_BANK, ROM_BANK_SELECT, ROM_BANK_TEMP};
use super::field_restore::CopyFormat;

pub const AUTO_BG_ENABLED: u16 = 0xffba;
pub const AUTO_BG_PORTION: u16 = 0xffbb;
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
    use super::super::display_lcd_work::DisplayInstructionBus;
    use super::*;
    use crate::alloc_prelude::*;
    use std::collections::BTreeMap;
    #[derive(serde::Deserialize)]
    struct Case {
        bank: u8,
        tiles: u8,
        source: u16,
        destination: u16,
        double: bool,
        bg_before: u8,
        loaded_before: u8,
        instructions: Vec<DisplayInstruction>,
    }
    // Explicit synthetic owner seam: this validates the foreground against
    // ROM instructions, not video/ISR work or its elapsed sleeping duration.
    struct Bus {
        ports: BTreeMap<u16, u8>,
        returns: u64,
        body: Vec<DisplayInstruction>,
        capture: bool,
        reject_next: bool,
    }
    impl DisplayInstructionBus for Bus {
        type Error = ();
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, ()> {
            if self.reject_next {
                self.reject_next = false;
                return Err(());
            }
            if self.capture {
                self.body.push(i);
            }
            Ok(match i.access {
                Some(DisplayAccess::Read { port, .. }) => {
                    Some(*self.ports.get(&port).expect("real bound port"))
                }
                Some(DisplayAccess::Write { port, value, .. }) => {
                    self.ports.insert(port, value);
                    None
                }
                None => None,
            })
        }
    }
    impl DelayFrameBus for Bus {
        fn interrupt_returns(&self) -> u64 {
            self.returns
        }
    }
    #[test]
    fn normal_rom_foreground_instructions_and_writes_match_through_terminal_zero_wait() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("video_copy_callers_1162.json")).unwrap();
        assert!(!cases.is_empty());
        for c in cases {
            let mut work = VideoCopyCallerWork::new(
                c.bank,
                c.tiles,
                c.source,
                c.destination,
                if c.double {
                    CopyFormat::OneBppDouble
                } else {
                    CopyFormat::TwoBpp
                },
            );
            let mut bus = Bus {
                ports: BTreeMap::from([
                    (AUTO_BG_ENABLED, c.bg_before),
                    (LOADED_ROM_BANK, c.loaded_before),
                ]),
                returns: 0,
                body: Vec::new(),
                capture: true,
                reject_next: false,
            };
            let mut waits = 0;
            while !work.finished() {
                bus.capture = work.step != Step::Delay;
                let progress = work.step(&mut bus).unwrap();
                if progress == DelayProgress::Sleeping {
                    waits += 1;
                    assert_eq!(bus.ports[&AUTO_BG_ENABLED], 0);
                    // Service-size clear and wait-flag clear alone cannot wake
                    // a foreground that has not received the owner's RETI.
                    bus.ports.insert(work.size_port(), 0);
                    bus.ports
                        .insert(super::super::display_delay_work::VBLANK_FLAG_PORT, 0);
                    assert_eq!(work.step(&mut bus).unwrap(), DelayProgress::Sleeping);
                    let (restored,): (VideoCopyCallerWork,) =
                        serde_json::from_str(&serde_json::to_string(&(&work,)).unwrap()).unwrap();
                    assert_eq!(restored, work);
                    bus.returns += 1;
                }
            }
            assert_eq!(waits, usize::from(c.tiles) / 8 + 1);
            assert_eq!(
                bus.body, c.instructions,
                "normal ROM caller mismatch: tiles={} double={}",
                c.tiles, c.double
            );
            assert_eq!(bus.ports[&AUTO_BG_ENABLED], c.bg_before);
            assert_eq!(bus.ports[&LOADED_ROM_BANK], c.loaded_before);
            assert_eq!(bus.ports[&ROM_BANK_SELECT], c.loaded_before);
        }
    }
    #[test]
    fn failed_instruction_keeps_saved_control_and_pending_bank_write() {
        let mut work = VideoCopyCallerWork::new(4, 12, 0x4567, 0x8000, CopyFormat::TwoBpp);
        let mut bus = Bus {
            ports: BTreeMap::from([(AUTO_BG_ENABLED, 0xa5), (LOADED_ROM_BANK, 7)]),
            returns: 0,
            body: Vec::new(),
            capture: true,
            reject_next: false,
        };
        while work.step != Step::SetBank {
            work.step(&mut bus).unwrap();
        }
        let before = work.clone();
        let writes = bus.body.len();
        let ports = bus.ports.clone();
        bus.reject_next = true;
        assert_eq!(work.step(&mut bus), Err(()));
        assert_eq!(work, before);
        assert_eq!(bus.body.len(), writes);
        assert_eq!(bus.ports, ports);
        assert_eq!(work.saved_bg, 0xa5);
        assert_eq!(bus.ports[&ROM_BANK_TEMP], 7);
        let mut restored: VideoCopyCallerWork =
            serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
        restored.step(&mut bus).unwrap();
        assert_eq!(bus.ports[&LOADED_ROM_BANK], 4);
    }
}
