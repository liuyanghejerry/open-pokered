//! Source sound_ret channel-ending tail after the failed SOUND_CALL branch.
//! No native channel/APU state is changed by this retained CPU-work cursor.
const DISABLE_MASKS: [u8; 8] = [0xee, 0xdd, 0xbb, 0x77, 0xee, 0xdd, 0xbb, 0x77];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ChannelEndInput {
    pub bank: u8,
    pub channel: u8,
    pub source_sfx_id: u8,
    pub flags1: u8,
    pub flags2: u8,
    pub disable_output: Option<u8>,
    pub saved_volume: Option<u8>,
    pub panning: Option<u8>,
    pub command_ptr: Option<u16>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ChannelEndResult {
    pub ended: bool,
    pub flags1: u8,
    pub flags2: u8,
    pub disable_output_write: Option<u8>,
    pub clear_saved_volume: bool,
    pub panning_write: Option<u8>,
    pub command_ptr_write: Option<u16>,
    pub wave_restarted: bool,
    pub volume_restore: Option<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Kind(u8),
    NoiseFlags(u8),
    WaveRestart(u8),
    SkipWave,
    DisableCheck(u8),
    DisableReset(u8),
    AfterDisableJump,
    Disable(u8),
    CryPrefix(u8),
    CryMaybe(u8),
    CryNoJump,
    CryChannel(u8),
    RewindCall,
    CryCheck(u8),
    CryEnd(u8),
    CryYes,
    CryNo(u8),
    RewindBranch,
    Rewind(u8),
    GoBackCall,
    ConditionalRet,
    Restore(u8),
    Clear(u8),
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ChannelEndInstructions {
    input: ChannelEndInput,
    stage: Stage,
    result: ChannelEndResult,
    cycles: u32,
}
impl ChannelEndInstructions {
    /// Caller has consumed sound_ret's command fetch, comparison, SOUND_CALL
    /// check and untaken JR nz. Source B is zero; HL points at channel flags1.
    pub fn new(input: ChannelEndInput) -> Option<Self> {
        let cry = (20..134).contains(&input.source_sfx_id);
        let disable = input.channel < 3 || (input.channel == 6 && input.disable_output? != 0);
        if !matches!(input.bank, 2 | 31)
            || input.channel >= 8
            || input.flags1 & 2 != 0
            || (disable && input.panning.is_none())
            || (cry && input.channel == 4 && input.saved_volume.is_none())
            || (cry && input.channel != 4 && input.command_ptr.is_none())
        {
            return None;
        }
        Some(Self {
            input,
            stage: Stage::Kind(0),
            result: ChannelEndResult {
                ended: false,
                flags1: input.flags1,
                flags2: input.flags2,
                disable_output_write: None,
                clear_saved_volume: false,
                panning_write: None,
                command_ptr_write: None,
                wave_restarted: false,
                volume_restore: None,
            },
            cycles: 0,
        })
    }
    pub fn result(&self) -> Option<ChannelEndResult> {
        if self.stage == Stage::Done {
            Some(self.result)
        } else {
            None
        }
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn try_step<E>(&mut self, execute: impl FnOnce(u8) -> Result<(), E>) -> Result<bool, E> {
        let Some(t) = self.next_cycles() else {
            return Ok(true);
        };
        execute(t)?;
        self.advance();
        self.cycles += u32::from(t);
        Ok(self.stage == Stage::Done)
    }
    fn next_cycles(&self) -> Option<u8> {
        use Stage::*;
        let ch = self.input.channel;
        let id = self.input.source_sfx_id;
        Some(match self.stage {
            Kind(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if ch >= 3 {
                    12
                } else {
                    8
                }
            }
            NoiseFlags(n) => {
                if n < 5 {
                    [16, 12, 8, 16, 8][n as usize]
                } else if ch != 6 {
                    12
                } else {
                    8
                }
            }
            WaveRestart(n) => [8, 12, 8, 12][n as usize],
            SkipWave => {
                if ch != 6 {
                    12
                } else {
                    8
                }
            }
            DisableCheck(n) => {
                if n < 2 {
                    [16, 4][n as usize]
                } else if self.input.disable_output == Some(0) {
                    12
                } else {
                    8
                }
            }
            DisableReset(n) => [4, 16, 12][n as usize],
            AfterDisableJump => 12,
            Disable(n) => [12, 8, 12, 8, 12][n as usize],
            CryPrefix(n) => {
                if n < 2 {
                    [16, 8][n as usize]
                } else if id >= 20 {
                    12
                } else {
                    8
                }
            }
            CryNoJump => 12,
            CryMaybe(n) => {
                if n < 2 {
                    [16, 8][n as usize]
                } else if n == 2 {
                    if id == 134 {
                        12
                    } else {
                        8
                    }
                } else if id < 134 {
                    12
                } else {
                    8
                }
            }
            CryChannel(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if ch == 4 {
                    12
                } else {
                    8
                }
            }
            RewindCall => 24,
            CryCheck(n) => {
                if n < 2 {
                    [16, 8][n as usize]
                } else if id >= 20 {
                    12
                } else {
                    8
                }
            }
            CryEnd(n) => {
                if n == 0 {
                    8
                } else if n == 1 {
                    if id == 134 {
                        12
                    } else {
                        8
                    }
                } else if id < 134 {
                    12
                } else {
                    8
                }
            }
            CryYes => 4,
            CryNo(n) => {
                if n == 2 {
                    16
                } else {
                    4
                }
            }
            RewindBranch => {
                if (20..134).contains(&id) {
                    8
                } else {
                    12
                }
            }
            Rewind(n) => [12, 4, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 4, 16][n as usize],
            GoBackCall => 24,
            ConditionalRet => 20,
            Restore(n) => [16, 12, 4, 16][n as usize],
            Clear(n) => [12, 8, 8, 16][n as usize],
            Done => return None,
        })
    }
    fn advance(&mut self) {
        use Stage::*;
        let ch = self.input.channel;
        let id = self.input.source_sfx_id;
        self.stage = match self.stage {
            Kind(n) => {
                if n == 2 {
                    if ch >= 3 {
                        NoiseFlags(0)
                    } else {
                        DisableReset(2)
                    }
                } else {
                    Kind(n + 1)
                }
            }
            NoiseFlags(n) => {
                if n == 0 {
                    self.result.flags1 &= !4;
                }
                if n == 3 {
                    self.result.flags2 &= !1;
                }
                if n == 5 {
                    if ch == 6 {
                        WaveRestart(0)
                    } else {
                        SkipWave
                    }
                } else {
                    NoiseFlags(n + 1)
                }
            }
            WaveRestart(n) => {
                if n == 3 {
                    self.result.wave_restarted = true;
                    SkipWave
                } else {
                    WaveRestart(n + 1)
                }
            }
            SkipWave => {
                if ch != 6 {
                    AfterDisableJump
                } else {
                    DisableCheck(0)
                }
            }
            DisableCheck(n) => {
                if n == 2 {
                    if self.input.disable_output == Some(0) {
                        AfterDisableJump
                    } else {
                        DisableReset(0)
                    }
                } else {
                    DisableCheck(n + 1)
                }
            }
            DisableReset(n) => {
                if n == 1 {
                    self.result.disable_output_write = Some(0);
                }
                if n == 2 {
                    Disable(0)
                } else {
                    DisableReset(n + 1)
                }
            }
            AfterDisableJump => CryPrefix(0),
            Disable(n) => {
                if n == 4 {
                    self.result.panning_write = Some(
                        self.input.panning.expect("validated panning read")
                            & DISABLE_MASKS[ch as usize],
                    );
                    CryPrefix(0)
                } else {
                    Disable(n + 1)
                }
            }
            CryPrefix(n) => {
                if n == 2 {
                    if id >= 20 {
                        CryMaybe(0)
                    } else {
                        CryNoJump
                    }
                } else {
                    CryPrefix(n + 1)
                }
            }
            CryNoJump => Clear(0),
            CryMaybe(n) => {
                if n == 2 && id == 134 {
                    Clear(0)
                } else if n == 3 {
                    if id < 134 {
                        CryChannel(0)
                    } else {
                        CryNoJump
                    }
                } else {
                    CryMaybe(n + 1)
                }
            }
            CryChannel(n) => {
                if n == 2 {
                    if ch == 4 {
                        Restore(0)
                    } else {
                        GoBackCall
                    }
                } else {
                    CryChannel(n + 1)
                }
            }
            GoBackCall => RewindCall,
            RewindCall => CryCheck(0),
            CryCheck(n) => {
                if n == 2 {
                    if id >= 20 {
                        CryEnd(0)
                    } else {
                        CryEnd(3)
                    }
                } else {
                    CryCheck(n + 1)
                }
            }
            CryEnd(3) => CryNo(0),
            CryEnd(n) => {
                if n == 1 && id == 134 {
                    CryNo(0)
                } else if n == 2 {
                    if id < 134 {
                        CryYes
                    } else {
                        CryNo(0)
                    }
                } else {
                    CryEnd(n + 1)
                }
            }
            CryYes => CryNo(2),
            CryNo(n) => {
                if n == 2 {
                    RewindBranch
                } else {
                    CryNo(n + 1)
                }
            }
            RewindBranch => Rewind(0),
            Rewind(n) => {
                if n == 12 {
                    self.result.command_ptr_write = Some(
                        self.input
                            .command_ptr
                            .expect("validated source pointer read")
                            .wrapping_sub(1),
                    );
                }
                if n == 14 {
                    ConditionalRet
                } else {
                    Rewind(n + 1)
                }
            }
            ConditionalRet => Done,
            Restore(n) => {
                if n == 1 {
                    self.result.volume_restore = self.input.saved_volume;
                }
                if n == 3 {
                    self.result.clear_saved_volume = true;
                    Clear(0)
                } else {
                    Restore(n + 1)
                }
            }
            Clear(n) => {
                if n == 2 {
                    self.result.ended = true;
                }
                if n == 3 {
                    Done
                } else {
                    Clear(n + 1)
                }
            }
            Done => Done,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        input: ChannelEndInput,
        result: ChannelEndResult,
        trace: alloc::vec::Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: alloc::vec::Vec<Case>,
    }
    #[test]
    fn ending_tails_match_original_at_every_instruction_and_source_pointer_wrap() {
        let f: Fixture =
            serde_json::from_str(include_str!("channel_end_instructions_golden.json")).unwrap();
        assert_eq!(f.cases.len(), 660);
        for (index, c) in f.cases.into_iter().enumerate() {
            let mut w = ChannelEndInstructions::new(c.input).unwrap();
            let mut cycles = 0;
            for (pc, t) in c.trace {
                assert_eq!(w.result(), None);
                let frozen = w.clone();
                assert_eq!(w.try_step(|_| Err("pause")), Err("pause"));
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                w.try_step(|actual| {
                    assert_eq!(actual, t, "case {index} source PC {pc:04x}");
                    Ok::<(), ()>(())
                })
                .unwrap();
                cycles += u32::from(t);
                assert_eq!(w.cycles(), cycles);
            }
            assert_eq!(w.result(), Some(c.result), "case {index}");
            assert!(w
                .try_step(|_| -> Result<(), ()> {
                    panic!("ending tail must not replay a write or RET")
                })
                .unwrap());
        }
    }
    #[test]
    fn unread_optional_state_is_not_fabricated_and_required_state_cannot_default() {
        let input = ChannelEndInput {
            bank: 31,
            channel: 3,
            source_sfx_id: 0,
            flags1: 4,
            flags2: 1,
            disable_output: None,
            saved_volume: None,
            panning: None,
            command_ptr: None,
        };
        let mut w = ChannelEndInstructions::new(input).unwrap();
        while w.result().is_none() {
            w.try_step(|_| Ok::<(), ()>(())).unwrap();
        }
        let result = w.result().unwrap();
        assert!(result.ended);
        assert_eq!(result.flags1, 0);
        assert_eq!(result.flags2, 0);
        assert_eq!(result.panning_write, None);
        assert_eq!(result.command_ptr_write, None);
        assert_eq!(result.volume_restore, None);
        assert!(ChannelEndInstructions::new(ChannelEndInput { bank: 8, ..input }).is_none());
        assert!(ChannelEndInstructions::new(ChannelEndInput { flags1: 2, ..input }).is_none());
        assert!(ChannelEndInstructions::new(ChannelEndInput {
            channel: 0,
            ..input
        })
        .is_none());
        assert!(ChannelEndInstructions::new(ChannelEndInput {
            channel: 6,
            ..input
        })
        .is_none());
        assert!(ChannelEndInstructions::new(ChannelEndInput {
            source_sfx_id: 20,
            ..input
        })
        .is_none());
        assert!(ChannelEndInstructions::new(ChannelEndInput {
            channel: 4,
            source_sfx_id: 20,
            ..input
        })
        .is_none());
    }
}
