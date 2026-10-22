//! Music command dispatch through entry to note-length handling.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
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
    state: MusicCommandState,
    vibrato_reload: u8,
) -> Option<(MusicCommandState, u8, usize, u32)> {
    use super::note_stream_instructions::{StreamInput, StreamInstructions};
    if channel >= 4 {
        return None;
    }
    // This legacy linear API deliberately excludes controls: its caller accounts
    // for those separately. The retained stream cursor itself handles controls.
    let mut ptr = 0;
    loop {
        let command = *bytes.get(ptr)?;
        ptr += 1;
        match command {
            0xd0..=0xdf => {
                if channel != 3 {
                    bytes.get(ptr)?;
                    ptr += 1;
                }
            }
            0xec => {
                bytes.get(ptr)?;
                ptr += 1;
            }
            0xe0..=0xe7 => (),
            0x00..=0xcf => break,
            _ => return None,
        }
    }
    let mut work = StreamInstructions::new(StreamInput {
        channel,
        ptr: 0,
        return_ptr: 0,
        loop_counter: 0,
        state,
        vibrato_reload,
    })
    .ok()?;
    while !work.finished() {
        work.try_step(bytes, |_| Ok::<(), core::convert::Infallible>(()))
            .ok()?;
    }
    let result = work.result()?;
    Some((result.state, result.note, result.next_ptr, work.cycles()))
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
