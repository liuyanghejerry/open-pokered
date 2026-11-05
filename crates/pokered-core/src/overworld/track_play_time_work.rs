//! TrackPlayTime and its CountDownIgnoreInputBitReset child. Logical game
//! state is suspended with this work; the common owner applies its resulting
//! fields before returning the ISR. The helper's RET is not TrackPlayTime's
//! RET, and neither return is RETI.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use crate::save::game_data::PlayTime;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TrackTimeState {
    pub ignore_counter: u8,
    pub flags5: u8,
    pub flags6: u8,
    pub joy_pressed: u8,
    pub joy_held: u8,
    pub time: PlayTime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    CallIgnore,
    IgnoreRead,
    IgnoreTest,
    IgnoreBranch,
    IgnoreZero,
    IgnoreJump,
    IgnoreDec,
    IgnoreStore,
    IgnoreAnd,
    IgnoreReturn,
    Flags5Read,
    Res1,
    Res2,
    TestDisabled,
    ResDisabled,
    Flags5Store,
    ReturnNotDisabled,
    ClearA,
    ClearPressed,
    ClearHeld,
    IgnoreRet,
    Flags6Read,
    CountingBit,
    ReturnNotCounting,
    MaxRead,
    MaxTest,
    ReturnMaxed,
    FieldRead(u8),
    Increment(u8),
    FieldStore(u8),
    Compare(u8),
    ReturnUnequal(u8),
    Zero(u8),
    StoreZero(u8),
    MaxValue,
    MaxStore,
    Return,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TrackPlayTimeWork {
    state: TrackTimeState,
    step: Step,
    a: u8,
    zero: bool,
    cleared_pressed: bool,
    cleared_held: bool,
}
impl TrackPlayTimeWork {
    pub fn new(state: TrackTimeState) -> Self {
        Self {
            state,
            step: Step::CallIgnore,
            a: 0,
            zero: false,
            cleared_pressed: false,
            cleared_held: false,
        }
    }
    pub fn state(&self) -> TrackTimeState {
        self.state
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    /// Actual source writes authorize establishing known zero in the native
    /// foreground ledger, even when the diagnostic bytes were already zero.
    /// Unchanged input bytes alone never establish previously unknown history.
    pub fn joypad_clear_effects(&self) -> (bool, bool) {
        (self.cleared_pressed, self.cleared_held)
    }
    fn field(&self, n: u8) -> u8 {
        match n {
            0 => self.state.time.frames,
            1 => self.state.time.seconds,
            2 => self.state.time.minutes,
            3 => self.state.time.hours,
            _ => unreachable!(),
        }
    }
    fn set_field(&mut self, n: u8, v: u8) {
        match n {
            0 => self.state.time.frames = v,
            1 => self.state.time.seconds = v,
            2 => self.state.time.minutes = v,
            3 => self.state.time.hours = v,
            _ => unreachable!(),
        }
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        let tcycles = match self.step {
            Done => return Ok(false),
            CallIgnore => 24,
            IgnoreRead | IgnoreStore | Flags5Read | Flags5Store | Flags6Read | MaxRead
            | FieldRead(_) | FieldStore(_) | StoreZero(_) | MaxStore | IgnoreRet | Return => 16,
            IgnoreBranch => {
                if self.zero {
                    8
                } else {
                    12
                }
            }
            IgnoreJump | ClearPressed | ClearHeld => 12,
            IgnoreZero | Res1 | Res2 | TestDisabled | ResDisabled | CountingBit | Compare(_)
            | MaxValue => 8,
            IgnoreReturn | ReturnMaxed | ReturnUnequal(_) => {
                if self.zero {
                    8
                } else {
                    20
                }
            }
            ReturnNotDisabled | ReturnNotCounting => {
                if self.zero {
                    20
                } else {
                    8
                }
            }
            _ => 4,
        };
        bus.execute(DisplayInstruction {
            tcycles,
            access: None,
        })?;
        self.step = match self.step {
            CallIgnore => IgnoreRead,
            IgnoreRead => {
                self.a = self.state.ignore_counter;
                IgnoreTest
            }
            IgnoreTest => {
                self.zero = self.a == 0;
                IgnoreBranch
            }
            IgnoreBranch => {
                if self.zero {
                    IgnoreZero
                } else {
                    IgnoreDec
                }
            }
            IgnoreZero => {
                self.a = 255;
                IgnoreJump
            }
            IgnoreJump => IgnoreStore,
            IgnoreDec => {
                self.a = self.a.wrapping_sub(1);
                IgnoreStore
            }
            IgnoreStore => {
                self.state.ignore_counter = self.a;
                IgnoreAnd
            }
            IgnoreAnd => {
                self.zero = self.a == 0;
                IgnoreReturn
            }
            IgnoreReturn => {
                if self.zero {
                    Flags5Read
                } else {
                    Flags6Read
                }
            }
            Flags5Read => {
                self.a = self.state.flags5;
                Res1
            }
            Res1 => {
                self.a &= !2;
                Res2
            }
            Res2 => {
                self.a &= !4;
                TestDisabled
            }
            TestDisabled => {
                self.zero = self.a & 32 == 0;
                ResDisabled
            }
            ResDisabled => {
                self.a &= !32;
                Flags5Store
            }
            Flags5Store => {
                self.state.flags5 = self.a;
                ReturnNotDisabled
            }
            ReturnNotDisabled => {
                if self.zero {
                    Flags6Read
                } else {
                    ClearA
                }
            }
            ClearA => {
                self.a = 0;
                ClearPressed
            }
            ClearPressed => {
                self.state.joy_pressed = self.a;
                self.cleared_pressed = true;
                ClearHeld
            }
            ClearHeld => {
                self.state.joy_held = self.a;
                self.cleared_held = true;
                IgnoreRet
            }
            IgnoreRet => Flags6Read,
            Flags6Read => {
                self.a = self.state.flags6;
                CountingBit
            }
            CountingBit => {
                self.zero = self.a & 1 == 0;
                ReturnNotCounting
            }
            ReturnNotCounting => {
                if self.zero {
                    Done
                } else {
                    MaxRead
                }
            }
            MaxRead => {
                self.a = if self.state.time.maxed { 255 } else { 0 };
                MaxTest
            }
            MaxTest => {
                self.zero = self.a == 0;
                ReturnMaxed
            }
            ReturnMaxed => {
                if self.zero {
                    FieldRead(0)
                } else {
                    Done
                }
            }
            FieldRead(n) => {
                self.a = self.field(n);
                Increment(n)
            }
            Increment(n) => {
                self.a = self.a.wrapping_add(1);
                FieldStore(n)
            }
            FieldStore(n) => {
                self.set_field(n, self.a);
                Compare(n)
            }
            Compare(n) => {
                self.zero = self.a == if n == 3 { 255 } else { 60 };
                ReturnUnequal(n)
            }
            ReturnUnequal(n) => {
                if !self.zero {
                    Done
                } else if n == 3 {
                    MaxValue
                } else {
                    Zero(n)
                }
            }
            Zero(n) => {
                self.a = 0;
                StoreZero(n)
            }
            StoreZero(n) => {
                self.set_field(n, self.a);
                FieldRead(n + 1)
            }
            MaxValue => {
                self.a = 255;
                MaxStore
            }
            MaxStore => {
                self.state.time.maxed = self.a != 0;
                Return
            }
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
        before: TrackTimeState,
        wrote_pressed: bool,
        wrote_held: bool,
        after: TrackTimeState,
        #[serde(rename = "T")]
        cycles: u64,
        #[serde(rename = "instruction_T")]
        instructions: Vec<u8>,
        state_after_each_instruction: Vec<TrackTimeState>,
    }
    struct Trace<'a> {
        time: &'a mut DisplayWorkTime,
        cycles: Vec<u8>,
    }
    impl DisplayInstructionBus for Trace<'_> {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
            let v = self.time.execute(i)?;
            self.cycles.push(i.tcycles);
            Ok(v)
        }
    }
    fn actual_state() -> TrackTimeState {
        TrackTimeState {
            ignore_counter: 189,
            flags5: 0,
            flags6: 3,
            joy_pressed: 17,
            joy_held: 129,
            time: PlayTime {
                hours: 0,
                maxed: false,
                minutes: 4,
                seconds: 6,
                frames: 54,
            },
        }
    }
    #[test]
    fn original_track_time_controls_match_every_instruction_and_logical_store() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("track_time_cases_1141.json")).unwrap();
        assert_eq!(cases.len(), 74);
        for c in cases {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            let mut w = TrackPlayTimeWork::new(c.before);
            let mut bus = Trace {
                time: &mut t,
                cycles: vec![],
            };
            let mut states = vec![];
            while w.step(&mut bus).unwrap() {
                states.push(w.state());
            }
            assert!(w.finished());
            assert_eq!(bus.cycles, c.instructions, "input {:?}", c.before);
            assert_eq!(
                states, c.state_after_each_instruction,
                "input {:?}",
                c.before
            );
            assert_eq!(w.state(), c.after);
            assert_eq!(w.joypad_clear_effects(), (c.wrote_pressed, c.wrote_held));
            assert_eq!(t.elapsed(), c.cycles);
            assert_eq!(t.interrupt_returns(), 0);
        }
    }
    #[test]
    fn ignore_helper_return_does_not_return_track_time_or_skip_the_real_clock_instruction() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        let mut w = TrackPlayTimeWork::new(actual_state());
        while w.step != Step::Flags6Read {
            assert!(w.step(&mut t).unwrap());
        }
        assert_eq!(t.elapsed(), 100);
        assert!(!w.finished());
        assert_eq!(w.state().ignore_counter, 188);
        assert_eq!(w.state().time.frames, 54);
        while w.step(&mut t).unwrap() {}
        assert_eq!(t.elapsed(), 224);
        assert_eq!(w.state().time.frames, 55);
        assert_eq!(t.interrupt_returns(), 0);
    }
    #[test]
    fn paused_timer_still_expires_ignore_flags_and_clears_input_across_budget_and_snapshot() {
        let mut input = actual_state();
        input.flags6 = 0;
        input.flags5 = 255;
        input.ignore_counter = 1;
        let mut w = TrackPlayTimeWork::new(input);
        let mut t = DisplayWorkTime::default();
        let prior = w.clone();
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
        assert_eq!(w, prior);
        t.begin_native_update();
        while w.step != Step::ClearHeld {
            assert!(w.step(&mut t).unwrap());
        }
        assert_eq!(w.state().flags5, 255 & !0x26);
        assert_eq!(w.state().joy_pressed, 0);
        assert_eq!(w.state().joy_held, 129);
        assert_eq!(w.joypad_clear_effects(), (true, false));
        assert!(!w.finished());
        let saved = serde_json::to_string(&(&w, &t)).unwrap();
        let (mut other, mut ot): (TrackPlayTimeWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        t.idle_to_target().unwrap();
        ot.idle_to_target().unwrap();
        let before = w.clone();
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
        assert_eq!(w, before);
        t.begin_native_update();
        ot.begin_native_update();
        while w.step(&mut t).unwrap() {}
        while other.step(&mut ot).unwrap() {}
        assert_eq!(w, other);
        assert_eq!(t.elapsed(), ot.elapsed());
        assert_eq!(w.state().ignore_counter, 0);
        assert_eq!(w.state().joy_held, 0);
        assert_eq!(w.joypad_clear_effects(), (true, true));
        assert_eq!(w.state().time, input.time);
        assert_eq!(t.interrupt_returns(), 0);
    }
}
