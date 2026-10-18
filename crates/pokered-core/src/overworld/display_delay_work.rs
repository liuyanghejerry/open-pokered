//! Foreground DelayFrame from home/vblank.asm, on the common time/IRQ owner.
//! A device edge, size-port clear, or wait-flag clear is not an ISR return.
//! The owner services the actual IRQ and publishes its RETI receipt; this
//! child executes only foreground instructions, never a surrogate IRQ tail.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};

pub const VBLANK_FLAG_PORT: u16 = 0xffd6; // hVBlankOccurred in pinned retail Red
pub trait DelayFrameBus: DisplayInstructionBus {
    fn interrupt_returns(&self) -> u64;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayProgress {
    Instruction,
    Sleeping,
    Returned,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    #[default]
    LoadOne,
    StoreFlag,
    Halt,
    Sleeping,
    ReadFlag,
    And,
    Repeat,
    Return,
    Done,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DelayFrameWork {
    step: Step,
    a: u8,
    halt_returns: u64,
}
impl DelayFrameWork {
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    pub fn sleeping(&self) -> bool {
        self.step == Step::Sleeping
    }
    pub fn step<B: DelayFrameBus>(&mut self, bus: &mut B) -> Result<DelayProgress, B::Error> {
        use Step::*;
        if self.step == Done {
            return Ok(DelayProgress::Returned);
        }
        if self.step == Sleeping {
            if bus.interrupt_returns() <= self.halt_returns {
                return Ok(DelayProgress::Sleeping);
            }
            self.step = ReadFlag;
        }
        let (tcycles, access) = match self.step {
            LoadOne => (8, None),
            StoreFlag => (
                12,
                Some(DisplayAccess::Write {
                    port: VBLANK_FLAG_PORT,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            Halt => (4, None),
            ReadFlag => (
                12,
                Some(DisplayAccess::Read {
                    port: VBLANK_FLAG_PORT,
                    at_tcycle: 8,
                }),
            ),
            And => (4, None),
            Repeat => (if self.a == 0 { 8 } else { 12 }, None),
            Return => (16, None),
            Sleeping | Done => unreachable!(),
        };
        let value = bus.execute(DisplayInstruction { tcycles, access })?;
        self.step = match self.step {
            LoadOne => {
                self.a = 1;
                StoreFlag
            }
            StoreFlag => Halt,
            Halt => {
                self.halt_returns = bus.interrupt_returns();
                Sleeping
            }
            ReadFlag => {
                self.a = value.expect("wait flag read receipt");
                And
            }
            And => Repeat,
            Repeat => {
                if self.a == 0 {
                    Return
                } else {
                    Halt
                }
            }
            Return => Done,
            Sleeping | Done => unreachable!(),
        };
        Ok(if self.finished() {
            DelayProgress::Returned
        } else {
            DelayProgress::Instruction
        })
    }
}

/// DelayFrames from home/delay.asm. C=0 deliberately wraps through 256 waits;
/// the source enters its loop before decrementing C. This caller adds only its
/// real CALL/DEC/JR/RET instructions, never a host-frame duration estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DelayFramesWork {
    remaining: u8,
    step: FramesStep,
    child: DelayFrameWork,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum FramesStep {
    Call,
    Child,
    Decrement,
    Repeat,
    Return,
    Done,
}
impl DelayFramesWork {
    pub fn new(frames: u8) -> Self {
        Self {
            remaining: frames,
            step: FramesStep::Call,
            child: DelayFrameWork::default(),
        }
    }
    pub fn finished(&self) -> bool {
        self.step == FramesStep::Done
    }
    pub fn sleeping(&self) -> bool {
        self.step == FramesStep::Child && self.child.sleeping()
    }
    pub fn step<B: DelayFrameBus>(&mut self, bus: &mut B) -> Result<DelayProgress, B::Error> {
        use FramesStep::*;
        if self.step == Done {
            return Ok(DelayProgress::Returned);
        }
        if self.step == Child {
            return match self.child.step(bus)? {
                DelayProgress::Returned => {
                    self.step = Decrement;
                    Ok(DelayProgress::Instruction)
                }
                progress => Ok(progress),
            };
        }
        let tcycles = match self.step {
            Call => 24,
            Decrement => 4,
            Repeat => {
                if self.remaining == 0 {
                    8
                } else {
                    12
                }
            }
            Return => 16,
            Child | Done => unreachable!(),
        };
        bus.execute(DisplayInstruction {
            tcycles,
            access: None,
        })?;
        self.step = match self.step {
            Call => {
                self.child = DelayFrameWork::default();
                Child
            }
            Decrement => {
                self.remaining = self.remaining.wrapping_sub(1);
                Repeat
            }
            Repeat => {
                if self.remaining == 0 {
                    Return
                } else {
                    Call
                }
            }
            Return => Done,
            Child | Done => unreachable!(),
        };
        Ok(if self.finished() {
            DelayProgress::Returned
        } else {
            DelayProgress::Instruction
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use super::*;
    fn clear_flag(time: &mut DisplayWorkTime) {
        time.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write {
                port: VBLANK_FLAG_PORT,
                value: 0,
                at_tcycle: 8,
            }),
        })
        .unwrap();
    }
    fn enter_halt(time: &mut DisplayWorkTime, work: &mut DelayFrameWork) {
        for _ in 0..3 {
            assert_eq!(work.step(time).unwrap(), DelayProgress::Instruction);
        }
        assert!(work.sleeping());
    }
    #[test]
    fn edge_and_flag_clear_cannot_resume_before_real_interrupt_return_receipt() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        let mut work = DelayFrameWork::default();
        enter_halt(&mut time, &mut work);
        time.idle_to_target().unwrap();
        assert!(time.device.vblank_epoch() > 0);
        assert_eq!(work.step(&mut time).unwrap(), DelayProgress::Sleeping);
        time.begin_native_update();
        clear_flag(&mut time);
        assert_eq!(work.step(&mut time).unwrap(), DelayProgress::Sleeping);
        // Synthetic owner receipt tests the seam only, not a model of IRQ cost.
        time.execute(DisplayInstruction {
            tcycles: 16,
            access: None,
        })
        .unwrap();
        time.record_interrupt_return();
        while !work.finished() {
            work.step(&mut time).unwrap();
        }
        assert!(work.finished());
    }
    #[test]
    fn interrupt_before_halt_does_not_wake_a_later_halt_and_snapshot_keeps_it() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        let mut work = DelayFrameWork::default();
        work.step(&mut time).unwrap();
        work.step(&mut time).unwrap();
        clear_flag(&mut time);
        time.record_interrupt_return(); // before HALT
        work.step(&mut time).unwrap();
        assert!(work.sleeping());
        assert_eq!(work.step(&mut time).unwrap(), DelayProgress::Sleeping);
        // A stale/mismatched owner with an older receipt must not wake it.
        let mut older_owner = DisplayWorkTime::default();
        older_owner.begin_native_update();
        assert_eq!(
            work.step(&mut older_owner).unwrap(),
            DelayProgress::Sleeping
        );
        let saved = serde_json::to_string(&(work, &time)).unwrap();
        let (mut restored, mut restored_time): (DelayFrameWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        assert_eq!(
            restored.step(&mut restored_time).unwrap(),
            DelayProgress::Sleeping
        );
        for pair in [(&mut work, &mut time), (&mut restored, &mut restored_time)] {
            pair.1.record_interrupt_return();
            while !pair.0.finished() {
                pair.0.step(pair.1).unwrap();
            }
        }
        assert_eq!(work, restored);
        assert_eq!(time.elapsed(), restored_time.elapsed());
    }
    #[test]
    fn unrelated_interrupt_repeats_halt_and_budget_does_not_skip_flag_poll() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        let mut work = DelayFrameWork::default();
        enter_halt(&mut time, &mut work);
        time.record_interrupt_return(); // no VBlank flag clear
        for _ in 0..4 {
            work.step(&mut time).unwrap();
        }
        assert!(work.sleeping(), "non-VBlank IRQ must not finish DelayFrame");
        time.idle_to_target().unwrap();
        clear_budget_check(&mut time, &mut work);
        time.begin_native_update();
        clear_flag(&mut time);
        time.record_interrupt_return();
        while !work.finished() {
            work.step(&mut time).unwrap();
        }
    }
    fn clear_budget_check(time: &mut DisplayWorkTime, work: &mut DelayFrameWork) {
        time.record_interrupt_return();
        let before = time.elapsed();
        assert_eq!(work.step(time), Err(DisplayWorkError::Budget));
        assert_eq!(time.elapsed(), before);
        assert!(!work.finished());
    }
    #[test]
    fn delay_frames_requires_each_return_and_preserves_zero_count_underflow() {
        for (count, expected_returns) in [(1, 1), (3, 3), (0, 256)] {
            let mut time = DisplayWorkTime::default();
            time.begin_native_update();
            let mut work = DelayFramesWork::new(count);
            let mut interrupts = 0;
            let mut instructions = 0;
            while !work.finished() {
                instructions += 1;
                assert!(instructions < 10000);
                match work.step(&mut time).unwrap() {
                    DelayProgress::Sleeping => {
                        assert!(work.sleeping());
                        // Synthetic IRQ seam; no original IRQ cost claimed.
                        clear_flag(&mut time);
                        assert_eq!(work.step(&mut time).unwrap(), DelayProgress::Sleeping);
                        time.record_interrupt_return();
                        interrupts += 1;
                    }
                    _ => (),
                }
            }
            assert_eq!(interrupts, expected_returns);
            // Foreground source instructions only, plus the synthetic flag
            // stores above. This is not a scene duration or an IRQ surcharge.
            assert_eq!(
                time.elapsed(),
                expected_returns * (24 + 64 + 4 + 12 + 12) + 12
            );
        }
    }
}
