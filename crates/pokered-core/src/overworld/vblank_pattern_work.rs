//! Finite VBlankCopy / VBlankCopyDouble bodies from home/vcopy.asm.
//!
//! The common owner supplies time and IRQ entry/return. This child does not
//! arm a request, accept an IRQ, or resume foreground DelayFrame by itself.
//! Each step is a source instruction, including both bus reads of POP DE.
use super::field_restore::{CopyFormat, PatternCopy};
use super::pattern_copy_work::{PatternAccess, PatternInstruction, PatternInstructionBus};

// Source instructions, not an aggregate transfer-time charge.
const HEADER: [u8; 21] = [
    12, 4, 8, 12, 4, 12, 4, 12, 12, 4, 12, 4, 8, 12, 4, 12, 4, 12, 4, 4, 12,
];
const FOOTER: [u8; 15] = [4, 12, 4, 12, 12, 4, 12, 4, 12, 12, 4, 12, 4, 8, 16];

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Header(u8),
    Pop,
    Write,
    Increment,
    Decrement,
    Repeat,
    Footer(u8),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankPatternWork {
    copy: PatternCopy,
    source: u16,
    destination: u16,
    tiles: u8,
    size_port: u8,
    pair: u8,
    write: u8,
    de: u16,
    step: Step,
}
impl VBlankPatternWork {
    pub fn new(copy: PatternCopy) -> Self {
        Self {
            source: copy.source_offset,
            destination: copy.destination,
            tiles: copy.tiles,
            size_port: copy.tiles,
            copy,
            pair: 0,
            write: 0,
            de: 0,
            step: Step::Header(0),
        }
    }
    pub fn request(&self) -> PatternCopy {
        self.copy
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    /// The ISR clears size before copying, not when its RET has completed.
    pub fn size_port(&self) -> u8 {
        self.size_port
    }
    pub fn source_offset(&self) -> u16 {
        self.source
    }
    pub fn destination(&self) -> u16 {
        self.destination
    }
    fn double(&self) -> bool {
        self.copy.format == CopyFormat::OneBppDouble
    }
    fn writes_per_pair(&self) -> u8 {
        if self.double() {
            4
        } else {
            2
        }
    }
    fn pairs_per_tile(&self) -> u8 {
        if self.double() {
            4
        } else {
            8
        }
    }
    fn last_increment(&self) -> bool {
        self.pair + 1 == self.pairs_per_tile() && self.write + 1 == self.writes_per_pair()
    }
    pub fn instruction(&self) -> Option<PatternInstruction> {
        use Step::*;
        let (tcycles, access) = match self.step {
            Header(2) if self.tiles == 0 => (20, None), // taken RET Z
            Header(n) => (HEADER[usize::from(n)], None),
            Pop => (
                12,
                Some(PatternAccess::ReadPair {
                    source: self.copy.source,
                    offset: self.source,
                    first_at_tcycle: 4,
                    second_at_tcycle: 8,
                }),
            ),
            Write => {
                let low = if self.double() {
                    self.write < 2
                } else {
                    self.write == 0
                };
                (
                    8,
                    Some(PatternAccess::Write {
                        destination: self.destination,
                        value: if low {
                            self.de as u8
                        } else {
                            (self.de >> 8) as u8
                        },
                        at_tcycle: 4,
                    }),
                )
            }
            Increment => (if self.last_increment() { 8 } else { 4 }, None),
            Decrement => (4, None),
            Repeat => (if self.tiles == 0 { 8 } else { 12 }, None),
            Footer(n) => (FOOTER[usize::from(n)], None),
            Done => return None,
        };
        Some(PatternInstruction { tcycles, access })
    }
    /// A bus refusal leaves the whole instruction unexecuted. POP DE is never
    /// partially committed across a grant, and writes cannot be replayed.
    pub fn step<B: PatternInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<PatternInstruction>, B::Error> {
        use Step::*;
        let Some(instruction) = self.instruction() else {
            return Ok(None);
        };
        let value = bus.execute(instruction)?;
        self.step = match self.step {
            Header(2) if self.tiles == 0 => Done,
            Header(20) => {
                self.size_port = 0;
                Pop
            }
            Header(n) => Header(n + 1),
            Pop => {
                self.de = value.expect("POP source receipt");
                self.source = self.source.wrapping_add(2);
                self.write = 0;
                Write
            }
            Write => Increment,
            Increment => {
                self.destination = if self.last_increment() {
                    self.destination.wrapping_add(1)
                } else {
                    (self.destination & 0xff00)
                        | u16::from((self.destination as u8).wrapping_add(1))
                };
                self.write += 1;
                if self.write < self.writes_per_pair() {
                    Write
                } else {
                    self.pair += 1;
                    if self.pair < self.pairs_per_tile() {
                        Pop
                    } else {
                        Decrement
                    }
                }
            }
            Decrement => {
                self.tiles = self.tiles.wrapping_sub(1);
                Repeat
            }
            Repeat if self.tiles == 0 => Footer(0),
            Repeat => {
                self.pair = 0;
                Pop
            }
            Footer(14) => Done,
            Footer(n) => Footer(n + 1),
            Done => Done,
        };
        Ok(Some(instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        display_work_time::{DisplayWorkError, DisplayWorkTime},
        field_restore::{PatternAssets, PatternMemory, PatternSource},
    };
    use super::*;
    struct Bytes;
    impl PatternAssets for Bytes {
        fn read_pattern_byte(&self, _: PatternSource, offset: u16) -> Option<u8> {
            Some((offset as u8).wrapping_mul(3))
        }
    }
    fn copy(format: CopyFormat, tiles: u8) -> PatternCopy {
        PatternCopy {
            source: PatternSource::WalkingPlayer,
            source_offset: 17,
            destination: 0x8800,
            tiles,
            format,
        }
    }
    #[test]
    fn actual_original_traced_leaf_costs_and_all_bytes_match() {
        // Independent normal-retail traces: audit1108, 2 identical replays.
        for (format, tiles, expected) in [
            (CopyFormat::TwoBpp, 0, 36),
            (CopyFormat::TwoBpp, 4, 1532),
            (CopyFormat::TwoBpp, 8, 2764),
            (CopyFormat::OneBppDouble, 0, 36),
            (CopyFormat::OneBppDouble, 8, 2380),
        ] {
            let mut time = DisplayWorkTime::default();
            time.device.write_port(0xff40, 0x11, 0xc3).unwrap();
            time.begin_native_update();
            let mut memory = PatternMemory::default();
            let mut work = VBlankPatternWork::new(copy(format, tiles));
            while work
                .step(&mut time.pattern_bus(&mut memory, &Bytes))
                .unwrap()
                .is_some()
            {}
            assert_eq!(time.elapsed(), expected);
            assert!(work.finished());
            assert_eq!(work.size_port(), 0);
            assert_eq!(work.destination(), 0x8800 + u16::from(tiles) * 16);
            let source_bytes = u16::from(tiles) * format.source_bytes_per_tile();
            assert_eq!(work.source_offset(), 17 + source_bytes);
            for n in 0..u16::from(tiles) * 16 {
                let source_n = if format == CopyFormat::OneBppDouble {
                    n / 2
                } else {
                    n
                };
                assert_eq!(
                    memory.read(0x8800 + n),
                    Some(((17 + source_n) as u8).wrapping_mul(3))
                );
            }
        }
    }
    #[test]
    fn size_clear_does_not_return_and_budget_snapshot_retains_pending_pop() {
        let mut work = VBlankPatternWork::new(copy(CopyFormat::TwoBpp, 8));
        let mut time = DisplayWorkTime::default();
        let mut memory = PatternMemory::default();
        let before = work;
        assert_eq!(
            work.step(&mut time.pattern_bus(&mut memory, &Bytes)),
            Err(DisplayWorkError::Budget)
        );
        assert_eq!(work, before);
        time.device.write_port(0xff40, 0x11, 0xc3).unwrap();
        time.begin_native_update();
        for _ in 0..21 {
            work.step(&mut time.pattern_bus(&mut memory, &Bytes))
                .unwrap();
        }
        assert_eq!(work.size_port(), 0);
        assert!(!work.finished());
        assert_eq!(memory.read(0x8800), Some(0));
        let saved: VBlankPatternWork =
            serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
        assert_eq!(work, saved);
        time.idle_to_target().unwrap();
        assert_eq!(
            work.step(&mut time.pattern_bus(&mut memory, &Bytes)),
            Err(DisplayWorkError::Budget)
        );
        assert_eq!(work, saved);
        time.begin_native_update();
        work.step(&mut time.pattern_bus(&mut memory, &Bytes))
            .unwrap(); // POP, no VRAM write
        assert_eq!(memory.read(0x8800), Some(0));
        work.step(&mut time.pattern_bus(&mut memory, &Bytes))
            .unwrap();
        assert_eq!(memory.read(0x8800), Some(51));
    }
    #[test]
    fn instruction_service_requires_ret_before_resume_and_terminal_zero_irq() {
        use super::super::field_restore::{RestoreError, VideoCopyWork, VideoServiceError};
        let mut work = VideoCopyWork::new(copy(CopyFormat::TwoBpp, 8));
        work.arm(10).unwrap();
        let mut time = DisplayWorkTime::default();
        time.device.write_port(0xff40, 0x11, 0xc3).unwrap();
        time.begin_native_update();
        let mut memory = PatternMemory::default();
        assert_eq!(
            work.service_instruction(10, &mut time.pattern_bus(&mut memory, &Bytes)),
            Err(VideoServiceError::Protocol(RestoreError::StaleVBlank))
        );
        assert_eq!(time.elapsed(), 0);
        for _ in 0..21 {
            assert!(!work
                .service_instruction(11, &mut time.pattern_bus(&mut memory, &Bytes))
                .unwrap());
        }
        assert_eq!(work.requested_tiles(), Some(0));
        assert_eq!(work.resume(), Err(RestoreError::NotServiced));
        let json = serde_json::to_string(&work).unwrap();
        let mut restored: VideoCopyWork = serde_json::from_str(&json).unwrap();
        let mut other_time = time.clone();
        let mut other_memory = memory.clone();
        assert_eq!(
            work.service_instruction(12, &mut time.pattern_bus(&mut memory, &Bytes)),
            Err(VideoServiceError::Protocol(RestoreError::StaleVBlank))
        );
        assert_eq!(
            work.service(11, &mut memory, &Bytes),
            Err(RestoreError::ServiceInProgress)
        );
        while !work
            .service_instruction(11, &mut time.pattern_bus(&mut memory, &Bytes))
            .unwrap()
        {}
        while !restored
            .service_instruction(11, &mut other_time.pattern_bus(&mut other_memory, &Bytes))
            .unwrap()
        {}
        assert_eq!(time.elapsed(), 2764);
        assert_eq!(memory.bytes(), other_memory.bytes());
        assert_eq!(work, restored);
        work.resume().unwrap();
        assert!(!work.finished());
        work.arm(11).unwrap();
        assert_eq!(work.requested_tiles(), Some(0));
        assert_eq!(work.resume(), Err(RestoreError::NotServiced));
        while !work
            .service_instruction(12, &mut time.pattern_bus(&mut memory, &Bytes))
            .unwrap()
        {}
        assert_eq!(time.elapsed(), 2764 + 36);
        work.resume().unwrap();
        assert!(work.finished());
    }
}
