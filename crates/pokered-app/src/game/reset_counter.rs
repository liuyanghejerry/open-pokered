//! The original hSoftReset byte, independent of input-owner scheduling.
use pokered_renderer::input::GbButton;

pub(super) struct SoftResetCounter {
    remaining: u8,
}

impl SoftResetCounter {
    pub(super) const fn new() -> Self {
        Self {
            remaining: super::SOFT_RESET_HOLD_FRAMES,
        }
    }

    pub(super) fn is_combo(raw: u8) -> bool {
        const BUTTONS: u8 = (1 << GbButton::A as u8)
            | (1 << GbButton::B as u8)
            | (1 << GbButton::Select as u8)
            | (1 << GbButton::Start as u8);
        raw == BUTTONS
    }

    /// A caller supplies a real raw sample. Only TrySoftReset's exact
    /// PAD_BUTTONS path decrements; a different input never reseeds Init.
    /// True means the caller must perform reset before another poll.
    pub(super) fn poll(&mut self, raw: u8) -> bool {
        if !Self::is_combo(raw) {
            return false;
        }
        self.complete_delay()
    }

    fn complete_delay(&mut self) -> bool {
        self.remaining = self.remaining.saturating_sub(1);
        self.remaining == 0
    }
}

use pokered_core::overworld::display_delay_work::{DelayFrameBus, DelayFrameWork, DelayProgress};
use pokered_core::overworld::display_lcd_work::{DisplayAccess, DisplayInstruction};

#[derive(Default)]
enum ResetStep {
    #[default]
    CallDelay,
    Delay,
    LoadSelect,
    Deselect,
    LoadCounter,
    Decrement,
    JumpReset,
    JumpJoypad,
}

pub(super) enum ResetProgress {
    Instruction,
    Sleeping,
    RetryJoypad,
    Reset,
}

/// engine/joypad.asm::TrySoftReset, entered only after an exact raw $0f poll.
/// The accepted branch owns its DelayFrame even if keys change during the IRQ.
/// It neither polls foreground history nor decrements before that real return.
#[derive(Default)]
pub(super) struct TrySoftResetWork {
    step: ResetStep,
    delay: DelayFrameWork,
    reset: bool,
}

impl TrySoftResetWork {
    pub(super) fn step<B: DelayFrameBus>(
        &mut self,
        bus: &mut B,
        counter: &mut SoftResetCounter,
    ) -> Result<ResetProgress, B::Error> {
        use ResetStep::*;
        if matches!(self.step, Delay) {
            return Ok(match self.delay.step(bus)? {
                DelayProgress::Instruction => ResetProgress::Instruction,
                DelayProgress::Sleeping => ResetProgress::Sleeping,
                DelayProgress::Returned => {
                    self.step = LoadSelect;
                    ResetProgress::Instruction
                }
            });
        }
        let tcycles = match self.step {
            CallDelay => 24,
            LoadSelect => 8,
            Deselect | LoadCounter | Decrement => 12,
            JumpReset => if self.reset { 16 } else { 12 },
            JumpJoypad => 16,
            Delay => unreachable!(),
        };
        let access = matches!(self.step, Deselect).then_some(DisplayAccess::Write {
            port: 0xff00, value: 0x30, at_tcycle: 8,
        });
        bus.execute(DisplayInstruction { tcycles, access })?;
        self.step = match self.step {
            CallDelay => Delay,
            LoadSelect => Deselect,
            Deselect => LoadCounter,
            LoadCounter => Decrement,
            Decrement => {
                self.reset = counter.complete_delay();
                JumpReset
            }
            JumpReset if self.reset => return Ok(ResetProgress::Reset),
            JumpReset => JumpJoypad,
            JumpJoypad => return Ok(ResetProgress::RetryJoypad),
            Delay => unreachable!(),
        };
        Ok(ResetProgress::Instruction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_buttons_decrement_all_256_raw_masks() {
        for raw in 0u8..=255 {
            let mut counter = SoftResetCounter::new();
            assert!(!counter.poll(raw));
            assert_eq!(counter.remaining, if raw == 0x0f { 15 } else { 16 });
        }
    }

    #[test]
    fn eight_combo_neutral_and_eight_combo_reach_reset() {
        let mut counter = SoftResetCounter::new();
        for _ in 0..8 {
            assert!(!counter.poll(0x0f));
        }
        assert!(!counter.poll(0));
        assert!(!counter.poll(0x4f), "extra UP is not a reset poll");
        for _ in 0..7 {
            assert!(!counter.poll(0x0f));
        }
        assert!(counter.poll(0x0f));
    }

    #[test]
    fn a_new_boot_counter_requires_all_sixteen_polls_again() {
        let mut old = SoftResetCounter::new();
        for _ in 0..15 {
            assert!(!old.poll(0x0f));
        }
        assert!(old.poll(0x0f));
        let mut after_init = SoftResetCounter::new();
        for _ in 0..15 {
            assert!(!after_init.poll(0x0f));
        }
        assert!(after_init.poll(0x0f));
    }

    #[test]
    fn delay_requires_irq_return_and_execution_budget_before_each_decrement() {
        use pokered_core::overworld::display_delay_work::VBLANK_FLAG_PORT;
        use pokered_core::overworld::display_lcd_work::DisplayInstructionBus;
        use pokered_core::overworld::display_work_time::DisplayWorkError;
        // A synthetic bus exercises the continuation seam only. Production
        // IRQ scheduling is covered by SAVE inputs and default-ROM replays.
        #[derive(Default)]
        struct Bus { budget: u32, returns: u64, flag: u8 }
        impl DisplayInstructionBus for Bus {
            type Error = DisplayWorkError;
            fn execute(&mut self, instruction: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
                if self.budget == 0 { return Err(DisplayWorkError::Budget); }
                self.budget = self.budget.saturating_sub(u32::from(instruction.tcycles));
                Ok(match instruction.access {
                    Some(DisplayAccess::Read { port: VBLANK_FLAG_PORT, .. }) => Some(self.flag),
                    Some(DisplayAccess::Write { port: VBLANK_FLAG_PORT, value, .. }) => {
                        self.flag = value; None
                    }
                    Some(DisplayAccess::Write { port: 0xff00, value: 0x30, .. }) | None => None,
                    access => panic!("unexpected reset access: {access:?}"),
                })
            }
        }
        impl DelayFrameBus for Bus {
            fn interrupt_returns(&self) -> u64 { self.returns }
        }
        let mut counter = SoftResetCounter::new();
        let mut time = Bus::default();
        for expected in (0..16).rev() {
            time.budget = 4096;
            let mut work = TrySoftResetWork::default();
            for _ in 0..32 {
                if matches!(work.step(&mut time, &mut counter).unwrap(), ResetProgress::Sleeping) { break; }
            }
            assert!(work.delay.sleeping());
            assert_eq!(counter.remaining, expected + 1);
            time.flag = 0;
            time.budget = 0;
            assert!(matches!(work.step(&mut time, &mut counter).unwrap(), ResetProgress::Sleeping));
            time.returns += 1;
            assert!(matches!(work.step(&mut time, &mut counter), Err(DisplayWorkError::Budget)));
            assert_eq!(counter.remaining, expected + 1);
            time.budget = 4096;
            let reset = loop {
                match work.step(&mut time, &mut counter).unwrap() {
                    ResetProgress::RetryJoypad => break false,
                    ResetProgress::Reset => break true,
                    ResetProgress::Sleeping => panic!("completed IRQ must release DelayFrame"),
                    ResetProgress::Instruction => (),
                }
            };
            assert_eq!(counter.remaining, expected);
            assert_eq!(reset, expected == 0);
        }
    }
}
