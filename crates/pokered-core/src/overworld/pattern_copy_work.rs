//! Resumable CopyData body for native pattern transfers (home/copy.asm).
//! Reads and writes occur on their source instruction, not at stage entry
//! followed by an aggregate duration. The caller/common bus owns time and IRQs.
use super::field_restore::{PatternCopy, PatternSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternAccess {
    /// Mutable VRAM is sampled at the access cycle, subject to LCD gating.
    ReadVram { address: u16, at_tcycle: u8 },
    /// POP DE reads both stack bytes within one indivisible instruction.
    ReadPair {
        source: PatternSource,
        offset: u16,
        first_at_tcycle: u8,
        second_at_tcycle: u8,
    },
    Read {
        source: PatternSource,
        offset: u16,
        at_tcycle: u8,
    },
    Write {
        destination: u16,
        value: u8,
        at_tcycle: u8,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PatternInstruction {
    pub tcycles: u8,
    pub access: Option<PatternAccess>,
}
pub trait PatternInstructionBus {
    type Error;
    /// Error means this instruction has not executed. The common bus runs
    /// device/IRQ events at the specified access before returning its receipt.
    fn execute(&mut self, instruction: PatternInstruction) -> Result<Option<u16>, Self::Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum CopyStep {
    Read,
    Write,
    IncrementDestination,
    DecrementCount,
    LoadCountLow,
    OrCountHigh,
    Repeat,
    Return,
    Done,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatternCopyDataWork {
    source: PatternSource,
    source_offset: u16,
    destination: u16,
    remaining: u16,
    a: u8,
    step: CopyStep,
}
impl PatternCopyDataWork {
    /// CopyData uses a 16-bit byte length. Zero deliberately is not an empty
    /// copy: the source decrements it to $ffff after its first write.
    pub fn new(source: PatternSource, source_offset: u16, destination: u16, bytes: u16) -> Self {
        Self {
            source,
            source_offset,
            destination,
            remaining: bytes,
            a: 0,
            step: CopyStep::Read,
        }
    }
    pub fn from_2bpp(copy: PatternCopy) -> Option<Self> {
        (copy.format == super::field_restore::CopyFormat::TwoBpp).then(|| {
            Self::new(
                copy.source,
                copy.source_offset,
                copy.destination,
                u16::from(copy.tiles) * 16,
            )
        })
    }
    pub fn finished(&self) -> bool {
        self.step == CopyStep::Done
    }
    #[inline(always)]
    pub fn instruction(&self) -> Option<PatternInstruction> {
        use CopyStep::*;
        let (tcycles, access) = match self.step {
            Read => (
                8,
                Some(PatternAccess::Read {
                    source: self.source,
                    offset: self.source_offset,
                    at_tcycle: 4,
                }),
            ),
            Write => (
                8,
                Some(PatternAccess::Write {
                    destination: self.destination,
                    value: self.a,
                    at_tcycle: 4,
                }),
            ),
            IncrementDestination | DecrementCount => (8, None),
            LoadCountLow | OrCountHigh => (4, None),
            Repeat => (if self.a == 0 { 8 } else { 12 }, None),
            Return => (16, None),
            Done => return None,
        };
        Some(PatternInstruction { tcycles, access })
    }
    #[inline(always)]
    pub fn step<B: PatternInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<PatternInstruction>, B::Error> {
        use CopyStep::*;
        let Some(instruction) = self.instruction() else {
            return Ok(None);
        };
        let value = bus.execute(instruction)?;
        self.step = match self.step {
            Read => {
                self.a = value.expect("asset read receipt") as u8;
                self.source_offset = self.source_offset.wrapping_add(1);
                Write
            }
            Write => IncrementDestination,
            IncrementDestination => {
                self.destination = self.destination.wrapping_add(1);
                DecrementCount
            }
            DecrementCount => {
                self.remaining = self.remaining.wrapping_sub(1);
                LoadCountLow
            }
            LoadCountLow => {
                self.a = self.remaining as u8;
                OrCountHigh
            }
            OrCountHigh => {
                self.a |= (self.remaining >> 8) as u8;
                Repeat
            }
            Repeat => {
                if self.a == 0 {
                    Return
                } else {
                    Read
                }
            }
            Return | Done => Done,
        };
        Ok(Some(instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::*;
    struct Memory {
        bytes: Vec<u8>,
        writes: usize,
        cycles: u32,
        blocked: bool,
    }
    impl PatternInstructionBus for Memory {
        type Error = ();
        fn execute(&mut self, i: PatternInstruction) -> Result<Option<u16>, ()> {
            if self.blocked {
                return Err(());
            }
            self.cycles += u32::from(i.tcycles);
            Ok(match i.access {
                Some(PatternAccess::ReadVram { .. }) => unreachable!("asset-copy test bus"),
                Some(PatternAccess::Read { offset, .. }) => {
                    Some(u16::from((offset as u8).wrapping_mul(3)))
                }
                Some(PatternAccess::ReadPair { offset, .. }) => Some(
                    u16::from((offset as u8).wrapping_mul(3))
                        | (u16::from((offset.wrapping_add(1) as u8).wrapping_mul(3)) << 8),
                ),
                Some(PatternAccess::Write {
                    destination, value, ..
                }) => {
                    self.bytes[usize::from(destination - 0x8000)] = value;
                    self.writes += 1;
                    None
                }
                None => None,
            })
        }
    }
    fn memory() -> Memory {
        Memory {
            bytes: vec![0; 8192],
            writes: 0,
            cycles: 0,
            blocked: false,
        }
    }
    #[test]
    fn each_byte_is_published_only_on_its_write_instruction() {
        let mut bus = memory();
        let mut work = PatternCopyDataWork::new(PatternSource::Sprite(3), 17, 0x80c0, 192);
        work.step(&mut bus).unwrap();
        assert_eq!(bus.writes, 0);
        work.step(&mut bus).unwrap();
        assert_eq!(bus.writes, 1);
        assert_eq!(bus.bytes[0xc0], 51);
        while work.step(&mut bus).unwrap().is_some() {}
        assert_eq!(bus.writes, 192);
        assert_eq!(bus.cycles, 52 * 192 + 12);
        for n in 0..192 {
            assert_eq!(bus.bytes[0xc0 + n], ((17 + n) as u8).wrapping_mul(3));
        }
    }
    #[test]
    fn mid_byte_restore_and_block_do_not_repeat_a_committed_write() {
        let mut a = PatternCopyDataWork::new(PatternSource::Sprite(4), 0, 0x8000, 12);
        let mut bus = memory();
        for _ in 0..9 {
            a.step(&mut bus).unwrap();
        }
        let mut b: PatternCopyDataWork =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        let writes = bus.writes;
        let saved = a;
        bus.blocked = true;
        assert_eq!(a.step(&mut bus), Err(()));
        assert_eq!(a, saved);
        bus.blocked = false;
        let mut other = Memory {
            bytes: bus.bytes.clone(),
            writes: bus.writes,
            cycles: bus.cycles,
            blocked: false,
        };
        while a.step(&mut bus).unwrap().is_some() {}
        while b.step(&mut other).unwrap().is_some() {}
        assert_eq!(bus.bytes, other.bytes);
        assert_eq!(bus.writes, 12);
        assert_eq!(other.writes, 12);
        assert!(writes > 0);
        assert_eq!(a, b);
        assert_eq!(bus.cycles, other.cycles);
    }
    #[test]
    fn zero_copydata_length_really_underflows_instead_of_silently_completing() {
        let mut work = PatternCopyDataWork::new(PatternSource::Sprite(1), 0, 0x8000, 0);
        let mut bus = memory();
        for _ in 0..7 {
            work.step(&mut bus).unwrap();
        }
        assert!(!work.finished());
        assert_eq!(work.remaining, 0xffff);
        assert_eq!(bus.writes, 1);
    }
}
