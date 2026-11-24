//! ClearSprites from home/clear_sprites.asm. These stores clear wShadowOAM;
//! hardware OAM remains unchanged until the existing DMA routine transfers it.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClearShadowOamWork {
    next: u8,
    offset: u8,
}

impl ClearShadowOamWork {
    pub fn finished(&self) -> bool {
        self.next == 7
    }

    pub fn run<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<(), B::Error> {
        if self.next == 0 && bus.execute_shadow_oam_clear(3876)? {
            self.next = 7;
            self.offset = 159;
            return Ok(());
        }
        while !self.finished() {
            let mut trial = *self;
            let mut generated = 0usize;
            let accepted = bus.execute_shadow_oam_run(core::iter::from_fn(|| {
                let mut plain = super::display_lcd_work::PlainInstructionBus(0);
                match trial.step(&mut plain) {
                    Ok(Some(instruction)) => {
                        generated += 1;
                        Some(instruction)
                    }
                    Ok(None) => None,
                    Err(error) => match error {},
                }
            }))?;
            if accepted == 0 {
                if self.step(bus)?.is_none() {
                    return Ok(());
                }
            } else if accepted == generated && trial.finished() {
                *self = trial;
            } else {
                let mut plain = super::display_lcd_work::PlainInstructionBus(0);
                for _ in 0..accepted {
                    let _ = self.step(&mut plain);
                }
            }
        }
        Ok(())
    }

    pub fn step<B: DisplayInstructionBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<DisplayInstruction>, B::Error> {
        let (tcycles, access) = match self.next {
            0 => (4, None),  // XOR A
            1 => (12, None), // LD HL,wShadowOAM
            2 => (8, None),  // LD B,160
            3 => (
                8,
                Some(DisplayAccess::Write {
                    port: 0xc300 + u16::from(self.offset),
                    value: 0,
                    at_tcycle: 4,
                }),
            ), // LD [HLI],A
            4 => (4, None),  // DEC B
            5 => (if self.offset == 159 { 8 } else { 12 }, None), // JR NZ
            6 => (16, None), // RET
            7 => return Ok(None),
            _ => unreachable!(),
        };
        let instruction = DisplayInstruction { tcycles, access };
        bus.execute(instruction)?;
        if self.next == 5 && self.offset != 159 {
            self.offset += 1;
            self.next = 3;
        } else {
            self.next += 1;
        }
        Ok(Some(instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_dma_work::DisplayDmaWork;
    use crate::overworld::display_work_time::{DisplayWorkError, DisplayWorkTime};

    #[test]
    fn shadow_clear_is_partial_and_requires_dma_before_hardware_changes() {
        let mut time = DisplayWorkTime::default();
        time.begin_native_update();
        time.set_shadow_oam(&[0x5a; 160]).unwrap();
        let mut dma = DisplayDmaWork::default();
        while dma.step(&mut time).unwrap().is_some() {}
        assert!(time.device().oam().iter().all(|&b| b == 0x5a));
        while time.remaining() > 32 {
            time.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let mut clear = ClearShadowOamWork::default();
        let start = time.elapsed();
        loop {
            match clear.run(&mut time) {
                Ok(()) | Err(DisplayWorkError::Budget) => break,
                other => panic!("unexpected clear result {other:?}"),
            }
        }
        assert_eq!(time.shadow_oam()[0], 0);
        assert_eq!(time.shadow_oam()[1], 0x5a);
        assert!(time.device().oam().iter().all(|&b| b == 0x5a));
        let state = serde_json::to_string(&(clear, &time)).unwrap();
        let (mut other_clear, mut other_time): (ClearShadowOamWork, DisplayWorkTime) =
            serde_json::from_str(&state).unwrap();
        time.begin_native_update();
        other_time.begin_native_update();
        clear.run(&mut time).unwrap();
        other_clear.run(&mut other_time).unwrap();
        assert_eq!(time.elapsed() - start, 3876);
        assert!(clear.finished());
        assert_eq!(clear, other_clear);
        assert_eq!(time.shadow_oam(), other_time.shadow_oam());
        assert!(time.shadow_oam().iter().all(|&b| b == 0));
        assert!(time.device().oam().iter().all(|&b| b == 0x5a));
        let mut dma = DisplayDmaWork::default();
        while dma.step(&mut time).unwrap().is_some() {}
        assert!(time.device().oam().iter().all(|&b| b == 0));
    }

    #[test]
    fn foreground_clear_batch_matches_plain_instruction_execution() {
        let mut batched = DisplayWorkTime::default();
        batched.begin_native_update();
        batched.set_vblank_dispatch_enabled(true);
        batched.set_shadow_oam(&[0x5a; 160]).unwrap();
        let mut stepped = batched.clone();
        stepped.set_vblank_dispatch_enabled(false);
        let start = batched.elapsed();
        let mut batch_work = ClearShadowOamWork::default();
        let mut step_work = ClearShadowOamWork::default();
        batch_work.run(&mut batched).unwrap();
        step_work.run(&mut stepped).unwrap();
        assert_eq!(batch_work, step_work);
        assert_eq!(batched.elapsed() - start, 3876);
        assert_eq!(stepped.elapsed() - start, 3876);
        assert_eq!(batched.shadow_oam(), stepped.shadow_oam());
        assert_eq!(batched.device(), stepped.device());
        assert!(!batched.active_vblank());
    }
}
