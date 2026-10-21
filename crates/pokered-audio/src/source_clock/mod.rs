//! Source Audio3 instruction work on the live native sequencer state.
//!
//! The native sequencer remains the only channel owner. This port describes
//! the branch work of its supported update, then checks the actual native
//! delay result; it never applies a captured channel/result or service cost.
//! The caller must consume the returned T-cycles in its foreground scheduler.
mod audio_math_work;
mod audio_next_note_work;
mod audio_note_dispatch_work;
mod audio_output_work;
mod audio_stream_work;
mod audio_work;
mod press_ab_work;

use crate::{sequencer::Sequencer, NOTE_FREQUENCIES};
use audio_next_note_work::music_next_note_work_with_sfx;
use audio_note_dispatch_work::MusicCommandState;
use audio_work::{audio_dispatch_work, channel_effect_work_cycles, ChannelEffectWorkInput};
use press_ab_work::{press_ab_next_note, PressAbWork, SOURCE_PRESS_AB_ID};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioClockError {
    UnsupportedSoundBank,
    OutputUnavailable,
    ActiveSfx(u8),
    UnsupportedEffect(u8),
    UnsupportedCommand { channel: u8, command: Option<u8> },
    NativeStateMismatch(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFrameWork {
    pub cycles: u32,
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
    expected: [Option<ExpectedDelay>; 8],
    expected_tempo: u16,
    expected_sfx: Option<PressAbWork>,
    expected_saved_frequency: [Option<u8>; 4],
    paused_modulation: [Option<(u8, u8, u8)>; 4],
}

impl PreparedAudioFrame {
    /// This bounded port handles Silph and Rock Tunnel music after the first
    /// ordinary native tick. Noise rests/termination and finite loops are
    /// included, together with finite PressAB and its paired music branches.
    /// Other SFX/drums/pitch-slide/duty-rotation stop explicitly.
    /// No inactive voice is substituted for an active unsupported channel.
    pub fn from_audio3(seq: &Sequencer) -> Result<Self, AudioClockError> {
        for i in 4..8 {
            let ch = &seq.channels[i];
            if ch.active
                && (i != 4
                    || ch.sound_id != crate::sfx_data::SfxId::PressAB as u8
                    || ch.data != crate::sfx_data::SFX_PRESS_AB_CH5
                    || ch.note_speed != 1
                    || ch.flags1.bits() != 4
                    || ch.flags2.bits() != 0
                    || seq.sfx_tempo != 0x100)
            {
                return Err(AudioClockError::ActiveSfx(i as u8));
            }
        }
        let source_sfx = if seq.channels[4].active {
            SOURCE_PRESS_AB_ID
        } else {
            0
        };
        let mut expected_sfx = None;
        let mut expected_saved_frequency = [None; 4];
        let mut paused_modulation = [None; 4];
        let mut ids = [0; 8];
        let mut effects = [0; 8];
        let mut expected: [Option<ExpectedDelay>; 8] = [None; 8];
        let mut tempo = seq.music_tempo;
        let mut fractions: [u8; 8] = core::array::from_fn(|i| seq.channels[i].delay_frac);
        for (i, ch) in seq.channels.iter().enumerate() {
            ids[i] = if ch.active { ch.sound_id } else { 0 };
            if ids[i] == 0 {
                continue;
            }
            if i == 4 {
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
                    let w = press_ab_next_note(&ch.data, ch.ptr, ch.delay_frac, ch.duty_cycle << 6)
                        .ok_or(AudioClockError::UnsupportedEffect(4))?;
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
                    expected_sfx = Some(w);
                } else if ch.delay_counter > 1 {
                    effects[i] = channel_effect_work_cycles(ChannelEffectWorkInput {
                        channel: 4,
                        paired_sfx_active: false,
                        note_delay: ch.delay_counter,
                        flags1: ch.flags1.bits(),
                        flags2: ch.flags2.bits(),
                        vibrato_delay: ch.vibrato.delay_counter,
                        vibrato_extent: ch.vibrato.extent,
                        vibrato_rate: ch.vibrato.rate,
                        frequency_low: ch.freq_lo_saved,
                    })
                    .ok_or(AudioClockError::UnsupportedEffect(4))?;
                    expected[i] = Some(ExpectedDelay {
                        delay: ch.delay_counter - 1,
                        fraction: ch.delay_frac,
                        ended_ptr: None,
                        stream: None,
                    });
                } else {
                    return Err(AudioClockError::UnsupportedEffect(4));
                }
                continue;
            }
            let input = ChannelEffectWorkInput {
                channel: i as u8,
                paired_sfx_active: seq.channels[i + 4].active,
                note_delay: ch.delay_counter,
                flags1: ch.flags1.bits(),
                flags2: ch.flags2.bits(),
                vibrato_delay: ch.vibrato.delay_counter,
                vibrato_extent: ch.vibrato.extent,
                vibrato_rate: ch.vibrato.rate,
                frequency_low: ch.freq_lo_saved,
            };
            if ch.delay_counter == 1 {
                if i == 3
                    && ch.flags1.bits() & 2 == 0
                    && ch.data.get(ch.ptr) == Some(&0xff)
                    && !seq.channels[4..].iter().any(|sfx| sfx.active)
                {
                    effects[i] = 8 + 12 + 8 + 8 + 8 + 16 + audio_work::noise_channel_end_work();
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
                    flags1: ch.flags1.bits(),
                    vibrato_delay: ch.vibrato.delay_counter,
                };
                let stream = audio_stream_work::music_stream_work(
                    i as u8,
                    &ch.data,
                    ch.ptr,
                    ch.return_ptr,
                    ch.loop_counter,
                    state,
                    ch.vibrato.delay_reload,
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
                let mut result = music_next_note_work_with_sfx(
                    i as u8,
                    &stream.linear_commands,
                    stream.state,
                    stream.vibrato_reload,
                    ch.flags2.bits(),
                    fractions[i],
                    tempo,
                    ch.freq_lo_saved,
                    NOTE_FREQUENCIES.get((stream.note >> 4) as usize).copied(),
                    seq.channels[i + 4].active,
                    source_sfx,
                )
                .ok_or(AudioClockError::UnsupportedCommand {
                    channel: i as u8,
                    command: ch.data.get(ch.ptr).copied(),
                })?;
                expected_saved_frequency[i] = Some(result.frequency_low);
                result.cycles += stream.control_cycles;
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
            } else {
                // The generic native engine treats zero as a ready entry;
                // source zero wraps its countdown. Until those states have
                // an explicit common constructor, no timing is asserted.
                if ch.delay_counter == 0 {
                    return Err(AudioClockError::UnsupportedEffect(i as u8));
                }
                if seq.channels[i + 4].active {
                    paused_modulation[i] =
                        Some((ch.vibrato.delay_counter, ch.vibrato.rate, ch.flags1.bits()));
                }
                effects[i] = channel_effect_work_cycles(input)
                    .ok_or(AudioClockError::UnsupportedEffect(i as u8))?;
                expected[i] = Some(ExpectedDelay {
                    ended_ptr: None,
                    delay: ch.delay_counter - 1,
                    fraction: fractions[i],
                    stream: None,
                });
            }
        }
        Ok(Self {
            work: AudioFrameWork {
                cycles: audio_dispatch_work(&ids, 0, &effects).0,
            },
            expected,
            expected_tempo: tempo,
            expected_sfx,
            expected_saved_frequency,
            paused_modulation,
        })
    }

    pub fn finish(self, actual: &Sequencer) -> Result<AudioFrameWork, AudioClockError> {
        if actual.music_tempo != self.expected_tempo {
            return Err(AudioClockError::NativeStateMismatch(0));
        }
        for i in 0..4 {
            let ch = &actual.channels[i];
            if self.expected_saved_frequency[i].is_some_and(|low| low != ch.freq_lo_saved)
                || self.paused_modulation[i].is_some_and(|v| {
                    v != (ch.vibrato.delay_counter, ch.vibrato.rate, ch.flags1.bits())
                })
            {
                return Err(AudioClockError::NativeStateMismatch(i as u8));
            }
        }
        if let Some(w) = self.expected_sfx.filter(|w| !w.ended) {
            let ch = &actual.channels[4];
            if ch.duty_cycle << 6 != w.duty
                || ch.volume_envelope != w.envelope
                || ch.frequency != w.frequency
            {
                return Err(AudioClockError::NativeStateMismatch(4));
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
