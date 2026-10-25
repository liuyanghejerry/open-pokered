//! ReadJoypad from home/joypad.asm. Every repeated JOYP read samples the live
//! input bus; only the last read in each group supplies its four result bits.
//! The common owner separately decides hDisableJoypadPolling and CALL/RETI.
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    #[default]
    DirectionValue,
    ClearC,
    SelectDirections,
    Directions(u8),
    InvertDirections,
    MaskDirections,
    SwapDirections,
    SaveDirections,
    ButtonValue,
    SelectButtons,
    Buttons(u8),
    InvertButtons,
    MaskButtons,
    Combine,
    StoreInput,
    DeselectValue,
    Deselect,
    Return,
    Done,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReadJoypadWork {
    step: Step,
    a: u8,
    b: u8,
}
impl ReadJoypadWork {
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        let (tcycles, access) = match self.step {
            Done => return Ok(false),
            DirectionValue | ClearC | ButtonValue | MaskDirections | MaskButtons
            | SwapDirections | DeselectValue => (8, None),
            SelectDirections | SelectButtons | Deselect => (
                12,
                Some(DisplayAccess::Write {
                    port: 0xff00,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            Directions(_) | Buttons(_) => (
                12,
                Some(DisplayAccess::Read {
                    port: 0xff00,
                    at_tcycle: 8,
                }),
            ),
            StoreInput => (
                12,
                Some(DisplayAccess::Write {
                    port: 0xfff8,
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            Return => (16, None),
            _ => (4, None),
        };
        let value = bus.execute(DisplayInstruction { tcycles, access })?;
        self.step = match self.step {
            DirectionValue => {
                self.a = 0x20;
                ClearC
            }
            ClearC => SelectDirections,
            SelectDirections => Directions(0),
            Directions(n) => {
                self.a = value.expect("directional JOYP sample");
                if n == 5 {
                    InvertDirections
                } else {
                    Directions(n + 1)
                }
            }
            InvertDirections => {
                self.a = !self.a;
                MaskDirections
            }
            MaskDirections => {
                self.a &= 15;
                SwapDirections
            }
            SwapDirections => {
                self.a = self.a.rotate_left(4);
                SaveDirections
            }
            SaveDirections => {
                self.b = self.a;
                ButtonValue
            }
            ButtonValue => {
                self.a = 0x10;
                SelectButtons
            }
            SelectButtons => Buttons(0),
            Buttons(n) => {
                self.a = value.expect("action JOYP sample");
                if n == 9 {
                    InvertButtons
                } else {
                    Buttons(n + 1)
                }
            }
            InvertButtons => {
                self.a = !self.a;
                MaskButtons
            }
            MaskButtons => {
                self.a &= 15;
                Combine
            }
            Combine => {
                self.a |= self.b;
                StoreInput
            }
            StoreInput => DeselectValue,
            DeselectValue => {
                self.a = 0x30;
                Deselect
            }
            Deselect => Return,
            Return => Done,
            Done => Done,
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        display_delay_work::DelayFrameBus,
        display_work_time::{DisplayWorkError, DisplayWorkTime},
    };
    use super::*;
    use crate::alloc_prelude::*;
    #[derive(serde::Deserialize)]
    struct Case {
        initial: u8,
        changes: Vec<(u64, u8)>,
        output: u8,
        #[serde(rename = "T")]
        cycles: u64,
        #[serde(rename = "instruction_T")]
        instructions: Vec<u8>,
        reads: Vec<(u8, u64)>,
        writes: Vec<(u16, u8, u64)>,
    }
    struct Trace<'a> {
        time: &'a mut DisplayWorkTime,
        case: &'a Case,
        steps: Vec<u8>,
        reads: Vec<(u8, u64)>,
        writes: Vec<(u16, u8, u64)>,
    }
    impl DisplayInstructionBus for Trace<'_> {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
            let before = self.time.elapsed();
            if let Some(DisplayAccess::Read {
                port: 0xff00,
                at_tcycle,
            }) = i.access
            {
                // Synthetic external input is chosen for this atomic read's
                // access point. Electrical edges/joypad IRQs are not modeled.
                let at = before + u64::from(at_tcycle);
                let mut held = self.case.initial;
                for &(t, k) in &self.case.changes {
                    if t <= at {
                        held = k;
                    }
                }
                self.time.set_physical_buttons(held);
            }
            let v = self.time.execute(i)?;
            self.steps.push(i.tcycles);
            match i.access {
                Some(DisplayAccess::Read {
                    port: 0xff00,
                    at_tcycle,
                }) => self.reads.push((v.unwrap(), before + u64::from(at_tcycle))),
                Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle,
                }) => self
                    .writes
                    .push((port, value, before + u64::from(at_tcycle))),
                _ => (),
            }
            Ok(v)
        }
    }
    #[test]
    fn original_readjoypad_instructions_match_all_virtual_masks_and_changed_final_samples() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("joypad_cases_1141.json")).unwrap();
        assert_eq!(cases.len(), 259);
        for c in cases {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            t.set_physical_buttons(c.initial);
            assert_eq!(t.source_joy_input(), 0);
            let mut w = ReadJoypadWork::default();
            let mut bus = Trace {
                time: &mut t,
                case: &c,
                steps: vec![],
                reads: vec![],
                writes: vec![],
            };
            while w.step(&mut bus).unwrap() {}
            assert!(w.finished());
            assert_eq!(bus.steps, c.instructions);
            assert_eq!(bus.reads, c.reads);
            assert_eq!(bus.writes, c.writes);
            assert_eq!(t.source_joy_input(), c.output);
            assert_eq!(t.elapsed(), c.cycles);
            assert_eq!(t.interrupt_returns(), 0);
            let low = t
                .execute(DisplayInstruction {
                    tcycles: 12,
                    access: Some(DisplayAccess::Read {
                        port: 0xff00,
                        at_tcycle: 8,
                    }),
                })
                .unwrap()
                .unwrap()
                & 15;
            assert_eq!(low, 15, "function deselects both rows before RET");
        }
    }
    #[test]
    fn mid_poll_snapshot_and_budget_keep_latched_direction_but_take_live_button_row() {
        let mut t = DisplayWorkTime::default();
        let mut w = ReadJoypadWork::default();
        let prior = w;
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
        assert_eq!(w, prior);
        t.begin_native_update();
        t.set_physical_buttons(0x10);
        while w.step != Step::Directions(5) {
            assert!(w.step(&mut t).unwrap());
        }
        t.set_physical_buttons(0x80);
        w.step(&mut t).unwrap();
        t.set_physical_buttons(0x11);
        while w.step != Step::Buttons(9) {
            assert!(w.step(&mut t).unwrap());
        }
        assert_eq!(t.source_joy_input(), 0);
        let snapshot = serde_json::to_string(&(&w, &t)).unwrap();
        let (mut other, mut ot): (ReadJoypadWork, DisplayWorkTime) =
            serde_json::from_str(&snapshot).unwrap();
        t.idle_to_target().unwrap();
        ot.idle_to_target().unwrap();
        let prior = w;
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
        assert_eq!(w, prior);
        t.begin_native_update();
        ot.begin_native_update();
        t.set_physical_buttons(0x42);
        ot.set_physical_buttons(0x42);
        while w.step(&mut t).unwrap() {}
        while other.step(&mut ot).unwrap() {}
        assert_eq!(w, other);
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            serde_json::to_value(&ot).unwrap()
        );
        assert_eq!(t.source_joy_input(), 0x82);
        assert_eq!(t.interrupt_returns(), 0);
    }
    #[test]
    fn irq_raw_refresh_does_not_execute_a_foreground_joypad_poll() {
        use crate::foreground_input::ForegroundJoypad;
        let mut ledger = ForegroundJoypad::default();
        ledger.read_raw(0);
        ledger.poll();
        let last = ledger.last();
        let pressed = ledger.pressed();
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.set_physical_buttons(0x11);
        assert_eq!(t.elapsed(), 0);
        let mut work = ReadJoypadWork::default();
        while work.step(&mut t).unwrap() {}
        assert_eq!(ledger.last(), last);
        assert_eq!(ledger.pressed(), pressed);
        assert_eq!(ledger.raw(), 0);
        ledger.read_raw(t.source_joy_input());
        assert_eq!(ledger.last(), last);
        assert_eq!(ledger.pressed(), pressed);
        assert_eq!(ledger.poll().pressed, Some(0x11));
    }
}
