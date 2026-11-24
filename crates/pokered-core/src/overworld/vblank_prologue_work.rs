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
fn plain<B: DisplayInstructionBus>(bus: &mut B, cycles: &[u8], total: u64) -> Result<(), B::Error> {
    let count = bus.execute_plain_cycles_total(cycles, total)?;
    assert_eq!(count, cycles.len());
    Ok(())
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
    pub fn run<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
        remaining: u64,
    ) -> Result<(), B::Error> {
        let cycles = if self.shadows.disable_wy == 0 {
            216
        } else {
            196
        };
        if self.step != 0 || remaining < cycles {
            while self.step != 16 {
                let start = usize::from(self.step);
                let (instructions, total): (&[u8], u64) = match self.step {
                    0..=3 => (&[16, 16, 16, 16][start..], ((4 - start) * 16) as u64),
                    5 | 6 => (&[16, 12][start - 5..], (28 - (start - 5) * 16) as u64),
                    8 | 13 => (&[12], 12),
                    10..=12 => {
                        let disable_wy = if self.step == 10 {
                            self.shadows.disable_wy
                        } else {
                            self.a
                        };
                        let total = if disable_wy != 0 {
                            match self.step {
                                10 => 32,
                                11 => 16,
                                _ => 12,
                            }
                        } else {
                            match self.step {
                                10 => 40,
                                11 => 24,
                                _ => 20,
                            }
                        };
                        (
                            &[16, 4, if disable_wy != 0 { 12 } else { 8 }, 12]
                                [start - 10..if disable_wy != 0 { 3 } else { 4 }],
                            total,
                        )
                    }
                    15 => (&[24], 24),
                    _ => (&[], 0),
                };
                debug_assert_eq!(total, instructions.iter().map(|&cycles| u64::from(cycles)).sum::<u64>());
                if !instructions.is_empty() {
                    let count = bus.execute_plain_cycles_total(instructions, total)?;
                    if count == instructions.len() {
                        match self.step {
                            0..=3 => self.step += count as u8,
                            5 => {
                                self.saved_bank = self.a;
                                self.a = self.shadows.scx;
                                self.step = 7;
                            }
                            6 => {
                                self.a = self.shadows.scx;
                                self.step = 7;
                            }
                            8 => {
                                self.a = self.shadows.scy;
                                self.step = 9;
                            }
                            13 => {
                                self.a = self.shadows.wy;
                                self.step = 14;
                            }
                            10..=12 => {
                                let disable_wy = if self.step == 10 {
                                    self.shadows.disable_wy
                                } else {
                                    self.a
                                };
                                if disable_wy == 0 {
                                    self.a = self.shadows.wy;
                                    self.step = 14;
                                } else {
                                    self.a = disable_wy;
                                    self.step = 15;
                                }
                            }
                            15 => self.step = 16,
                            _ => unreachable!(),
                        }
                    } else if count != 0 {
                        let mut replay = super::display_lcd_work::PlainInstructionBus(0);
                        for _ in 0..count {
                            assert!(self.step(&mut replay).unwrap());
                        }
                    }
                    if count != 0 {
                        continue;
                    }
                }
                self.step(bus)?;
            }
            return Ok(());
        }
        plain(bus, &[16, 16, 16, 16], 64)?;
        self.step = 4;
        self.step(bus)?;
        self.saved_bank = self.a;
        self.a = self.shadows.scx;
        self.step = 7;
        plain(bus, &[16, 12], 28)?;
        self.step(bus)?;
        self.a = self.shadows.scy;
        self.step = 9;
        plain(bus, &[12], 12)?;
        self.step(bus)?;
        self.a = self.shadows.disable_wy;
        self.step = 12;
        plain(bus, &[16, 4, if self.a != 0 { 12 } else { 8 }], if self.a != 0 { 32 } else { 28 })?;
        if self.a == 0 {
            plain(bus, &[12], 12)?;
            self.a = self.shadows.wy;
            self.step = 14;
            self.step(bus)?;
        }
        self.step = 15;
        plain(bus, &[24], 24)?;
        self.step = 16;
        Ok(())
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

    #[test]
    fn partial_runs_match_single_instructions_at_every_budget_boundary() {
        for disable_wy in [0, 1] {
            for budget in 0..=216 {
                let shadows = VBlankVideoShadows {
                    scx: 17,
                    scy: 29,
                    wy: 144,
                    disable_wy,
                };
                let mut actual = VBlankPrologueWork::new(shadows);
                let mut expected = actual;
                let mut bus = DisplayWorkTime::default();
                let mut reference = bus.clone();
                bus.begin_native_update();
                reference.begin_native_update();
                bus.device_mut().request_interrupt(1);
                reference.device_mut().request_interrupt(1);
                bus.enter_pending_vblank().unwrap();
                reference.enter_pending_vblank().unwrap();
                while bus.remaining() > budget {
                    bus.execute(DisplayInstruction {
                        tcycles: 4,
                        access: None,
                    })
                    .unwrap();
                    reference
                        .execute(DisplayInstruction {
                            tcycles: 4,
                            access: None,
                        })
                        .unwrap();
                }
                while actual.saved_bank().is_none() {
                    match actual.run(&mut bus.display_bus(), 0) {
                        Ok(()) | Err(DisplayWorkError::Budget) => (),
                        Err(error) => panic!("unexpected {error:?}"),
                    }
                    loop {
                        match expected.step(&mut reference) {
                            Ok(true) => (),
                            Ok(false) => break,
                            Err(DisplayWorkError::Budget) => break,
                            Err(error) => panic!("unexpected {error:?}"),
                        }
                    }
                    assert_eq!(actual, expected, "disable_wy={disable_wy} budget={budget}");
                    assert_eq!(bus.elapsed(), reference.elapsed(), "disable_wy={disable_wy} budget={budget}");
                    assert_eq!(bus.device(), reference.device(), "disable_wy={disable_wy} budget={budget}");
                    if actual.saved_bank().is_none() {
                        bus.begin_native_update();
                        reference.begin_native_update();
                    }
                }
            }
        }
    }
}
