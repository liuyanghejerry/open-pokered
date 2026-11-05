//! AutoBgMapTransfer and the unrolled six-row TransferBgRows child.
//! Caller supplies the real logical tile buffer frozen while foreground is
//! suspended. Writes use the shared pattern bus and its actual VRAM gating.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use super::pattern_copy_work::{PatternAccess, PatternInstruction, PatternInstructionBus};
use crate::alloc_prelude::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Enabled,
    Test,
    ReturnZero,
    SaveSp,
    SpHigh,
    SaveHigh,
    SpLow,
    SaveLow,
    Portion,
    TestPortion,
    TopBranch,
    DecrementPortion,
    MiddleBranch,
    TileSource,
    TileStack,
    DestHigh,
    SetHigh,
    DestLow,
    SetLow,
    StrideDe,
    AddStride,
    NextPortion,
    JumpTransfer,
    StorePortion,
    Rows,
    Pop,
    WriteLow,
    IncLow1,
    WriteHigh,
    IncLow2,
    RowStride,
    AddLow,
    StoreLow,
    RowCarry,
    IncHigh,
    DecRows,
    Repeat,
    RestoreHigh,
    RestoreH,
    RestoreLow,
    RestoreL,
    RestoreSp,
    Return,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutoBgTransferWork {
    step: Step,
    enabled: u8,
    portion: u8,
    destination: u16,
    tiles: Vec<u8>,
    #[serde(default = "tile_map_base")]
    source_base: u16,
    a: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    b: u8,
    carry: bool,
    sp: u16,
    saved_sp: u16,
    pair: u8,
    section: u8,
}
fn tile_map_base() -> u16 {
    0xc3a0
}
impl AutoBgTransferWork {
    pub fn new(
        enabled: u8,
        portion: u8,
        destination: u16,
        sp: u16,
        tiles: Vec<u8>,
    ) -> Option<Self> {
        if tiles.len() != 360 {
            return None;
        }
        Some(Self {
            step: Step::Enabled,
            enabled,
            portion,
            destination,
            tiles,
            source_base: tile_map_base(),
            a: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            b: 0,
            carry: false,
            sp,
            saved_sp: 0,
            pair: 0,
            section: 0,
        })
    }
    /// The same source TransferBgRows tail is also called by VBlankCopyBgMap.
    /// Zero rows deliberately wraps through256 rows. The native caller owns
    /// the physical return continuation and supplies immutable logical bytes.
    pub(crate) fn queued_rows(
        source: u16,
        destination: u16,
        rows: u8,
        saved_sp: u16,
        tiles: Vec<u8>,
    ) -> Option<Self> {
        let count = if rows == 0 { 256 } else { usize::from(rows) };
        if tiles.len() != count * 20 {
            return None;
        }
        let [h, l] = destination.to_be_bytes();
        Some(Self {
            step: Step::Pop,
            enabled: 1,
            portion: 0,
            destination,
            tiles,
            source_base: source,
            a: 0,
            d: 0,
            e: 0,
            h,
            l,
            b: rows,
            carry: false,
            sp: source,
            saved_sp,
            pair: 0,
            section: 0,
        })
    }
    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    fn set_hl(&mut self, v: u16) {
        [self.h, self.l] = v.to_be_bytes();
    }
    pub fn next_portion(&self) -> u8 {
        self.portion
    }
    pub fn stack_pointer(&self) -> u16 {
        // The restored data-stack value before logical RET; the native caller
        // owns its return continuation rather than an emulated CPU stack word.
        self.sp
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
        let t = match self.step {
            Done => return Ok(false),
            Enabled | SaveSp | SaveHigh | SaveLow | Portion | TileSource | DestHigh | DestLow
            | StrideDe | StorePortion | Pop | RestoreHigh | RestoreLow => 12,
            ReturnZero => {
                if self.a == 0 {
                    20
                } else {
                    8
                }
            }
            TopBranch | MiddleBranch => {
                if self.a == 0 {
                    12
                } else {
                    8
                }
            }
            TileStack | AddStride | Rows | WriteLow | WriteHigh | RowStride | RestoreSp => 8,
            NextPortion => {
                if self.section == 2 {
                    4
                } else {
                    8
                }
            }
            JumpTransfer => 12,
            RowCarry => {
                if !self.carry {
                    12
                } else {
                    8
                }
            }
            Repeat => {
                if self.b != 0 {
                    12
                } else {
                    8
                }
            }
            Return => 16,
            _ => 4,
        };
        match self.step {
            WriteLow | WriteHigh => {
                PatternInstructionBus::execute(
                    bus,
                    PatternInstruction {
                        tcycles: t,
                        access: Some(PatternAccess::Write {
                            destination: self.hl(),
                            value: if self.step == WriteLow {
                                self.e
                            } else {
                                self.d
                            },
                            at_tcycle: 4,
                        }),
                    },
                )?;
            }
            _ => {
                DisplayInstructionBus::execute(
                    bus,
                    DisplayInstruction {
                        tcycles: t,
                        access: None,
                    },
                )?;
            }
        }
        self.step = match self.step {
            Enabled => {
                self.a = self.enabled;
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
                SpHigh
            }
            SpHigh => {
                self.a = self.h;
                SaveHigh
            }
            SaveHigh => {
                self.saved_sp = u16::from(self.a) << 8;
                SpLow
            }
            SpLow => {
                self.a = self.l;
                SaveLow
            }
            SaveLow => {
                self.saved_sp |= u16::from(self.a);
                Portion
            }
            Portion => {
                self.a = self.portion;
                TestPortion
            }
            TestPortion => TopBranch,
            TopBranch => {
                if self.a == 0 {
                    self.section = 0;
                    TileSource
                } else {
                    DecrementPortion
                }
            }
            DecrementPortion => {
                self.a = self.a.wrapping_sub(1);
                MiddleBranch
            }
            MiddleBranch => {
                self.section = if self.a == 0 { 1 } else { 2 };
                TileSource
            }
            TileSource => {
                self.set_hl(0xc3a0 + u16::from(self.section) * 120);
                TileStack
            }
            TileStack => {
                self.sp = self.hl();
                DestHigh
            }
            DestHigh => {
                self.a = (self.destination >> 8) as u8;
                SetHigh
            }
            SetHigh => {
                self.h = self.a;
                DestLow
            }
            DestLow => {
                self.a = self.destination as u8;
                SetLow
            }
            SetLow => {
                self.l = self.a;
                if self.section == 0 {
                    NextPortion
                } else {
                    StrideDe
                }
            }
            StrideDe => {
                [self.d, self.e] = (u16::from(self.section) * 192).to_be_bytes();
                AddStride
            }
            AddStride => {
                self.set_hl(self.hl().wrapping_add(u16::from_be_bytes([self.d, self.e])));
                NextPortion
            }
            NextPortion => {
                self.a = if self.section == 2 {
                    0
                } else {
                    self.section + 1
                };
                if self.section == 1 {
                    StorePortion
                } else {
                    JumpTransfer
                }
            }
            JumpTransfer => StorePortion,
            StorePortion => {
                self.portion = self.a;
                Rows
            }
            Rows => {
                self.b = 6;
                self.pair = 0;
                Pop
            }
            Pop => {
                let at = usize::from(self.sp.wrapping_sub(self.source_base));
                self.e = self.tiles[at];
                self.d = self.tiles[at + 1];
                self.sp = self.sp.wrapping_add(2);
                WriteLow
            }
            WriteLow => IncLow1,
            IncLow1 => {
                self.l = self.l.wrapping_add(1);
                WriteHigh
            }
            WriteHigh => {
                if self.pair == 9 {
                    RowStride
                } else {
                    IncLow2
                }
            }
            IncLow2 => {
                self.l = self.l.wrapping_add(1);
                self.pair += 1;
                Pop
            }
            RowStride => {
                self.a = 13;
                AddLow
            }
            AddLow => {
                let sum = u16::from(self.a) + u16::from(self.l);
                self.a = sum as u8;
                self.carry = sum > 255;
                StoreLow
            }
            StoreLow => {
                self.l = self.a;
                RowCarry
            }
            RowCarry => {
                if self.carry {
                    IncHigh
                } else {
                    DecRows
                }
            }
            IncHigh => {
                self.h = self.h.wrapping_add(1);
                DecRows
            }
            DecRows => {
                self.b = self.b.wrapping_sub(1);
                Repeat
            }
            Repeat => {
                if self.b != 0 {
                    self.pair = 0;
                    Pop
                } else {
                    RestoreHigh
                }
            }
            RestoreHigh => {
                self.a = (self.saved_sp >> 8) as u8;
                RestoreH
            }
            RestoreH => {
                self.h = self.a;
                RestoreLow
            }
            RestoreLow => {
                self.a = self.saved_sp as u8;
                RestoreL
            }
            RestoreL => {
                self.l = self.a;
                RestoreSp
            }
            RestoreSp => {
                self.sp = self.hl();
                Return
            }
            Return => Done,
            Done => unreachable!(),
        };
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_work_time::DisplayWorkError;
    use crate::overworld::display_work_time::DisplayWorkTime;
    use crate::overworld::field_restore::{PatternAssets, PatternMemory, PatternSource};
    struct NoAssets;
    impl PatternAssets for NoAssets {
        fn read_pattern_byte(&self, _: PatternSource, _: u16) -> Option<u8> {
            None
        }
    }
    #[derive(serde::Deserialize)]
    struct RomCase {
        enabled: u8,
        portion: u8,
        destination: u16,
        #[serde(rename = "T")]
        ticks: u64,
        next_portion: u8,
        writes: Vec<(u16, u8)>,
    }
    #[test]
    fn original_rom_controls_cover_page_crossing_and_all_portions() {
        let cases: Vec<RomCase> =
            serde_json::from_str(include_str!("auto_bg_cases_1136.json")).unwrap();
        for c in cases {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
            let bytes: Vec<u8> = (0x8000..0xa000).map(|a| (a * 5 + 9) as u8).collect();
            let mut expected = bytes.clone();
            for (address, value) in c.writes {
                expected[usize::from(address - 0x8000)] = value;
            }
            let mut mem = PatternMemory::from_bytes(bytes).unwrap();
            let tiles = (0..360).map(|x| (x * 7 + 31) as u8).collect();
            let mut w = AutoBgTransferWork::new(c.enabled, c.portion, c.destination, 0xdfed, tiles)
                .unwrap();
            while w.step(&mut t.pattern_bus(&mut mem, &NoAssets)).unwrap() {}
            assert_eq!(mem.bytes(), expected);
            assert_eq!(t.elapsed(), c.ticks);
            assert_eq!(w.next_portion(), c.next_portion);
        }
    }
    #[test]
    fn actual_lcd_off_irq_input_matches_observed_background_output() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("auto_bg_real_case_1136.json")).unwrap();
        let get = |k: &str| v["state"][k].as_u64().unwrap();
        let tiles: Vec<u8> = serde_json::from_value(v["state"]["tiles"].clone()).unwrap();
        let before: Vec<u8> = serde_json::from_value(v["bg_maps_before"].clone()).unwrap();
        let expected: Vec<u8> = serde_json::from_value(v["bg_maps_after"].clone()).unwrap();
        let mut bytes = vec![0; 0x2000];
        bytes[0x1800..].copy_from_slice(&before);
        let mut mem = PatternMemory::from_bytes(bytes).unwrap();
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        let mut w = AutoBgTransferWork::new(
            get("enabled") as u8,
            get("portion") as u8,
            get("destination") as u16,
            get("sp") as u16,
            tiles,
        )
        .unwrap();
        while w.step(&mut t.pattern_bus(&mut mem, &NoAssets)).unwrap() {}
        assert_eq!(&mem.bytes()[0x1800..], expected);
        assert_eq!(t.elapsed(), v["body_T"].as_u64().unwrap());
        assert_eq!(
            u64::from(w.next_portion()),
            v["next_portion"].as_u64().unwrap()
        );
        assert_eq!(
            u64::from(w.stack_pointer()),
            v["restored_sp_before_logical_RET"].as_u64().unwrap()
        );
    }
    #[test]
    fn pending_entry_prologue_and_bg_child_match_real_irq_prefix_without_returning_foreground() {
        use crate::overworld::display_delay_work::DelayFrameBus;
        use crate::overworld::display_interrupt_entry_work::VBlankEntryWork;
        use crate::overworld::display_lcd_work::DisplayAccess;
        use crate::overworld::vblank_prologue_work::{VBlankPrologueWork, VBlankVideoShadows};
        let v: serde_json::Value =
            serde_json::from_str(include_str!("auto_bg_real_case_1136.json")).unwrap();
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        for (port, value) in [
            (0xffff, 0),
            (0xffb8, v["prologue"]["loaded_bank"].as_u64().unwrap() as u8),
        ] {
            t.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
        }
        t.idle_to_target().unwrap();
        t.begin_native_update();
        t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        t.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write {
                port: 0xffff,
                value: 1,
                at_tcycle: 8,
            }),
        })
        .unwrap();
        let start = t.elapsed();
        let mut entry = VBlankEntryWork::select(true, t.device().pending_interrupts()).unwrap();
        while entry.step(&mut t).unwrap() {}
        let shadows: VBlankVideoShadows = serde_json::from_value(v["prologue"].clone()).unwrap();
        let mut prologue = VBlankPrologueWork::new(shadows);
        while prologue.step(&mut t).unwrap() {}
        assert_eq!(
            prologue.saved_bank(),
            Some(v["prologue"]["saved_bank"].as_u64().unwrap() as u8)
        );
        let get = |k: &str| v["state"][k].as_u64().unwrap();
        let tiles: Vec<u8> = serde_json::from_value(v["state"]["tiles"].clone()).unwrap();
        let before: Vec<u8> = serde_json::from_value(v["bg_maps_before"].clone()).unwrap();
        let mut bytes = vec![0; 0x2000];
        bytes[0x1800..].copy_from_slice(&before);
        let mut mem = PatternMemory::from_bytes(bytes).unwrap();
        let mut bg = AutoBgTransferWork::new(
            get("enabled") as u8,
            get("portion") as u8,
            get("destination") as u16,
            get("sp") as u16,
            tiles,
        )
        .unwrap();
        while bg.step(&mut t.pattern_bus(&mut mem, &NoAssets)).unwrap() {}
        assert_eq!(
            t.elapsed() - start,
            v["irq_accept_to_bg_return_T"].as_u64().unwrap()
        );
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        use crate::overworld::bg_redraw_work::BgRedrawWork;
        use crate::overworld::queued_bg_copy_work::{BgCopyRequest, QueuedBgCopyWork};
        let request: BgCopyRequest = serde_json::from_value(v["next_bg_request"].clone()).unwrap();
        t.execute(DisplayInstruction {
            tcycles: 24,
            access: None,
        })
        .unwrap();
        let mut queued = QueuedBgCopyWork::new(request, 0xdfed, Vec::new()).unwrap();
        while queued
            .step(&mut t.pattern_bus(&mut mem, &NoAssets))
            .unwrap()
        {}
        t.execute(DisplayInstruction {
            tcycles: 24,
            access: None,
        })
        .unwrap();
        let mut redraw = BgRedrawWork::new(
            v["next_redraw"]["mode"].as_u64().unwrap() as u8,
            v["next_redraw"]["destination"].as_u64().unwrap() as u16,
            Vec::new(),
        )
        .unwrap();
        while redraw
            .step(&mut t.pattern_bus(&mut mem, &NoAssets))
            .unwrap()
        {}
        assert_eq!(
            t.elapsed() - start,
            v["irq_accept_to_redraw_return_T"].as_u64().unwrap()
        );
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
    }
    #[test]
    fn suspended_pair_write_and_snapshot_preserve_completed_bytes() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        while t.remaining() > 600 {
            DisplayInstructionBus::execute(
                &mut t,
                DisplayInstruction {
                    tcycles: 4,
                    access: None,
                },
            )
            .unwrap();
        }
        let mut mem = PatternMemory::default();
        let mut w =
            AutoBgTransferWork::new(1, 0, 0x9c00, 0xdfed, (0..360).map(|x| x as u8).collect())
                .unwrap();
        loop {
            let old = w.clone();
            match w.step(&mut t.pattern_bus(&mut mem, &NoAssets)) {
                Ok(true) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(w, old);
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        let (mut copy, mut owner, mut memory): (
            AutoBgTransferWork,
            DisplayWorkTime,
            PatternMemory,
        ) = serde_json::from_str(&serde_json::to_string(&(&w, &t, &mem)).unwrap()).unwrap();
        t.begin_native_update();
        owner.begin_native_update();
        while w.step(&mut t.pattern_bus(&mut mem, &NoAssets)).unwrap() {}
        while copy
            .step(&mut owner.pattern_bus(&mut memory, &NoAssets))
            .unwrap()
        {}
        assert_eq!(w, copy);
        assert_eq!(t.elapsed(), owner.elapsed());
        assert_eq!(mem, memory);
    }
    #[test]
    fn six_rows_and_portion_rotation_preserve_padding_and_stack() {
        for (portion, next, ticks) in [(0, 1, 2640), (1, 2, 2660), (2, 0, 2664), (255, 0, 2664)] {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            t.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
            let tiles: Vec<u8> = (0..360).map(|x| (x * 7 + 31) as u8).collect();
            let mut mem = PatternMemory::default();
            let mut w = AutoBgTransferWork::new(1, portion, 0x9c00, 0xdfed, tiles.clone()).unwrap();
            while w.step(&mut t.pattern_bus(&mut mem, &NoAssets)).unwrap() {}
            assert_eq!(t.elapsed(), ticks);
            assert_eq!(w.next_portion(), next);
            assert_eq!(w.stack_pointer(), 0xdfed);
            let section = if portion < 2 { portion } else { 2 };
            for y in 0..6 {
                for x in 0..32 {
                    assert_eq!(
                        mem.read(0x9c00 + u16::from(section) * 192 + y * 32 + x),
                        Some(if x < 20 {
                            tiles[usize::from(section) * 120 + usize::from(y) * 20 + usize::from(x)]
                        } else {
                            0
                        })
                    );
                }
            }
        }
    }
    #[test]
    fn disabled_transfer_does_not_touch_memory_portion_or_stack() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        let mut mem = PatternMemory::default();
        let before = mem.clone();
        let mut w = AutoBgTransferWork::new(0, 2, 0x9c00, 0xdfed, vec![13; 360]).unwrap();
        while w.step(&mut t.pattern_bus(&mut mem, &NoAssets)).unwrap() {}
        assert_eq!(t.elapsed(), 36);
        assert_eq!(mem, before);
        assert_eq!(w.next_portion(), 2);
        assert_eq!(w.stack_pointer(), 0xdfed);
    }
}
