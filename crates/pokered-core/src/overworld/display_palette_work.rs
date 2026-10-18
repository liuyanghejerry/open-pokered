//! Finite GBPalWhiteOut / lit LoadGBPal instructions on the shared display bus.
//! Source: home/palettes.asm and home/fade.asm. FLASH has already cleared
//! wMapPalOffset, so LoadGBPal takes the offset-zero, no-borrow table branch.
//! The returned child does not publish an entire native frame or supply a
//! substitute IRQ return. The raster/presentation owner observes these stores.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Routine {
    White,
    LitMap,
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

    /// LoadGBPal with wMapPalOffset=0 (after successful FLASH). Other map
    /// darkness/fade branches are not silently mapped to this lit branch.
    pub fn lit_map() -> Self {
        Self {
            routine: Routine::LitMap,
            next: 0,
        }
    }

    pub fn finished(&self) -> bool {
        self.next
            == match self.routine {
                Routine::White => 5,
                Routine::LitMap => 14,
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
            Routine::LitMap => match self.next {
                0 => (16, None),         // LD A,[wMapPalOffset], already zero
                1 => (4, None),          // LD B,A
                2 => (12, None),         // LD HL,FadePal4 ($2116 in pinned Red)
                3..=5 => (4, None),      // LD A,L / SUB B / LD L,A
                6 => (12, None),         // JR NC,.ok, no borrow for offset zero
                7 | 9 | 11 => (8, None), // LD A,[HLI], original FadePal4
                8 => (12, Some((0xff47, 0xe4))),
                10 => (12, Some((0xff48, 0xd0))),
                12 => (12, Some((0xff49, 0xe0))),
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
            assert_eq!(bus.time.device.read_port(port).unwrap(), 0);
        }
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
        assert_eq!(time.device.read_port(0xff47).unwrap(), 0xe4);
        assert_eq!(time.device.read_port(0xff48).unwrap(), 0);
        let saved = serde_json::to_string(&(work, &time)).unwrap();
        let (mut copy, mut other): (DisplayPaletteWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        time.begin_native_update();
        other.begin_native_update();
        while work.step(&mut time).unwrap().is_some() {}
        while copy.step(&mut other).unwrap().is_some() {}
        assert_eq!(work, copy);
        assert_eq!(time.elapsed() - start, 132);
        assert_eq!(time.device, other.device);
        assert_eq!(time.device.read_port(0xff48).unwrap(), 0xd0);
        assert_eq!(time.device.read_port(0xff49).unwrap(), 0xe0);
    }
}
