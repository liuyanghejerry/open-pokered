//! ReloadMapSpriteTilePatterns' farcall InitMapSprites, through Bankswitch.
//! The body is a real boundary. Returning restores the bank before EnableLCD.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::far_pattern_copy_work::{LOADED_ROM_BANK, ROM_BANK_SELECT};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpriteBankCallWork {
    step: u8,
    a: u8,
    saved: u8,
    #[serde(default = "sprite_bank")]
    target_bank: u8,
}
fn sprite_bank() -> u8 {
    5
}
impl Default for SpriteBankCallWork {
    fn default() -> Self {
        Self {
            step: 0,
            a: 0,
            saved: 0,
            target_bank: sprite_bank(),
        }
    }
}
impl SpriteBankCallWork {
    /// The same farcall/Bankswitch instruction sequence also wraps Random_
    /// and TrackPlayTime. The suspended native continuation owns registers.
    pub fn for_bank(target_bank: u8) -> Self {
        Self {
            target_bank,
            ..Self::default()
        }
    }
    pub fn finished(&self) -> bool {
        self.step == 17
    }
    pub fn body_returned(&mut self) -> bool {
        if self.step != 11 {
            return false;
        }
        self.step = 12;
        true
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        let (t, access) = match self.step {
            0 => (8, None),
            1 | 8 => (12, None),
            2 => (24, None),
            3 => (
                12,
                Some(DisplayAccess::Read {
                    port: LOADED_ROM_BANK,
                    at_tcycle: 8,
                }),
            ),
            4 | 9 => (16, None),
            5 | 10 | 13 => (4, None),
            6 | 14 => (
                12,
                Some(DisplayAccess::Write {
                    port: LOADED_ROM_BANK,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            7 | 15 => (
                16,
                Some(DisplayAccess::Write {
                    port: ROM_BANK_SELECT,
                    value: self.a,
                    at_tcycle: 12,
                }),
            ),
            12 => (12, None),
            16 => (16, None),
            11 | 17 => return Ok(false),
            _ => unreachable!(),
        };
        let v = bus.execute(DisplayInstruction { tcycles: t, access })?;
        match self.step {
            3 => self.a = v.expect("loaded bank read"),
            4 => self.saved = self.a,
            5 => self.a = self.target_bank,
            13 => self.a = self.saved,
            _ => (),
        };
        self.step += 1;
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_work_time::DisplayWorkTime;
    #[test]
    fn actual_bank_call_has_body_boundary_and_restores_before_return() {
        let mut w = SpriteBankCallWork::default();
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        while w.step(&mut bus).unwrap() {}
        assert_eq!(bus.elapsed(), 136);
        assert_eq!(bus.source_loaded_rom_bank(), 5);
        assert_eq!(bus.selected_rom_bank(), 5);
        assert!(w.body_returned());
        while w.step(&mut bus).unwrap() {}
        assert!(w.finished());
        assert_eq!(bus.elapsed(), 196);
        assert_eq!(bus.source_loaded_rom_bank(), 1);
        assert_eq!(bus.selected_rom_bank(), 1);
        assert!(!w.body_returned());
    }
}
