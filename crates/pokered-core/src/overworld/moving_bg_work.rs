//! UpdateMovingBgTiles from home/vcopy.asm. Counters are logical IRQ inputs;
//! water reads the live shared VRAM bus. This child neither accepts an IRQ
//! nor returns the interrupted foreground continuation.
use super::display_lcd_work::{DisplayInstruction, DisplayInstructionBus};
use super::pattern_copy_work::{PatternAccess, PatternInstruction, PatternInstructionBus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MovingBgState {
    pub animations: u8,
    pub counter1: u8,
    pub counter2: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Step {
    Enabled,
    Test,
    ReturnZero,
    Counter,
    Increment,
    StoreCounter,
    Compare20,
    ReturnCarry,
    Compare21,
    FlowerBranch,
    WaterPointer,
    WaterCount,
    WaterCounter,
    WaterIncrement,
    WaterMask,
    StoreWaterCounter,
    WaterDirection,
    LeftBranch,
    WaterRead,
    Rotate,
    WaterWrite,
    WaterDecrement,
    WaterLoop,
    JumpDone,
    ReadEnabled,
    RotateEnabled,
    ReturnEven,
    Zero,
    Reset,
    Return,
    FlowerZero,
    FlowerReset,
    FlowerCounter,
    FlowerMask,
    Compare2,
    Flower1,
    FlowerLessBranch,
    Flower2,
    FlowerEqualBranch,
    Flower3,
    FlowerDestination,
    FlowerCount,
    FlowerRead,
    FlowerWrite,
    FlowerNext,
    FlowerDecrement,
    FlowerLoop,
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MovingBgWork {
    state: MovingBgState,
    flowers: [[u8; 16]; 3],
    step: Step,
    a: u8,
    c: u8,
    address: u16,
    flower: u8,
    flower_at: u8,
    left: bool,
    carry: bool,
    zero: bool,
}
impl MovingBgWork {
    pub fn new(state: MovingBgState, flowers: [[u8; 16]; 3]) -> Self {
        Self {
            state,
            flowers,
            step: Step::Enabled,
            a: 0,
            c: 0,
            address: 0,
            flower: 0,
            flower_at: 0,
            left: false,
            carry: false,
            zero: false,
        }
    }
    pub fn state(&self) -> MovingBgState {
        self.state
    }
    pub fn finished(&self) -> bool {
        self.step == Step::Done
    }
    pub fn step<B>(&mut self, bus: &mut B) -> Result<bool, <B as DisplayInstructionBus>::Error>
    where
        B: DisplayInstructionBus
            + PatternInstructionBus<Error = <B as DisplayInstructionBus>::Error>,
    {
        use Step::*;
        let t = match self.step {
            Done => return Ok(false),
            Enabled | Counter | StoreCounter | WaterPointer | ReadEnabled | Reset | FlowerReset
            | Flower1 | Flower2 | Flower3 | FlowerDestination | JumpDone => 12,
            WaterCounter | StoreWaterCounter | Return => 16,
            ReturnZero => {
                if self.zero {
                    20
                } else {
                    8
                }
            }
            ReturnCarry | ReturnEven => {
                if self.carry {
                    20
                } else {
                    8
                }
            }
            FlowerBranch => {
                if self.zero {
                    12
                } else {
                    8
                }
            }
            LeftBranch => {
                if self.left {
                    12
                } else {
                    8
                }
            }
            FlowerLessBranch => {
                if self.carry {
                    12
                } else {
                    8
                }
            }
            FlowerEqualBranch => {
                if self.zero {
                    12
                } else {
                    8
                }
            }
            WaterLoop | FlowerLoop => {
                if self.c != 0 {
                    12
                } else {
                    8
                }
            }
            Compare20 | Compare21 | WaterCount | WaterMask | WaterDirection | WaterRead
            | WaterWrite | FlowerMask | Compare2 | FlowerCount | FlowerRead | FlowerWrite
            | FlowerNext => 8,
            FlowerCounter => 16,
            _ => 4,
        };
        let read = match self.step {
            WaterRead => PatternInstructionBus::execute(
                bus,
                PatternInstruction {
                    tcycles: t,
                    access: Some(PatternAccess::ReadVram {
                        address: self.address,
                        at_tcycle: 4,
                    }),
                },
            )?
            .map(|n| n as u8),
            WaterWrite | FlowerWrite => {
                PatternInstructionBus::execute(
                    bus,
                    PatternInstruction {
                        tcycles: t,
                        access: Some(PatternAccess::Write {
                            destination: self.address,
                            value: self.a,
                            at_tcycle: 4,
                        }),
                    },
                )?;
                None
            }
            _ => {
                DisplayInstructionBus::execute(
                    bus,
                    DisplayInstruction {
                        tcycles: t,
                        access: None,
                    },
                )?;
                None
            }
        };
        self.step = match self.step {
            Enabled => {
                self.a = self.state.animations;
                Test
            }
            Test => {
                self.zero = self.a == 0;
                ReturnZero
            }
            ReturnZero => {
                if self.zero {
                    Done
                } else {
                    Counter
                }
            }
            Counter => {
                self.a = self.state.counter1;
                Increment
            }
            Increment => {
                self.a = self.a.wrapping_add(1);
                StoreCounter
            }
            StoreCounter => {
                self.state.counter1 = self.a;
                Compare20
            }
            Compare20 => {
                self.carry = self.a < 20;
                ReturnCarry
            }
            ReturnCarry => {
                if self.carry {
                    Done
                } else {
                    Compare21
                }
            }
            Compare21 => {
                self.zero = self.a == 21;
                FlowerBranch
            }
            FlowerBranch => {
                if self.zero {
                    FlowerZero
                } else {
                    WaterPointer
                }
            }
            WaterPointer => {
                self.address = 0x9140;
                WaterCount
            }
            WaterCount => {
                self.c = 16;
                WaterCounter
            }
            WaterCounter => {
                self.a = self.state.counter2;
                WaterIncrement
            }
            WaterIncrement => {
                self.a = self.a.wrapping_add(1);
                WaterMask
            }
            WaterMask => {
                self.a &= 7;
                StoreWaterCounter
            }
            StoreWaterCounter => {
                self.state.counter2 = self.a;
                WaterDirection
            }
            WaterDirection => {
                self.left = self.a & 4 != 0;
                LeftBranch
            }
            LeftBranch => WaterRead,
            WaterRead => {
                self.a = read.expect("VRAM read receipt");
                Rotate
            }
            Rotate => {
                self.a = if self.left {
                    self.a.rotate_left(1)
                } else {
                    self.a.rotate_right(1)
                };
                WaterWrite
            }
            WaterWrite => {
                self.address = self.address.wrapping_add(1);
                WaterDecrement
            }
            WaterDecrement => {
                self.c -= 1;
                WaterLoop
            }
            WaterLoop => {
                if self.c != 0 {
                    WaterRead
                } else if self.left {
                    ReadEnabled
                } else {
                    JumpDone
                }
            }
            JumpDone => ReadEnabled,
            ReadEnabled => {
                self.a = self.state.animations;
                RotateEnabled
            }
            RotateEnabled => {
                self.carry = self.a & 1 == 0;
                self.a = self.a.rotate_right(1);
                ReturnEven
            }
            ReturnEven => {
                if self.carry {
                    Done
                } else {
                    Zero
                }
            }
            Zero => {
                self.a = 0;
                Reset
            }
            Reset => {
                self.state.counter1 = self.a;
                Return
            }
            Return => Done,
            FlowerZero => {
                self.a = 0;
                FlowerReset
            }
            FlowerReset => {
                self.state.counter1 = self.a;
                FlowerCounter
            }
            FlowerCounter => {
                self.a = self.state.counter2;
                FlowerMask
            }
            FlowerMask => {
                self.a &= 3;
                Compare2
            }
            Compare2 => {
                self.zero = self.a == 2;
                self.carry = self.a < 2;
                Flower1
            }
            Flower1 => {
                self.flower = 0;
                FlowerLessBranch
            }
            FlowerLessBranch => {
                if self.carry {
                    FlowerDestination
                } else {
                    Flower2
                }
            }
            Flower2 => {
                self.flower = 1;
                FlowerEqualBranch
            }
            FlowerEqualBranch => {
                if self.zero {
                    FlowerDestination
                } else {
                    Flower3
                }
            }
            Flower3 => {
                self.flower = 2;
                FlowerDestination
            }
            FlowerDestination => {
                self.address = 0x9030;
                FlowerCount
            }
            FlowerCount => {
                self.c = 16;
                FlowerRead
            }
            FlowerRead => {
                self.a = self.flowers[usize::from(self.flower)][usize::from(self.flower_at)];
                self.flower_at += 1;
                FlowerWrite
            }
            FlowerWrite => FlowerNext,
            FlowerNext => {
                self.address = self.address.wrapping_add(1);
                FlowerDecrement
            }
            FlowerDecrement => {
                self.c -= 1;
                FlowerLoop
            }
            FlowerLoop => {
                if self.c == 0 {
                    Return
                } else {
                    FlowerRead
                }
            }
            Done => Done,
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        display_work_time::{DisplayWorkError, DisplayWorkTime},
        field_restore::{PatternAssets, PatternMemory, PatternSource},
    };
    use super::*;
    use crate::alloc_prelude::*;
    struct NoAssets;
    impl PatternAssets for NoAssets {
        fn read_pattern_byte(&self, _: PatternSource, _: u16) -> Option<u8> {
            None
        }
    }
    #[derive(serde::Deserialize)]
    struct Case {
        input: MovingBgState,
        output: MovingBgState,
        #[serde(rename = "T")]
        cycles: u64,
        writes: Vec<(u16, u8, u64)>,
        reads: Vec<(u16, u8, u64)>,
        #[serde(rename = "instruction_T")]
        instruction_cycles: Vec<u8>,
    }
    #[derive(serde::Deserialize)]
    struct Cases {
        flowers: [[u8; 16]; 3],
        cases: Vec<Case>,
    }
    struct Trace<'a> {
        time: &'a mut DisplayWorkTime,
        memory: &'a mut PatternMemory,
        steps: Vec<u8>,
        writes: Vec<(u16, u8, u64)>,
        reads: Vec<(u16, u8, u64)>,
    }
    impl DisplayInstructionBus for Trace<'_> {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, Self::Error> {
            let v = DisplayInstructionBus::execute(self.time, i)?;
            self.steps.push(i.tcycles);
            Ok(v)
        }
    }
    impl PatternInstructionBus for Trace<'_> {
        type Error = DisplayWorkError;
        fn execute(&mut self, i: PatternInstruction) -> Result<Option<u16>, Self::Error> {
            let start = self.time.elapsed();
            let v = PatternInstructionBus::execute(
                &mut self.time.pattern_bus(self.memory, &NoAssets),
                i,
            )?;
            self.steps.push(i.tcycles);
            match i.access {
                Some(PatternAccess::Write {
                    destination,
                    value,
                    at_tcycle,
                }) => self
                    .writes
                    .push((destination, value, start + u64::from(at_tcycle))),
                Some(PatternAccess::ReadVram { address, at_tcycle }) => {
                    self.reads
                        .push((address, v.unwrap() as u8, start + u64::from(at_tcycle)))
                }
                _ => (),
            }
            Ok(v)
        }
    }
    fn seeded() -> PatternMemory {
        let mut m = PatternMemory::default();
        for a in 0x8000u16..0xa000 {
            m.write(a, (a as u8).wrapping_mul(13).wrapping_add(37))
                .unwrap();
        }
        m
    }
    #[test]
    fn original_rom_moving_bg_controls_match_instructions_live_reads_and_every_byte() {
        let all: Cases = serde_json::from_str(include_str!("moving_bg_cases_1139.json")).unwrap();
        assert_eq!(all.cases.len(), 91);
        for c in all.cases {
            let mut time = DisplayWorkTime::default();
            time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
            time.begin_native_update();
            let mut memory = seeded();
            let mut expected = memory.clone();
            for &(a, v, _) in &c.writes {
                expected.write(a, v).unwrap();
            }
            let mut w = MovingBgWork::new(c.input, all.flowers);
            let mut bus = Trace {
                time: &mut time,
                memory: &mut memory,
                steps: vec![],
                writes: vec![],
                reads: vec![],
            };
            while w.step(&mut bus).unwrap() {}
            assert!(w.finished());
            assert_eq!(w.state(), c.output, "{:?}", c.input);
            assert_eq!(bus.steps, c.instruction_cycles, "{:?}", c.input);
            assert_eq!(bus.time.elapsed(), c.cycles, "{:?}", c.input);
            assert_eq!(bus.writes, c.writes, "{:?}", c.input);
            assert_eq!(bus.reads, c.reads, "{:?}", c.input);
            assert_eq!(bus.memory.bytes(), expected.bytes(), "{:?}", c.input);
        }
    }
    #[test]
    fn moving_bg_budget_and_mid_rotation_snapshot_preserve_live_memory_and_counters() {
        let mut time = DisplayWorkTime::default();
        let mut memory = seeded();
        let mut work = MovingBgWork::new(
            MovingBgState {
                animations: 2,
                counter1: 19,
                counter2: 3,
            },
            [[0; 16]; 3],
        );
        let before = work.clone();
        assert_eq!(
            work.step(&mut time.pattern_bus(&mut memory, &NoAssets)),
            Err(DisplayWorkError::Budget)
        );
        assert_eq!(before, work);
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        time.begin_native_update();
        while work.step != Step::Rotate {
            assert!(work
                .step(&mut time.pattern_bus(&mut memory, &NoAssets))
                .unwrap());
        }
        // The original byte has been read but not rotated/stored. Changing
        // live memory now must not replace the latched read receipt.
        let value = memory.read(0x9140).unwrap();
        memory.write(0x9140, value ^ 0xff).unwrap();
        let snap = serde_json::to_string(&(&work, &time, &memory)).unwrap();
        let (mut other, mut other_time, mut other_mem): (
            MovingBgWork,
            DisplayWorkTime,
            PatternMemory,
        ) = serde_json::from_str(&snap).unwrap();
        while work
            .step(&mut time.pattern_bus(&mut memory, &NoAssets))
            .unwrap()
        {}
        while other
            .step(&mut other_time.pattern_bus(&mut other_mem, &NoAssets))
            .unwrap()
        {}
        assert_eq!(work, other);
        assert_eq!(time.elapsed(), other_time.elapsed());
        assert_eq!(memory.bytes(), other_mem.bytes());
        assert_eq!(memory.read(0x9140), Some(value.rotate_left(1)));
        assert_eq!(
            work.state(),
            MovingBgState {
                animations: 2,
                counter1: 20,
                counter2: 4
            }
        );
        assert_eq!(
            super::super::display_delay_work::DelayFrameBus::interrupt_returns(&time),
            0
        );
    }
    #[test]
    fn moving_bg_early_counter_store_does_not_complete_the_child_or_an_irq() {
        let mut time = DisplayWorkTime::default();
        time.device_mut().write_port(0xff40, 0x11, 0xc3).unwrap();
        time.begin_native_update();
        let mut memory = seeded();
        let mut w = MovingBgWork::new(
            MovingBgState {
                animations: 1,
                counter1: 20,
                counter2: 2,
            },
            [[0; 16]; 3],
        );
        while w.state().counter1 != 0 {
            assert!(w
                .step(&mut time.pattern_bus(&mut memory, &NoAssets))
                .unwrap());
        }
        assert!(!w.finished());
        assert_eq!(
            memory.read(0x9030),
            Some(0x30u8.wrapping_mul(13).wrapping_add(37))
        );
        let saved = w.clone();
        time.idle_to_target().unwrap();
        assert_eq!(
            w.step(&mut time.pattern_bus(&mut memory, &NoAssets)),
            Err(DisplayWorkError::Budget)
        );
        assert_eq!(w, saved);
        time.begin_native_update();
        while w
            .step(&mut time.pattern_bus(&mut memory, &NoAssets))
            .unwrap()
        {}
        assert_eq!(&memory.bytes()[0x1030..0x1040], &[0; 16]);
        assert_eq!(
            super::super::display_delay_work::DelayFrameBus::interrupt_returns(&time),
            0
        );
    }
    #[test]
    fn vram_read_observes_mode_at_access_and_denied_write_preserves_the_byte() {
        let mut t = DisplayWorkTime::default();
        t.begin_native_update();
        let mut m = seeded();
        while !(t.device().mode() == 2 && t.device().tcycles_until_event() == Some(4)) {
            DisplayInstructionBus::execute(
                &mut t,
                DisplayInstruction {
                    tcycles: 4,
                    access: None,
                },
            )
            .unwrap();
        }
        let before = m.read(0x9140);
        let i = PatternInstruction {
            tcycles: 8,
            access: Some(PatternAccess::ReadVram {
                address: 0x9140,
                at_tcycle: 4,
            }),
        };
        assert_eq!(
            PatternInstructionBus::execute(&mut t.pattern_bus(&mut m, &NoAssets), i),
            Ok(Some(0xff))
        );
        PatternInstructionBus::execute(
            &mut t.pattern_bus(&mut m, &NoAssets),
            PatternInstruction {
                tcycles: 8,
                access: Some(PatternAccess::Write {
                    destination: 0x9140,
                    value: 0,
                    at_tcycle: 4,
                }),
            },
        )
        .unwrap();
        assert_eq!(m.read(0x9140), before);
        while !(t.device().mode() == 3 && t.device().tcycles_until_event() == Some(4)) {
            DisplayInstructionBus::execute(
                &mut t,
                DisplayInstruction {
                    tcycles: 4,
                    access: None,
                },
            )
            .unwrap();
        }
        assert_eq!(
            PatternInstructionBus::execute(&mut t.pattern_bus(&mut m, &NoAssets), i),
            Ok(before.map(u16::from))
        );
    }
}
