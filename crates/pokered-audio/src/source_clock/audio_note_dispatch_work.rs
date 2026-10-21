//! Music command dispatch through entry to note-length handling.
use super::audio_math_work::next_music_byte_work_cycles;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusicCommandState {
    pub speed: u8,
    pub volume: u8,
    pub duty: u8,
    pub octave: u8,
    pub flags1: u8,
    pub vibrato_delay: u8,
}

/// Supports speed/volume, duty, octave and ordinary notes for music channels.
/// Calls, loops, other control commands and noise/SFX require separate paths.
pub fn music_dispatch_work(
    channel: u8,
    bytes: &[u8],
    mut state: MusicCommandState,
    vibrato_reload: u8,
) -> Option<(MusicCommandState, u8, usize, u32)> {
    if channel >= 4 {
        return None;
    }
    state.flags1 &= !(16 | 32);
    state.vibrato_delay = vibrato_reload;
    let mut cycles = 12 + 8 + 8 + 12 + 8 + 8 + 12 + 8 + 16 + 16 + 24;
    let mut cursor = 0;
    loop {
        let command = *bytes.get(cursor)?;
        cursor += 1;
        if command == 0xff || command == 0xfd || command == 0xfe {
            return None;
        }
        cycles += 24 + next_music_byte_work_cycles() + 4 + 8 + 16;
        cycles += 8 + 16 + 8 + 16; // failed call and loop dispatch
        if command & 0xf0 == 0xd0 {
            cycles += 8 + 8 + 12; // successful note_type dispatch
            state.speed = command & 15;
            cycles += 4 + 8 + 8 + 12 + 8 + 8 + 4 + 8 + 8;
            if channel == 3 {
                cycles += 4 + 16; // JR z taken; noise has no parameter byte.
                continue;
            }
            let parameter = *bytes.get(cursor)?;
            cursor += 1;
            cycles += 24 + next_music_byte_work_cycles() + 4 + 4 + 8;
            if channel == 2 {
                cycles += 12 + 12 + 4 + 8 + 8 + 4 + 8 + 8 + 4;
                state.volume = (parameter & 0x30) << 1;
            } else {
                cycles += 8 + 8 + 12;
                state.volume = parameter;
            }
            cycles += 8 + 12 + 8 + 8 + 16;
            continue;
        }
        cycles += 8 + 8 + 16;
        // toggle-perfect-pitch reloads D into A before comparing; vibrato and
        // pitch-slide then compare the same command.
        cycles += 4 + 8 + 12 + 8 + 12 + 8 + 12;
        if command == 0xec {
            cycles += 8 + 8;
            let parameter = *bytes.get(cursor)?;
            cursor += 1;
            state.duty = (parameter & 3) << 6;
            cycles += 24 + next_music_byte_work_cycles() + 4 + 4 + 8 + 8 + 12 + 8 + 8 + 16;
            continue;
        }
        // Duty, tempo, stereo, unknown-EF, duty-pattern, volume, execute-music.
        cycles += 7 * (8 + 12);
        if command & 0xf8 == 0xe0 {
            state.octave = command & 15;
            cycles += 8 + 8 + 8 + 12 + 8 + 8 + 4 + 8 + 8 + 16;
            continue;
        }
        if command >= 0xe0 {
            return None;
        }
        cycles += 8 + 8 + 12; // failed octave dispatch
        cycles += 8 + 12; // sfx-note comparison
        if command & 0xf0 == 0x20 {
            cycles += 4 + 8 + 12 - 4; // sfx-note match uses an untaken JR nz
        }
        cycles += 4 + 8 + 12; // pitch sweep: music channel
        if channel == 3 {
            if command < 0xc0 {
                return None;
            } // Drum paths are separate.
            cycles += 4 + 8 + 8 + 4 + 8 + 8 + 8 + 12;
        } else {
            cycles += 4 + 8 + 12;
        } // ordinary note or noise rest
        return Some((state, command, cursor, cycles));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    #[test]
    fn command_dispatch_matches_original_state_and_work() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("audio_note_dispatch_work_golden.json")).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 158);
        for (index, c) in cases.iter().enumerate() {
            let initial = &c["initial"];
            let byte = |v: &serde_json::Value, key: &str| v[key].as_u64().unwrap() as u8;
            let state = MusicCommandState {
                speed: byte(initial, "speed"),
                volume: byte(initial, "volume"),
                duty: byte(initial, "duty"),
                octave: byte(initial, "octave"),
                flags1: byte(initial, "flags1"),
                vibrato_delay: 0,
            };
            let commands: Vec<u8> = c["commands"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_u64().unwrap() as u8)
                .collect();
            let result = music_dispatch_work(
                byte(c, "channel"),
                &commands,
                state,
                byte(initial, "vibrato_reload"),
            )
            .unwrap();
            let expected = &c["result"];
            assert_eq!(
                result,
                (
                    MusicCommandState {
                        speed: byte(expected, "speed"),
                        volume: byte(expected, "volume"),
                        duty: byte(expected, "duty"),
                        octave: byte(expected, "octave"),
                        flags1: byte(expected, "flags1"),
                        vibrato_delay: byte(expected, "vibrato_delay")
                    },
                    byte(expected, "note"),
                    commands.len(),
                    expected["cycles"].as_u64().unwrap() as u32
                ),
                "case {index}"
            );
        }
    }
}
