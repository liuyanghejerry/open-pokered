//! Finite GBPalWhiteOut / LoadGBPal instructions on the shared display bus.
//! Source: home/palettes.asm and home/fade.asm. Party return retains the current
//! map offset; FLASH explicitly clears it before entering this routine.
//! The returned child does not publish an entire native frame or supply a
//! substitute IRQ return. The raster/presentation owner observes these stores.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Routine {
    White,
    Map { offset: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayPaletteWork {
    routine: Routine,
    next: u8,
}

impl DisplayPaletteWork {
    pub fn white_out() -> Self {
        Self {
            routine: Routine::White,
            next: 0,
        }
    }

    /// LoadGBPal with wMapPalOffset=0 after successful FLASH.
    pub fn lit_map() -> Self {
        Self::map_palette(0).unwrap()
    }

    /// FadePal4 minus the live wMapPalOffset. The map's 0/6 offsets and the
    /// intervening original fade-table bytes all remain in the same ROM page.
    pub fn map_palette(offset: u8) -> Result<Self, u8> {
        if offset > 9 { return Err(offset); }
        Ok(Self {
            routine: Routine::Map { offset },
            next: 0,
        })
    }

    fn map_byte(offset: u8, index: usize) -> u8 {
        // FadePal1..FadePal4, home/fade.asm. Read actual adjacent bytes,
        // including offsets that do not start on a three-byte boundary.
        const FADE: [u8; 12] = [0xff, 0xff, 0xff, 0xfe, 0xfe, 0xf8,
            0xf9, 0xe4, 0xe4, 0xe4, 0xd0, 0xe0];
        FADE[9 - usize::from(offset) + index]
    }

    pub fn finished(&self) -> bool {
        self.next
            == match self.routine {
                Routine::White => 5,
                Routine::Map { .. } => 14,
            }
    }

    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<DisplayInstruction>, B::Error> {
        if self.finished() {
            return Ok(None);
        }
        let (tcycles, store) = match self.routine {
            Routine::White => match self.next {
                0 => (4, None), // XOR A
                1 => (12, Some((0xff47, 0))),
                2 => (12, Some((0xff48, 0))),
                3 => (12, Some((0xff49, 0))),
                4 => (16, None), // RET
                _ => unreachable!(),
            },
            Routine::Map { offset } => match self.next {
                0 => (16, None),         // LD A,[wMapPalOffset]
                1 => (4, None),          // LD B,A
                2 => (12, None),         // LD HL,FadePal4 ($2116 in pinned Red)
                3..=5 => (4, None),      // LD A,L / SUB B / LD L,A
                6 => (12, None),         // JR NC,.ok; all valid offsets stay in-page
                7 | 9 | 11 => (8, None), // LD A,[HLI]
                8 => (12, Some((0xff47, Self::map_byte(offset, 0)))),
                10 => (12, Some((0xff48, Self::map_byte(offset, 1)))),
                12 => (12, Some((0xff49, Self::map_byte(offset, 2)))),
                13 => (16, None), // RET
                _ => unreachable!(),
            },
        };
        let instruction = DisplayInstruction {
            tcycles,
            access: store.map(|(port, value)| DisplayAccess::Write {
                port,
                value,
                at_tcycle: 8,
            }),
        };
        bus.execute(instruction)?;
        self.next += 1;
        Ok(Some(instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::*;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};

    #[test]
    fn palette_stores_follow_source_instruction_order_and_access_cycles() {
        struct Trace {
            time: DisplayWorkTime,
            stores: Vec<(u64, u16, u8)>,
        }
        impl DisplayInstructionBus for Trace {
            type Error = DisplayWorkError;
            fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
                let start = self.time.elapsed();
                let result = self.time.execute(i)?;
                if let Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle,
                }) = i.access
                {
                    self.stores
                        .push((start + u64::from(at_tcycle), port, value));
                }
                Ok(result)
            }
        }
        let mut bus = Trace {
            time: DisplayWorkTime::default(),
            stores: Vec::new(),
        };
        bus.time.begin_native_update();
        let mut lit = DisplayPaletteWork::lit_map();
        while lit.step(&mut bus).unwrap().is_some() {}
        assert_eq!(
            bus.stores,
            [(72, 0xff47, 0xe4), (92, 0xff48, 0xd0), (112, 0xff49, 0xe0)]
        );
        assert_eq!(bus.time.elapsed(), 132);
        bus.stores.clear();
        let mut white = DisplayPaletteWork::white_out();
        while white.step(&mut bus).unwrap().is_some() {}
        assert_eq!(
            bus.stores,
            [(144, 0xff47, 0), (156, 0xff48, 0), (168, 0xff49, 0)]
        );
        assert_eq!(bus.time.elapsed(), 188);
        for port in 0xff47..=0xff49 {
            assert_eq!(bus.time.device().read_port(port).unwrap(), 0);
        }
    }

    #[test]
    fn dark_map_return_keeps_the_original_palette_and_instruction_duration() {
        for (offset, palettes) in [(0, [0xe4, 0xd0, 0xe0]), (3, [0xf9, 0xe4, 0xe4]),
            (6, [0xfe, 0xfe, 0xf8]), (9, [0xff, 0xff, 0xff])] {
            let mut time = DisplayWorkTime::default();
            time.begin_native_update();
            let mut work = DisplayPaletteWork::map_palette(offset).unwrap();
            while work.step(&mut time).unwrap().is_some() {}
            assert_eq!(time.elapsed(), 132);
            for (port, palette) in (0xff47..=0xff49).zip(palettes) {
                assert_eq!(time.device().read_port(port).unwrap(), palette);
            }
        }
        assert_eq!(DisplayPaletteWork::map_palette(10), Err(10));
    }

    #[test]
    fn grant_boundary_and_snapshot_retain_partial_palette_writes() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        while time.remaining() > 76 {
            time.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let start = time.elapsed();
        let mut work = DisplayPaletteWork::lit_map();
        loop {
            let before = work;
            match work.step(&mut time) {
                Ok(Some(_)) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(before, work);
                    break;
                }
                other => panic!("unexpected palette result {other:?}"),
            }
        }
        assert_eq!(time.device().read_port(0xff47).unwrap(), 0xe4);
        assert_eq!(time.device().read_port(0xff48).unwrap(), 0);
        let saved = serde_json::to_string(&(work, &time)).unwrap();
        let (mut copy, mut other): (DisplayPaletteWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        time.begin_native_update();
        other.begin_native_update();
        while work.step(&mut time).unwrap().is_some() {}
        while copy.step(&mut other).unwrap().is_some() {}
        assert_eq!(work, copy);
        assert_eq!(time.elapsed() - start, 132);
        assert_eq!(time.device(), other.device());
        assert_eq!(time.device().read_port(0xff48).unwrap(), 0xd0);
        assert_eq!(time.device().read_port(0xff49).unwrap(), 0xe0);
    }
}
