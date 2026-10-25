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
    copy: Option<PatternCopy>,
    source: Option<u16>,
    destination: Option<u16>,
    #[serde(default = "two_bpp")]
    idle_format: CopyFormat,
    #[serde(default)]
    live_idle: bool,
    tiles: u8,
    size_port: u8,
    pair: u8,
    write: u8,
    de: u16,
    step: Step,
    #[serde(default)]
    request_base: Option<u16>,
    #[serde(default)]
    request_low: u8,
}
fn two_bpp() -> CopyFormat {
    CopyFormat::TwoBpp
}
impl VBlankPatternWork {
    pub fn new(copy: PatternCopy) -> Self {
        Self {
            source: Some(copy.source_offset),
            destination: Some(copy.destination),
            tiles: copy.tiles,
            size_port: copy.tiles,
            copy: Some(copy),
            idle_format: copy.format,
            live_idle: false,
            pair: 0,
            write: 0,
            de: 0,
            step: Step::Header(0),
            request_base: None,
            request_low: 0,
        }
    }
    /// Bind actual caller-written ports. Legacy/test constructors keep their
    /// explicit immutable inputs; this path reads and updates the shared bus.
    pub fn from_live_request(
        time: &super::display_work_time::DisplayWorkTime,
        binding: super::video_request_ports::PatternRomBinding,
    ) -> Result<Self, super::video_request_ports::VideoBindingError> {
        let copy = binding.request(time.video_requests(), time.source_loaded_rom_bank())?;
        let mut work = Self::new(copy);
        work.request_base = Some(binding.address);
        Ok(work)
    }
    /// A known idle size needs no resource, bank or address. Foreground must
    /// remain suspended after construction, as for every VBlank input.
    pub fn idle_from_live(
        time: &super::display_work_time::DisplayWorkTime,
        format: CopyFormat,
    ) -> Result<Self, super::video_request_ports::VideoBindingError> {
        use super::video_request_ports::VideoBindingError;
        match time.video_requests().snapshot(format).size {
            None => return Err(VideoBindingError::UnknownPorts),
            Some(0) => (),
            Some(_) => return Err(VideoBindingError::ActiveRequest),
        }
        Ok(Self {
            copy: None,
            source: None,
            destination: None,
            idle_format: format,
            live_idle: true,
            tiles: 0,
            size_port: 0,
            pair: 0,
            write: 0,
            de: 0,
            step: Step::Header(0),
            request_base: None,
            request_low: 0,
        })
    }
    pub fn format(&self) -> CopyFormat {
        self.copy.map_or(self.idle_format, |copy| copy.format)
    }
    fn request_size_port(&self) -> u16 {
        if self.double() {
            super::video_copy_caller_work::DOUBLE_SIZE
        } else {
            super::video_copy_caller_work::COPY_SIZE
        }
    }
    pub fn unstarted(&self) -> bool {
        self.step == Step::Header(0)
    }
    pub fn uses_live_ports(&self) -> bool {
        self.live_idle || self.request_base.is_some()
    }
    pub fn request(&self) -> Option<PatternCopy> {
        self.copy
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    /// The ISR clears size before copying, not when its RET has completed.
    pub fn size_port(&self) -> u8 {
        self.size_port
    }
    pub fn source_offset(&self) -> Option<u16> {
        self.source
    }
    pub fn destination(&self) -> Option<u16> {
        self.destination
    }
    fn double(&self) -> bool {
        self.format() == CopyFormat::OneBppDouble
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
                    source: self.copy.expect("active pattern identity").source,
                    offset: self.source.expect("active source offset"),
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
                        destination: self.destination.expect("active destination"),
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
        let access = if let Some(base) = self.request_base {
            use super::display_lcd_work::DisplayAccess;
            let port = self.request_size_port();
            let read = |port| {
                Some(PatternAccess::Display(DisplayAccess::Read {
                    port,
                    at_tcycle: 8,
                }))
            };
            let write = |port, value| {
                Some(PatternAccess::Display(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle: 8,
                }))
            };
            match self.step {
                Header(0) | Header(17) => read(port),
                Header(8) => read(port + 1),
                Header(10) => read(port + 2),
                Header(13) => read(port + 3),
                Header(15) => read(port + 4),
                Header(20) => write(port, 0),
                Footer(1) => write(
                    port + 3,
                    self.destination.expect("active destination") as u8,
                ),
                Footer(3) => write(
                    port + 4,
                    (self.destination.expect("active destination") >> 8) as u8,
                ),
                Footer(6) => write(
                    port + 1,
                    base.wrapping_add(self.source.expect("active source offset")) as u8,
                ),
                Footer(8) => write(
                    port + 2,
                    (base.wrapping_add(self.source.expect("active source offset")) >> 8) as u8,
                ),
                _ => access,
            }
        } else if self.live_idle && self.step == Step::Header(0) {
            Some(PatternAccess::Display(
                super::display_lcd_work::DisplayAccess::Read {
                    port: self.request_size_port(),
                    at_tcycle: 8,
                },
            ))
        } else {
            access
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
        if self.live_idle && self.step == Header(0) {
            assert_eq!(
                value,
                Some(0),
                "idle request changed while foreground must be suspended"
            );
        }
        if let Some(base) = self.request_base {
            match self.step {
                Header(0) | Header(17) => {
                    let size = value.expect("live size port") as u8;
                    self.tiles = size;
                    self.size_port = size;
                }
                Header(8) | Header(13) => self.request_low = value.expect("live pointer low") as u8,
                Header(10) => {
                    self.source = Some(
                        u16::from_le_bytes([
                            self.request_low,
                            value.expect("live source high") as u8,
                        ])
                        .wrapping_sub(base),
                    )
                }
                Header(15) => {
                    self.destination = Some(u16::from_le_bytes([
                        self.request_low,
                        value.expect("live destination high") as u8,
                    ]))
                }
                _ => (),
            }
        }
        self.step = match self.step {
            Header(2) if self.tiles == 0 => Done,
            Header(20) => {
                self.size_port = 0;
                Pop
            }
            Header(n) => Header(n + 1),
            Pop => {
                self.de = value.expect("POP source receipt");
                self.source = Some(self.source.expect("active source offset").wrapping_add(2));
                self.write = 0;
                Write
            }
            Write => Increment,
            Increment => {
                let destination = self.destination.expect("active destination");
                self.destination = Some(if self.last_increment() {
                    destination.wrapping_add(1)
                } else {
                    (destination & 0xff00) | u16::from((destination as u8).wrapping_add(1))
                });
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
            time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
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
            assert_eq!(work.destination(), Some(0x8800 + u16::from(tiles) * 16));
            let source_bytes = u16::from(tiles) * format.source_bytes_per_tile();
            assert_eq!(work.source_offset(), Some(17 + source_bytes));
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
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
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
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
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
    #[test]
    fn old_double_copy_snapshot_keeps_format_and_continuation_without_new_mode_fields() {
        let new = VBlankPatternWork::new(copy(CopyFormat::OneBppDouble, 8));
        let mut old = serde_json::to_value(new).unwrap();
        for field in ["idle_format", "live_idle", "request_base", "request_low"] {
            old.as_object_mut().unwrap().remove(field);
        }
        let mut restored: VBlankPatternWork = serde_json::from_value(old).unwrap();
        assert_eq!(restored.format(), CopyFormat::OneBppDouble);
        assert_eq!(restored.request(), new.request());
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        let mut memory = PatternMemory::default();
        while restored
            .step(&mut time.pattern_bus(&mut memory, &Bytes))
            .unwrap()
            .is_some()
        {}
        assert_eq!(time.elapsed(), 2380);
        assert_eq!(restored.source_offset(), Some(17 + 64));
        for n in 0..128u16 {
            assert_eq!(
                memory.read(0x8800 + n),
                Some(((17 + n / 2) as u8).wrapping_mul(3))
            );
        }
    }
}
