//! VBlank bank restoration, POPs and RETI. Construction is restricted to the
//! core ISR owner, after all body children have returned. Native suspended
//! continuations retain foreground registers; physical stack aliasing and
//! nested/non-VBlank interrupts are not modeled by this bounded work.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::far_pattern_copy_work::{LOADED_ROM_BANK, ROM_BANK_SELECT};
pub trait VBlankReturnBus: DisplayInstructionBus {
    fn check_active_vblank(&self) -> Result<(), Self::Error>;
    /// Execute the actual RETI before issuing the one return receipt.
    fn execute_reti(&mut self) -> Result<(), Self::Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankReturnWork {
    saved_bank: u8,
    step: u8,
}
impl VBlankReturnWork {
    pub(crate) fn new(saved_bank: u8) -> Self {
        Self {
            saved_bank,
            step: 0,
        }
    }
    pub fn finished(&self) -> bool {
        self.step == 8
    }
    pub fn step<B: VBlankReturnBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        if self.finished() {
            return Ok(false);
        }
        bus.check_active_vblank()?;
        if self.step == 7 {
            bus.execute_reti()?;
        } else {
            let (tcycles, access) = match self.step {
                0 => (16, None),
                1 => (
                    12,
                    Some(DisplayAccess::Write {
                        port: LOADED_ROM_BANK,
                        value: self.saved_bank,
                        at_tcycle: 8,
                    }),
                ),
                2 => (
                    16,
                    Some(DisplayAccess::Write {
                        port: ROM_BANK_SELECT,
                        value: self.saved_bank,
                        at_tcycle: 12,
                    }),
                ),
                3..=6 => (12, None),
                _ => unreachable!(),
            };
            bus.execute(DisplayInstruction { tcycles, access })?;
        }
        self.step += 1;
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::{
        display_delay_work::DelayFrameBus,
        display_work_time::{DisplayWorkError, DisplayWorkTime},
    };
    fn active() -> DisplayWorkTime {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.idle_to_target().unwrap();
        t.begin_native_update();
        t.enter_pending_vblank().unwrap();
        t
    }
    #[test]
    fn actual_return_restores_bank_and_receipts_only_after_the_reti_instruction() {
        let mut t = active();
        for port in [LOADED_ROM_BANK, ROM_BANK_SELECT] {
            t.execute(DisplayInstruction {
                tcycles: 16,
                access: Some(DisplayAccess::Write {
                    port,
                    value: 31,
                    at_tcycle: 12,
                }),
            })
            .unwrap();
        }
        let start = t.elapsed();
        let mut w = VBlankReturnWork::new(4);
        w.step(&mut t).unwrap();
        assert_eq!(t.source_loaded_rom_bank(), 31);
        w.step(&mut t).unwrap();
        assert_eq!(t.source_loaded_rom_bank(), 4);
        assert_eq!(t.selected_rom_bank(), 31);
        w.step(&mut t).unwrap();
        assert_eq!(t.selected_rom_bank(), 4);
        while w.step < 7 {
            w.step(&mut t).unwrap();
        }
        assert_eq!(t.elapsed() - start, 92);
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        assert!(!w.finished());
        assert!(w.step(&mut t).unwrap());
        assert!(w.finished());
        assert_eq!(t.elapsed() - start, 108);
        assert!(!t.active_vblank());
        assert_eq!(t.interrupt_returns(), 1);
        let end = t.elapsed();
        assert!(!w.step(&mut t).unwrap());
        assert_eq!(t.elapsed(), end);
        assert_eq!(t.interrupt_returns(), 1);
    }
    #[test]
    fn missing_irq_and_exhausted_reti_grant_cannot_mutate_bank_or_publish_return() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        let mut w = VBlankReturnWork::new(4);
        let prior = w;
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::NoActiveVBlank));
        assert_eq!(w, prior);
        assert_eq!(t.elapsed(), 0);
        assert_eq!(t.source_loaded_rom_bank(), 1);
        let mut t = active();
        while t.remaining() > 92 {
            t.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let mut w = VBlankReturnWork::new(4);
        while w.step < 7 {
            w.step(&mut t).unwrap();
        }
        assert_eq!(t.remaining(), 0);
        let before = w;
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
        assert_eq!(w, before);
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        let saved = serde_json::to_string(&(&w, &t)).unwrap();
        let (mut other, mut ot): (VBlankReturnWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        t.begin_native_update();
        ot.begin_native_update();
        w.step(&mut t).unwrap();
        other.step(&mut ot).unwrap();
        assert_eq!(w, other);
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            serde_json::to_value(&ot).unwrap()
        );
        assert_eq!(t.interrupt_returns(), 1);
        assert_ne!(
            t.device.pending_interrupts() & 1,
            0,
            "new request while IRQ was active must survive RETI"
        );
    }
}
