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
        assert!(time.device.oam().iter().all(|&b| b == 0x5a));
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
            let before = clear;
            match clear.step(&mut time) {
                Ok(Some(_)) => (),
                Err(DisplayWorkError::Budget) => {
                    assert_eq!(clear, before);
                    break;
                }
                other => panic!("unexpected clear result {other:?}"),
            }
        }
        assert_eq!(time.shadow_oam()[0], 0);
        assert_eq!(time.shadow_oam()[1], 0x5a);
        assert!(time.device.oam().iter().all(|&b| b == 0x5a));
        let state = serde_json::to_string(&(clear, &time)).unwrap();
        let (mut other_clear, mut other_time): (ClearShadowOamWork, DisplayWorkTime) =
            serde_json::from_str(&state).unwrap();
        time.begin_native_update();
        other_time.begin_native_update();
        while clear.step(&mut time).unwrap().is_some() {}
        while other_clear.step(&mut other_time).unwrap().is_some() {}
        assert_eq!(time.elapsed() - start, 3876);
        assert!(clear.finished());
        assert_eq!(clear, other_clear);
        assert_eq!(time.shadow_oam(), other_time.shadow_oam());
        assert!(time.shadow_oam().iter().all(|&b| b == 0));
        assert!(time.device.oam().iter().all(|&b| b == 0x5a));
        let mut dma = DisplayDmaWork::default();
        while dma.step(&mut time).unwrap().is_some() {}
        assert!(time.device.oam().iter().all(|&b| b == 0));
    }
}
