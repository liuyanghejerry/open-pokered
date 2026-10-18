//! hDMARoutine from engine/gfx/oam_dma.asm. The common device owns the actual
//! byte transfer. This finite HRAM child executes the source LD/LDH and
//! DEC/JR loop, without bulk-copying OAM or charging an aggregate delay.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    #[default]
    LoadSource,
    StartDma,
    LoadCount,
    Decrement,
    Repeat,
    Return,
    Done,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisplayDmaWork {
    step: Step,
    a: u8,
}
impl DisplayDmaWork {
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<DisplayInstruction>, B::Error> {
        use Step::*;
        let (tcycles, access) = match self.step {
            LoadSource | LoadCount => (8, None),
            StartDma => (
                12,
                Some(DisplayAccess::Write {
                    port: 0xff46,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            Decrement => (4, None),
            Repeat => (if self.a == 0 { 8 } else { 12 }, None),
            Return => (16, None),
            Done => return Ok(None),
        };
        let instruction = DisplayInstruction { tcycles, access };
        bus.execute(instruction)?;
        self.step = match self.step {
            LoadSource => {
                self.a = 0xc3;
                StartDma
            }
            StartDma => LoadCount,
            LoadCount => {
                self.a = 0x28;
                Decrement
            }
            Decrement => {
                self.a = self.a.wrapping_sub(1);
                Repeat
            }
            Repeat => {
                if self.a == 0 {
                    Return
                } else {
                    Decrement
                }
            }
            Return | Done => Done,
        };
        Ok(Some(instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::super::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use super::*;
    fn bytes() -> [u8; 160] {
        core::array::from_fn(|n| (n as u8).wrapping_mul(7).wrapping_add(3))
    }
    #[test]
    fn actual_dma_progresses_after_port_store_and_mid_transfer_snapshot_replays() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        let source = bytes();
        time.set_shadow_oam(&source).unwrap();
        let mut work = DisplayDmaWork::default();
        work.step(&mut time).unwrap();
        assert!(!time.device.dma_active());
        work.step(&mut time).unwrap();
        assert!(time.device.dma_active());
        assert_ne!(time.device.oam(), source);
        for _ in 0..18 {
            work.step(&mut time).unwrap();
        }
        assert!(time.device.dma_active());
        let saved = serde_json::to_string(&(work, &time)).unwrap();
        let (mut other, mut other_time): (DisplayDmaWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        while work.step(&mut time).unwrap().is_some() {}
        while other.step(&mut other_time).unwrap().is_some() {}
        assert_eq!(work, other);
        assert!(work.finished());
        assert!(!time.device.dma_active());
        assert_eq!(time.device.oam(), source);
        assert_eq!(time.device, other_time.device);
        assert_eq!(time.elapsed(), 680); // source instructions, excludes caller CALL
    }
    #[test]
    fn grant_exhaustion_preserves_dma_child_and_does_not_replay_the_port_store() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        let source = bytes();
        time.set_shadow_oam(&source).unwrap();
        while time.remaining() > 40 {
            time.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let start = time.elapsed();
        let mut work = DisplayDmaWork::default();
        loop {
            let before = work;
            match work.step(&mut time) {
                Ok(Some(_)) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(work, before);
                    break;
                }
                other => panic!("unexpected DMA result {other:?}"),
            }
        }
        assert!(time.device.dma_active());
        let elapsed = time.elapsed();
        let prior = time.device.clone();
        let child = work;
        time.begin_native_update();
        assert_eq!(time.elapsed(), elapsed);
        assert_eq!(time.device, prior);
        assert_eq!(work, child);
        while work.step(&mut time).unwrap().is_some() {}
        assert_eq!(time.elapsed() - start, 680);
        assert_eq!(time.device.oam(), source);
        assert!(!time.device.dma_active());
    }
}
