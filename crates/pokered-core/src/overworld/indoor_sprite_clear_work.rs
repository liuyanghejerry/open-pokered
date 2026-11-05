//! The sixteen-slot zeroStoredPictureIDLoop and InitMapSprites RET.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndoorSpriteClearWork {
    step: u8,
    a: u8,
    b: u8,
    l: u8,
    pictures: [u8; 16],
}
impl IndoorSpriteClearWork {
    pub fn new(raw: &[u8]) -> Option<Self> {
        if raw.is_empty() || raw.len() > 15 {
            return None;
        }
        let mut pictures = [0; 16];
        pictures[0] = 1;
        pictures[1..raw.len() + 1].copy_from_slice(raw);
        Some(Self {
            step: 0,
            a: 0,
            b: 0,
            l: 0,
            pictures,
        })
    }
    pub fn pictures(&self) -> Option<&[u8; 16]> {
        (self.step == 10).then_some(&self.pictures)
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        let t = match self.step {
            0 => 12,
            1 | 3 | 4 => 8,
            8 => {
                if self.b != 0 {
                    12
                } else {
                    8
                }
            }
            9 => 16,
            10 => return Ok(false),
            _ => 4,
        };
        bus.execute(DisplayInstruction {
            tcycles: t,
            access: None,
        })?;
        self.step = match self.step {
            0 => {
                self.l = 0x0d;
                1
            }
            1 => {
                self.b = 16;
                2
            }
            2 => {
                self.a = 0;
                3
            }
            3 => {
                self.pictures[usize::from(self.l >> 4)] = self.a;
                4
            }
            4 => {
                self.a = 16;
                5
            }
            5 => {
                self.a = self.a.wrapping_add(self.l);
                6
            }
            6 => {
                self.l = self.a;
                7
            }
            7 => {
                self.b = self.b.wrapping_sub(1);
                8
            }
            8 => {
                if self.b != 0 {
                    2
                } else {
                    9
                }
            }
            9 => 10,
            _ => unreachable!(),
        };
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::display_work_time::DisplayWorkTime;
    #[test]
    fn all_sixteen_picture_ids_are_cleared_before_real_return() {
        let mut w = IndoorSpriteClearWork::new(&[14, 14, 14, 12, 6, 6, 6]).unwrap();
        let mut bus = DisplayWorkTime::default();
        bus.begin_native_update();
        while w.step(&mut bus).unwrap() {}
        assert_eq!(bus.elapsed(), 736);
        assert_eq!(w.pictures(), Some(&[0; 16]));
        assert!(IndoorSpriteClearWork::new(&[]).is_none());
    }
}
