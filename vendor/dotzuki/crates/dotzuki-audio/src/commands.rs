//! Audio command decoding and frequency calculation.
//!
//! Parses the byte stream format used by the audio engine.
//! Command byte ranges:
//! - $00-$AF: note (high nibble = pitch, low nibble = length-1)
//! - $B0-$BF: drum_note (low nibble = length-1, next byte = instrument)
//! - $C0-$CF: rest (low nibble = length-1)
//! - $D0-$DF: note_type (low nibble = speed, param bytes follow)
//! - $E0-$E7: octave (octave = 8 - low nibble)
//! - $E8+:    control commands (vibrato, pitch slide, duty, tempo, etc.)

use crate::{NOTE_FREQUENCIES, NUM_NOTES};

// ── Command Enum ─────────────────────────────────────────────────────────

/// Decoded audio command.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Play a note. pitch = 0-11 (C through B), length = 1-16.
    Note { pitch: u8, length: u8 },

    /// Play a drum/noise note. length = 1-16, instrument = noise instrument ID.
    DrumNote { length: u8, instrument: u8 },

    /// Rest (silence). length = 1-16.
    Rest { length: u8 },

    /// Set note type: speed (1-16) + volume/envelope byte.
    /// For wave channel: the param byte's low nibble is wave instrument index.
    NoteType { speed: u8, param: u8 },

    /// Set octave (0-7 raw encoded). In the byte stream: $E0 = octave 8, $E7 = octave 1.
    /// We store the raw encoded value (byte & 0x07), not the musical octave.
    Octave(u8),

    /// Toggle perfect pitch flag.
    TogglePerfectPitch,

    /// Set vibrato parameters.
    /// delay = initial delay before vibrato starts, depth_rate = packed (depth<<4 | rate).
    Vibrato { delay: u8, depth_rate: u8 },

    /// Initiate pitch slide.
    /// length_modifier = duration adjustment, octave_pitch = packed (octave<<4 | pitch).
    PitchSlide {
        length_modifier: u8,
        octave_pitch: u8,
    },

    /// Set duty cycle (0-3).
    DutyCycle(u8),

    /// Set tempo (16-bit big-endian in original; we store as u16).
    Tempo(u16),

    /// Set stereo panning byte.
    StereoPanning(u8),

    /// Unknown command 0xEF (mostly unused, 1 param byte).
    UnknownEF(u8),

    /// Set master volume (written to NR50).
    Volume(u8),

    /// Toggle execute_music flag on SFX channel.
    ExecuteMusic,

    /// Set duty cycle rotation pattern (4 x 2-bit packed).
    DutyCyclePattern(u8),

    /// Call a subroutine at the given data offset.
    SoundCall { offset: u16 },

    /// Loop: count (0 = infinite), target offset.
    SoundLoop { count: u8, offset: u16 },

    /// Return from sound_call subroutine.
    SoundRet,

    /// Pitch sweep for channel 1 (NR10).
    /// Only valid on SFX channels (CHAN5-CHAN8) when not in execute_music mode.
    /// The param byte is written directly to NR10 (0xFF10).
    PitchSweep { param: u8 },

    /// SFX square note — directly specifies volume envelope + frequency.
    /// Used by the square-note SFX command family ($20-$2F) on SFX channels (CHAN4+).
    /// `length` is from the low nibble of the command byte (0-15).
    /// `volume_envelope` is the vol/fade byte (high nibble=vol, low=fade).
    /// `frequency` is the 11-bit frequency register value (little-endian in data).
    SfxSquareNote {
        length: u8,
        volume_envelope: u8,
        frequency: u16,
    },

    /// SFX noise note — directly specifies volume envelope + noise params.
    /// Used by the noise-note SFX command family ($20-$2F) on noise SFX channels (CHAN8).
    /// `length` is from the low nibble of the command byte (0-15).
    /// `volume_envelope` is the vol/fade byte.
    /// `noise_params` is the polynomial counter byte.
    SfxNoiseNote {
        length: u8,
        volume_envelope: u8,
        noise_params: u8,
    },

    /// End of data / unrecognized.
    EndOfData,
}

// ── Command Byte Constants ───────────────────────────────────────────────

pub const CMD_PITCH_SWEEP_CMD: u8 = 0x10;
pub const CMD_SFX_NOTE_CMD: u8 = 0x20;

pub const CMD_TOGGLE_PERFECT_PITCH: u8 = 0xE8;
// 0xE9 is unused
pub const CMD_VIBRATO: u8 = 0xEA;
pub const CMD_PITCH_SLIDE: u8 = 0xEB;
pub const CMD_DUTY_CYCLE: u8 = 0xEC;
pub const CMD_TEMPO: u8 = 0xED;
pub const CMD_STEREO_PANNING: u8 = 0xEE;
pub const CMD_UNKNOWN_EF: u8 = 0xEF;
pub const CMD_VOLUME: u8 = 0xF0;
// $F1-$F7 unused
pub const CMD_EXECUTE_MUSIC: u8 = 0xF8;
// $F9-$FB unused
pub const CMD_DUTY_CYCLE_PATTERN: u8 = 0xFC;
pub const CMD_SOUND_CALL: u8 = 0xFD;
pub const CMD_SOUND_LOOP: u8 = 0xFE;
pub const CMD_SOUND_RET: u8 = 0xFF;

// ── Decode ───────────────────────────────────────────────────────────────

/// Decode one command from a byte slice starting at `pos`.
///
/// `is_noise_channel` must be true when decoding for CHAN4/CHAN8 (noise channels).
/// `is_sfx_channel` must be true when decoding for CHAN5-CHAN8 (SFX channels).
/// `execute_music` must be true when the channel's EXECUTE_MUSIC flag is set.
///
/// Returns (command, new_pos).
pub fn decode_command(
    data: &[u8],
    pos: usize,
    is_noise_channel: bool,
    is_sfx_channel: bool,
    execute_music: bool,
) -> (Command, usize) {
    if pos >= data.len() {
        return (Command::EndOfData, pos);
    }

    let byte = data[pos];
    let mut next = pos + 1;

    match byte {
        // SFX note ($20-$2F): only on SFX channels (>= CHAN4) when not execute_music.
        // On non-SFX channels or with execute_music, falls through to regular note.
        b if (b & 0xF0) == CMD_SFX_NOTE_CMD && is_sfx_channel && !execute_music => {
            let length = b & 0x0F;
            let volume_envelope = read_byte_at(data, &mut next);
            if is_noise_channel {
                let noise_params = read_byte_at(data, &mut next);
                (
                    Command::SfxNoiseNote {
                        length,
                        volume_envelope,
                        noise_params,
                    },
                    next,
                )
            } else {
                let freq_lo = read_byte_at(data, &mut next);
                let freq_hi = read_byte_at(data, &mut next);
                let frequency = (freq_hi as u16) << 8 | freq_lo as u16;
                (
                    Command::SfxSquareNote {
                        length,
                        volume_envelope,
                        frequency,
                    },
                    next,
                )
            }
        }

        // Pitch sweep ($10): only on SFX channels (>= CHAN5) when not execute_music.
        CMD_PITCH_SWEEP_CMD if is_sfx_channel && !execute_music => {
            let param = read_byte_at(data, &mut next);
            (Command::PitchSweep { param }, next)
        }

        // Notes: $00-$AF
        0x00..=0xAF => {
            let pitch = byte >> 4;
            let length = (byte & 0x0F) + 1;
            (Command::Note { pitch, length }, next)
        }

        // $B0-$BF: On noise channel this is drum_note (2 bytes).
        // On other channels this is note B natural (pitch 11), 1 byte.
        0xB0..=0xBF => {
            if is_noise_channel {
                // Drum note: low nibble = length - 1, next byte = instrument ID
                let length = (byte & 0x0F) + 1;
                let instrument = if next < data.len() {
                    let v = data[next];
                    next += 1;
                    v
                } else {
                    0
                };
                (Command::DrumNote { length, instrument }, next)
            } else {
                // Regular note with pitch 11 (B)
                let pitch = byte >> 4; // = 11
                let length = (byte & 0x0F) + 1;
                (Command::Note { pitch, length }, next)
            }
        }

        // Rest: $C0-$CF
        0xC0..=0xCF => {
            let length = (byte & 0x0F) + 1;
            (Command::Rest { length }, next)
        }

        // Note type: $D0-$DF
        // Low nibble = speed (0-15).
        // On noise channel: no param byte (speed only).
        // On other channels: next byte = volume_envelope (or wave instrument).
        0xD0..=0xDF => {
            let speed = byte & 0x0F;
            if is_noise_channel {
                // Noise channel: note_type has 0 params
                (Command::NoteType { speed, param: 0 }, next)
            } else {
                let param = if next < data.len() {
                    let v = data[next];
                    next += 1;
                    v
                } else {
                    0
                };
                (Command::NoteType { speed, param }, next)
            }
        }

        // Octave: $E0-$E7
        // The raw encoded value is stored directly (0-7).
        // In the original engine, octave N is encoded as (8-N), so byte & 0x07
        // gives values 0-7 which map to musical octaves 8-1.
        // calculate_frequency expects this raw encoded value.
        0xE0..=0xE7 => {
            let octave = byte & 0x07;
            (Command::Octave(octave), next)
        }

        // Control commands $E8+
        CMD_TOGGLE_PERFECT_PITCH => (Command::TogglePerfectPitch, next),

        0xE9 => (Command::EndOfData, next), // unused

        CMD_VIBRATO => {
            let delay = read_byte_at(data, &mut next);
            let depth_rate = read_byte_at(data, &mut next);
            (Command::Vibrato { delay, depth_rate }, next)
        }

        CMD_PITCH_SLIDE => {
            let length_modifier = read_byte_at(data, &mut next);
            let octave_pitch = read_byte_at(data, &mut next);
            (
                Command::PitchSlide {
                    length_modifier,
                    octave_pitch,
                },
                next,
            )
        }

        CMD_DUTY_CYCLE => {
            let duty = read_byte_at(data, &mut next);
            (Command::DutyCycle(duty), next)
        }

        CMD_TEMPO => {
            // Original is big-endian: high byte first, then low byte
            let hi = read_byte_at(data, &mut next) as u16;
            let lo = read_byte_at(data, &mut next) as u16;
            (Command::Tempo((hi << 8) | lo), next)
        }

        CMD_STEREO_PANNING => {
            let pan = read_byte_at(data, &mut next);
            (Command::StereoPanning(pan), next)
        }

        CMD_UNKNOWN_EF => {
            let param = read_byte_at(data, &mut next);
            (Command::UnknownEF(param), next)
        }

        CMD_VOLUME => {
            let vol = read_byte_at(data, &mut next);
            (Command::Volume(vol), next)
        }

        0xF1..=0xF7 => (Command::EndOfData, next), // unused range

        CMD_EXECUTE_MUSIC => (Command::ExecuteMusic, next),

        0xF9..=0xFB => (Command::EndOfData, next), // unused range

        CMD_DUTY_CYCLE_PATTERN => {
            let pattern = read_byte_at(data, &mut next);
            (Command::DutyCyclePattern(pattern), next)
        }

        CMD_SOUND_CALL => {
            // 2-byte little-endian offset (in original this is a ROM address,
            // but we treat it as an offset into the data stream)
            let lo = read_byte_at(data, &mut next) as u16;
            let hi = read_byte_at(data, &mut next) as u16;
            let offset = (hi << 8) | lo;
            (Command::SoundCall { offset }, next)
        }

        CMD_SOUND_LOOP => {
            let count = read_byte_at(data, &mut next);
            let lo = read_byte_at(data, &mut next) as u16;
            let hi = read_byte_at(data, &mut next) as u16;
            let offset = (hi << 8) | lo;
            (Command::SoundLoop { count, offset }, next)
        }

        CMD_SOUND_RET => (Command::SoundRet, next),
    }
}

/// Helper: read one byte at `pos` from `data`, advancing pos.
fn read_byte_at(data: &[u8], pos: &mut usize) -> u8 {
    if *pos < data.len() {
        let v = data[*pos];
        *pos += 1;
        v
    } else {
        0
    }
}

// ── Frequency Calculation ────────────────────────────────────────────────

/// Calculate the 11-bit GB frequency register value for a given note and octave.
///
/// Replicates the classic GB audio frequency calculation:
/// 1. Look up the 16-bit base value from NOTE_FREQUENCIES[note]
/// 2. Arithmetic shift right by (7 - octave) positions
/// 3. Add 0x0800 to the high byte
/// 4. Return the 11-bit frequency register value (low 11 bits)
///
/// `note` is 0-11 (C through B), `octave` is 1-8 (engine convention).
pub fn calculate_frequency(note: u8, octave: u8) -> u16 {
    if note as usize >= NUM_NOTES {
        return 0;
    }

    let base = NOTE_FREQUENCIES[note as usize];

    // Arithmetic shift right by (7 - octave) — the original uses octave 1-8
    // where octave 8 means no shift and octave 1 means 7 shifts.
    // The engine stores octave as 1-8 (from command $E0-$E7 mapping).
    let shift = if octave <= 7 { 7 - octave } else { 0 };

    // Arithmetic right shift on a 16-bit signed value
    let mut freq = base as i16;
    for _ in 0..shift {
        freq >>= 1; // Rust >> on i16 is arithmetic shift
    }

    // Add 0x08 to high byte (= add 0x0800 to the 16-bit value)
    let result = (freq as u16).wrapping_add(0x0800);

    // The 11-bit frequency register value
    result & 0x07FF
}

/// Calculate note delay (frames) from note length, speed, and tempo.
///
/// Replicates the classic GB note-length calculation:
/// ```text
/// raw = note_length * note_speed    (8-bit multiply)
/// frac_accum += raw * tempo_lo
/// delay = raw * tempo_hi + carry_from_frac
/// ```
///
/// Returns (delay_counter, new_frac_accumulator).
pub fn calculate_delay(note_length: u8, note_speed: u8, tempo: u16, frac_accum: u8) -> (u8, u8) {
    let raw = (note_length as u16) * (note_speed as u16);

    // Multiply raw by tempo (16-bit), accumulating into 24-bit result
    // tempo is big-endian in the original: tempo_hi is the integer part,
    // tempo_lo is the fractional part.
    let tempo_hi = (tempo >> 8) as u16;
    let tempo_lo = (tempo & 0xFF) as u16;

    // Fractional part: raw * tempo_lo + previous frac accumulator
    let frac_product = raw * tempo_lo + frac_accum as u16;
    let new_frac = (frac_product & 0xFF) as u8;
    let carry = (frac_product >> 8) as u16;

    // Integer part: raw * tempo_hi + carry from fractional
    let int_product = raw * tempo_hi + carry;
    let delay = (int_product & 0xFF) as u8;

    // Clamp to at least 1 frame
    let delay = if delay == 0 { 1 } else { delay };

    (delay, new_frac)
}

// ── Encode ───────────────────────────────────────────────────────────────

/// Encode one command back into the byte stream, appending to `out`.
///
/// This is the exact inverse of [`decode_command`]. `is_noise_channel` must
/// match the channel the command belongs to (CHAN4 / CHAN8): on noise channels
/// a [`Command::NoteType`] carries no parameter byte, mirroring the decoder.
///
/// [`Command::EndOfData`] encodes to nothing — it is the implicit terminator,
/// not a real byte in well-formed streams.
///
/// Note the endianness quirks that mirror the decoder: `Tempo` is big-endian,
/// while `SoundCall` / `SoundLoop` offsets and `SfxSquareNote` frequency are
/// little-endian. Note/drum/rest lengths are stored as `length - 1`; SFX-note
/// lengths are stored raw (0-15).
pub fn encode_command(cmd: &Command, is_noise_channel: bool, out: &mut Vec<u8>) {
    match *cmd {
        Command::Note { pitch, length } => {
            out.push((pitch << 4) | (length.wrapping_sub(1) & 0x0F));
        }
        Command::DrumNote { length, instrument } => {
            out.push(0xB0 | (length.wrapping_sub(1) & 0x0F));
            out.push(instrument);
        }
        Command::Rest { length } => {
            out.push(0xC0 | (length.wrapping_sub(1) & 0x0F));
        }
        Command::NoteType { speed, param } => {
            out.push(0xD0 | (speed & 0x0F));
            if !is_noise_channel {
                out.push(param);
            }
        }
        Command::Octave(value) => out.push(0xE0 | (value & 0x07)),
        Command::TogglePerfectPitch => out.push(CMD_TOGGLE_PERFECT_PITCH),
        Command::Vibrato { delay, depth_rate } => {
            out.push(CMD_VIBRATO);
            out.push(delay);
            out.push(depth_rate);
        }
        Command::PitchSlide {
            length_modifier,
            octave_pitch,
        } => {
            out.push(CMD_PITCH_SLIDE);
            out.push(length_modifier);
            out.push(octave_pitch);
        }
        Command::DutyCycle(value) => {
            out.push(CMD_DUTY_CYCLE);
            out.push(value);
        }
        Command::Tempo(value) => {
            out.push(CMD_TEMPO);
            out.push((value >> 8) as u8); // big-endian: high byte first
            out.push((value & 0xFF) as u8);
        }
        Command::StereoPanning(value) => {
            out.push(CMD_STEREO_PANNING);
            out.push(value);
        }
        Command::UnknownEF(value) => {
            out.push(CMD_UNKNOWN_EF);
            out.push(value);
        }
        Command::Volume(value) => {
            out.push(CMD_VOLUME);
            out.push(value);
        }
        Command::ExecuteMusic => out.push(CMD_EXECUTE_MUSIC),
        Command::DutyCyclePattern(value) => {
            out.push(CMD_DUTY_CYCLE_PATTERN);
            out.push(value);
        }
        Command::SoundCall { offset } => {
            out.push(CMD_SOUND_CALL);
            out.push((offset & 0xFF) as u8); // little-endian: low byte first
            out.push((offset >> 8) as u8);
        }
        Command::SoundLoop { count, offset } => {
            out.push(CMD_SOUND_LOOP);
            out.push(count);
            out.push((offset & 0xFF) as u8);
            out.push((offset >> 8) as u8);
        }
        Command::SoundRet => out.push(CMD_SOUND_RET),
        Command::PitchSweep { param } => {
            out.push(CMD_PITCH_SWEEP_CMD);
            out.push(param);
        }
        Command::SfxSquareNote {
            length,
            volume_envelope,
            frequency,
        } => {
            out.push(CMD_SFX_NOTE_CMD | (length & 0x0F));
            out.push(volume_envelope);
            out.push((frequency & 0xFF) as u8); // little-endian
            out.push((frequency >> 8) as u8);
        }
        Command::SfxNoiseNote {
            length,
            volume_envelope,
            noise_params,
        } => {
            out.push(CMD_SFX_NOTE_CMD | (length & 0x0F));
            out.push(volume_envelope);
            out.push(noise_params);
        }
        Command::EndOfData => {}
    }
}

/// Encode a whole channel's command list into a byte stream.
pub fn encode_channel(commands: &[Command], is_noise_channel: bool) -> Vec<u8> {
    let mut out = Vec::new();
    for cmd in commands {
        encode_command(cmd, is_noise_channel, &mut out);
    }
    out
}

/// Linearly decode an entire channel byte stream into a command list.
///
/// Unlike [`decode_command`], this walks the whole slice front-to-back and
/// tracks the `execute_music` toggle (command `$F8`) as playback would, so the
/// SFX note macros (`$10`, `$20-$2F`) decode consistently. It does **not**
/// follow `sound_call` / `sound_loop` offsets — those are emitted verbatim as
/// [`Command::SoundCall`] / [`Command::SoundLoop`] payloads.
///
/// `encode_channel(&decode_channel(bytes, n, s), n)` reproduces `bytes` for any
/// well-formed stream (see the round-trip tests in `pokered-audio`).
pub fn decode_channel(data: &[u8], is_noise_channel: bool, is_sfx_channel: bool) -> Vec<Command> {
    let mut out = Vec::new();
    let mut pos = 0;
    let mut execute_music = false;
    while pos < data.len() {
        let (cmd, next) =
            decode_command(data, pos, is_noise_channel, is_sfx_channel, execute_music);
        if next <= pos {
            break; // safety: guarantee forward progress
        }
        if matches!(cmd, Command::ExecuteMusic) {
            execute_music = !execute_music;
        }
        pos = next;
        out.push(cmd);
    }
    out
}

#[cfg(test)]
mod encode_tests {
    use super::*;

    /// Round-trip a single command through encode → decode on a music channel.
    fn rt_music(cmd: Command) -> Command {
        let mut bytes = Vec::new();
        encode_command(&cmd, false, &mut bytes);
        decode_command(&bytes, 0, false, false, false).0
    }

    #[test]
    fn note_roundtrips() {
        for pitch in 0..=11u8 {
            for length in 1..=16u8 {
                assert_eq!(
                    rt_music(Command::Note { pitch, length }),
                    Command::Note { pitch, length }
                );
            }
        }
    }

    #[test]
    fn control_commands_roundtrip() {
        assert_eq!(
            rt_music(Command::Rest { length: 4 }),
            Command::Rest { length: 4 }
        );
        assert_eq!(rt_music(Command::Octave(3)), Command::Octave(3));
        assert_eq!(rt_music(Command::Tempo(0x0140)), Command::Tempo(0x0140));
        assert_eq!(rt_music(Command::DutyCycle(2)), Command::DutyCycle(2));
        assert_eq!(rt_music(Command::Volume(0x77)), Command::Volume(0x77));
        assert_eq!(
            rt_music(Command::NoteType {
                speed: 12,
                param: 0x92
            }),
            Command::NoteType {
                speed: 12,
                param: 0x92
            }
        );
        assert_eq!(
            rt_music(Command::SoundLoop {
                count: 0,
                offset: 0x1234
            }),
            Command::SoundLoop {
                count: 0,
                offset: 0x1234
            }
        );
        assert_eq!(
            rt_music(Command::SoundCall { offset: 0x00AB }),
            Command::SoundCall { offset: 0x00AB }
        );
    }

    #[test]
    fn noise_note_type_has_no_param() {
        // On the noise channel, NoteType is a single byte (no envelope param).
        let mut bytes = Vec::new();
        encode_command(&Command::NoteType { speed: 8, param: 0 }, true, &mut bytes);
        assert_eq!(bytes, vec![0xD8]);
        // …and decodes back with param 0.
        assert_eq!(
            decode_command(&bytes, 0, true, false, false).0,
            Command::NoteType { speed: 8, param: 0 }
        );
    }

    #[test]
    fn drum_note_roundtrips_on_noise() {
        let cmd = Command::DrumNote {
            length: 2,
            instrument: 5,
        };
        let bytes = encode_channel(std::slice::from_ref(&cmd), true);
        assert_eq!(decode_channel(&bytes, true, false), vec![cmd]);
    }

    #[test]
    fn sfx_notes_roundtrip() {
        let square = Command::SfxSquareNote {
            length: 3,
            volume_envelope: 0xA2,
            frequency: 0x0567,
        };
        let bytes = encode_channel(std::slice::from_ref(&square), false);
        assert_eq!(decode_channel(&bytes, false, true), vec![square]);

        let noise = Command::SfxNoiseNote {
            length: 1,
            volume_envelope: 0x91,
            noise_params: 0x33,
        };
        let bytes = encode_channel(std::slice::from_ref(&noise), true);
        assert_eq!(decode_channel(&bytes, true, true), vec![noise]);
    }
}
