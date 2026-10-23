//! APU stores attached to the existing Audio1/3 timer/effect instruction cursors.
//! The caller owns the clock, actual APU and once-only native state commit.
use super::{
    AudioClockError, ChannelEffectWorkInput, EffectInstructionWork, EffectStage, FrameStage,
    TimerFrameContext, TimerFrameWork,
};
use crate::source_clock::channel_output_instructions::bus::ApuWrite;

impl EffectInstructionWork {
    /// The supported paths only store the vibrato frequency low byte. Duty
    /// rotation and reachable pitch slides remain rejected by the CPU cursor.
    pub(crate) fn pending_apu_write(&self) -> Option<ApuWrite> {
        if self.stage != EffectStage::Tail(0) {
            return None;
        }
        let input = self.input;
        Some(ApuWrite {
            address: [0xff13, 0xff18, 0xff1d, 0xff22][usize::from(input.channel % 4)],
            value: if input.flags1 & 8 != 0 {
                input
                    .frequency_low
                    .saturating_sub(input.vibrato_extent & 15)
            } else {
                input
                    .frequency_low
                    .saturating_add(input.vibrato_extent >> 4)
            },
        })
    }

    /// Advance this same CPU cursor with its actual instruction's APU store.
    /// The enclosing frame must validate bank 2/31 and retain exclusive source
    /// inputs. Refusal must precede any clock/write side effect. These are store
    /// instruction associations, not subinstruction hardware access timestamps.
    pub fn try_step_with_apu<E>(
        &mut self,
        execute: impl FnOnce(u8, Option<ApuWrite>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let Some(cycles) = self.peek() else {
            return Ok(true);
        };
        execute(cycles, self.pending_apu_write())?;
        self.advance();
        Ok(self.finished())
    }
}

/// One explicitly banked effect call. No APU, clock or second sequencer is owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct EffectBusInstructions {
    cpu: EffectInstructionWork,
    bank: u8,
    cycles: u32,
}

impl EffectBusInstructions {
    pub fn new(bank: u8, input: ChannelEffectWorkInput) -> Option<Self> {
        if !matches!(bank, 2 | 31) {
            return None;
        }
        Some(Self {
            cpu: EffectInstructionWork::new(input)?,
            bank,
            cycles: 0,
        })
    }
    pub fn bank(&self) -> u8 {
        self.bank
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn finished(&self) -> bool {
        self.cpu.finished()
    }
    pub fn try_step<E>(
        &mut self,
        execute: impl FnOnce(u8, Option<ApuWrite>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let Some(cycles) = self.cpu.peek() else {
            return Ok(true);
        };
        let done = self.cpu.try_step_with_apu(execute)?;
        self.cycles += u32::from(cycles);
        Ok(done)
    }
}

/// A complete timer-only update using the existing frame cursor and its bank,
/// mute and active-channel validation. Native channel state is not committed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct TimerFrameBusInstructions {
    cpu: TimerFrameWork,
}

impl TimerFrameBusInstructions {
    pub fn new(context: TimerFrameContext) -> Result<Self, AudioClockError> {
        // The scalar paired flag must describe the actual supplied channel-ID
        // table. Otherwise an audible branch could be replaced by an early RET.
        for channel in 0..4 {
            if let Some(input) = context.effects[channel] {
                if input.paired_sfx_active != (context.sound_ids[channel + 4] != 0) {
                    return Err(AudioClockError::UnsupportedEffect(channel as u8));
                }
            }
        }
        Ok(Self {
            cpu: TimerFrameWork::new(context)?,
        })
    }
    pub fn context(&self) -> &TimerFrameContext {
        self.cpu.context()
    }
    pub fn cycles(&self) -> u32 {
        self.cpu.cycles()
    }
    pub fn finished(&self) -> bool {
        self.cpu.finished()
    }
    /// The owner accepts the instruction and its store together, or refuses
    /// before either takes effect. Completion is Audio*_UpdateMusic's RET only.
    pub fn try_step<E>(
        &mut self,
        execute: impl FnOnce(u8, Option<ApuWrite>) -> Result<(), E>,
    ) -> Result<bool, E> {
        let write = if let FrameStage::Effect(effect) = &self.cpu.stage {
            effect.pending_apu_write()
        } else {
            None
        };
        self.cpu.try_step(|cycles| execute(cycles, write))
    }
}

#[cfg(test)]
#[path = "timer_bus_tests.rs"]
mod tests;
