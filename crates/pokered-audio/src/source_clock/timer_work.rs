//! Retained CPU instruction work for source timer/effect frames. Native channel
//! updates commit once after the source RET. APU port-write timing is not owned
//! here. Next-note/duty-rotation/pitch-slide paths remain explicit unsupported.
pub use super::audio_work::ChannelEffectWorkInput;
use super::{AudioClockError, AudioFrameWork};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum EffectStage {
    Entry(u8),
    Gate,
    Paired(u8),
    Duty(u8),
    Execute(u8),
    Noise(u8),
    Pitch(u8),
    Delay(u8),
    Extent(u8),
    Rate(u8),
    Apply(u8),
    Direction,
    Subtract(u8),
    Add(u8),
    Output(u8),
    Register(u8),
    Tail(u8),
    Decay(u8),
    Return,
    Done,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct EffectInstructionWork {
    input: ChannelEffectWorkInput,
    stage: EffectStage,
}
impl EffectInstructionWork {
    pub fn new(input: ChannelEffectWorkInput) -> Option<Self> {
        if input.channel >= 8 || input.note_delay == 1 {
            return None;
        }
        let paired = input.channel < 4 && input.paired_sfx_active;
        let noise = input.flags2 & 1 == 0 && input.flags1 & 4 != 0;
        if !paired && (input.flags1 & 64 != 0 || (!noise && input.flags1 & 16 != 0)) {
            return None;
        }
        Some(Self {
            input,
            stage: EffectStage::Entry(0),
        })
    }
    pub fn finished(&self) -> bool {
        self.stage == EffectStage::Done
    }
    fn peek(&self) -> Option<u8> {
        use EffectStage::*;
        let i = self.input;
        Some(match self.stage {
            Entry(n) => [8, 12, 8, 8, 8, 12, 4, 8, 4, 8][n as usize],
            Gate => {
                if i.channel >= 4 {
                    12
                } else {
                    8
                }
            }
            Paired(n) => {
                if n < 4 {
                    [12, 8, 8, 4][n as usize]
                } else if i.paired_sfx_active {
                    8
                } else {
                    12
                }
            }
            Duty(n) => [12, 8, 12, 12][n as usize],
            Execute(n) => {
                if n < 4 {
                    [8, 12, 8, 12][n as usize]
                } else if i.flags2 & 1 != 0 {
                    12
                } else {
                    8
                }
            }
            Noise(n) => {
                if n < 3 {
                    [12, 8, 12][n as usize]
                } else if i.flags1 & 4 != 0 {
                    12
                } else {
                    8
                }
            }
            Pitch(n) => [12, 8, 12, 12][n as usize],
            Delay(n) | Extent(n) => {
                if n < 4 {
                    [12, 8, 8, 4][n as usize]
                } else if matches!(self.stage, Delay(_)) {
                    if i.vibrato_delay == 0 {
                        12
                    } else {
                        8
                    }
                } else if i.vibrato_extent != 0 {
                    12
                } else {
                    8
                }
            }
            Rate(n) => {
                if n < 6 {
                    [4, 12, 8, 8, 8, 4][n as usize]
                } else if i.vibrato_rate & 15 == 0 {
                    12
                } else {
                    8
                }
            }
            Apply(n) => [8, 16, 8, 8, 12, 8, 8, 12, 8, 12][n as usize],
            Direction => {
                if i.flags1 & 8 == 0 {
                    12
                } else {
                    8
                }
            }
            Subtract(n) => {
                if n < 6 {
                    [16, 4, 8, 4, 4, 4][n as usize]
                } else if n == 6 {
                    if i.frequency_low >= i.vibrato_extent & 15 {
                        12
                    } else {
                        8
                    }
                } else if n == 7 {
                    8
                } else {
                    12
                }
            }
            Add(n) => {
                if n < 5 {
                    [16, 4, 8, 8, 4][n as usize]
                } else if n == 5 {
                    if u16::from(i.frequency_low) + u16::from(i.vibrato_extent >> 4) <= 255 {
                        12
                    } else {
                        8
                    }
                } else {
                    8
                }
            }
            Output(n) => [4, 8, 24][n as usize],
            // Both pinned Audio1/3 table low bytes ($17/$8b) plus channels0..7
            // cannot carry. The source JR nc skips INC H, not a merged 8+4 pseudo-op.
            Register(n) => [4, 12, 4, 12, 4, 8, 4, 4, 8, 16][n as usize],
            Tail(n) => [8, 16][n as usize],
            Decay(n) => [12, 16][n as usize],
            Return => 16,
            Done => return None,
        })
    }
    fn advance(&mut self) {
        use EffectStage::*;
        let i = self.input;
        self.stage = match self.stage {
            Entry(n) => {
                if n == 9 {
                    Gate
                } else {
                    Entry(n + 1)
                }
            }
            Gate => {
                if i.channel >= 4 {
                    Duty(0)
                } else {
                    Paired(0)
                }
            }
            Paired(n) => {
                if n == 4 {
                    if i.paired_sfx_active {
                        Return
                    } else {
                        Duty(0)
                    }
                } else {
                    Paired(n + 1)
                }
            }
            Duty(n) => {
                if n == 3 {
                    Execute(0)
                } else {
                    Duty(n + 1)
                }
            }
            Execute(n) => {
                if n == 4 {
                    if i.flags2 & 1 != 0 {
                        Pitch(0)
                    } else {
                        Noise(0)
                    }
                } else {
                    Execute(n + 1)
                }
            }
            Noise(n) => {
                if n == 3 {
                    if i.flags1 & 4 != 0 {
                        Return
                    } else {
                        Pitch(0)
                    }
                } else {
                    Noise(n + 1)
                }
            }
            Pitch(n) => {
                if n == 3 {
                    Delay(0)
                } else {
                    Pitch(n + 1)
                }
            }
            Delay(n) => {
                if n == 4 {
                    if i.vibrato_delay == 0 {
                        Extent(0)
                    } else {
                        Decay(0)
                    }
                } else {
                    Delay(n + 1)
                }
            }
            // Early DEC [HL],RET has 12,16 rather than hardware store8,RET.
            Extent(n) => {
                if n == 4 {
                    if i.vibrato_extent != 0 {
                        Rate(0)
                    } else {
                        Return
                    }
                } else {
                    Extent(n + 1)
                }
            }
            Rate(n) => {
                if n == 6 {
                    if i.vibrato_rate & 15 == 0 {
                        Apply(0)
                    } else {
                        Decay(0)
                    }
                } else {
                    Rate(n + 1)
                }
            }
            Apply(n) => {
                if n == 9 {
                    Direction
                } else {
                    Apply(n + 1)
                }
            }
            Direction => {
                if i.flags1 & 8 != 0 {
                    Subtract(0)
                } else {
                    Add(0)
                }
            }
            Subtract(n) => {
                if n == 6 && i.frequency_low >= i.vibrato_extent & 15 {
                    Subtract(8)
                } else if n == 8 {
                    Output(0)
                } else {
                    Subtract(n + 1)
                }
            }
            Add(n) => {
                if (n == 5 && u16::from(i.frequency_low) + u16::from(i.vibrato_extent >> 4) <= 255)
                    || n == 6
                {
                    Output(0)
                } else {
                    Add(n + 1)
                }
            }
            Output(n) => {
                if n == 2 {
                    Register(0)
                } else {
                    Output(n + 1)
                }
            }
            Register(n) => {
                if n == 9 {
                    Tail(0)
                } else {
                    Register(n + 1)
                }
            }
            Tail(n) => {
                if n == 1 {
                    Done
                } else {
                    Tail(1)
                }
            }
            Decay(n) => {
                if n == 1 {
                    Done
                } else {
                    Decay(1)
                }
            }
            Return => Done,
            Done => Done,
        };
    }
}
impl Iterator for EffectInstructionWork {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        let cycles = self.peek()?;
        self.advance();
        Some(cycles)
    }
}

/// Source bank and mute byte must come from the interrupt owner. This body
/// currently implements Audio1/3 with an explicit zero mute byte, timer/effect
/// channels only. It does not infer those bytes from a previous recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct TimerFrameContext {
    pub bank: u8,
    pub mute: u8,
    pub sound_ids: [u8; 8],
    pub effects: [Option<ChannelEffectWorkInput>; 8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum FrameStage {
    Init,
    Channel(u8),
    Kind(u8),
    Mute(u8),
    Call,
    Effect(EffectInstructionWork),
    Tail(u8),
    Return,
    Done,
}

/// Retains the next actual source instruction. Acceptance is per instruction;
/// an execution refusal consumes neither that instruction nor a native tick.
/// This is CPU work only; APU writes and canonical channel updates are not
/// performed by this cursor. AudioManager supplies the once-only commit.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct TimerFrameWork {
    context: TimerFrameContext,
    stage: FrameStage,
    channel: u8,
    cycles: u32,
    pub(crate) committed: Option<Result<AudioFrameWork, AudioClockError>>,
}

impl TimerFrameWork {
    pub fn new(context: TimerFrameContext) -> Result<Self, AudioClockError> {
        if !matches!(context.bank, 2 | 31) || context.mute != 0 {
            return Err(AudioClockError::UnsupportedSoundBank);
        }
        for channel in 0..8 {
            match (context.sound_ids[channel], context.effects[channel]) {
                (0, None) => (),
                (id, Some(input))
                    if id != 0
                        && input.channel == channel as u8
                        && input.note_delay > 1
                        && EffectInstructionWork::new(input).is_some() =>
                {
                    ()
                }
                _ => return Err(AudioClockError::UnsupportedEffect(channel as u8)),
            }
        }
        Ok(Self {
            context,
            stage: FrameStage::Init,
            channel: 0,
            cycles: 0,
            committed: None,
        })
    }

    pub fn context(&self) -> &TimerFrameContext {
        &self.context
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn finished(&self) -> bool {
        self.stage == FrameStage::Done
    }

    /// Executes at most one instruction. The callback must accept the whole
    /// instruction or refuse before advancing its clock. A returned true means
    /// the source RET has completed; it does not itself commit native state.
    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        let Some(cycles) = self.next_cycles() else {
            return Ok(true);
        };
        execute(cycles)?;
        self.advance();
        self.cycles += u32::from(cycles);
        Ok(self.finished())
    }

    fn next_cycles(&self) -> Option<u8> {
        use FrameStage::*;
        Some(match self.stage {
            Init => 8,
            Channel(n) => {
                if n < 5 {
                    [8, 12, 8, 8, 4][n as usize]
                } else if self.context.sound_ids[self.channel as usize] == 0 {
                    12
                } else {
                    8
                }
            }
            Kind(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if self.channel >= 4 {
                    12
                } else {
                    8
                }
            }
            Mute(n) => [16, 4, 12][n as usize],
            Call => 24,
            Effect(effect) => effect.peek()?,
            Tail(n) => {
                if n < 3 {
                    [4, 4, 8][n as usize]
                } else if self.channel == 7 {
                    8
                } else {
                    12
                }
            }
            Return => 16,
            Done => return None,
        })
    }

    fn advance(&mut self) {
        use FrameStage::*;
        self.stage = match self.stage {
            Init => Channel(0),
            Channel(n) => {
                if n == 5 {
                    if self.context.sound_ids[self.channel as usize] == 0 {
                        Tail(0)
                    } else {
                        Kind(0)
                    }
                } else {
                    Channel(n + 1)
                }
            }
            Kind(n) => {
                if n == 2 {
                    if self.channel >= 4 {
                        Call
                    } else {
                        Mute(0)
                    }
                } else {
                    Kind(n + 1)
                }
            }
            Mute(n) => {
                if n == 2 {
                    Call
                } else {
                    Mute(n + 1)
                }
            }
            Call => Effect(
                EffectInstructionWork::new(self.context.effects[self.channel as usize].unwrap())
                    .unwrap(),
            ),
            Effect(mut effect) => {
                effect.advance();
                if effect.finished() {
                    Tail(0)
                } else {
                    Effect(effect)
                }
            }
            Tail(n) => {
                if n == 3 {
                    if self.channel == 7 {
                        Return
                    } else {
                        self.channel += 1;
                        Channel(0)
                    }
                } else {
                    Tail(n + 1)
                }
            }
            Return => Done,
            Done => Done,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct EffectCase {
        input: ChannelEffectWorkInput,
        bank: u8,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct FrameCase {
        context: TimerFrameContext,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        effects: alloc::vec::Vec<EffectCase>,
        frames: alloc::vec::Vec<FrameCase>,
    }
    fn fixture() -> Fixture {
        serde_json::from_str(include_str!("timer_instructions_golden.json")).unwrap()
    }

    #[test]
    fn effects_match_each_original_rom_instruction_in_both_banks() {
        let fixture = fixture();
        assert_eq!(fixture.effects.len(), 160);
        for (case_index, case) in fixture.effects.into_iter().enumerate() {
            assert!(matches!(case.bank, 2 | 31));
            let mut work = EffectInstructionWork::new(case.input).unwrap();
            for (pc, expected) in case.trace {
                // Serialize at every boundary, not only an entry/finished cursor.
                work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
                assert_eq!(
                    work.peek(),
                    Some(expected),
                    "case {case_index}, original PC {pc:04x}"
                );
                assert_eq!(work.next(), Some(expected));
            }
            assert!(work.finished());
            assert_eq!(work.next(), None);
        }
    }

    #[test]
    fn complete_timer_bodies_match_rom_and_refusals_retain_the_instruction() {
        let fixture = fixture();
        assert_eq!(fixture.frames.len(), 8);
        for (case_index, case) in fixture.frames.into_iter().enumerate() {
            let mut work = TimerFrameWork::new(case.context).unwrap();
            let mut cycles = 0;
            for (pc, expected) in case.trace {
                let frozen = work.clone();
                for _ in 0..2 {
                    assert_eq!(
                        work.try_step(|actual| {
                            assert_eq!(actual, expected);
                            Err("no room")
                        }),
                        Err("no room")
                    );
                    assert_eq!(work, frozen);
                }
                work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
                work.try_step(|actual| {
                    assert_eq!(actual, expected, "case {case_index} PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                cycles += u32::from(expected);
                assert_eq!(work.cycles(), cycles);
            }
            assert!(work.finished());
            assert!(work
                .try_step(|_| -> Result<(), ()> { panic!("completed work executes nothing") })
                .unwrap());
        }
    }

    #[test]
    fn unknown_paths_are_rejected_without_a_substitute_timer() {
        let mut context = fixture().frames.pop().unwrap().context;
        for bank in [0, 8, 255] {
            context.bank = bank;
            assert!(TimerFrameWork::new(context).is_err());
        }
        context.bank = 31;
        context.mute = 1;
        assert!(TimerFrameWork::new(context).is_err());
        context.mute = 0;
        for delay in [0, 1] {
            context.effects[0].as_mut().unwrap().note_delay = delay;
            assert!(TimerFrameWork::new(context).is_err());
        }
        context.effects[0].as_mut().unwrap().note_delay = 2;
        for flags in [16, 64] {
            context.effects[4].as_mut().unwrap().flags1 = flags;
            assert!(TimerFrameWork::new(context).is_err());
        }
    }
}
