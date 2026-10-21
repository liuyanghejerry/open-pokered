//! Foreground DelayFrame from home/vblank.asm, on the common time/IRQ owner.
//! A device edge, size-port clear, or wait-flag clear is not an ISR return.
//! The owner services the actual IRQ and publishes its RETI receipt; this
//! child executes only foreground instructions, never a surrogate IRQ tail.
use super::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};

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
