//! Controlled owner seams, not elapsed-time/native gameplay evidence. The
//! sparse original trace is checked by copy.rs; these owner receipts are
//! deliberately synthetic so refusal/HALT/zero-size boundaries are isolated.
use super::delay::{
    DelayFrameBus, DelayFrameWork, DelayFramesWork, DelayProgress, VBLANK_FLAG_PORT,
};
use super::*;
use crate::alloc_prelude::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Refused,
    Unbound(u16),
}
pub(super) struct Bus {
    pub ports: BTreeMap<u16, u8>,
    pub returns: u64,
    pub body: Vec<DisplayInstruction>,
    pub reject_next: bool,
    pub elapsed: u64,
    source_base: u16,
    destination_base: u16,
    pub source: Vec<u8>,
    pub destination: Vec<u8>,
}
impl Bus {
    pub fn new(bg: u8, bank: u8, source: u16, destination: u16) -> Self {
        Self {
            ports: BTreeMap::from([(copy::AUTO_BG_ENABLED, bg), (LOADED_ROM_BANK, bank)]),
            returns: 0,
            body: Vec::new(),
            reject_next: false,
            elapsed: 0,
            source_base: source,
            destination_base: destination,
            source: (0..1024).map(|i| i as u8).collect(),
            destination: vec![0; 2048],
        }
    }
    pub fn pair(&self, low: u16) -> u16 {
        u16::from(self.ports[&low]) | (u16::from(self.ports[&(low + 1)]) << 8)
    }
    pub fn set_pair(&mut self, low: u16, value: u16) {
        self.ports.insert(low, value as u8);
        self.ports.insert(low + 1, (value >> 8) as u8);
    }
    pub fn wake_only(&mut self) {
        self.ports.insert(VBLANK_FLAG_PORT, 0);
        self.returns += 1;
    }
    /// Synthetic request service, explicitly not a source ISR instruction or
    /// hardware timing implementation. Never used by the actual game.
    pub fn service_copy(&mut self, format: CopyFormat) {
        let size_port = if format == CopyFormat::TwoBpp {
            copy::COPY_SIZE
        } else {
            copy::DOUBLE_SIZE
        };
        let tiles = usize::from(self.ports[&size_port]);
        assert!(tiles <= 8);
        let source = self.pair(size_port + 1);
        let dest = self.pair(size_port + 3);
        let input_bytes = tiles * if format == CopyFormat::TwoBpp { 16 } else { 8 };
        let input_start = usize::from(source.wrapping_sub(self.source_base));
        let output_start = usize::from(dest.wrapping_sub(self.destination_base));
        for i in 0..input_bytes {
            let value = self.source[input_start + i];
            if format == CopyFormat::TwoBpp {
                self.destination[output_start + i] = value;
            } else {
                self.destination[output_start + i * 2] = value;
                self.destination[output_start + i * 2 + 1] = value;
            }
        }
        self.set_pair(size_port + 1, source.wrapping_add(input_bytes as u16));
        self.set_pair(size_port + 3, dest.wrapping_add((tiles * 16) as u16));
        self.ports.insert(size_port, 0);
        self.wake_only();
    }
}
impl DisplayInstructionBus for Bus {
    type Error = Error;
    fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Error> {
        if self.reject_next {
            self.reject_next = false;
            return Err(Error::Refused);
        }
        let result = match instruction.access {
            Some(DisplayAccess::Read { port, .. }) => {
                Some(*self.ports.get(&port).ok_or(Error::Unbound(port))?)
            }
            Some(DisplayAccess::Write { port, value, .. }) => {
                self.ports.insert(port, value);
                None
            }
            None => None,
        };
        self.elapsed += u64::from(instruction.tcycles);
        self.body.push(instruction);
        Ok(result)
    }
}
impl DelayFrameBus for Bus {
    fn interrupt_returns(&self) -> u64 {
        self.returns
    }
}

#[test]
fn flag_clear_and_old_interrupt_receipts_do_not_wake_a_later_halt() {
    let mut bus = Bus::new(1, 1, 0xa188, 0x9000);
    let mut work = DelayFrameWork::default();
    work.step(&mut bus).unwrap();
    work.step(&mut bus).unwrap();
    bus.wake_only(); // completed before HALT; cannot wake a later HALT
    work.step(&mut bus).unwrap();
    assert!(work.sleeping());
    assert_eq!(work.step(&mut bus).unwrap(), DelayProgress::Sleeping);
    let mut restored: DelayFrameWork =
        serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
    assert_eq!(restored, work);
    bus.wake_only();
    while !work.finished() {
        work.step(&mut bus).unwrap();
    }
    while !restored.finished() {
        restored.step(&mut bus).unwrap();
    }
    assert_eq!(work, restored);
}
#[test]
fn delay_frames_uses_each_return_and_preserves_zero_underflow() {
    for (count, expected) in [(0, 256), (1, 1), (3, 3)] {
        let mut bus = Bus::new(1, 1, 0xa188, 0x9000);
        let mut work = DelayFramesWork::new(count);
        let mut wakes = 0;
        for _ in 0..10000 {
            if work.finished() {
                break;
            }
            if work.step(&mut bus).unwrap() == DelayProgress::Sleeping {
                bus.ports.insert(VBLANK_FLAG_PORT, 0);
                assert_eq!(work.step(&mut bus).unwrap(), DelayProgress::Sleeping);
                bus.returns += 1;
                wakes += 1;
            }
        }
        assert!(work.finished());
        assert_eq!(wakes, expected);
        // Foreground only. Synthetic service above adds no claimed ISR time.
        assert_eq!(bus.elapsed, expected * 104 + 12);
    }
}
