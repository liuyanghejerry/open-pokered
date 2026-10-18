//! Inline ld a,1 / ld [wUpdateSpritesEnabled],a in map restoration.
//! No implicit CALL/RET, host-frame delay or interrupt-return receipt.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
pub const SPRITE_UPDATES_PORT: u16 = 0xcfcb;
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EnableSpriteUpdatesWork {
    next: u8,
}
impl EnableSpriteUpdatesWork {
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        let instruction = match self.next {
            0 => DisplayInstruction {
                tcycles: 8,
                access: None,
            },
            1 => DisplayInstruction {
                tcycles: 16,
                access: Some(DisplayAccess::Write {
                    port: SPRITE_UPDATES_PORT,
                    value: 1,
                    at_tcycle: 12,
                }),
            },
            _ => return Ok(false),
        };
        bus.execute(instruction)?;
        self.next += 1;
        Ok(true)
    }
}
