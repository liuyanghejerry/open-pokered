//! Declarative, file-based audio format (game-agnostic).
//!
//! The sequencer plays raw byte-code channel streams; that is efficient for the
//! runtime but opaque to author. This module defines an editable JSON
//! representation — [`TrackDef`] (a song or SFX) made of [`ChannelDef`]s, each a
//! list of [`AudioCommand`]s — and the lossless conversions to/from the byte
//! streams the [`crate::sequencer::Sequencer`] consumes.
//!
//! [`AudioCommand`] mirrors [`Command`](crate::commands::Command) one-to-one but
//! uses only struct/unit variants so it serializes as clean, internally-tagged
//! JSON, e.g. `{"type":"note","pitch":0,"length":8}`. Higher-level niceties
//! (note names, octave translation, volume/fade split) belong in the editor UI,
//! not here, so this stays a faithful, bulletproof round-trip.
//!
//! Requires the `serde` feature.

use serde::{Deserialize, Serialize};

use crate::commands::{decode_channel, encode_channel, Command};
use crate::HwChannel;

/// Default tempo for a music track (matches the sequencer's power-on default of
/// 1.0 in the engine's big-endian 8.8 fixed-point format). SFX ignore tempo.
pub const DEFAULT_TEMPO: u16 = 0x0100;

fn default_tempo() -> u16 {
    DEFAULT_TEMPO
}

/// Whether a track is background music (channels 1-4) or a sound effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Music,
    Sfx,
}

/// A single audio command — the editable, serde-friendly mirror of
/// [`Command`](crate::commands::Command).
///
/// Serializes internally-tagged with a `type` discriminator in `snake_case`,
/// e.g. `{"type":"note_type","speed":12,"param":146}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AudioCommand {
    /// Play a note. `pitch` = 0-11 (C..B), `length` = 1-16.
    Note { pitch: u8, length: u8 },
    /// Noise-channel drum note. `length` = 1-16.
    DrumNote { length: u8, instrument: u8 },
    /// Rest (silence). `length` = 1-16.
    Rest { length: u8 },
    /// Set note speed + volume/envelope byte. On the wave channel the param's
    /// low nibble selects the wave instrument.
    NoteType { speed: u8, param: u8 },
    /// Set octave. `value` is the raw encoded value (0-7); musical octave = 8 - value.
    Octave { value: u8 },
    /// Toggle the perfect-pitch flag.
    TogglePerfectPitch,
    /// Set vibrato: `delay` before onset, `depth_rate` = packed (depth<<4 | rate).
    Vibrato { delay: u8, depth_rate: u8 },
    /// Begin a pitch slide. `octave_pitch` = packed (octave<<4 | pitch).
    PitchSlide {
        length_modifier: u8,
        octave_pitch: u8,
    },
    /// Set duty cycle (0-3).
    DutyCycle { value: u8 },
    /// Set tempo (8.8 fixed-point; high byte = integer part).
    Tempo { value: u16 },
    /// Set the stereo panning byte (NR51 format).
    StereoPanning { value: u8 },
    /// Unused command `$EF` (preserved verbatim for round-tripping).
    UnknownEf { value: u8 },
    /// Set master volume (NR50).
    Volume { value: u8 },
    /// Toggle the SFX channel's execute-music flag.
    ExecuteMusic,
    /// Set the duty-cycle rotation pattern (4 x 2-bit).
    DutyCyclePattern { value: u8 },
    /// Call a subroutine at a byte offset within the channel stream.
    SoundCall { offset: u16 },
    /// Loop `count` times (0 = infinite) back to `offset`.
    SoundLoop { count: u8, offset: u16 },
    /// Return from a `sound_call`.
    SoundRet,
    /// Pitch sweep (channel 1 / NR10). SFX channels only.
    PitchSweep { param: u8 },
    /// SFX square note with explicit envelope + 11-bit frequency.
    SfxSquareNote {
        length: u8,
        volume_envelope: u8,
        frequency: u16,
    },
    /// SFX noise note with explicit envelope + polynomial-counter byte.
    SfxNoiseNote {
        length: u8,
        volume_envelope: u8,
        noise_params: u8,
    },
    /// End of data / terminator.
    EndOfData,
}

impl From<&Command> for AudioCommand {
    fn from(c: &Command) -> Self {
        match *c {
            Command::Note { pitch, length } => AudioCommand::Note { pitch, length },
            Command::DrumNote { length, instrument } => {
                AudioCommand::DrumNote { length, instrument }
            }
            Command::Rest { length } => AudioCommand::Rest { length },
            Command::NoteType { speed, param } => AudioCommand::NoteType { speed, param },
            Command::Octave(value) => AudioCommand::Octave { value },
            Command::TogglePerfectPitch => AudioCommand::TogglePerfectPitch,
            Command::Vibrato { delay, depth_rate } => AudioCommand::Vibrato { delay, depth_rate },
            Command::PitchSlide {
                length_modifier,
                octave_pitch,
            } => AudioCommand::PitchSlide {
                length_modifier,
                octave_pitch,
            },
            Command::DutyCycle(value) => AudioCommand::DutyCycle { value },
            Command::Tempo(value) => AudioCommand::Tempo { value },
            Command::StereoPanning(value) => AudioCommand::StereoPanning { value },
            Command::UnknownEF(value) => AudioCommand::UnknownEf { value },
            Command::Volume(value) => AudioCommand::Volume { value },
            Command::ExecuteMusic => AudioCommand::ExecuteMusic,
            Command::DutyCyclePattern(value) => AudioCommand::DutyCyclePattern { value },
            Command::SoundCall { offset } => AudioCommand::SoundCall { offset },
            Command::SoundLoop { count, offset } => AudioCommand::SoundLoop { count, offset },
            Command::SoundRet => AudioCommand::SoundRet,
            Command::PitchSweep { param } => AudioCommand::PitchSweep { param },
            Command::SfxSquareNote {
                length,
                volume_envelope,
                frequency,
            } => AudioCommand::SfxSquareNote {
                length,
                volume_envelope,
                frequency,
            },
            Command::SfxNoiseNote {
                length,
                volume_envelope,
                noise_params,
            } => AudioCommand::SfxNoiseNote {
                length,
                volume_envelope,
                noise_params,
            },
            Command::EndOfData => AudioCommand::EndOfData,
        }
    }
}

impl AudioCommand {
    /// Convert back to the low-level [`Command`].
    pub fn to_command(&self) -> Command {
        match *self {
            AudioCommand::Note { pitch, length } => Command::Note { pitch, length },
            AudioCommand::DrumNote { length, instrument } => {
                Command::DrumNote { length, instrument }
            }
            AudioCommand::Rest { length } => Command::Rest { length },
            AudioCommand::NoteType { speed, param } => Command::NoteType { speed, param },
            AudioCommand::Octave { value } => Command::Octave(value),
            AudioCommand::TogglePerfectPitch => Command::TogglePerfectPitch,
            AudioCommand::Vibrato { delay, depth_rate } => Command::Vibrato { delay, depth_rate },
            AudioCommand::PitchSlide {
                length_modifier,
                octave_pitch,
            } => Command::PitchSlide {
                length_modifier,
                octave_pitch,
            },
            AudioCommand::DutyCycle { value } => Command::DutyCycle(value),
            AudioCommand::Tempo { value } => Command::Tempo(value),
            AudioCommand::StereoPanning { value } => Command::StereoPanning(value),
            AudioCommand::UnknownEf { value } => Command::UnknownEF(value),
            AudioCommand::Volume { value } => Command::Volume(value),
            AudioCommand::ExecuteMusic => Command::ExecuteMusic,
            AudioCommand::DutyCyclePattern { value } => Command::DutyCyclePattern(value),
            AudioCommand::SoundCall { offset } => Command::SoundCall { offset },
            AudioCommand::SoundLoop { count, offset } => Command::SoundLoop { count, offset },
            AudioCommand::SoundRet => Command::SoundRet,
            AudioCommand::PitchSweep { param } => Command::PitchSweep { param },
            AudioCommand::SfxSquareNote {
                length,
                volume_envelope,
                frequency,
            } => Command::SfxSquareNote {
                length,
                volume_envelope,
                frequency,
            },
            AudioCommand::SfxNoiseNote {
                length,
                volume_envelope,
                noise_params,
            } => Command::SfxNoiseNote {
                length,
                volume_envelope,
                noise_params,
            },
            AudioCommand::EndOfData => Command::EndOfData,
        }
    }
}

/// One hardware channel of a track: which channel it drives + its commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelDef {
    /// Target hardware channel (`pulse1`, `pulse2`, `wave`, `noise`).
    pub hw: HwChannel,
    /// The command list for this channel.
    pub commands: Vec<AudioCommand>,
}

impl ChannelDef {
    /// Encode this channel's commands to a byte stream.
    pub fn to_bytes(&self) -> Vec<u8> {
        let is_noise = self.hw == HwChannel::Noise;
        let cmds: Vec<Command> = self.commands.iter().map(AudioCommand::to_command).collect();
        encode_channel(&cmds, is_noise)
    }

    /// Decode a raw byte stream into a channel definition.
    pub fn from_bytes(hw: HwChannel, bytes: &[u8], is_sfx: bool) -> Self {
        let is_noise = hw == HwChannel::Noise;
        let commands = decode_channel(bytes, is_noise, is_sfx)
            .iter()
            .map(AudioCommand::from)
            .collect();
        ChannelDef { hw, commands }
    }
}

/// A complete track — one song or one sound effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackDef {
    /// Stable identifier, referenced by maps/scripts (e.g. `"StartTown"`).
    pub id: String,
    /// Music or SFX.
    pub kind: TrackKind,
    /// Optional human-readable display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Playback tempo (music only; SFX play at a fixed tempo).
    #[serde(default = "default_tempo")]
    pub tempo: u16,
    /// The channels, tagged by hardware channel.
    pub channels: Vec<ChannelDef>,
}

impl TrackDef {
    /// Build channel byte streams indexed by hardware channel
    /// (`pulse1`=0 .. `noise`=3), ready for [`Sequencer::play_music`].
    ///
    /// Channels are placed at their `hw` index; unused leading/interior slots
    /// become empty streams and trailing empties are trimmed. Real GB music uses
    /// channels contiguously from `pulse1`, so this matches the engine's data.
    ///
    /// [`Sequencer::play_music`]: crate::sequencer::Sequencer::play_music
    pub fn to_music_channels(&self) -> Vec<Vec<u8>> {
        let mut slots: [Vec<u8>; 4] = Default::default();
        let mut max_used = 0usize;
        for ch in &self.channels {
            let idx = (ch.hw as usize).min(3);
            slots[idx] = ch.to_bytes();
            max_used = max_used.max(idx);
        }
        slots.into_iter().take(max_used + 1).collect()
    }

    /// Build `(channel_data, start_channel)` for [`Sequencer::play_sfx`].
    ///
    /// Channels are ordered by hardware index; `start_channel` is the lowest
    /// index used. Assumes the used channels are contiguous (as real SFX are).
    ///
    /// [`Sequencer::play_sfx`]: crate::sequencer::Sequencer::play_sfx
    pub fn to_sfx_channels(&self) -> (Vec<Vec<u8>>, usize) {
        let mut used: Vec<(usize, Vec<u8>)> = self
            .channels
            .iter()
            .map(|ch| ((ch.hw as usize).min(3), ch.to_bytes()))
            .collect();
        used.sort_by_key(|(idx, _)| *idx);
        let start = used.first().map(|(i, _)| *i).unwrap_or(0);
        let data = used.into_iter().map(|(_, d)| d).collect();
        (data, start)
    }

    /// Reconstruct a track from raw channel byte streams (as stored in the
    /// generated pokered tables). `channels` pairs each hardware channel with
    /// its byte stream.
    pub fn from_raw_channels(
        id: impl Into<String>,
        kind: TrackKind,
        tempo: u16,
        channels: &[(HwChannel, &[u8])],
    ) -> Self {
        let is_sfx = kind == TrackKind::Sfx;
        let channels = channels
            .iter()
            .map(|(hw, bytes)| ChannelDef::from_bytes(*hw, bytes, is_sfx))
            .collect();
        TrackDef {
            id: id.into(),
            kind,
            name: None,
            tempo,
            channels,
        }
    }

    /// Start this track playing on `seq` (music → channels 1-4, SFX → its
    /// hardware channels). The numeric sound id is set to 0; identity is by name
    /// in the file-based world.
    pub fn play_on(&self, seq: &mut crate::sequencer::Sequencer) {
        match self.kind {
            TrackKind::Music => {
                let channels = self.to_music_channels();
                seq.play_music(0, &channels, self.tempo);
            }
            TrackKind::Sfx => {
                let (channels, start) = self.to_sfx_channels();
                seq.play_sfx(0, &channels, start, DEFAULT_TEMPO);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_dto_roundtrips_all_variants() {
        let cases = [
            Command::Note {
                pitch: 5,
                length: 8,
            },
            Command::DrumNote {
                length: 2,
                instrument: 7,
            },
            Command::Rest { length: 16 },
            Command::NoteType {
                speed: 12,
                param: 0x92,
            },
            Command::Octave(3),
            Command::TogglePerfectPitch,
            Command::Vibrato {
                delay: 4,
                depth_rate: 0x21,
            },
            Command::PitchSlide {
                length_modifier: 8,
                octave_pitch: 0x34,
            },
            Command::DutyCycle(2),
            Command::Tempo(0x0140),
            Command::StereoPanning(0xF0),
            Command::UnknownEF(0x12),
            Command::Volume(0x77),
            Command::ExecuteMusic,
            Command::DutyCyclePattern(0x1B),
            Command::SoundCall { offset: 0x00AB },
            Command::SoundLoop {
                count: 0,
                offset: 0x1234,
            },
            Command::SoundRet,
            Command::PitchSweep { param: 0x15 },
            Command::SfxSquareNote {
                length: 3,
                volume_envelope: 0xA2,
                frequency: 0x0567,
            },
            Command::SfxNoiseNote {
                length: 1,
                volume_envelope: 0x91,
                noise_params: 0x33,
            },
            Command::EndOfData,
        ];
        for cmd in cases {
            let dto = AudioCommand::from(&cmd);
            assert_eq!(dto.to_command(), cmd, "DTO round-trip failed for {cmd:?}");
            // …and survives a JSON round-trip.
            let json = serde_json::to_string(&dto).unwrap();
            let back: AudioCommand = serde_json::from_str(&json).unwrap();
            assert_eq!(back, dto, "JSON round-trip failed for {json}");
        }
    }

    #[test]
    fn json_shape_is_internally_tagged() {
        let json = serde_json::to_string(&AudioCommand::Note {
            pitch: 0,
            length: 8,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"note","pitch":0,"length":8}"#);
        let json = serde_json::to_string(&AudioCommand::Octave { value: 3 }).unwrap();
        assert_eq!(json, r#"{"type":"octave","value":3}"#);
        let hw = serde_json::to_string(&HwChannel::Pulse1).unwrap();
        assert_eq!(hw, r#""pulse1""#);
    }

    #[test]
    fn track_json_roundtrips() {
        let track = TrackDef {
            id: "TestSong".into(),
            kind: TrackKind::Music,
            name: Some("Test".into()),
            tempo: 0x00A0,
            channels: vec![ChannelDef {
                hw: HwChannel::Pulse1,
                commands: vec![
                    AudioCommand::Tempo { value: 0x00A0 },
                    AudioCommand::NoteType {
                        speed: 12,
                        param: 0x92,
                    },
                    AudioCommand::Octave { value: 4 },
                    AudioCommand::Note {
                        pitch: 0,
                        length: 8,
                    },
                    AudioCommand::Rest { length: 4 },
                ],
            }],
        };
        let json = serde_json::to_string_pretty(&track).unwrap();
        let back: TrackDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, track);
    }
}
