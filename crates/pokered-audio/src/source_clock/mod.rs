//! Source Audio1/Audio3 instruction work on the live native sequencer state.
//!
//! The native sequencer remains the only channel owner. This port describes
//! the branch work of its supported update, then checks the actual native
//! delay result; it never applies a captured channel/result or service cost.
//! The caller must consume the returned T-cycles in its foreground scheduler.
pub mod apu_bus;
mod audio_math_work;
mod audio_next_note_work;
mod audio_note_dispatch_work;
mod audio_output_work;
mod audio_stream_work;
mod audio_work;
pub mod channel_end_instructions;
pub mod channel_output_instructions;
pub mod drum_start_instructions;
pub mod frame_instructions;
pub mod music_note_instructions;
pub mod note_math_instructions;
pub mod note_pitch_instructions;
pub mod note_preparation;
pub mod note_stream_instructions;
pub mod pitch_slide_instructions;
pub mod press_ab_instructions;
mod press_ab_work;
pub mod timer_work;
mod wave_patterns;

use crate::{sequencer::Sequencer, NOTE_FREQUENCIES};
use audio_note_dispatch_work::MusicCommandState;
use audio_work::{audio_dispatch_work, channel_effect_work_cycles, ChannelEffectWorkInput};
use pitch_slide_instructions::{PitchSlideResult, PitchSlideState};
use press_ab_work::{noise_next_note, press_ab_next_note, PressAbWork};
use timer_work::EffectInstructionWork;

fn noise_word(data: &[u8], offset: usize) -> u32 {
    u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap())
}
pub(super) fn noise_data_eq(expected: Option<&[u8]>, actual: &[u8]) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    if expected.len() != actual.len() {
        return false;
    }
    match actual.len() {
        0 => true,
        4 => noise_word(expected, 0) == noise_word(actual, 0),
        8 => {
            noise_word(expected, 0) == noise_word(actual, 0)
                && noise_word(expected, 4) == noise_word(actual, 4)
        }
        16 => {
            noise_word(expected, 0) == noise_word(actual, 0)
                && noise_word(expected, 4) == noise_word(actual, 4)
                && noise_word(expected, 8) == noise_word(actual, 8)
                && noise_word(expected, 12) == noise_word(actual, 12)
        }
        19 => {
            noise_word(expected, 0) == noise_word(actual, 0)
                && noise_word(expected, 4) == noise_word(actual, 4)
                && noise_word(expected, 8) == noise_word(actual, 8)
                && noise_word(expected, 12) == noise_word(actual, 12)
                && expected[16] == actual[16]
                && expected[17] == actual[17]
                && expected[18] == actual[18]
        }
        _ => {
            let mut offset = 0;
            while offset + 4 <= actual.len() {
                if noise_word(expected, offset) != noise_word(actual, offset) {
                    return false;
                }
                offset += 4;
            }
            while offset < actual.len() {
                if expected[offset] != actual[offset] {
                    return false;
                }
                offset += 1;
            }
            true
        }
    }
}

fn canonical_noise_table(seq: &Sequencer) -> bool {
    seq.noise_instruments.len() == 19
        && seq.noise_instruments.iter().enumerate().all(|(i, data)| {
            noise_data_eq(
                press_ab_instructions::PressAbInstructions::noise_data(i as u8 + 1),
                data,
            )
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum AudioClockError {
    UnsupportedSoundBank,
    OutputUnavailable,
    ActiveSfx(u8),
    UnsupportedEffect(u8),
    UnsupportedCommand { channel: u8, command: Option<u8> },
    NativeStateMismatch(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct AudioFrameWork {
    pub cycles: u32,
}

/// Supported native song bank and live volume for the field IRQ handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioIrqContext {
    pub bank: u8,
    pub nr50: u8,
}

/// Admission and completion outcomes for guarded audio work. Source progress
/// and the native sequencer commit are distinct: preserve retained work after
/// any accepted instruction and follow the particular API's resume contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioAdmissionError<E> {
    /// Input rejected before this call accepts source work. Full-frame APIs use
    /// this only before their first instruction; legacy single-step callers
    /// must still preserve any progress accepted by earlier calls.
    Preflight(AudioClockError),
    /// No new instruction executed and the cursor is unchanged, but it belongs
    /// to the other CPU/bus API. It may already contain source effects; resume
    /// using its original mode rather than discarding it or falling back.
    WrongMode,
    /// The next source instruction was refused before its effects; the native
    /// sequencer has not committed. Earlier instructions in this or previous
    /// calls may have advanced the cursor/APU: retain the cursor to resume.
    Admission(E),
    /// Accepted source instructions already had effects, then a resume guard or
    /// end-of-call invariant failed before the native commit. Retain this
    /// terminal outcome; do not restart or replay the frame's earlier accesses.
    SourceFailed(AudioClockError),
    /// One native update already happened. Retrying would duplicate that tick.
    Advanced(AudioClockError),
}

#[derive(Clone, Copy)]
struct ExpectedDelay {
    delay: u8,
    ended_ptr: Option<usize>,
    fraction: u8,
    stream: Option<(usize, usize, u8, u8)>,
}

pub(crate) struct PreparedAudioFrame {
    work: AudioFrameWork,
    ids: [u8; 8],
    expected: [Option<ExpectedDelay>; 8],
    expected_tempo: u16,
    expected_panning: u8,
    expected_sfx: [Option<PressAbWork>; 4],
    expected_saved_frequency: [Option<u8>; 4],
    paused_modulation: [Option<(u8, u8, u8)>; 4],
    expected_slide: [Option<(PitchSlideResult, u16)>; 4],
    prepared_notes: [Option<note_preparation::PreparationResult>; 4],
    no_note_cycles: Option<u32>,
}

impl PreparedAudioFrame {
    pub(crate) fn work(&self) -> AudioFrameWork {
        self.work
    }

    /// This bounded port handles field music after the first ordinary native
    /// tick. Noise rests/termination and finite loops are
    /// included, together with finite PressAB and its paired music branches.
    /// Audio1/3 update code is identical after symbol normalization at the
    /// pinned original revision. Only their PlaySound header selection differs;
    /// this method observes an existing sequencer and does not start sounds.
    /// Canonical noise instruments include same-frame starts and busy drops.
    /// Other SFX and duty-rotation still stop explicitly.
    /// No inactive voice is substituted for an active unsupported channel.
    pub fn from_shared_music(
        seq: &Sequencer,
        song: frame_instructions::SourceSong,
    ) -> Result<Self, AudioClockError> {
        Self::from_shared_music_inner(seq, song, true)
    }

    /// Final-state preparation for a fresh source cursor. Its executed
    /// instructions own exact cycle accounting instead of a second effect run.
    pub fn from_shared_music_for_cursor(
        seq: &Sequencer,
        song: frame_instructions::SourceSong,
    ) -> Result<Self, AudioClockError> {
        Self::from_shared_music_inner(seq, song, false)
    }

    fn from_shared_music_inner(
        seq: &Sequencer,
        song: frame_instructions::SourceSong,
        exact_cycles: bool,
    ) -> Result<Self, AudioClockError> {
        let new_music_note = seq.channels[..4]
            .iter()
            .any(|ch| ch.active && ch.delay_counter <= 1);
        if seq.channels[3].active && new_music_note && !canonical_noise_table(seq) {
            return Err(AudioClockError::UnsupportedEffect(3));
        }
        let mut source_sfx = 0;
        let mut noise_id = 0;
        for i in 4..8 {
            let ch = &seq.channels[i];
            let valid_program = if !ch.active {
                true
            } else {
                match i {
                    4 | 5 => {
                        let id = press_ab_work::source_pulse_id(i, ch.sound_id, &ch.data);
                        if i == 4 {
                            source_sfx = id.unwrap_or(0);
                        }
                        id.is_some() && seq.sfx_tempo == 0x100
                    }
                    7 => {
                        noise_id =
                            press_ab_work::source_noise_id(ch.sound_id, &ch.data).unwrap_or(0);
                        noise_id != 0 && ch.duty_cycle == 0
                    }
                    _ => false,
                }
            };
            if ch.active
                && (!valid_program
                    || ch.note_speed != 1
                    || ch.flags1.bits() != 4
                    || ch.flags2.bits() != 0)
            {
                return Err(AudioClockError::ActiveSfx(i as u8));
            }
        }
        let mut expected_sfx = [None; 4];
        let mut expected_saved_frequency = [None; 4];
        let mut paused_modulation = [None; 4];
        let mut expected_slide = [None; 4];
        let mut prepared_notes = [None; 4];
        let mut ids = [0; 8];
        let mut effects = [0; 8];
        let mut expected: [Option<ExpectedDelay>; 8] = [None; 8];
        let mut tempo = seq.music_tempo;
        let mut panning = seq.stereo_panning;
        let mut fractions: [u8; 8] = core::array::from_fn(|i| seq.channels[i].delay_frac);
        // Scalar constructor only: canonical data is borrowed below, not copied
        // into a second sequencer or a speculative playback owner.
        let mut started_noise: Option<crate::ChannelState> = None;
        for (i, ch) in seq.channels.iter().enumerate() {
            let ch = if i == 7 {
                started_noise.as_ref().unwrap_or(ch)
            } else {
                ch
            };
            let data = if i == 7 && started_noise.is_some() {
                press_ab_instructions::PressAbInstructions::noise_data(ch.sound_id)
                    .expect("validated drum table")
            } else {
                &ch.data
            };
            // Native PALLET_TOWN is enum value zero. The source dispatch tests
            // original header IDs, where Pallet Town is 186, not channel silence.
            ids[i] = if !ch.active {
                0
            } else if i < 4 {
                song.raw_id()
            } else if i == 4 {
                source_sfx
            } else if i == 5 {
                press_ab_work::SOURCE_SWAP_ID
            } else if i == 7 {
                if started_noise.is_some() {
                    press_ab_work::source_noise_id(ch.sound_id, data)
                        .expect("validated noise program")
                } else {
                    noise_id
                }
            } else {
                ch.sound_id
            };
            if ids[i] == 0 {
                continue;
            }
            if matches!(i, 4 | 5 | 7) {
                if ch.delay_counter == 1
                    || (ch.delay_counter == 0
                        && ch.ptr == 0
                        && ch.duty_cycle == 0
                        && ch.volume_envelope == 0xf0
                        && ch.delay_frac == 0)
                {
                    // The ordinary native constructor's ready-zero maps to
                    // the original sound constructor's ready-one. This finite
                    // stream never loops back to offset zero.
                    let w = if i <= 5 && ch.sound_id == crate::sfx_data::SfxId::Swap as u8 {
                        press_ab_work::swap_next_note(song.bank(), i as u8, source_sfx,
                            data, ch.ptr, ch.delay_frac, ch.duty_cycle << 6)
                    } else if i == 4 {
                        press_ab_next_note(data, ch.ptr, ch.delay_frac, ch.duty_cycle << 6)
                    } else {
                        noise_next_note(ids[i], data, ch.ptr, ch.delay_frac, source_sfx)
                    }
                    .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
                    effects[i] = 8 + 12 + 8 + 8 + 8 + 16 + w.cycles;
                    expected[i] = Some(ExpectedDelay {
                        delay: w.delay,
                        fraction: w.fraction,
                        ended_ptr: if w.ended { Some(w.next_ptr) } else { None },
                        stream: if w.ended {
                            None
                        } else {
                            Some((
                                w.next_ptr,
                                ch.return_ptr,
                                ch.flags1.bits() & 2,
                                ch.loop_counter,
                            ))
                        },
                    });
                    expected_sfx[i - 4] = Some(w);
                    if i == 4 && w.ended {
                        source_sfx = 0;
                    }
                } else if ch.delay_counter > 1 {
                    let input = ChannelEffectWorkInput {
                        channel: i as u8,
                        paired_sfx_active: false,
                        note_delay: ch.delay_counter,
                        flags1: ch.flags1.bits(),
                        flags2: ch.flags2.bits(),
                        vibrato_delay: ch.vibrato.delay_counter,
                        vibrato_extent: ch.vibrato.extent,
                        vibrato_rate: ch.vibrato.rate,
                        frequency_low: ch.freq_lo_saved,
                    };
                    if exact_cycles {
                        effects[i] = channel_effect_work_cycles(input)
                            .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
                    } else {
                        EffectInstructionWork::new(input)
                            .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
                    }
                    expected[i] = Some(ExpectedDelay {
                        delay: ch.delay_counter - 1,
                        fraction: ch.delay_frac,
                        ended_ptr: None,
                        stream: None,
                    });
                } else {
                    return Err(AudioClockError::UnsupportedEffect(i as u8));
                }
                continue;
            }
            let paired_sfx_active = seq.channels[i + 4].active;
            let flags1 = ch.flags1.bits();
            let input = ChannelEffectWorkInput {
                channel: i as u8,
                paired_sfx_active,
                note_delay: ch.delay_counter,
                flags1,
                flags2: ch.flags2.bits(),
                vibrato_delay: ch.vibrato.delay_counter,
                vibrato_extent: ch.vibrato.extent,
                vibrato_rate: ch.vibrato.rate,
                frequency_low: ch.freq_lo_saved,
            };
            if ch.delay_counter == 1 {
                if i == 3 && flags1 & 2 == 0 && ch.data.get(ch.ptr) == Some(&0xff) {
                    effects[i] = 8
                        + 12
                        + 8
                        + 8
                        + 8
                        + 16
                        + audio_work::noise_channel_end_work_with_sfx(source_sfx)
                            .ok_or(AudioClockError::UnsupportedEffect(3))?;
                    expected[i] = Some(ExpectedDelay {
                        ended_ptr: Some(ch.ptr + 1),
                        delay: 0,
                        fraction: 0,
                        stream: None,
                    });
                    continue;
                }
                let state = MusicCommandState {
                    speed: ch.note_speed,
                    volume: if i == 2 {
                        (ch.volume_envelope & 0x30) << 1
                    } else {
                        ch.volume_envelope
                    },
                    duty: ch.duty_cycle << 6,
                    // The converted stream and native channel already keep
                    // the source opcode's raw octave nibble.
                    octave: ch.octave,
                    flags1,
                    vibrato_delay: ch.vibrato.delay_counter,
                };
                let stream = audio_stream_work::music_stream_work_with_drum(
                    i as u8,
                    &ch.data,
                    ch.ptr,
                    ch.return_ptr,
                    ch.loop_counter,
                    state,
                    ch.vibrato.delay_reload,
                    Some(seq.channels[7].active),
                )
                .ok_or(AudioClockError::UnsupportedCommand {
                    channel: i as u8,
                    command: ch.data.get(ch.ptr).copied(),
                })?;
                if let Some(new_tempo) = stream.tempo {
                    tempo = new_tempo;
                    fractions[..4].fill(0);
                    // A later channel resets earlier channels' fractional
                    // carries without re-timing their integer delays.
                    for earlier in &mut expected[..i] {
                        if let Some(earlier) = earlier {
                            earlier.fraction = 0;
                        }
                    }
                }
                if let Some(new_panning) = stream.panning {
                    panning = new_panning;
                }
                let mut result = audio_next_note_work::music_next_note_work_with_drum_prepared(
                    i as u8,
                    &stream,
                    ch.flags2.bits(),
                    fractions[i],
                    tempo,
                    ch.freq_lo_saved,
                    NOTE_FREQUENCIES.get((stream.note >> 4) as usize).copied(),
                    paired_sfx_active,
                    source_sfx,
                )
                .ok_or(AudioClockError::UnsupportedCommand {
                    channel: i as u8,
                    command: ch.data.get(ch.ptr).copied(),
                })?;
                expected_saved_frequency[i] = Some(result.frequency_low);
                if let Some(slide) = result.pitch_slide {
                    let mut frequency = slide.returned_frequency;
                    if !paired_sfx_active && slide.flags1 & 1 != 0 {
                        frequency =
                            (frequency & 0xff00) | u16::from((frequency as u8).wrapping_add(1));
                    }
                    expected_slide[i] = Some((
                        PitchSlideResult {
                            state: slide.state,
                            flags1: slide.flags1,
                        },
                        frequency,
                    ));
                }
                result.cycles += stream.control_cycles;
                prepared_notes[i] = Some(note_preparation::PreparationResult {
                    stream: result.stream,
                    fraction: result.fraction,
                    delay: result.delay,
                    boundary: if i == 3 {
                        note_preparation::PreparationBoundary::NoiseReturned
                    } else {
                        note_preparation::PreparationBoundary::NotePitch
                    },
                    cycles: result.preparation_cycles + stream.control_cycles,
                });
                effects[i] = 8 + 12 + 8 + 8 + 8 + 16 + result.cycles;
                expected[i] = Some(ExpectedDelay {
                    ended_ptr: None,
                    delay: result.delay,
                    fraction: result.fraction,
                    stream: Some((
                        stream.next_ptr,
                        stream.return_ptr,
                        stream.state.flags1 & 2,
                        stream.loop_counter,
                    )),
                });
                if let Some(drum) = stream.drum.filter(|drum| drum.started) {
                    // Source PlaySound leaves the fractional carry untouched.
                    // Canonical CHAN8 programs always keep it zero; refuse a
                    // foreign carry instead of silently normalizing its work.
                    if seq.channels[7].delay_frac != 0 {
                        return Err(AudioClockError::UnsupportedEffect(7));
                    }
                    let noise = started_noise.insert(crate::ChannelState::new());
                    noise.active = true;
                    noise.sound_id = drum.instrument;
                    noise.note_speed = 1;
                    noise.flags1.insert(crate::ChannelFlags1::NOISE_OR_SFX);
                }
            } else {
                // The generic native engine treats zero as a ready entry;
                // source zero wraps its countdown. Until those states have
                // an explicit common constructor, no timing is asserted.
                if ch.delay_counter == 0 {
                    return Err(AudioClockError::UnsupportedEffect(i as u8));
                }
                if paired_sfx_active {
                    paused_modulation[i] = Some((ch.vibrato.delay_counter, ch.vibrato.rate, flags1));
                }
                if input.flags1 & 16 == 0 {
                    if exact_cycles {
                        effects[i] = channel_effect_work_cycles(input)
                            .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
                    } else {
                        EffectInstructionWork::new(input)
                            .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
                    }
                } else {
                    let slide = PitchSlideState::from_native(&ch.pitch_slide);
                    let mut effect = EffectInstructionWork::with_pitch_slide(input, Some(slide))
                        .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
                    let mut cycles = 0;
                    while !effect.finished() {
                        let prefix = effect.plain_prefix();
                        if prefix.len() > 1 {
                            cycles += effect.accept_plain_prefix(prefix.len(), prefix);
                        } else {
                            cycles +=
                                u32::from(effect.next().expect("pending pitch-slide instruction"));
                        }
                    }
                    effects[i] = cycles;
                    if let Some(result) = effect.slide_result() {
                        // Native arithmetic guards 16-bit wrap instead of writing
                        // the wrapped value. Refuse those foreign states up front.
                        if result.flags1 & 16 != 0
                            && ((input.flags1 & 32 != 0 && result.state.current > slide.current)
                                || (input.flags1 & 32 == 0 && result.state.current < slide.current))
                        {
                            return Err(AudioClockError::UnsupportedEffect(i as u8));
                        }
                        let frequency = if result.flags1 & 16 != 0 {
                            result.state.current
                        } else {
                            ch.frequency
                        };
                        expected_slide[i] = Some((result, frequency));
                    } else if input.flags1 & 16 != 0 && input.paired_sfx_active {
                        expected_slide[i] = Some((
                            PitchSlideResult {
                                state: slide,
                                flags1: input.flags1,
                            },
                            ch.frequency,
                        ));
                    }
                }
                expected[i] = Some(ExpectedDelay {
                    ended_ptr: None,
                    delay: ch.delay_counter - 1,
                    fraction: fractions[i],
                    stream: None,
                });
            }
        }
        let work = AudioFrameWork {
            cycles: if exact_cycles {
                audio_dispatch_work(&ids, 0, &effects).0
            } else {
                0
            },
        };
        let no_note_cycles =
            if exact_cycles && (0..8).all(|i| ids[i] == 0 || seq.channels[i].delay_counter > 1) {
                let mut cycles = 8 + 16;
                for i in 0..8 {
                    if ids[i] == 0 {
                        cycles += if i == 0 { 80 } else { 76 };
                        continue;
                    }
                    cycles += if i == 0 {
                        100
                    } else if i < 4 {
                        68
                    } else {
                        72
                    };
                    cycles += 24 + effects[i] + if i == 7 { 24 } else { 28 };
                }
                Some(cycles)
            } else {
                None
            };
        Ok(Self {
            work,
            ids,
            expected,
            expected_tempo: tempo,
            expected_panning: panning,
            expected_sfx,
            expected_saved_frequency,
            paused_modulation,
            expected_slide,
            prepared_notes,
            no_note_cycles,
        })
    }

    pub(crate) fn note_preparation(
        &self,
        channel: usize,
    ) -> Option<note_preparation::PreparationResult> {
        self.prepared_notes[channel]
    }

    pub(crate) fn no_note_cycles(&self) -> Option<u32> {
        self.no_note_cycles
    }

    pub(crate) fn ids(&self) -> [u8; 8] {
        self.ids
    }

    pub fn finish(&self, actual: &Sequencer) -> Result<AudioFrameWork, AudioClockError> {
        if actual.music_tempo != self.expected_tempo
            || actual.stereo_panning != self.expected_panning
        {
            return Err(AudioClockError::NativeStateMismatch(0));
        }
        for i in 0..4 {
            let ch = &actual.channels[i];
            if let Some((slide, frequency)) = self.expected_slide[i] {
                if PitchSlideState::from_native(&ch.pitch_slide) != slide.state
                    || ch.flags1.bits() != slide.flags1
                    || ch.frequency != frequency
                {
                    return Err(AudioClockError::NativeStateMismatch(i as u8));
                }
            }
            if self.expected_saved_frequency[i].is_some_and(|low| low != ch.freq_lo_saved)
                || self.paused_modulation[i].is_some_and(|v| {
                    v != (ch.vibrato.delay_counter, ch.vibrato.rate, ch.flags1.bits())
                })
            {
                return Err(AudioClockError::NativeStateMismatch(i as u8));
            }
        }
        for (index, work) in self.expected_sfx.iter().enumerate() {
            if let Some(w) = work.as_ref().filter(|w| !w.ended) {
                let ch = &actual.channels[index + 4];
                if ch.duty_cycle << 6 != w.duty
                    || ch.volume_envelope != w.envelope
                    || ch.frequency != w.frequency
                {
                    return Err(AudioClockError::NativeStateMismatch(index as u8 + 4));
                }
            }
        }
        for (i, expected) in self.expected.iter().enumerate() {
            if let Some(expected) = expected {
                let ch = &actual.channels[i];
                if let Some(ptr) = expected.ended_ptr {
                    if ch.active || ch.ptr != ptr {
                        return Err(AudioClockError::NativeStateMismatch(i as u8));
                    }
                    continue;
                }
                if !ch.active
                    || ch.delay_counter != expected.delay
                    || ch.delay_frac != expected.fraction
                {
                    return Err(AudioClockError::NativeStateMismatch(i as u8));
                }
                if let Some((ptr, return_ptr, call, loops)) = expected.stream {
                    if (ch.ptr, ch.return_ptr, ch.flags1.bits() & 2, ch.loop_counter)
                        != (ptr, return_ptr, call, loops)
                    {
                        return Err(AudioClockError::NativeStateMismatch(i as u8));
                    }
                }
            }
        }
        Ok(self.work)
    }
}

#[cfg(test)]
mod native_owner_tests {
    use crate::{audio_manager::AudioManager, music_data::MusicId};

    #[test]
    fn unsupported_pokecenter_effect_and_other_music_still_tick_once() {
        use crate::{sfx_data::SfxId, source_clock::AudioClockError};
        for unsupported_effect in [false, true] {
            let mut ordinary = AudioManager::new();
            let mut clocked = AudioManager::new();
            let music = if unsupported_effect {
                MusicId::POKECENTER
            } else {
                MusicId::WILD_BATTLE
            };
            for manager in [&mut ordinary, &mut clocked] {
                manager.play_music(music);
                manager.update_frame();
                if unsupported_effect {
                    manager.play_sfx(SfxId::Pokeflute);
                }
            }
            ordinary.update_frame();
            let result = clocked.update_frame_with_source_work();
            if unsupported_effect {
                assert!(matches!(result, Err(AudioClockError::ActiveSfx(_))));
            } else {
                assert_eq!(result, Err(AudioClockError::UnsupportedSoundBank));
            }
            for channel in 0..8 {
                assert_eq!(
                    std::format!("{:?}", ordinary.sequencer.channels[channel]),
                    std::format!("{:?}", clocked.sequencer.channels[channel]),
                );
            }
        }
    }

    #[test]
    fn pokecenter_work_matches_ten_thousand_original_updates() {
        // Controlled unit oracle: original readonly channel inputs are placed
        // into a native sequencer. This is not a normal-boot gameplay test.
        let bytes =
            include_bytes!("../../../../docs/audits/hof-pc-load/pokecenter-audio-cases.bin");
        assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 10000);
        let mut at = 4;
        for tick in 0..10000 {
            let mut manager = AudioManager::new();
            manager.play_music(MusicId::POKECENTER);
            manager.update_frame();
            manager.sequencer.music_tempo =
                u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap());
            at += 2;
            let expected = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            at += 4;
            for channel in 0..3 {
                let ch = &mut manager.sequencer.channels[channel];
                ch.ptr = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
                ch.return_ptr =
                    u16::from_le_bytes(bytes[at + 2..at + 4].try_into().unwrap()) as usize;
                at += 4;
                let f = &bytes[at..at + 14];
                ch.flags1 = dotzuki_audio::ChannelFlags1::from_bits_retain(f[0]);
                ch.flags2 = dotzuki_audio::ChannelFlags2::from_bits_retain(f[1]);
                ch.duty_cycle = f[2] >> 6;
                ch.vibrato.delay_counter = f[3];
                ch.vibrato.extent = f[4];
                ch.vibrato.rate = f[5];
                ch.freq_lo_saved = f[6];
                ch.vibrato.delay_reload = f[7];
                ch.delay_counter = f[8];
                ch.loop_counter = f[9];
                ch.note_speed = f[10];
                ch.delay_frac = f[11];
                ch.octave = f[12];
                ch.volume_envelope = if channel == 2 { f[13] >> 1 } else { f[13] };
                at += 14;
            }
            let actual = manager
                .update_frame_with_source_work()
                .unwrap_or_else(|e| panic!("tick {tick}: {e:?}"));
            assert_eq!(actual.cycles, expected, "original tick {tick}");
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    fn pokecenter_shared_update_covers_song_and_confirmation_phases() {
        use crate::sfx_data::SfxId;
        for confirmation in [false, true] {
            let mut ordinary = AudioManager::new();
            let mut clocked = AudioManager::new();
            ordinary.play_music(MusicId::POKECENTER);
            clocked.play_music(MusicId::POKECENTER);
            ordinary.update_frame();
            clocked.update_frame();
            for frame in 1..=20000 {
                if confirmation && frame % 37 == 0 {
                    ordinary.play_sfx(SfxId::PressAB);
                    clocked.play_sfx(SfxId::PressAB);
                }
                ordinary.update_frame();
                let work = clocked.update_frame_with_source_work().unwrap_or_else(|e| {
                    panic!("finish frame {frame}, confirmation {confirmation}: {e:?}")
                });
                assert!(work.cycles > 0);
                assert_eq!(
                    ordinary.sequencer.music_tempo,
                    clocked.sequencer.music_tempo
                );
                for channel in 0..8 {
                    assert_eq!(
                        std::format!("{:?}", ordinary.sequencer.channels[channel]),
                        std::format!("{:?}", clocked.sequencer.channels[channel]),
                        "frame {frame}, channel {channel}, confirmation {confirmation}",
                    );
                }
            }
        }
    }

    #[test]
    fn dungeon3_native_clock_covers_every_song_phase() {
        let mut ordinary = AudioManager::new();
        let mut clocked = AudioManager::new();
        ordinary.play_music(MusicId::DUNGEON3);
        clocked.play_music(MusicId::DUNGEON3);
        ordinary.update_frame();
        clocked.update_frame();
        let mut failures = alloc::collections::BTreeMap::new();
        for frame in 1..=20000 {
            ordinary.update_frame();
            if let Err(error) = clocked.update_frame_with_source_work() {
                *failures.entry(std::format!("{error:?}")).or_insert(0) += 1;
            }
            for i in 0..8 {
                assert_eq!(
                    std::format!("{:?}", ordinary.sequencer.channels[i]),
                    std::format!("{:?}", clocked.sequencer.channels[i]),
                    "frame{frame} channel{i}"
                );
            }
        }
        assert!(
            failures.is_empty(),
            "unsupported normal song phases: {failures:?}"
        );
    }

    #[test]
    fn every_normal_silph_song_phase_uses_the_single_real_channel_owner() {
        let mut ordinary = AudioManager::new();
        let mut clocked = AudioManager::new();
        ordinary.play_music(MusicId::SILPH_CO);
        clocked.play_music(MusicId::SILPH_CO);
        // The real constructor is primed by the ordinary native update,
        // as the Game does before the farewell caller (and legacy restore).
        ordinary.update_frame();
        clocked.update_frame();
        let mut seen = [false; 256];
        for frame in 1..=20000 {
            for ch in &clocked.sequencer.channels[..3] {
                if ch.active && ch.delay_counter == 1 {
                    if let Some(command) = ch.data.get(ch.ptr) {
                        seen[*command as usize] = true;
                    }
                }
            }
            ordinary.update_frame();
            let work = clocked
                .update_frame_with_source_work()
                .unwrap_or_else(|e| panic!("song frame {frame}: {e:?}"));
            assert!(work.cycles > 0);
            assert_eq!(
                ordinary.sequencer.music_tempo, clocked.sequencer.music_tempo,
                "frame{frame}"
            );
            for i in 0..8 {
                assert_eq!(
                    std::format!("{:?}", ordinary.sequencer.channels[i]),
                    std::format!("{:?}", clocked.sequencer.channels[i]),
                    "channel{i}, frame{frame}"
                );
            }
        }
        for command in [0xed, 0xfd, 0xfe, 0xff] {
            assert!(seen[command], "command {command:x} was not reached");
        }
    }

    #[test]
    fn clock_observes_real_native_song_without_advancing_a_second_sequencer() {
        let mut ordinary = AudioManager::new();
        let mut clocked = AudioManager::new();
        ordinary.play_music(MusicId::SILPH_CO);
        clocked.play_music(MusicId::SILPH_CO);
        let mut supported = 0;
        let mut rejected = 0;
        for frame in 0..1200 {
            ordinary.update_frame();
            match clocked.update_frame_with_source_work() {
                Ok(work) => {
                    assert!(work.cycles > 0);
                    supported += 1;
                }
                Err(error) => {
                    rejected += 1;
                    std::println!("unsupported frame {frame}: {error:?}");
                }
            }
            for i in 0..8 {
                let a = &ordinary.sequencer.channels[i];
                let b = &clocked.sequencer.channels[i];
                assert_eq!(
                    (
                        a.active,
                        a.ptr,
                        a.delay_counter,
                        a.delay_frac,
                        a.frequency,
                        a.flags1.bits(),
                        a.flags2.bits(),
                        a.vibrato.delay_counter
                    ),
                    (
                        b.active,
                        b.ptr,
                        b.delay_counter,
                        b.delay_frac,
                        b.frequency,
                        b.flags1.bits(),
                        b.flags2.bits(),
                        b.vibrato.delay_counter
                    ),
                    "channel {i}, frame {frame}"
                );
            }
        }
        assert_eq!(supported, 1199);
        assert_eq!(
            rejected, 1,
            "only the unprimed constructor remains outside this clock entry"
        );
    }
}

#[cfg(test)]
mod confirmation_owner_tests {
    use crate::{audio_manager::AudioManager, music_data::MusicId, sfx_data::SfxId};
    #[test]
    fn confirmation_updates_preserve_modeled_state_at_early_song_phases() {
        let mut errors = alloc::vec::Vec::new();
        for music in [MusicId::DUNGEON3, MusicId::SILPH_CO] {
            for warmup in 1..=64 {
                let mut manager = AudioManager::new();
                manager.apu.write_register(0xff26, 0x80);
                manager.play_music(music);
                for _ in 0..warmup {
                    manager.update_frame();
                }
                manager.play_sfx(SfxId::PressAB);
                for tick in 0..18 {
                    let before = manager.sequencer.channels[0].freq_lo_saved;
                    if let Err(error) = manager.update_frame_with_source_work() {
                        errors.push((
                            music,
                            warmup,
                            tick,
                            error,
                            before,
                            manager.sequencer.channels[0].freq_lo_saved,
                        ));
                    }
                }
            }
        }
        let mut kinds = alloc::collections::BTreeMap::new();
        for (_, _, _, error, before, after) in &errors {
            *kinds
                .entry(std::format!(
                    "{error:?}, changed_saved_frequency={}",
                    before != after
                ))
                .or_insert(0) += 1;
        }
        assert!(
            errors.is_empty(),
            "{} errors {:?}, first: {:?}",
            errors.len(),
            kinds,
            &errors[..errors.len().min(8)]
        );
    }
}
