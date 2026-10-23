use super::{After, NotePitchInstructions, Stage};
use crate::source_clock::apu_bus::{ApuAccess, ApuInstruction, BusStepError, NoteBusInput};

impl NotePitchInstructions {
    /// The source loads a wave only for an audible, non-rest channel-3 note.
    pub fn requires_wave(&self) -> bool {
        self.input.note < 0xc0 && self.input.channel == 2 && !self.input.paired_sfx_active
    }

    /// The command-result input belongs to this retained child. Reject absent
    /// wave data before accepting any instruction or nested output helper.
    pub fn try_step_with_bus<E>(
        &mut self,
        input: NoteBusInput,
        execute: impl FnOnce(ApuInstruction) -> Result<Option<u8>, E>,
    ) -> Result<bool, BusStepError<E>> {
        if self.finished() {
            return Ok(true);
        }
        if self.requires_wave() && input.wave.is_none() {
            return Err(BusStepError::MissingWave);
        }
        if let Stage::Output { work, after } = &mut self.stage {
            let mut accepted = 0;
            let done = work.try_step_with_bus(input, |instruction| {
                let value = execute(instruction)?;
                accepted = instruction.tcycles;
                Ok(value)
            })?;
            self.cycles += u32::from(accepted);
            if done {
                self.stage = match *after {
                    After::RestRestart => Stage::RestRestart(0),
                    After::Envelope => Stage::EnvelopeStore(0),
                    After::Duty => Stage::EnableCall,
                    After::Enable => Stage::RestorePerfect(0),
                    After::Wave => {
                        self.result.wave_return_frequency = work.frequency();
                        Stage::Return
                    }
                };
            }
            return Ok(false);
        }
        if matches!(self.stage, Stage::Frequency(_)) {
            return self
                .try_step(|t| execute(ApuInstruction::plain(t)).map(|_| ()))
                .map_err(BusStepError::Execute);
        }
        let tcycles = self.next_cycles().expect("unfinished pitch instruction");
        let base = [0xff10, 0xff15, 0xff1a][usize::from(self.input.channel)];
        let instruction = match self.stage {
            Stage::EnvelopeStore(0) => ApuInstruction::write(tcycles, base + 2, input.volume),
            Stage::RestRestart(1) => ApuInstruction::write(tcycles, base + 2, 8),
            Stage::RestRestart(4) => ApuInstruction::write(tcycles, base + 4, 0x80),
            Stage::RestWave(3) => ApuInstruction::read(tcycles, 0xff25),
            Stage::RestWave(5) => ApuInstruction::write(
                tcycles,
                0xff25,
                self.rest_nr51.expect("wave rest passed its NR51 read") & !0x44,
            ),
            _ => ApuInstruction::plain(tcycles),
        };
        let value = execute(instruction).map_err(BusStepError::Execute)?;
        if matches!(instruction.access, Some(ApuAccess::Read { .. })) {
            self.rest_nr51 = Some(value.expect("accepted APU read supplies its actual value"));
        }
        self.advance();
        self.cycles += u32::from(tcycles);
        Ok(self.finished())
    }
}
