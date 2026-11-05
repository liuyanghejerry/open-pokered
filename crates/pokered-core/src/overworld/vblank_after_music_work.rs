//! VBlank.afterMusic through RETI. The common ISR owner may enter here only
//! after its actual audio child returns. This does not model or replace music,
//! nor does creating this continuation authorize a foreground resume.
use super::{
    display_lcd_work::{DisplayAccess, DisplayInstruction},
    read_joypad_work::ReadJoypadWork,
    sprite_bank_call_work::SpriteBankCallWork,
    track_play_time_work::{TrackPlayTimeWork, TrackTimeState},
    vblank_return_work::{VBlankReturnBus, VBlankReturnWork},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    EnterTimer,
    Timer,
    LeaveTimer,
    ReadPollingGate,
    TestPollingGate,
    CallJoypad,
    Joypad,
    Return,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankAfterMusicWork {
    step: Step,
    bank: SpriteBankCallWork,
    timer: TrackPlayTimeWork,
    joypad: ReadJoypadWork,
    epilogue: VBlankReturnWork,
    polling_disabled: u8,
    zero: bool,
}

impl VBlankAfterMusicWork {
    pub(crate) fn new(state: TrackTimeState, saved_bank: u8) -> Self {
        Self {
            step: Step::EnterTimer,
            bank: SpriteBankCallWork::for_bank(6),
            timer: TrackPlayTimeWork::new(state),
            joypad: ReadJoypadWork::default(),
            epilogue: VBlankReturnWork::new(saved_bank),
            polling_disabled: 0,
            zero: false,
        }
    }

    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }

    pub fn timer_state(&self) -> TrackTimeState {
        self.timer.state()
    }

    pub fn joypad_clear_effects(&self) -> (bool, bool) {
        self.timer.joypad_clear_effects()
    }

    pub fn step<B: VBlankReturnBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        loop {
            if self.finished() {
                return Ok(false);
            }
            bus.check_active_vblank()?;
            match self.step {
                Step::EnterTimer | Step::LeaveTimer => {
                    if self.bank.step(bus)? {
                        return Ok(true);
                    }
                    self.step = if self.bank.finished() {
                        Step::ReadPollingGate
                    } else {
                        Step::Timer
                    };
                }
                Step::Timer => {
                    if self.timer.step(bus)? {
                        return Ok(true);
                    }
                    assert!(self.bank.body_returned());
                    self.step = Step::LeaveTimer;
                }
                Step::ReadPollingGate => {
                    self.polling_disabled = bus
                        .execute(DisplayInstruction {
                            tcycles: 12,
                            access: Some(DisplayAccess::Read {
                                port: 0xfff9,
                                at_tcycle: 8,
                            }),
                        })?
                        .expect("live hDisableJoypadPolling");
                    self.step = Step::TestPollingGate;
                    return Ok(true);
                }
                Step::TestPollingGate => {
                    bus.execute(DisplayInstruction {
                        tcycles: 4,
                        access: None,
                    })?;
                    self.zero = self.polling_disabled == 0;
                    self.step = Step::CallJoypad;
                    return Ok(true);
                }
                Step::CallJoypad => {
                    bus.execute(DisplayInstruction {
                        tcycles: if self.zero { 24 } else { 12 },
                        access: None,
                    })?;
                    self.step = if self.zero {
                        Step::Joypad
                    } else {
                        Step::Return
                    };
                    return Ok(true);
                }
                Step::Joypad => {
                    if self.joypad.step(bus)? {
                        return Ok(true);
                    }
                    self.step = Step::Return;
                }
                Step::Return => {
                    let executed = self.epilogue.step(bus)?;
                    if self.epilogue.finished() {
                        self.step = Step::Done;
                    }
                    return Ok(executed);
                }
                Step::Done => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::{
        display_delay_work::DelayFrameBus,
        display_lcd_work::DisplayInstructionBus,
        display_work_time::{DisplayWorkError, DisplayWorkTime},
    };

    #[derive(serde::Deserialize)]
    struct OriginalCase {
        timer_input: TrackTimeState,
        loaded_bank_at_entry: u8,
        saved_bank: u8,
        #[serde(rename = "total_T")]
        total_cycles: u64,
    }
    fn original() -> OriginalCase {
        serde_json::from_str(include_str!("vblank_after_music_case_1142.json")).unwrap()
    }
    fn state() -> TrackTimeState {
        original().timer_input
    }
    fn active() -> DisplayWorkTime {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        t.idle_to_target().unwrap();
        t.begin_native_update();
        t.enter_pending_vblank().unwrap();
        t
    }

    #[test]
    fn original_observed_tail_896t_does_not_resume_on_timer_or_joypad_ret() {
        let mut t = active();
        let case = original();
        for port in [
            super::super::far_pattern_copy_work::LOADED_ROM_BANK,
            super::super::far_pattern_copy_work::ROM_BANK_SELECT,
        ] {
            t.execute(DisplayInstruction {
                tcycles: 16,
                access: Some(DisplayAccess::Write {
                    port,
                    value: case.loaded_bank_at_entry,
                    at_tcycle: 12,
                }),
            })
            .unwrap();
        }
        // Synthetic buttons exercise raw sampling; ReadJoypad's instruction
        // path is constant for all masks. Clock/flags/banks are source inputs.
        t.set_physical_buttons(0x81);
        let start = t.elapsed();
        let mut w = VBlankAfterMusicWork::new(case.timer_input, case.saved_bank);
        while w.step != Step::TestPollingGate {
            assert!(w.step(&mut t).unwrap());
        }
        assert_eq!(t.elapsed() - start, 432);
        assert_eq!(w.timer_state().ignore_counter, 188);
        assert_eq!(w.timer_state().time.frames, 55);
        assert_eq!(w.joypad_clear_effects(), (false, false));
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        while w.step != Step::Return {
            assert!(w.step(&mut t).unwrap());
        }
        assert_eq!(t.elapsed() - start, 804);
        assert_eq!(t.source_joy_input(), 0x81);
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        while w.step(&mut t).unwrap() {}
        assert!(w.finished());
        assert_eq!(t.elapsed() - start, case.total_cycles);
        assert_eq!(t.source_loaded_rom_bank(), 4);
        assert_eq!(t.selected_rom_bank(), 4);
        assert_eq!(t.interrupt_returns(), 1);
        assert!(!t.active_vblank());
        assert!(!w.step(&mut t).unwrap());
        assert_eq!(t.interrupt_returns(), 1);
    }

    #[test]
    fn nonzero_live_poll_gate_skips_raw_sampling_and_tail_resumes_from_snapshot() {
        for gate in [1, 128, 255] {
            let mut t = active();
            t.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xfff8,
                    value: 0x12,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
            t.set_physical_buttons(0x81);
            let mut w = VBlankAfterMusicWork::new(state(), 4);
            while !(w.step == Step::LeaveTimer && w.bank.finished()) {
                assert!(w.step(&mut t).unwrap());
            }
            // The gate is changed AFTER the timer child/bank wrapper return;
            // the owner must read the live port rather than a constructor copy.
            t.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port: 0xfff9,
                    value: gate,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
            w.step(&mut t).unwrap();
            assert_eq!(w.step, Step::TestPollingGate);
            let json = serde_json::to_string(&(&w, &t)).unwrap();
            let (mut other, mut ot): (VBlankAfterMusicWork, DisplayWorkTime) =
                serde_json::from_str(&json).unwrap();
            t.idle_to_target().unwrap();
            ot.idle_to_target().unwrap();
            let prior = w.clone();
            assert_eq!(w.step(&mut t), Err(DisplayWorkError::Budget));
            assert_eq!(w, prior);
            assert_eq!(t.interrupt_returns(), 0);
            t.begin_native_update();
            ot.begin_native_update();
            let start = t.elapsed();
            while w.step(&mut t).unwrap() {}
            while other.step(&mut ot).unwrap() {}
            assert_eq!(w, other);
            assert_eq!(t.elapsed(), ot.elapsed());
            assert_eq!(t.elapsed() - start, 4 + 12 + 108);
            assert_eq!(t.source_joy_input(), 0x12);
            assert_eq!(t.interrupt_returns(), 1);
            assert_eq!(t.source_loaded_rom_bank(), 4);
        }
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        let mut w = VBlankAfterMusicWork::new(state(), 4);
        let before = w.clone();
        assert_eq!(w.step(&mut t), Err(DisplayWorkError::NoActiveVBlank));
        assert_eq!(w, before);
        assert_eq!(t.elapsed(), 0);
    }
}
