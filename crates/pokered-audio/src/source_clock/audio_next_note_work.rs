//! Source instruction work for supported next notes; native playback remains
//! owned by the sequencer. The ISR scheduler consumes this work separately.
use super::audio_math_work::{calculate_frequency_work, music_note_length_work};
use super::audio_note_dispatch_work::{music_dispatch_work, MusicCommandState};
use super::audio_output_work::{
    duty_and_length_work_cycles, enable_channel_work_cycles, register_pointer_work_cycles,
    wave_frequency_work,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextNoteWorkResult {
    pub fraction: u8,
    pub delay: u8,
    pub frequency_low: u8,
    pub cycles: u32,
}

/// Music channels without a paired SFX, using supported commands. Pitch table
/// values are source data, not traced timing constants. Noise/SFX are separate.
pub fn music_next_note_work(
    channel: u8,
    commands: &[u8],
    state: MusicCommandState,
    vibrato_reload: u8,
    flags2: u8,
    fraction: u8,
    tempo: u16,
    frequency_low: u8,
    pitch_base: Option<u16>,
) -> Option<NextNoteWorkResult> {
    music_next_note_work_with_sfx(
        channel,
        commands,
        state,
        vibrato_reload,
        flags2,
        fraction,
        tempo,
        frequency_low,
        pitch_base,
        false,
        0,
    )
}

pub fn music_next_note_work_with_sfx(
    channel: u8,
    commands: &[u8],
    state: MusicCommandState,
    vibrato_reload: u8,
    flags2: u8,
    fraction: u8,
    tempo: u16,
    frequency_low: u8,
    pitch_base: Option<u16>,
    paired_sfx_active: bool,
    source_pulse_sfx_id: u8,
) -> Option<NextNoteWorkResult> {
    let (state, note, _, dispatch) = music_dispatch_work(channel, commands, state, vibrato_reload)?;
    let (fraction, delay, length) = music_note_length_work(
        channel,
        note,
        state.speed,
        fraction,
        tempo,
        state.flags1,
        flags2,
    )?;
    let mut cycles = dispatch + length;
    let mut low = frequency_low;
    if channel == 3 {
        if state.flags1 & 4 == 0 || flags2 & 1 != 0 {
            return None;
        }
        return Some(NextNoteWorkResult {
            fraction,
            delay,
            frequency_low,
            cycles: cycles + 16,
        });
    }
    cycles += 12 + 8 + 8;
    if note & 0xf0 == 0xc0 {
        if paired_sfx_active {
            cycles += 8 + 4 + 8 + 8 + 12 + 8 + 8 + 4 + 12 + 16 + 16;
            return Some(NextNoteWorkResult {
                fraction,
                delay,
                frequency_low,
                cycles,
            });
        }
        cycles += 8 + 4 + 8 + 8 + 12 + 8 + 8 + 4 + 8 + 4 + 8;
        if channel == 2 {
            cycles += 12 + 8 + 12 + 8 + 12 + 8 + 12 + 12;
        } else {
            cycles += 8 + 8 + 12 + 8 + 24 + register_pointer_work_cycles() + 8 + 8 + 8 + 8 + 8;
        }
        cycles += 16 + 16;
    } else {
        if note >> 4 >= 12 {
            return None;
        }
        cycles += 12 + 8 + 8 + 12 + 8 + 8 + 24;
        let (frequency, frequency_cycles) = calculate_frequency_work(pitch_base?, state.octave);
        cycles += frequency_cycles + 8 + 12 + 8 + 12 + 12;
        if paired_sfx_active {
            cycles += 16 + 4 + 8 + 8 + 12 + 8 + 4 + 8 + 8 + 4 + 12 + 12 + 16 + 16;
            return Some(NextNoteWorkResult {
                fraction,
                delay,
                frequency_low,
                cycles,
            });
        }
        cycles += 16 + 4 + 8 + 8 + 12 + 8 + 4 + 8 + 8 + 4 + 8 + 12;
        cycles += 8 + 12 + 8 + 8 + 8 + 24 + register_pointer_work_cycles() + 8;
        cycles += 24 + duty_and_length_work_cycles(channel)?;
        cycles += 24 + enable_channel_work_cycles(channel, false)? + 12;
        cycles += 8 + 12 + 8 + 12;
        low = frequency as u8;
        if state.flags1 & 1 == 0 {
            cycles += 12;
        } else {
            // INC E does not set carry. EnableChannelOutput's final OR clears
            // it, so the subsequent JR nc skips INC D even on low-byte wrap.
            low = low.wrapping_add(1);
            cycles += 8 + 4 + 12;
        }
        cycles += 12 + 8 + 8 + 24;
        let modified_frequency = (frequency & 0xff00) | u16::from(low);
        cycles +=
            wave_frequency_work(channel, source_pulse_sfx_id, 0, modified_frequency)?.1 + 16 + 16;
    }
    Some(NextNoteWorkResult {
        fraction,
        delay,
        frequency_low: low,
        cycles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    #[test]
    fn complete_next_note_matches_original_live_departure_calls() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("audio_next_note_work_golden.json")).unwrap();
        let cases = f["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 158);
        for (index, c) in cases.iter().enumerate() {
            let byte = |key: &str| c[key].as_u64().unwrap() as u8;
            let initial = &c["initial"];
            let ib = |key: &str| initial[key].as_u64().unwrap() as u8;
            let commands: Vec<u8> = c["commands"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b.as_u64().unwrap() as u8)
                .collect();
            let state = MusicCommandState {
                speed: ib("speed"),
                volume: ib("volume"),
                duty: ib("duty"),
                octave: ib("octave"),
                flags1: ib("flags1"),
                vibrato_delay: 0,
            };
            let actual = music_next_note_work(
                byte("channel"),
                &commands,
                state,
                ib("vibrato_reload"),
                byte("flags2"),
                byte("fraction"),
                c["tempo"].as_u64().unwrap() as u16,
                byte("frequency_low"),
                c["pitch_base"].as_u64().map(|p| p as u16),
            );
            let expected = &c["result"];
            assert_eq!(
                actual,
                Some(NextNoteWorkResult {
                    fraction: expected["fraction"].as_u64().unwrap() as u8,
                    delay: expected["delay"].as_u64().unwrap() as u8,
                    frequency_low: expected["frequency_low"].as_u64().unwrap() as u8,
                    cycles: expected["cycles"].as_u64().unwrap() as u32
                }),
                "case {index}"
            );
        }
    }
}

#[cfg(test)]
mod paired_sfx_tests {
    use super::*;
    #[test]
    fn music_next_notes_use_the_actual_paired_gate_and_global_source_sfx_id() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("paired_music_work_golden.json")).unwrap();
        let cases = v["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 18);
        for c in cases {
            let i = &c["input"];
            let ch = i["channel"].as_u64().unwrap() as u8;
            let note = i["data"][0].as_u64().unwrap() as u8;
            let state = MusicCommandState {
                speed: 6,
                volume: 179,
                duty: 128,
                octave: 6,
                flags1: 0,
                vibrato_delay: 0,
            };
            let result = music_next_note_work_with_sfx(
                ch,
                &[note],
                state,
                0,
                0,
                i["fraction"].as_u64().unwrap() as u8,
                0x100,
                77,
                crate::NOTE_FREQUENCIES.get((note >> 4) as usize).copied(),
                ch == 0,
                144,
            )
            .unwrap();
            assert_eq!(
                result.cycles,
                c["T"].as_u64().unwrap() as u32,
                "ch{ch} note{note}"
            );
            assert_eq!(result.delay, c["delay"].as_u64().unwrap() as u8);
            assert_eq!(result.fraction, c["fraction"].as_u64().unwrap() as u8);
            assert_eq!(
                result.frequency_low,
                c["frequency_low"].as_u64().unwrap() as u8
            );
        }
    }
}
