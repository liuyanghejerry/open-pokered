//! VBlank's CALL Random, wait-flag clear and frame countdown through CALL
//! FadeOutAudio. AudioReady is still inside the interrupt, never RETI.
use super::{
    display_delay_work::VBLANK_FLAG_PORT,
    display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus},
    sprite_bank_call_work::SpriteBankCallWork,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrqRandomState {
    pub add: u8,
    pub sub: u8,
    /// CPU carry at Random's return, not a persistent RNG byte. The next
    /// caller supplies its actual entry carry, rather than reusing this.
    pub carry: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum RandomStep {
    Push(u8),
    EnterBank,
    Leaf(u8),
    ReturnBank,
    Tail(u8),
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IrqRandomWork {
    step: RandomStep,
    state: IrqRandomState,
    bank: SpriteBankCallWork,
    a: u8,
    b: u8,
}
impl IrqRandomWork {
    pub fn new(carry: bool) -> Self {
        Self {
            step: RandomStep::Push(0),
            state: IrqRandomState {
                add: 0,
                sub: 0,
                carry,
            },
            bank: SpriteBankCallWork::for_bank(4),
            a: 0,
            b: 0,
        }
    }
    pub fn state(&self) -> IrqRandomState {
        self.state
    }
    pub fn finished(&self) -> bool {
        self.step == RandomStep::Done
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use RandomStep::*;
        loop {
            match self.step {
                Done => return Ok(false),
                EnterBank => {
                    if self.bank.step(bus)? {
                        return Ok(true);
                    }
                    self.step = Leaf(0);
                    continue;
                }
                ReturnBank => {
                    if self.bank.step(bus)? {
                        return Ok(true);
                    }
                    self.step = Tail(0);
                    continue;
                }
                _ => (),
            }
            let (tcycles, access) = match self.step {
                Push(_) => (16, None),
                Leaf(0 | 5) => (
                    12,
                    Some(DisplayAccess::Read {
                        port: 0xff04,
                        at_tcycle: 8,
                    }),
                ),
                Leaf(1 | 3 | 6 | 8) => (4, None),
                Leaf(2 | 7) | Tail(0) => (
                    12,
                    Some(DisplayAccess::Read {
                        port: if self.step == Leaf(7) { 0xffd4 } else { 0xffd3 },
                        at_tcycle: 8,
                    }),
                ),
                Leaf(4 | 9) => (
                    12,
                    Some(DisplayAccess::Write {
                        port: if self.step == Leaf(4) { 0xffd3 } else { 0xffd4 },
                        value: self.a,
                        at_tcycle: 8,
                    }),
                ),
                Leaf(10) => (16, None),
                Tail(1 | 2 | 3) => (12, None),
                Tail(4) => (16, None),
                _ => unreachable!(),
            };
            let read = bus.execute(DisplayInstruction { tcycles, access })?;
            self.step = match self.step {
                Push(2) => EnterBank,
                Push(n) => Push(n + 1),
                Leaf(n) => {
                    match n {
                        0 | 5 => self.a = read.expect("DIV receipt"),
                        1 | 6 => self.b = self.a,
                        2 => self.a = read.expect("random add receipt"),
                        3 => {
                            let sum =
                                u16::from(self.a) + u16::from(self.b) + u16::from(self.state.carry);
                            self.a = sum as u8;
                            self.state.carry = sum > 255;
                        }
                        4 => self.state.add = self.a,
                        7 => self.a = read.expect("random sub receipt"),
                        8 => {
                            let rhs = u16::from(self.b) + u16::from(self.state.carry);
                            self.state.carry = u16::from(self.a) < rhs;
                            self.a = self.a.wrapping_sub(rhs as u8);
                        }
                        9 => self.state.sub = self.a,
                        10 => {
                            assert!(self.bank.body_returned());
                        }
                        _ => unreachable!(),
                    }
                    if n == 10 {
                        ReturnBank
                    } else {
                        Leaf(n + 1)
                    }
                }
                Tail(4) => Done,
                Tail(0) => {
                    self.a = read.expect("Random return value");
                    Tail(1)
                }
                Tail(n) => Tail(n + 1),
                _ => unreachable!(),
            };
            return Ok(true);
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    CallRandom,
    Random,
    ReadFlag,
    TestFlag,
    SkipZero,
    Zero,
    ClearFlag,
    ReadCounter,
    TestCounter,
    SkipCounter,
    Decrement,
    StoreCounter,
    CallFade,
    AudioReady,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VBlankPreAudioWork {
    step: Step,
    random: IrqRandomWork,
    a: u8,
}
impl VBlankPreAudioWork {
    pub fn new(carry: bool) -> Self {
        Self {
            step: Step::CallRandom,
            random: IrqRandomWork::new(carry),
            a: 0,
        }
    }
    pub fn audio_ready(&self) -> bool {
        self.step == Step::AudioReady
    }
    pub fn random_state(&self) -> IrqRandomState {
        self.random.state()
    }
    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Step::*;
        if self.step == Random {
            if self.random.step(bus)? {
                return Ok(true);
            }
            self.step = ReadFlag;
        }
        let (tcycles, access) = match self.step {
            AudioReady => return Ok(false),
            CallRandom | CallFade => (24, None),
            ReadFlag | ReadCounter => (
                12,
                Some(DisplayAccess::Read {
                    port: if self.step == ReadFlag {
                        VBLANK_FLAG_PORT
                    } else {
                        0xffd5
                    },
                    at_tcycle: 8,
                }),
            ),
            ClearFlag | StoreCounter => (
                12,
                Some(DisplayAccess::Write {
                    port: if self.step == ClearFlag {
                        VBLANK_FLAG_PORT
                    } else {
                        0xffd5
                    },
                    value: self.a,
                    at_tcycle: 8,
                }),
            ),
            SkipZero | SkipCounter => (if self.a == 0 { 12 } else { 8 }, None),
            TestFlag | Zero | TestCounter | Decrement => (4, None),
            Random => unreachable!(),
        };
        let v = bus.execute(DisplayInstruction { tcycles, access })?;
        self.step = match self.step {
            CallRandom => Random,
            ReadFlag => {
                self.a = v.expect("wait-flag receipt");
                TestFlag
            }
            TestFlag => SkipZero,
            SkipZero => {
                if self.a == 0 {
                    ReadCounter
                } else {
                    Zero
                }
            }
            Zero => {
                self.a = 0;
                ClearFlag
            }
            ClearFlag => ReadCounter,
            ReadCounter => {
                self.a = v.expect("frame-count receipt");
                TestCounter
            }
            TestCounter => SkipCounter,
            SkipCounter => {
                if self.a == 0 {
                    CallFade
                } else {
                    Decrement
                }
            }
            Decrement => {
                self.a = self.a.wrapping_sub(1);
                StoreCounter
            }
            StoreCounter => CallFade,
            CallFade => AudioReady,
            Random | AudioReady => unreachable!(),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::display_work_time::{DisplayWorkError, DisplayWorkTime};
    use super::*;
    use crate::alloc_prelude::*;
    #[derive(serde::Deserialize)]
    struct Case {
        input: IrqRandomState,
        phase: u64,
        caller_bank: u8,
        flag: u8,
        counter: u8,
        preaudio: bool,
        output: IrqRandomState,
        flag_after: u8,
        counter_after: u8,
        #[serde(rename = "T")]
        cycles: u64,
        #[serde(rename = "instruction_T")]
        instructions: Vec<u8>,
        div_reads: Vec<(u8, u64)>,
        writes: Vec<(u16, u8, u64)>,
    }
    fn write(t: &mut DisplayWorkTime, port: u16, value: u8) {
        t.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Write {
                port,
                value,
                at_tcycle: 8,
            }),
        })
        .unwrap();
    }
    fn read(t: &mut DisplayWorkTime, port: u16) -> u8 {
        t.execute(DisplayInstruction {
            tcycles: 12,
            access: Some(DisplayAccess::Read { port, at_tcycle: 8 }),
        })
        .unwrap()
        .unwrap()
    }
    fn setup(c: &Case) -> DisplayWorkTime {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        for (port, value) in [
            (0xffb8, c.caller_bank),
            (0xffd3, c.input.add),
            (0xffd4, c.input.sub),
            (VBLANK_FLAG_PORT, c.flag),
            (0xffd5, c.counter),
        ] {
            write(&mut t, port, value);
        }
        t.execute(DisplayInstruction {
            tcycles: 16,
            access: Some(DisplayAccess::Write {
                port: 0x2000,
                value: c.caller_bank,
                at_tcycle: 12,
            }),
        })
        .unwrap();
        // Explicit NOPs establish the supplied unit DIV phase, not an
        // assertion about original boot phase or a gameplay timing pad.
        while t.elapsed() & 65535 != c.phase {
            t.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        t
    }
    struct Trace<'a> {
        time: &'a mut DisplayWorkTime,
        start: u64,
        steps: Vec<u8>,
        div: Vec<(u8, u64)>,
        writes: Vec<(u16, u8, u64)>,
    }
    impl DisplayInstructionBus for Trace<'_> {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
            let at = self.time.elapsed() - self.start;
            let v = self.time.execute(i)?;
            self.steps.push(i.tcycles);
            match i.access {
                Some(DisplayAccess::Read {
                    port: 0xff04,
                    at_tcycle,
                }) => self.div.push((v.unwrap(), at + u64::from(at_tcycle))),
                Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle,
                }) => self.writes.push((port, value, at + u64::from(at_tcycle))),
                _ => (),
            }
            Ok(v)
        }
    }
    #[test]
    fn random_rom_controls_match_banks_live_div_carry_all_stores_and_wait_counter_branches() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("irq_random_cases_1140.json")).unwrap();
        assert_eq!(cases.len(), 34);
        for c in cases {
            let mut time = setup(&c);
            let start = time.elapsed();
            let mut bus = Trace {
                time: &mut time,
                start,
                steps: vec![],
                div: vec![],
                writes: vec![],
            };
            let output = if c.preaudio {
                let mut w = VBlankPreAudioWork::new(c.input.carry);
                while w.step(&mut bus).unwrap() {}
                assert!(w.audio_ready());
                w.random_state()
            } else {
                let mut w = IrqRandomWork::new(c.input.carry);
                while w.step(&mut bus).unwrap() {}
                assert!(w.finished());
                w.state()
            };
            assert_eq!(output, c.output, "phase{}", c.phase);
            assert_eq!(bus.steps, c.instructions);
            assert_eq!(bus.div, c.div_reads);
            assert_eq!(bus.writes, c.writes);
            assert_eq!(bus.time.elapsed() - start, c.cycles);
            assert_eq!(bus.time.source_loaded_rom_bank(), c.caller_bank);
            assert_eq!(bus.time.selected_rom_bank(), c.caller_bank);
            assert_eq!(read(&mut time, VBLANK_FLAG_PORT), c.flag_after);
            assert_eq!(read(&mut time, 0xffd5), c.counter_after);
            assert_eq!(read(&mut time, 0xffd3), c.output.add);
            assert_eq!(read(&mut time, 0xffd4), c.output.sub);
        }
    }
    #[test]
    fn random_budget_and_mid_div_snapshot_keep_bank_and_arithmetic_receipt() {
        let mut time = DisplayWorkTime::default();
        let mut w = IrqRandomWork::new(true);
        let prior = w.clone();
        assert_eq!(w.step(&mut time), Err(DisplayWorkError::Budget));
        assert_eq!(w, prior);
        time.begin_native_update();
        write(&mut time, 0xffb8, 31);
        write(&mut time, 0x2000, 31);
        write(&mut time, 0xffd3, 255);
        while w.step != RandomStep::Leaf(1) {
            assert!(w.step(&mut time).unwrap());
        }
        assert_eq!(time.source_loaded_rom_bank(), 4);
        let saved = serde_json::to_string(&(&time, &w)).unwrap();
        let (mut other_time, mut other): (DisplayWorkTime, IrqRandomWork) =
            serde_json::from_str(&saved).unwrap();
        time.idle_to_target().unwrap();
        other_time.idle_to_target().unwrap();
        let prior = w.clone();
        assert_eq!(w.step(&mut time), Err(DisplayWorkError::Budget));
        assert_eq!(w, prior);
        time.begin_native_update();
        other_time.begin_native_update();
        while w.step(&mut time).unwrap() {}
        while other.step(&mut other_time).unwrap() {}
        assert_eq!(w, other);
        assert_eq!(
            serde_json::to_value(&time).unwrap(),
            serde_json::to_value(&other_time).unwrap()
        );
        assert_eq!(time.source_loaded_rom_bank(), 31);
        assert_eq!(time.selected_rom_bank(), 31);
    }
    #[test]
    fn pre_audio_flag_clear_and_zero_counter_leave_halted_foreground_waiting_for_reti() {
        use super::super::display_delay_work::{DelayFrameBus, DelayFrameWork, DelayProgress};
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        let mut foreground = DelayFrameWork::default();
        while !foreground.sleeping() {
            foreground.step(&mut t).unwrap();
        }
        t.idle_to_target().unwrap();
        t.begin_native_update();
        t.enter_pending_vblank().unwrap();
        let mut body = VBlankPreAudioWork::new(false);
        while body.step(&mut t).unwrap() {}
        assert!(body.audio_ready());
        assert_eq!(read(&mut t, VBLANK_FLAG_PORT), 0);
        assert_eq!(read(&mut t, 0xffd5), 0);
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
        let before = t.elapsed();
        assert_eq!(foreground.step(&mut t), Ok(DelayProgress::Sleeping));
        assert_eq!(t.elapsed(), before);
    }
    #[test]
    fn shared_divider_reads_after_access_boundary_and_reset_and_snapshot_are_explicit() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        for _ in 0..63 {
            t.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        assert_eq!(
            read(&mut t, 0xff04),
            1,
            "DIV crosses256 before LDH IO access"
        );
        write(&mut t, 0xff04, 0xff);
        assert_eq!(
            read(&mut t, 0xff04),
            0,
            "writing any value resets this divider projection"
        );
        let mut other: DisplayWorkTime =
            serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        for _ in 0..80 {
            t.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
            other
                .execute(DisplayInstruction {
                    tcycles: 4,
                    access: None,
                })
                .unwrap();
        }
        assert_eq!(read(&mut t, 0xff04), read(&mut other, 0xff04));
    }
}

#[cfg(test)]
mod prefix_tests {
    use super::*;
    use crate::overworld::{
        display_delay_work::DelayFrameBus,
        field_restore::{PatternAssets, PatternSource},
    };
    struct NoAssets;
    impl PatternAssets for NoAssets {
        fn read_pattern_byte(&self, _: PatternSource, _: u16) -> Option<u8> {
            None
        }
    }
    #[test]
    fn real_video_and_pre_audio_prefix_match_random_bytes_and_phase_without_returning_the_irq() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("pre_audio_real_case_1140.json")).unwrap();
        let (mut t, mut mem, mut video, _, _) = crate::overworld::vblank_video_work::tests::build();
        let get = |k: &str| v[k].as_u64().unwrap() as u8;
        for (port, value) in [
            (0xffd3, get("random_add")),
            (0xffd4, get("random_sub")),
            (0xffd5, get("frame_counter")),
            (VBLANK_FLAG_PORT, get("vblank_flag")),
            (0xff04, 0),
        ] {
            t.execute(DisplayInstruction {
                tcycles: 12,
                access: Some(DisplayAccess::Write {
                    port,
                    value,
                    at_tcycle: 8,
                }),
            })
            .unwrap();
        }
        // Fixture initialization: establish the independently observed source
        // DIV phase before measuring work. No such warmup exists in Game.
        let phase = v["test_DIV_at_IRQ_accept"].as_u64().unwrap() as u16;
        assert_eq!(phase % 4, 0);
        for _ in 0..phase.wrapping_sub(4) / 4 {
            t.execute(DisplayInstruction {
                tcycles: 4,
                access: None,
            })
            .unwrap();
        }
        let start = t.elapsed();
        while video.step(&mut t, &mut mem, &NoAssets).unwrap() {}
        assert_eq!(t.elapsed() - start, 10716);
        let carry = video.random_entry_carry().expect("PrepareOAMData actually returned");
        assert_eq!(carry, v["random_entry_carry"].as_bool().unwrap());
        let mut after = VBlankPreAudioWork::new(carry);
        while after.step(&mut t).unwrap() {}
        assert!(after.audio_ready());
        assert_eq!(
            t.elapsed() - start,
            v["prefix_to_FadeOutAudio_entry_T"].as_u64().unwrap()
        );
        assert_eq!(after.random_state().add, get("random_add_after"));
        assert_eq!(after.random_state().sub, get("random_sub_after"));
        for (port, key) in [
            (0xffd5, "frame_counter_after"),
            (VBLANK_FLAG_PORT, "vblank_flag_after"),
        ] {
            assert_eq!(
                t.execute(DisplayInstruction {
                    tcycles: 12,
                    access: Some(DisplayAccess::Read { port, at_tcycle: 8 })
                })
                .unwrap(),
                Some(get(key))
            );
        }
        assert_eq!(video.saved_bank(), Some(4));
        assert_eq!(t.source_loaded_rom_bank(), 1);
        assert_eq!(t.selected_rom_bank(), 1);
        assert!(t.active_vblank());
        assert_eq!(t.interrupt_returns(), 0);
    }
}
