//! VBlank's audio-bank selection and actual child-call boundaries. Only the
//! common ISR owner resumes completed children. Unknown raw banks preserve
//! the source branch, but do not authorize executing Audio3 in a wrong bank.
use super::{
    display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus},
    far_pattern_copy_work::{LOADED_ROM_BANK, ROM_BANK_SELECT},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceAudioEngine {
    One,
    Two,
    Three,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    ReadBank,
    WriteLoadedBank,
    WriteMapper,
    CompareFirst,
    BranchFirst,
    CompareSecond,
    BranchSecond,
    CallAlarm,
    AwaitAlarm,
    CallMusic,
    AwaitMusic,
    JumpAfterMusic,
    Ready,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankAudioDispatchWork {
    bank: u8,
    step: Step,
    engine: Option<SourceAudioEngine>,
    zero: bool,
}
impl VBlankAudioDispatchWork {
    pub fn new(bank: u8) -> Self {
        Self {
            bank,
            step: Step::ReadBank,
            engine: None,
            zero: false,
        }
    }
    pub fn music_ready(&self) -> bool {
        self.step == Step::AwaitMusic
    }
    pub fn alarm_ready(&self) -> bool {
        self.step == Step::AwaitAlarm
    }
    pub fn after_music_ready(&self) -> bool {
        self.step == Step::Ready
    }
    pub fn bank(&self) -> u8 {
        self.bank
    }
    pub fn engine(&self) -> Option<SourceAudioEngine> {
        self.engine
    }
    pub(crate) fn alarm_returned(&mut self) -> bool {
        if !self.alarm_ready() {
            return false;
        }
        self.step = Step::CallMusic;
        true
    }
    pub(crate) fn music_returned(&mut self) -> bool {
        if !self.music_ready() {
            return false;
        }
        self.step = if self.engine == Some(SourceAudioEngine::Three) {
            Step::Ready
        } else {
            Step::JumpAfterMusic
        };
        true
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        let (tcycles, access) = match self.step {
            AwaitAlarm | AwaitMusic | Ready => return Ok(false),
            ReadBank => (16, None),
            WriteLoadedBank => (
                12,
                Some(DisplayAccess::Write {
                    port: LOADED_ROM_BANK,
                    value: self.bank,
                    at_tcycle: 8,
                }),
            ),
            WriteMapper => (
                16,
                Some(DisplayAccess::Write {
                    port: ROM_BANK_SELECT,
                    value: self.bank,
                    at_tcycle: 12,
                }),
            ),
            CompareFirst | CompareSecond => (8, None),
            BranchFirst | BranchSecond => (if self.zero { 8 } else { 12 }, None),
            CallAlarm | CallMusic => (24, None),
            JumpAfterMusic => (12, None),
        };
        bus.execute(DisplayInstruction { tcycles, access })?;
        self.step = match self.step {
            ReadBank => WriteLoadedBank,
            WriteLoadedBank => WriteMapper,
            WriteMapper => CompareFirst,
            CompareFirst => {
                self.zero = self.bank == 2;
                BranchFirst
            }
            BranchFirst => {
                if self.zero {
                    self.engine = Some(SourceAudioEngine::One);
                    CallMusic
                } else {
                    CompareSecond
                }
            }
            CompareSecond => {
                self.zero = self.bank == 8;
                BranchSecond
            }
            BranchSecond => {
                if self.zero {
                    self.engine = Some(SourceAudioEngine::Two);
                    CallAlarm
                } else {
                    self.engine = Some(SourceAudioEngine::Three);
                    CallMusic
                }
            }
            CallAlarm => AwaitAlarm,
            CallMusic => AwaitMusic,
            JumpAfterMusic => Ready,
            AwaitAlarm | AwaitMusic | Ready => unreachable!(),
        };
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        alloc_prelude::*,
        overworld::{
            display_delay_work::DelayFrameBus,
            display_work_time::{DisplayWorkError, DisplayWorkTime},
        },
    };
    #[derive(serde::Deserialize)]
    struct Case {
        bank: u8,
        #[serde(rename = "instruction_T")]
        instructions: Vec<u8>,
        #[serde(rename = "T")]
        cycles: u64,
        awaiting_alarm: bool,
        engine: u8,
    }
    struct Trace<'a> {
        t: &'a mut DisplayWorkTime,
        steps: Vec<u8>,
    }
    impl DisplayInstructionBus for Trace<'_> {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
            let v = self.t.execute(i)?;
            self.steps.push(i.tcycles);
            Ok(v)
        }
    }
    #[test]
    fn original_audio_bank_dispatch_waits_for_actual_children_and_never_returns_irq() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("audio_dispatch_cases_1144.json")).unwrap();
        assert_eq!(cases.len(), 6);
        for c in cases {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            let mut w = VBlankAudioDispatchWork::new(c.bank);
            assert!(!w.alarm_returned());
            assert!(!w.music_returned());
            let mut b = Trace {
                t: &mut t,
                steps: vec![],
            };
            while w.step(&mut b).unwrap() {}
            assert_eq!(b.steps, c.instructions);
            assert_eq!(t.elapsed(), c.cycles);
            assert_eq!(t.source_loaded_rom_bank(), c.bank);
            assert_eq!(t.selected_rom_bank(), c.bank);
            assert_eq!(w.alarm_ready(), c.awaiting_alarm);
            assert_eq!(w.music_ready(), !c.awaiting_alarm);
            assert_eq!(
                w.engine(),
                Some(match c.engine {
                    1 => SourceAudioEngine::One,
                    2 => SourceAudioEngine::Two,
                    _ => SourceAudioEngine::Three,
                })
            );
            assert!(!w.after_music_ready());
            assert_eq!(t.interrupt_returns(), 0);
            if c.awaiting_alarm {
                assert!(!w.music_returned());
                assert!(w.alarm_returned());
                let saved = serde_json::to_string(&(&w, &t)).unwrap();
                let (mut other, mut ot): (VBlankAudioDispatchWork, DisplayWorkTime) =
                    serde_json::from_str(&saved).unwrap();
                t.idle_to_target().unwrap();
                ot.idle_to_target().unwrap();
                let before = w;
                assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
                assert_eq!(w, before);
                t.begin_native_update();
                ot.begin_native_update();
                assert!(w.step(&mut t).unwrap());
                assert!(other.step(&mut ot).unwrap());
                assert_eq!(w, other);
                assert_eq!(t.elapsed(), ot.elapsed());
                assert!(w.music_ready());
            }
            // Continuation protocol, not a claimed callee implementation.
            assert!(w.music_returned());
            while w.step(&mut t).unwrap() {}
            assert!(w.after_music_ready());
            assert!(!w.music_returned());
            assert_eq!(t.interrupt_returns(), 0);
        }
    }
}
