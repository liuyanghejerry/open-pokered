//! FadeOutAudio logical state and instruction work. PlaySound children remain
//! explicit caller boundaries: reaching either entry is never a returned fade
//! or an interrupt return. The native audio owner binds logical stores to its
//! real state; this component neither synthesizes audio nor owns a host tick.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FadeAudioState {
    pub control: u8,
    pub counter: u8,
    pub reload: u8,
    pub flags2: u8,
    pub nr50: u8,
    pub audio_bank: u8,
    pub saved_bank: u8,
    pub new_sound: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    ReadControl,
    TestControl,
    BranchFading,
    ReadFlags,
    BitNoFade,
    ReturnNoFade,
    FullVolume,
    WriteVolume,
    Return,
    ReadCounter,
    TestCounter,
    BranchCounterZero,
    DecrementCounter,
    StoreCounter,
    ReadReload,
    StoreReload,
    ReadVolume,
    TestVolume,
    BranchComplete,
    SaveVolume,
    MaskRight,
    DecrementRight,
    SaveRight,
    RestoreVolume,
    MaskLeft,
    SwapLeft,
    DecrementLeft,
    SwapBack,
    CombineVolume,
    ReadCompleteControl,
    SaveTarget,
    ClearA,
    ClearControl,
    StopAllId,
    StoreStopId,
    CallStopSound,
    AwaitStopSound,
    ReadSavedBank,
    StoreAudioBank,
    RestoreTarget,
    StoreTarget,
    JumpStartSound,
    AwaitStartSound,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FadeAudioWork {
    state: FadeAudioState,
    step: Step,
    a: u8,
    b: u8,
    c: u8,
    zero: bool,
    nr50_written: Option<u8>,
}

impl FadeAudioWork {
    pub fn new(state: FadeAudioState) -> Self {
        Self {
            state,
            step: Step::ReadControl,
            a: 0,
            b: 0,
            c: 0,
            zero: false,
            nr50_written: None,
        }
    }
    pub fn state(&self) -> FadeAudioState {
        self.state
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    /// Actual volume store, including a write equal to its prior value.
    pub fn nr50_write_effect(&self) -> Option<u8> {
        self.nr50_written
    }
    pub fn awaiting_stop_sound(&self) -> bool {
        self.step == Step::AwaitStopSound
    }
    pub fn awaiting_start_sound(&self) -> bool {
        self.step == Step::AwaitStartSound
    }
    /// Only the common owner of the actual PlaySound child may resume this.
    pub(crate) fn stop_sound_returned(&mut self) -> bool {
        if !self.awaiting_stop_sound() {
            return false;
        }
        self.step = Step::ReadSavedBank;
        true
    }
    /// The second call is a tail jump: its real RET also returns FadeOutAudio.
    pub(crate) fn start_sound_returned(&mut self) -> bool {
        if !self.awaiting_start_sound() {
            return false;
        }
        self.step = Step::Done;
        true
    }

    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        let tcycles = match self.step {
            AwaitStopSound | AwaitStartSound | Done => return Ok(false),
            ReadControl | ReadFlags | ReadCounter | StoreCounter | ReadReload | StoreReload
            | Return | ReadCompleteControl | ClearControl | StoreStopId | ReadSavedBank
            | StoreAudioBank | StoreTarget | JumpStartSound => 16,
            BranchFading => {
                if self.zero {
                    8
                } else {
                    12
                }
            }
            BranchCounterZero | BranchComplete => {
                if self.zero {
                    12
                } else {
                    8
                }
            }
            ReturnNoFade => {
                if self.zero {
                    8
                } else {
                    20
                }
            }
            BitNoFade | FullVolume | MaskRight | MaskLeft | SwapLeft | SwapBack | StopAllId => 8,
            WriteVolume | ReadVolume => 12,
            CallStopSound => 24,
            _ => 4,
        };
        bus.execute(DisplayInstruction {
            tcycles,
            access: None,
        })?;
        self.step = match self.step {
            ReadControl => {
                self.a = self.state.control;
                TestControl
            }
            TestControl => {
                self.zero = self.a == 0;
                BranchFading
            }
            BranchFading => {
                if self.zero {
                    ReadFlags
                } else {
                    ReadCounter
                }
            }
            ReadFlags => {
                self.a = self.state.flags2;
                BitNoFade
            }
            BitNoFade => {
                self.zero = self.a & 2 == 0;
                ReturnNoFade
            }
            ReturnNoFade => {
                if self.zero {
                    FullVolume
                } else {
                    Done
                }
            }
            FullVolume => {
                self.a = 0x77;
                WriteVolume
            }
            WriteVolume => {
                self.state.nr50 = self.a;
                self.nr50_written = Some(self.a);
                Return
            }
            Return => Done,
            ReadCounter => {
                self.a = self.state.counter;
                TestCounter
            }
            TestCounter => {
                self.zero = self.a == 0;
                BranchCounterZero
            }
            BranchCounterZero => {
                if self.zero {
                    ReadReload
                } else {
                    DecrementCounter
                }
            }
            DecrementCounter => {
                self.a = self.a.wrapping_sub(1);
                StoreCounter
            }
            StoreCounter => {
                self.state.counter = self.a;
                Return
            }
            ReadReload => {
                self.a = self.state.reload;
                StoreReload
            }
            StoreReload => {
                self.state.counter = self.a;
                ReadVolume
            }
            ReadVolume => {
                self.a = self.state.nr50;
                TestVolume
            }
            TestVolume => {
                self.zero = self.a == 0;
                BranchComplete
            }
            BranchComplete => {
                if self.zero {
                    ReadCompleteControl
                } else {
                    SaveVolume
                }
            }
            SaveVolume => {
                self.b = self.a;
                MaskRight
            }
            MaskRight => {
                self.a &= 15;
                DecrementRight
            }
            DecrementRight => {
                self.a = self.a.wrapping_sub(1);
                SaveRight
            }
            SaveRight => {
                self.c = self.a;
                RestoreVolume
            }
            RestoreVolume => {
                self.a = self.b;
                MaskLeft
            }
            MaskLeft => {
                self.a &= 0xf0;
                SwapLeft
            }
            SwapLeft => {
                self.a = self.a.rotate_left(4);
                DecrementLeft
            }
            DecrementLeft => {
                self.a = self.a.wrapping_sub(1);
                SwapBack
            }
            SwapBack => {
                self.a = self.a.rotate_left(4);
                CombineVolume
            }
            CombineVolume => {
                self.a |= self.c;
                WriteVolume
            }
            ReadCompleteControl => {
                self.a = self.state.control;
                SaveTarget
            }
            SaveTarget => {
                self.b = self.a;
                ClearA
            }
            ClearA => {
                self.a = 0;
                ClearControl
            }
            ClearControl => {
                self.state.control = self.a;
                StopAllId
            }
            StopAllId => {
                self.a = 0xff;
                StoreStopId
            }
            StoreStopId => {
                self.state.new_sound = self.a;
                CallStopSound
            }
            CallStopSound => AwaitStopSound,
            ReadSavedBank => {
                self.a = self.state.saved_bank;
                StoreAudioBank
            }
            StoreAudioBank => {
                self.state.audio_bank = self.a;
                RestoreTarget
            }
            RestoreTarget => {
                self.a = self.b;
                StoreTarget
            }
            StoreTarget => {
                self.state.new_sound = self.a;
                JumpStartSound
            }
            JumpStartSound => AwaitStartSound,
            AwaitStopSound | AwaitStartSound | Done => unreachable!(),
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
        before: FadeAudioState,
        after: FadeAudioState,
        #[serde(rename = "T")]
        cycles: u64,
        #[serde(rename = "instruction_T")]
        instructions: Vec<u8>,
        state_after_each_instruction: Vec<FadeAudioState>,
        nr50_written: bool,
        awaiting_stop_sound: bool,
    }
    struct Trace<'a> {
        time: &'a mut DisplayWorkTime,
        costs: Vec<u8>,
    }
    impl DisplayInstructionBus for Trace<'_> {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
            let v = self.time.execute(i)?;
            self.costs.push(i.tcycles);
            Ok(v)
        }
    }
    fn initial() -> FadeAudioState {
        FadeAudioState {
            control: 38,
            counter: 0,
            reload: 10,
            flags2: 0,
            nr50: 0,
            audio_bank: 31,
            saved_bank: 8,
            new_sound: 0,
        }
    }
    #[test]
    fn original_fade_controls_match_every_instruction_store_and_real_child_boundary() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("fade_audio_cases_1144.json")).unwrap();
        assert_eq!(cases.len(), 277);
        for c in cases {
            let mut t = DisplayWorkTime::default();
            t.begin_native_update();
            let mut w = FadeAudioWork::new(c.before);
            let mut bus = Trace {
                time: &mut t,
                costs: vec![],
            };
            let mut states = vec![];
            while w.step(&mut bus).unwrap() {
                states.push(w.state());
            }
            assert_eq!(bus.costs, c.instructions, "{:?}", c.before);
            assert_eq!(states, c.state_after_each_instruction, "{:?}", c.before);
            assert_eq!(w.state(), c.after);
            assert_eq!(
                w.nr50_write_effect(),
                c.nr50_written.then_some(c.after.nr50)
            );
            assert_eq!(w.awaiting_stop_sound(), c.awaiting_stop_sound);
            assert_eq!(w.finished(), !c.awaiting_stop_sound);
            assert_eq!(t.elapsed(), c.cycles);
            assert_eq!(t.interrupt_returns(), 0);
        }
    }
    #[test]
    fn fade_completion_cannot_skip_either_actual_sound_child_or_invent_a_return() {
        let mut t = DisplayWorkTime::default();
        let mut w = FadeAudioWork::new(initial());
        let before = w.clone();
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
        assert_eq!(w, before);
        assert!(!w.stop_sound_returned());
        assert!(!w.start_sound_returned());
        t.begin_native_update();
        while w.step(&mut t).unwrap() {}
        assert!(w.awaiting_stop_sound());
        assert!(!w.finished());
        assert_eq!(w.state().control, 0);
        assert_eq!(w.state().new_sound, 0xff);
        let saved = serde_json::to_string(&(&w, &t)).unwrap();
        let (mut other, mut ot): (FadeAudioWork, DisplayWorkTime) =
            serde_json::from_str(&saved).unwrap();
        assert!(!w.start_sound_returned());
        // Continuation protocol only: no PlaySound body time is claimed.
        assert!(w.stop_sound_returned());
        assert!(other.stop_sound_returned());
        while w.step(&mut t).unwrap() {}
        while other.step(&mut ot).unwrap() {}
        assert_eq!(w, other);
        assert_eq!(t.elapsed(), ot.elapsed());
        assert_eq!(w.state().audio_bank, 8);
        assert_eq!(w.state().new_sound, 38);
        assert!(w.awaiting_start_sound());
        assert!(!w.finished());
        assert_eq!(t.interrupt_returns(), 0);
        assert!(!w.stop_sound_returned());
        assert!(w.start_sound_returned());
        assert!(w.finished());
        assert!(!w.start_sound_returned());
        assert_eq!(t.interrupt_returns(), 0);
    }
}
