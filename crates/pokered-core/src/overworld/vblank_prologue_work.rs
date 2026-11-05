//! VBlank prologue through CALL AutoBgMapTransfer. Foreground register values
//! remain in the suspended native continuation; this is not a CPU-stack model.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::far_pattern_copy_work::LOADED_ROM_BANK;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankVideoShadows {
    pub scx: u8,
    pub scy: u8,
    pub wy: u8,
    pub disable_wy: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankPrologueWork {
    step: u8,
    a: u8,
    saved_bank: u8,
    shadows: VBlankVideoShadows,
}
impl VBlankPrologueWork {
    pub fn new(shadows: VBlankVideoShadows) -> Self {
        Self {
            step: 0,
            a: 0,
            saved_bank: 0,
            shadows,
        }
    }
    pub fn saved_bank(&self) -> Option<u8> {
        (self.step == 16).then_some(self.saved_bank)
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        let (t, access) = match self.step {
            0..=3 => (16, None),
            4 => (
                12,
                Some(DisplayAccess::Read {
                    port: LOADED_ROM_BANK,
                    at_tcycle: 8,
                }),
            ),
            5 | 10 => (16, None),
            6 | 8 | 13 => (12, None),
            7 => (
                12,
                Some(DisplayAccess::Write {
                    port: 0xff43,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            9 => (
                12,
                Some(DisplayAccess::Write {
                    port: 0xff42,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            11 => (4, None),
            12 => (if self.a != 0 { 12 } else { 8 }, None),
            14 => (
                12,
                Some(DisplayAccess::Write {
                    port: 0xff4a,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            15 => (24, None),
            16 => return Ok(false),
            _ => unreachable!(),
        };
        let v = bus.execute(DisplayInstruction { tcycles: t, access })?;
        match self.step {
            4 => self.a = v.expect("loaded bank read"),
            5 => self.saved_bank = self.a,
            6 => self.a = self.shadows.scx,
            8 => self.a = self.shadows.scy,
            10 => self.a = self.shadows.disable_wy,
            13 => self.a = self.shadows.wy,
            _ => (),
        }
        self.step = if self.step == 12 && self.a != 0 {
            15
        } else {
            self.step + 1
        };
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};
    #[test]
    fn scroll_and_conditional_window_writes_precede_background_call() {
        for (disabled, expected, ticks) in [(0, 144, 216), (1, 19, 196), (255, 19, 196)] {
            let mut bus = DisplayWorkTime::default();
            bus.begin_native_update();
            bus.device_mut().write_port(0xff4a, 19, 0xc3).unwrap();
            let mut w = VBlankPrologueWork::new(VBlankVideoShadows {
                scx: 7,
                scy: 23,
                wy: 144,
                disable_wy: disabled,
            });
            let start = bus.elapsed();
            while w.step(&mut bus).unwrap() {}
            assert_eq!(bus.elapsed() - start, ticks);
            assert_eq!(w.saved_bank(), Some(1));
            assert_eq!(bus.device().read_port(0xff43).unwrap(), 7);
            assert_eq!(bus.device().read_port(0xff42).unwrap(), 23);
            assert_eq!(bus.device().read_port(0xff4a).unwrap(), expected);
        }
    }
    #[test]
    fn interrupted_store_keeps_its_operand_and_saved_bank() {
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        while bus.remaining() > 100 {
            bus.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let mut w = VBlankPrologueWork::new(VBlankVideoShadows {
            scx: 17,
            scy: 29,
            wy: 144,
            disable_wy: 0,
        });
        loop {
            let before = w;
            match w.step(&mut bus) {
                Ok(true) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(w, before);
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        let (mut restored, mut owner): (VBlankPrologueWork, DisplayWorkTime) =
            serde_json::from_str(&serde_json::to_string(&(&w, &bus)).unwrap()).unwrap();
        bus.begin_native_update();
        owner.begin_native_update();
        while w.step(&mut bus).unwrap() {}
        while restored.step(&mut owner).unwrap() {}
        assert_eq!(w, restored);
        assert_eq!(bus.elapsed(), owner.elapsed());
        assert_eq!(bus.device(), owner.device());
    }
}
