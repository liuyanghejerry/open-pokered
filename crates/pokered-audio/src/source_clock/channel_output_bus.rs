//! Retained channel-helper APU stores, attached to their actual instructions.
//! Inputs are immutable owner-supplied values, never inferred port defaults.
use super::{ChannelOutputInstructions, OutputInput, OutputRoutine, Stage};
use crate::source_clock::apu_bus::{ApuAccess, ApuInstruction, BusStepError, NoteBusInput};

#[cfg(test)]
#[path = "output_live_bus_tests.rs"]
mod live_tests;

impl ChannelOutputInstructions {
    /// Whether this helper's actual path consumes a wave pattern.
    pub fn requires_wave(&self) -> bool {
        self.routine == OutputRoutine::WaveFrequency && self.input.channel % 4 == 2
    }

    /// Execute one instruction through the live APU owner. NR51 is sampled by
    /// each original LDH instruction, including the second read on the panning
    /// path. Accepted values remain in this CPU cursor until their later store.
    pub fn try_step_with_bus<E>(
        &mut self,
        input: NoteBusInput,
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, BusStepError<E>> {
        let Some(tcycles) = self.next_cycles() else {
            return Ok(true);
        };
        if self.requires_wave() && input.wave.is_none() {
            return Err(BusStepError::MissingWave);
        }
        let channel = self.input.channel;
        let base = [0xff10, 0xff15, 0xff1a, 0xff1f][usize::from(channel % 4)];
        let instruction = match self.stage {
            Stage::EnablePrefix(3) | Stage::Panning(5) => ApuInstruction::read(tcycles, 0xff25),
            Stage::EnableStore(1) => {
                let nr51 = self.nr51.expect("live helper passed its NR51 read");
                let mask = 0x11u8 << (channel % 4);
                let value = if channel == 7 || (channel < 4 && !self.input.paired_sfx_active) {
                    (nr51 & !mask) | (input.panning & mask)
                } else {
                    nr51 | mask
                };
                ApuInstruction::write(tcycles, 0xff25, value)
            }
            Stage::DutyStore(0) => ApuInstruction::write(
                tcycles,
                base + 1,
                if channel % 4 == 2 {
                    input.note_delay
                } else {
                    (input.note_delay & 63) | input.duty
                },
            ),
            Stage::WaveData(12) => ApuInstruction::write(tcycles, 0xff1a, 0),
            Stage::WaveCopy(byte, 2) => ApuInstruction::write(
                tcycles,
                0xff30 + u16::from(byte),
                input.wave.expect("prevalidated wave pattern")[usize::from(byte)],
            ),
            Stage::WaveEnd(1) => ApuInstruction::write(tcycles, 0xff1a, 0x80),
            Stage::WaveStore(0) | Stage::ModifierStore(2) => {
                ApuInstruction::write(tcycles, base + 3, self.frequency as u8)
            }
            Stage::WaveStore(2) | Stage::ModifierStore(4) => {
                ApuInstruction::write(tcycles, base + 4, (self.frequency >> 8) as u8)
            }
            _ => ApuInstruction::plain(tcycles),
        };
        let value = execute(instruction).map_err(BusStepError::Execute)?;
        if matches!(instruction.access, Some(ApuAccess::Read { .. })) {
            self.nr51 = Some(value.expect("accepted APU read supplies its actual value"));
        }
        self.advance();
        self.cycles += u32::from(tcycles);
        Ok(self.finished())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ApuWrite {
    pub address: u16,
    pub value: u8,
}

/// Only values actually read by the selected routine/path are required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum OutputBusInput {
    RegisterPointer,
    DutyAndLength { note_delay: u8, duty: Option<u8> },
    EnableChannel { nr51: u8, panning: Option<u8> },
    WaveFrequency { wave: Option<[u8; 16]> },
}

/// Owns the existing instruction cursor and immutable data needed for its stores.
/// It does not own/tick an APU, a second sequencer or an independent CPU clock.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ChannelOutputBusInstructions {
    cpu: ChannelOutputInstructions,
    bus: OutputBusInput,
}
impl ChannelOutputBusInstructions {
    pub fn new(input: OutputInput, bus: OutputBusInput) -> Option<Self> {
        let wave_channel = input.channel % 4 == 2;
        let routine = match bus {
            OutputBusInput::RegisterPointer => OutputRoutine::RegisterPointer,
            OutputBusInput::DutyAndLength { duty, .. } => {
                if !wave_channel && duty.is_none() {
                    return None;
                }
                OutputRoutine::DutyAndLength
            }
            OutputBusInput::EnableChannel { panning, .. } => {
                if Self::uses_panning(input) && panning.is_none() {
                    return None;
                }
                OutputRoutine::EnableChannel
            }
            OutputBusInput::WaveFrequency { wave } => {
                if wave_channel && wave.is_none() {
                    return None;
                }
                OutputRoutine::WaveFrequency
            }
        };
        Some(Self {
            cpu: ChannelOutputInstructions::new(routine, input)?,
            bus,
        })
    }
    fn uses_panning(input: OutputInput) -> bool {
        input.channel == 7 || (input.channel < 4 && !input.paired_sfx_active)
    }
    pub fn cycles(&self) -> u32 {
        self.cpu.cycles()
    }
    pub fn finished(&self) -> bool {
        self.cpu.finished()
    }
    pub fn frequency(&self) -> Option<u16> {
        self.cpu.frequency()
    }

    fn next_write(&self) -> Option<ApuWrite> {
        let channel = self.cpu.input.channel;
        let base = [0xff10, 0xff15, 0xff1a, 0xff1f][usize::from(channel % 4)];
        let (address, value) = match (self.cpu.stage, self.bus) {
            (Stage::DutyStore(0), OutputBusInput::DutyAndLength { note_delay, duty }) => (
                base + 1,
                if channel % 4 == 2 {
                    note_delay
                } else {
                    (note_delay & 63) | duty.expect("validated duty")
                },
            ),
            (Stage::EnableStore(1), OutputBusInput::EnableChannel { nr51, panning }) => {
                let mask = 0x11u8 << (channel % 4);
                let value = if Self::uses_panning(self.cpu.input) {
                    (nr51 & !mask) | (panning.expect("validated panning") & mask)
                } else {
                    nr51 | mask
                };
                (0xff25, value)
            }
            (Stage::WaveData(12), OutputBusInput::WaveFrequency { .. }) => (0xff1a, 0),
            (Stage::WaveCopy(byte, 2), OutputBusInput::WaveFrequency { wave }) => (
                0xff30 + u16::from(byte),
                wave.expect("validated wave")[usize::from(byte)],
            ),
            (Stage::WaveEnd(1), OutputBusInput::WaveFrequency { .. }) => (0xff1a, 0x80),
            (
                Stage::WaveStore(0) | Stage::ModifierStore(2),
                OutputBusInput::WaveFrequency { .. },
            ) => (base + 3, self.cpu.frequency as u8),
            (
                Stage::WaveStore(2) | Stage::ModifierStore(4),
                OutputBusInput::WaveFrequency { .. },
            ) => (base + 4, (self.cpu.frequency >> 8) as u8),
            _ => return None,
        };
        Some(ApuWrite { address, value })
    }

    /// Accept one instruction and its associated store together. Refuse before
    /// advancing the clock or applying a write. Success means the owner accepted
    /// both; retrying a completed cursor calls nothing. Instruction retirement
    /// association is provided here, not subinstruction hardware bus timing.
    /// The owner must keep source inputs/ports exclusive until the helper returns.
    pub fn try_step<E>(
        &mut self,
        execute: impl FnOnce(u8, Option<ApuWrite>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let write = self.next_write();
        self.cpu.try_step(|t| execute(t, write))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    #[derive(serde::Deserialize)]
    struct Case {
        input: OutputInput,
        bus: OutputBusInput,
        trace: Vec<(u16, u8)>,
        writes: Vec<(usize, u16, u16, u8)>,
        frequency: Option<u16>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }
    #[test]
    fn all_helper_apu_stores_match_original_values_and_instruction_boundaries() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("channel_output_bus_golden.json")).unwrap();
        assert_eq!(fixture.cases.len(), 930);
        let mut write_count = 0;
        for (case_index, case) in fixture.cases.into_iter().enumerate() {
            let mut work = ChannelOutputBusInstructions::new(case.input, case.bus).unwrap();
            let mut writes = Vec::new();
            for (index, (pc, cycles)) in case.trace.iter().enumerate() {
                let before = work.clone();
                assert_eq!(work.try_step(|_, _| Err("blocked")), Err("blocked"));
                assert_eq!(work, before);
                work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
                let done = work
                    .try_step(|actual, write| {
                        assert_eq!(
                            actual, *cycles,
                            "case{case_index} instruction{index} PC{pc:04x}"
                        );
                        if let Some(write) = write {
                            writes.push((index, *pc, write.address, write.value));
                        }
                        Ok::<(), ()>(())
                    })
                    .unwrap();
                assert_eq!(done, index + 1 == case.trace.len());
            }
            assert_eq!(writes, case.writes, "case{case_index}");
            write_count += writes.len();
            assert_eq!(work.frequency(), case.frequency);
            assert!(work
                .try_step(|_, _| -> Result<(), ()> { panic!("repeated store") })
                .unwrap());
        }
        assert_eq!(write_count, 4320);
    }
    #[test]
    fn unknown_inputs_are_rejected_instead_of_substituting_zero_ports() {
        let input = OutputInput {
            bank: 2,
            channel: 0,
            paired_sfx_active: false,
            source_sfx_id: 0,
            frequency_modifier: 0,
            frequency: 0x550,
        };
        assert!(ChannelOutputBusInstructions::new(
            input,
            OutputBusInput::DutyAndLength {
                note_delay: 7,
                duty: None
            }
        )
        .is_none());
        assert!(ChannelOutputBusInstructions::new(
            input,
            OutputBusInput::EnableChannel {
                nr51: 0xa5,
                panning: None
            }
        )
        .is_none());
        let wave = OutputInput {
            channel: 2,
            ..input
        };
        assert!(ChannelOutputBusInstructions::new(
            wave,
            OutputBusInput::WaveFrequency { wave: None }
        )
        .is_none());
        // The original wave channel skips duty, and pulse output never reads a wave.
        assert!(ChannelOutputBusInstructions::new(
            wave,
            OutputBusInput::DutyAndLength {
                note_delay: 255,
                duty: None
            }
        )
        .is_some());
        assert!(ChannelOutputBusInstructions::new(
            input,
            OutputBusInput::WaveFrequency { wave: None }
        )
        .is_some());
        // Pulse SFX bypass stereo panning, but noise SFX still reads it.
        assert!(ChannelOutputBusInstructions::new(
            OutputInput {
                channel: 4,
                ..input
            },
            OutputBusInput::EnableChannel {
                nr51: 0xa5,
                panning: None
            }
        )
        .is_some());
        assert!(ChannelOutputBusInstructions::new(
            OutputInput {
                channel: 7,
                ..input
            },
            OutputBusInput::EnableChannel {
                nr51: 0xa5,
                panning: None
            }
        )
        .is_none());
    }
}
