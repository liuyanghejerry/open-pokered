//! Resumable native work from `home/fade_audio.asm`.
//!
//! This is the first child of VBlank's audio section, not a second sequencer
//! or a completed audio frame. All reads/writes use the existing manager/APU.
//! The caller must resume the banked music child before TrackPlayTime/RETI;
//! calling the atomic `update_frame` as well would execute the fade twice.
//!
//! Time origin is the start of the opcode-fetch M1, in whole M-cycles:
//! absolute data accesses start at +12 and LDH data accesses at +8 T-cycles.
//! This is not a T-strobe claim. PyBoy Python memory-method hook timestamps
//! have a different per-opcode origin and are not direct bus certification.

use crate::audio_manager::{AudioManager, FadeState};

/// Relevant live owner accesses. These are logical ports, not a fabricated
/// full WRAM image: the native manager stores the fade-control/status bits as
/// typed flags rather than preserving the original bytes' unrelated bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioAccess {
    FadeActive(bool),
    NoFadeOut(bool),
    ReadCounter(u8),
    ReadReload(u8),
    WriteCounter(u8),
    ReadNr50(u8),
    WriteNr50(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioReturn {
    /// RET: shared stack reads at +4/+8, total 16 T-cycles.
    Unconditional,
    /// RET NZ taken: shared stack reads at +8/+12, total 20 T-cycles.
    ConditionalTaken,
}

/// Compatible with the display child's reservation/absolute-time protocol.
/// No method advances a song or owns another clock.
pub trait AudioSourceBus {
    type Error;
    /// Err must leave both bus and manager entirely unchanged.
    fn begin_instruction(&mut self, tcycles: u8) -> Result<(), Self::Error>;
    fn elapse(&mut self, tcycles: u8);
    /// Observe an access *after* the live owner was read/written at this time.
    fn access(&mut self, access: AudioAccess);
    /// Consume this accepted RET using the caller-owned CPU and physical
    /// stack, including the offsets/total documented by `AudioReturn`.
    /// Do not release HALT here: the enclosing ISR still has to execute RETI.
    fn return_from_call(&mut self, kind: AudioReturn);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioWorkUnsupported {
    AlreadyPending,
    FadeCompletePlaySound,
    /// The source decrements each nibble with wrapping arithmetic, whereas
    /// e533's public master-volume setter clamps each side to 0..7.
    UnrepresentableVolume(u8),
}

#[derive(Debug, PartialEq, Eq)]
pub enum AudioWorkError<E> {
    Unsupported(AudioWorkUnsupported),
    Reservation(E),
}

/// Source line identifies the actual opcode, rather than a captured PC/cost
/// stream. Conditional branches report their selected opcode duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioInstruction {
    pub source_line: u8,
    pub tcycles: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    ReadControl,
    TestControl,
    BranchControl,
    ReadStatus,
    TestStatus,
    ConditionalReturn,
    LoadFull,
    WriteFull,
    ReturnFull,
    ReturnCounter,
    ReturnVolume,
    ReadCounter,
    TestCounter,
    BranchCounter,
    DecrementCounter,
    StoreCounter,
    ReadReload,
    StoreReload,
    ReadVolume,
    TestVolume,
    BranchVolume,
    SaveVolume,
    MaskRight,
    DecrementRight,
    SaveRight,
    LoadVolume,
    MaskLeft,
    SwapLeft,
    DecrementLeft,
    UnswapLeft,
    Combine,
    WriteVolume,
    Done,
}

/// Only the suspended source cursor and scratch registers live here. Song,
/// channel, fade-counter and APU state stay in their original owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FadeAudioWork {
    phase: Phase,
    a: u8,
    b: u8,
    c: u8,
    condition: bool,
}

impl AudioManager {
    /// Enter VBlank's FadeOutAudio call without executing an opcode. This
    /// neither updates music channels nor completes an IRQ/audio frame.
    pub fn begin_fade_audio_work(&mut self) -> Result<(), AudioWorkUnsupported> {
        if self.source_fade.is_some() {
            return Err(AudioWorkUnsupported::AlreadyPending);
        }
        let work = FadeAudioWork::begin(self)?;
        self.source_fade = Some(work);
        Ok(())
    }

    pub fn fade_audio_work(&self) -> Option<&FadeAudioWork> {
        self.source_fade.as_ref()
    }

    /// Execute at most one actual source opcode on the same live owner.
    /// Reservation Err restores the cursor and has no manager effects.
    /// None means there is no pending child; a successful RET clears it.
    pub fn step_fade_audio_work<B: AudioSourceBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<AudioInstruction>, AudioWorkError<B::Error>> {
        let Some(mut work) = self.source_fade.take() else {
            return Ok(None);
        };
        let result = work.step(self, bus);
        if !work.finished() {
            self.source_fade = Some(work);
        }
        result
    }
}

fn supported_volume(value: u8) -> Result<(), AudioWorkUnsupported> {
    if value == 0 {
        Err(AudioWorkUnsupported::FadeCompletePlaySound)
    } else if value & 0x88 != 0 || value & 0x0f == 0 || value & 0xf0 == 0 {
        Err(AudioWorkUnsupported::UnrepresentableVolume(value))
    } else {
        Ok(())
    }
}

impl FadeAudioWork {
    pub(crate) fn begin(manager: &AudioManager) -> Result<Self, AudioWorkUnsupported> {
        // Validate unsupported paths without advancing the live owner. The
        // NR50 check is repeated immediately before its real read, so even
        // an invalid external mutation cannot partially execute that opcode.
        if manager.engine.fade_state() != FadeState::None
            && manager.engine.sequencer.fade_counter == 0
        {
            supported_volume(manager.engine.apu.read_register(0xff24))?;
        }
        Ok(Self {
            phase: Phase::ReadControl,
            a: 0,
            b: 0,
            c: 0,
            condition: false,
        })
    }

    pub fn finished(&self) -> bool {
        self.phase == Phase::Done
    }

    pub fn instruction(&self) -> Option<AudioInstruction> {
        use Phase::*;
        let (source_line, tcycles) = match self.phase {
            ReadControl => (2, 16),
            TestControl => (3, 4),
            BranchControl => (4, if self.condition { 12 } else { 8 }),
            ReadStatus => (5, 16),
            TestStatus => (6, 8),
            ConditionalReturn => (7, if self.condition { 20 } else { 8 }),
            LoadFull => (8, 8),
            WriteFull => (9, 12),
            ReturnFull => (10, 16),
            ReturnCounter => (17, 16),
            ReturnVolume => (35, 16),
            ReadCounter => (12, 16),
            TestCounter => (13, 4),
            BranchCounter => (14, if self.condition { 12 } else { 8 }),
            DecrementCounter => (15, 4),
            StoreCounter => (16, 16),
            ReadReload => (19, 16),
            StoreReload => (20, 16),
            ReadVolume => (21, 12),
            TestVolume => (22, 4),
            BranchVolume => (23, if self.condition { 12 } else { 8 }),
            SaveVolume => (24, 4),
            MaskRight => (25, 8),
            DecrementRight => (26, 4),
            SaveRight => (27, 4),
            LoadVolume => (28, 4),
            MaskLeft => (29, 8),
            SwapLeft => (30, 8),
            DecrementLeft => (31, 4),
            UnswapLeft => (32, 8),
            Combine => (33, 4),
            WriteVolume => (34, 12),
            Done => return None,
        };
        Some(AudioInstruction {
            source_line,
            tcycles,
        })
    }

    pub(crate) fn step<B: AudioSourceBus>(
        &mut self,
        manager: &mut AudioManager,
        bus: &mut B,
    ) -> Result<Option<AudioInstruction>, AudioWorkError<B::Error>> {
        use Phase::*;
        let Some(instruction) = self.instruction() else {
            return Ok(None);
        };
        if self.phase == ReadVolume {
            supported_volume(manager.engine.apu.read_register(0xff24))
                .map_err(AudioWorkError::Unsupported)?;
        }
        bus.begin_instruction(instruction.tcycles)
            .map_err(AudioWorkError::Reservation)?;
        let read_offset = match self.phase {
            ReadControl | ReadStatus | ReadCounter | StoreCounter | ReadReload | StoreReload => 12,
            WriteFull | ReadVolume | WriteVolume => 8,
            _ => 0,
        };
        if read_offset != 0 {
            bus.elapse(read_offset);
        }
        self.phase = match self.phase {
            ReadControl => {
                self.condition = manager.engine.fade_state() != FadeState::None;
                bus.access(AudioAccess::FadeActive(self.condition));
                TestControl
            }
            TestControl => BranchControl,
            BranchControl => {
                if self.condition {
                    ReadCounter
                } else {
                    ReadStatus
                }
            }
            ReadStatus => {
                self.condition = manager.engine.no_audio_fade_out();
                bus.access(AudioAccess::NoFadeOut(self.condition));
                TestStatus
            }
            TestStatus => ConditionalReturn,
            ConditionalReturn => {
                if self.condition {
                    bus.return_from_call(AudioReturn::ConditionalTaken);
                    Done
                } else {
                    LoadFull
                }
            }
            LoadFull => {
                self.a = 0x77;
                WriteFull
            }
            WriteFull => {
                manager.engine.set_master_volume(7, 7);
                bus.access(AudioAccess::WriteNr50(self.a));
                ReturnFull
            }
            ReadCounter => {
                self.a = manager.engine.sequencer.fade_counter;
                bus.access(AudioAccess::ReadCounter(self.a));
                TestCounter
            }
            TestCounter => {
                self.condition = self.a == 0;
                BranchCounter
            }
            BranchCounter => {
                if self.condition {
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
                manager.engine.sequencer.fade_counter = self.a;
                bus.access(AudioAccess::WriteCounter(self.a));
                ReturnCounter
            }
            ReadReload => {
                self.a = manager.engine.fade_counter_reload;
                bus.access(AudioAccess::ReadReload(self.a));
                StoreReload
            }
            StoreReload => {
                manager.engine.sequencer.fade_counter = self.a;
                bus.access(AudioAccess::WriteCounter(self.a));
                ReadVolume
            }
            ReadVolume => {
                self.a = manager.engine.apu.read_register(0xff24);
                bus.access(AudioAccess::ReadNr50(self.a));
                TestVolume
            }
            TestVolume => {
                self.condition = self.a == 0;
                BranchVolume
            }
            BranchVolume => {
                debug_assert!(!self.condition);
                SaveVolume
            }
            SaveVolume => {
                self.b = self.a;
                MaskRight
            }
            MaskRight => {
                self.a &= 0x0f;
                DecrementRight
            }
            DecrementRight => {
                self.a = self.a.wrapping_sub(1);
                SaveRight
            }
            SaveRight => {
                self.c = self.a;
                LoadVolume
            }
            LoadVolume => {
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
                UnswapLeft
            }
            UnswapLeft => {
                self.a = self.a.rotate_left(4);
                Combine
            }
            Combine => {
                self.a |= self.c;
                WriteVolume
            }
            WriteVolume => {
                manager.engine.set_master_volume(self.a >> 4, self.a & 0x0f);
                bus.access(AudioAccess::WriteNr50(self.a));
                ReturnVolume
            }
            ReturnFull | ReturnCounter | ReturnVolume => {
                bus.return_from_call(AudioReturn::Unconditional);
                Done
            }
            Done => unreachable!(),
        };
        // RET's physical accesses and all its cycles belong to the shared
        // stack owner; ordinary instructions consume their remaining cycles.
        if instruction.source_line != 10
            && instruction.source_line != 17
            && instruction.source_line != 35
            && !(instruction.source_line == 7 && self.phase == Done)
        {
            bus.elapse(instruction.tcycles - read_offset);
        }
        Ok(Some(instruction))
    }
}

#[cfg(test)]
#[path = "source_work_tests.rs"]
mod tests;
