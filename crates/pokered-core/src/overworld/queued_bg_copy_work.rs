//! VBlankCopyBgMap foreground request consumer, sharing TransferBgRows with
//! AutoBgMapTransfer. Clearing source-low disables a request, not the IRQ.
use super::auto_bg_transfer_work::AutoBgTransferWork;
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use super::pattern_copy_work::PatternInstructionBus;
use crate::alloc_prelude::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BgCopyRequest {
    pub source: u16,
    pub destination: u16,
    pub rows: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Source,
    Test,
    ReturnZero,
    SaveSp,
    High,
    SaveHigh,
    Low,
    SaveLow,
    SourceLow,
    SetL,
    SourceHigh,
    SetH,
    SourceSp,
    DestLow,
    DestL,
    DestHigh,
    DestH,
    Count,
    SetB,
    Zero,
    Clear,
    JumpRows,
    Rows,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueuedBgCopyWork {
    step: Step,
    request: BgCopyRequest,
    sp: u16,
    saved_sp: u16,
    a: u8,
    h: u8,
    l: u8,
    b: u8,
    tiles: Vec<u8>,
    child: Option<AutoBgTransferWork>,
}
impl QueuedBgCopyWork {
    pub fn new(request: BgCopyRequest, sp: u16, tiles: Vec<u8>) -> Option<Self> {
        let count = if request.rows == 0 {
            256
        } else {
            usize::from(request.rows)
        };
        if request.source as u8 != 0 && tiles.len() != count * 20 {
            return None;
        }
        Some(Self {
            step: Step::Source,
            request,
            sp,
            saved_sp: 0,
            a: 0,
            h: 0,
            l: 0,
            b: 0,
            tiles,
            child: None,
        })
    }
    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    fn set_hl(&mut self, n: u16) {
        [self.h, self.l] = n.to_be_bytes();
    }
    pub fn request(&self) -> BgCopyRequest {
        self.request
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    pub fn step<B>(&mut self, bus: &mut B) -> Result<bool, <B as DisplayInstructionBus>::Error>
    where
        B: DisplayInstructionBus
            + PatternInstructionBus<Error = <B as DisplayInstructionBus>::Error>,
    {
        use Step::*;
        if self.step == Rows {
            if self.child.as_mut().expect("row child").step(bus)? {
                return Ok(true);
            }
            self.step = Done;
            return Ok(false);
        }
        let t = match self.step {
            Done => return Ok(false),
            Source | SaveSp | SaveHigh | SaveLow | SourceLow | SourceHigh | DestLow | DestHigh
            | Count | Clear | JumpRows => 12,
            ReturnZero => {
                if self.a == 0 {
                    20
                } else {
                    8
                }
            }
            SourceSp => 8,
            _ => 4,
        };
        DisplayInstructionBus::execute(
            bus,
            DisplayInstruction {
                tcycles: t,
                access: None,
            },
        )?;
        self.step = match self.step {
            Source => {
                self.a = self.request.source as u8;
                Test
            }
            Test => ReturnZero,
            ReturnZero => {
                if self.a == 0 {
                    Done
                } else {
                    SaveSp
                }
            }
            SaveSp => {
                self.set_hl(self.sp);
                High
            }
            High => {
                self.a = self.h;
                SaveHigh
            }
            SaveHigh => {
                self.saved_sp = u16::from(self.a) << 8;
                Low
            }
            Low => {
                self.a = self.l;
                SaveLow
            }
            SaveLow => {
                self.saved_sp |= u16::from(self.a);
                SourceLow
            }
            SourceLow => {
                self.a = self.request.source as u8;
                SetL
            }
            SetL => {
                self.l = self.a;
                SourceHigh
            }
            SourceHigh => {
                self.a = (self.request.source >> 8) as u8;
                SetH
            }
            SetH => {
                self.h = self.a;
                SourceSp
            }
            SourceSp => {
                self.sp = self.hl();
                DestLow
            }
            DestLow => {
                self.a = self.request.destination as u8;
                DestL
            }
            DestL => {
                self.l = self.a;
                DestHigh
            }
            DestHigh => {
                self.a = (self.request.destination >> 8) as u8;
                DestH
            }
            DestH => {
                self.h = self.a;
                Count
            }
            Count => {
                self.a = self.request.rows;
                SetB
            }
            SetB => {
                self.b = self.a;
                Zero
            }
            Zero => {
                self.a = 0;
                Clear
            }
            Clear => {
                self.request.source &= 0xff00;
                JumpRows
            }
            JumpRows => {
                self.child = Some(
                    AutoBgTransferWork::queued_rows(
                        self.sp,
                        self.hl(),
                        self.b,
                        self.saved_sp,
                        core::mem::take(&mut self.tiles),
                    )
                    .expect("validated row input"),
                );
                Rows
            }
            Rows | Done => unreachable!(),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::bg_redraw_work::BgRedrawWork;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use crate::overworld::field_restore::{PatternAssets, PatternMemory, PatternSource};
    struct NoAssets;
    impl PatternAssets for NoAssets {
        fn read_pattern_byte(&self, _: PatternSource, _: u16) -> Option<u8> {
            None
        }
    }
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    enum Work {
        Queued(QueuedBgCopyWork),
        Redraw(BgRedrawWork),
    }
    impl Work {
        fn step(
            &mut self,
            t: &mut DisplayWorkTime,
            m: &mut PatternMemory,
        ) -> Result<bool, DisplayWorkError> {
            match self {
                Self::Queued(w) => w.step(&mut t.pattern_bus(m, &NoAssets)),
                Self::Redraw(w) => w.step(&mut t.pattern_bus(m, &NoAssets)),
            }
        }
    }
    #[derive(serde::Deserialize)]
    struct Case {
        kind: String,
        source: u16,
        destination: u16,
        rows: u8,
        mode: u8,
        tiles: Vec<u8>,
        #[serde(rename = "T")]
        ticks: u64,
        source_after: u16,
        mode_after: u8,
        writes: Vec<(u16, u8)>,
    }
    fn drive(w: &mut Work, t: &mut DisplayWorkTime, m: &mut PatternMemory) {
        loop {
            match w.step(t, m) {
                Ok(true) => (),
                Ok(false) => break,
                Err(DisplayWorkError::Budget) => t.begin_native_update(),
                Err(e) => panic!("unexpected{e:?}"),
            }
        }
    }
    #[test]
    fn independent_rom_cases_cover_request_flags_zero_rows_and_wrap_quirks() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("bg_request_cases_1137.json")).unwrap();
        for c in cases {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
            let bytes: Vec<u8> = (0x8000..0xa000).map(|a| (a * 3 + 19) as u8).collect();
            let mut expected = bytes.clone();
            for (address, value) in c.writes {
                expected[usize::from(address - 0x8000)] = value;
            }
            let mut m = PatternMemory::from_bytes(bytes).unwrap();
            let mut w = if c.kind == "queued" {
                Work::Queued(
                    QueuedBgCopyWork::new(
                        BgCopyRequest {
                            source: c.source,
                            destination: c.destination,
                            rows: c.rows,
                        },
                        0xdfed,
                        c.tiles,
                    )
                    .unwrap(),
                )
            } else {
                Work::Redraw(BgRedrawWork::new(c.mode, c.destination, c.tiles).unwrap())
            };
            drive(&mut w, &mut t, &mut m);
            assert_eq!(t.elapsed(), c.ticks);
            assert_eq!(m.bytes(), expected);
            match w {
                Work::Queued(w) => {
                    assert_eq!(w.request().source, c.source_after);
                    assert!(w.finished());
                }
                Work::Redraw(w) => {
                    assert_eq!(w.mode(), c.mode_after);
                    assert!(w.finished());
                }
            }
        }
    }
    #[test]
    fn request_clear_precedes_first_write_and_does_not_finish_the_child() {
        for mut w in [
            Work::Queued(
                QueuedBgCopyWork::new(
                    BgCopyRequest {
                        source: 0xc3a0,
                        destination: 0x9800,
                        rows: 1,
                    },
                    0xdfed,
                    vec![27; 20],
                )
                .unwrap(),
            ),
            Work::Redraw(BgRedrawWork::new(1, 0x9800, vec![27; 36]).unwrap()),
        ] {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
            let mut m = PatternMemory::default();
            let before = m.clone();
            loop {
                assert!(w.step(&mut t, &mut m).unwrap());
                let cleared = match &w {
                    Work::Queued(w) => w.request().source & 255 == 0,
                    Work::Redraw(w) => w.mode() == 0,
                };
                if cleared {
                    break;
                }
            }
            assert_eq!(m, before);
            assert!(!match &w {
                Work::Queued(w) => w.finished(),
                Work::Redraw(w) => w.finished(),
            });
            drive(&mut w, &mut t, &mut m);
            assert_ne!(m, before);
        }
    }
    #[test]
    fn upper_half_ret_is_not_the_row_redraws_final_return() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        let mut m = PatternMemory::default();
        let mut w = BgRedrawWork::new(2, 0x9800, (0..40).collect()).unwrap();
        // Independent original-ROM first .DrawHalf RET at1124T.
        while t.elapsed() < 1124 {
            assert!(w.step(&mut t.pattern_bus(&mut m, &NoAssets)).unwrap());
        }
        assert_eq!(t.elapsed(), 1124);
        assert!(!w.finished());
        assert_eq!(m.read(0x9820), Some(0));
        while w.step(&mut t.pattern_bus(&mut m, &NoAssets)).unwrap() {}
        assert_eq!(t.elapsed(), 2132);
        assert_eq!(m.read(0x9820), Some(20));
    }
    #[test]
    fn snapshots_preserve_partial_writes_and_pending_instruction_after_budget() {
        for mut w in [
            Work::Queued(
                QueuedBgCopyWork::new(
                    BgCopyRequest {
                        source: 0xc3a0,
                        destination: 0x9c00,
                        rows: 6,
                    },
                    0xdfed,
                    (0..120).collect(),
                )
                .unwrap(),
            ),
            Work::Redraw(BgRedrawWork::new(2, 0x9800, (0..40).collect()).unwrap()),
        ] {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
            while t.remaining() > 600 {
                t.execute(DisplayInstruction {
                    tcycles: 4,
                    access: None,
                })
                .unwrap();
            }
            let mut m = PatternMemory::default();
            loop {
                let old = w.clone();
                match w.step(&mut t, &mut m) {
                    Ok(true) => (),
                    Err(DisplayWorkError::Budget) => {
                        assert_eq!(w, old);
                        break;
                    }
                    other => panic!("unexpected{other:?}"),
                }
            }
            let (mut clone, mut owner, mut memory): (Work, DisplayWorkTime, PatternMemory) =
                serde_json::from_str(&serde_json::to_string(&(&w, &t, &m)).unwrap()).unwrap();
            t.begin_native_update();
            owner.begin_native_update();
            drive(&mut w, &mut t, &mut m);
            drive(&mut clone, &mut owner, &mut memory);
            assert_eq!(w, clone);
            assert_eq!(t.elapsed(), owner.elapsed());
            assert_eq!(m, memory);
        }
    }
}
