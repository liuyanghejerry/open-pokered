//! Follow control commands in the native channel's own converted byte stream.
//! The supported tonal commands, noise rests and finite/infinite loops are accepted.
//! Pointer targets are native data offsets, never original ROM addresses.
use super::audio_math_work::next_music_byte_work_cycles;
use super::audio_note_dispatch_work::{music_dispatch_work, MusicCommandState};
use crate::alloc_prelude::*;

#[derive(Debug)]
pub struct MusicStreamWork {
    pub state: MusicCommandState,
    pub note: u8,
    pub next_ptr: usize,
    pub return_ptr: usize,
    pub loop_counter: u8,
    pub tempo: Option<u16>,
    pub vibrato_reload: u8,
    pub linear_commands: Vec<u8>,
    pub control_cycles: u32,
}

fn byte(data: &[u8], ptr: &mut usize) -> Option<u8> {
    let value = *data.get(*ptr)?;
    *ptr += 1;
    Some(value)
}
fn word(data: &[u8], ptr: &mut usize) -> Option<usize> {
    let low = byte(data, ptr)?;
    Some(usize::from(low) | (usize::from(byte(data, ptr)?) << 8))
}

/// The ordinary dispatch/arithmetic helpers handle a linear prefix. Resolve
/// each real call/return/loop here and retain the other commands in order.
/// The added work is each control's sound_ret→sound_ret instruction path;
/// the ordinary helper accounts for the single PlayNextNote entry and note.
/// Unrelated commands, truncation, and an unbounded immediate command loop
/// stop instead of fabricating a note or a channel return.
pub fn music_stream_work(
    channel: u8,
    data: &[u8],
    mut ptr: usize,
    mut return_ptr: usize,
    mut loop_counter: u8,
    mut state: MusicCommandState,
    mut vibrato_reload: u8,
) -> Option<MusicStreamWork> {
    if channel >= 4 {
        return None;
    }
    let next = next_music_byte_work_cycles();
    // CALL GetNextMusicByte / LD D,A / CP sound_ret / JP NZ.
    let not_return = 24 + next + 4 + 8 + 16;
    let mut tempo = None;
    let mut control_cycles = 0;
    let mut linear = Vec::new();
    for _ in 0..100 {
        let command = byte(data, &mut ptr)?;
        match command {
            0xfd => {
                let target = word(data, &mut ptr)?;
                return_ptr = ptr;
                ptr = target;
                state.flags1 |= 2;
                // CP/JP, two parameter fetches, save return address, install
                // target, set SOUND_CALL, and JP sound_ret.
                control_cycles += not_return
                    + 8
                    + 12
                    + 24
                    + next
                    + 16
                    + 24
                    + next
                    + 4
                    + 12
                    + 4
                    + 16
                    + 8
                    + 4
                    + 4
                    + 4
                    + 12
                    + 8
                    + 16
                    + 12
                    + 8
                    + 4
                    + 4
                    + 12
                    + 8
                    + 8
                    + 8
                    + 8
                    + 8
                    + 12
                    + 8
                    + 8
                    + 8
                    + 8
                    + 12
                    + 8
                    + 16
                    + 16;
            }
            0xfe => {
                let count = byte(data, &mut ptr)?;
                let target = word(data, &mut ptr)?;
                control_cycles += not_return + 8 + 16 + 8 + 12 + 24 + next + 4 + 4;
                let repeat = if count == 0 {
                    control_cycles += 12;
                    true
                } else {
                    control_cycles += 8 + 8 + 12 + 8 + 8 + 4;
                    let remaining = if loop_counter == 0 {
                        count
                    } else {
                        loop_counter
                    };
                    loop_counter = remaining.wrapping_sub(1);
                    if loop_counter == 0 {
                        // Source increments from 1 and resets to 1; the native
                        // channel stores remaining repeats. Both take this
                        // branch on the last pass. Two byte reads skip target.
                        control_cycles += 8 + 8 + 8 + 2 * (24 + next) + 16;
                        false
                    } else {
                        control_cycles += 12 + 4 + 8;
                        true
                    }
                };
                if repeat {
                    ptr = target;
                    control_cycles +=
                        24 + next + 16 + 24 + next + 4 + 8 + 4 + 4 + 4 + 12 + 8 + 12 + 8 + 8 + 16;
                }
            }
            0xff => {
                if state.flags1 & 2 == 0 {
                    return None;
                }
                state.flags1 &= !2;
                ptr = return_ptr;
                // The JP NZ is not taken, BIT SOUND_CALL branches to the
                // real stored return pointer, clears the flag, then JP.
                control_cycles += 24
                    + next
                    + 4
                    + 8
                    + 12
                    + 8
                    + 12
                    + 8
                    + 12
                    + 12
                    + 16
                    + 8
                    + 4
                    + 4
                    + 4
                    + 12
                    + 8
                    + 16
                    + 12
                    + 8
                    + 4
                    + 4
                    + 12
                    + 8
                    + 8
                    + 8
                    + 8
                    + 8
                    + 16;
            }
            0xe8 => {
                state.flags1 ^= 1;
                control_cycles += not_return
                    + 8
                    + 16
                    + 8
                    + 16
                    + 8
                    + 8
                    + 16
                    + 4
                    + 8
                    + 8
                    + 8
                    + 12
                    + 8
                    + 8
                    + 8
                    + 8
                    + 16;
            }
            0xea => {
                vibrato_reload = byte(data, &mut ptr)?;
                let _depth_rate = byte(data, &mut ptr)?;
                state.vibrato_delay = vibrato_reload;
                control_cycles += not_return
                    + 8
                    + 16
                    + 8
                    + 16
                    + 8
                    + 8
                    + 16
                    + 4
                    + 8
                    + 12
                    + 8
                    + 8
                    + 24
                    + next
                    + 8
                    + 12
                    + 8
                    + 8
                    + 12
                    + 8
                    + 8
                    + 24
                    + next
                    + 4
                    + 8
                    + 8
                    + 8
                    + 12
                    + 8
                    + 8
                    + 4
                    + 4
                    + 8
                    + 4
                    + 8
                    + 4
                    + 8
                    + 4
                    + 12
                    + 8
                    + 8
                    + 4
                    + 8
                    + 16;
            }
            0xed => {
                let high = byte(data, &mut ptr)?;
                tempo = Some((u16::from(high) << 8) | u16::from(byte(data, &mut ptr)?));
                control_cycles += not_return
                    + 8
                    + 16
                    + 8
                    + 16
                    + 8
                    + 8
                    + 16
                    + 4
                    + 8
                    + 12
                    + 3 * (8 + 12)
                    + 8
                    + 8
                    + 4
                    + 8
                    + 8
                    + 24
                    + next
                    + 16
                    + 24
                    + next
                    + 16
                    + 4
                    + 4 * 16
                    + 12
                    + 16;
            }
            0xf0 => {
                let _volume = byte(data, &mut ptr)?;
                control_cycles += not_return
                    + 8
                    + 16
                    + 8
                    + 16
                    + 8
                    + 8
                    + 16
                    + 4
                    + 8
                    + 12
                    + 7 * (8 + 12)
                    + 8
                    + 8
                    + 24
                    + next
                    + 12
                    + 16;
            }
            0xd0..=0xdf | 0xec => {
                linear.push(command);
                if channel != 3 || command == 0xec {
                    linear.push(byte(data, &mut ptr)?);
                }
            }
            0xe0..=0xe7 => linear.push(command),
            0x00..=0xcf => {
                linear.push(command);
                let (state, note, _, _) =
                    music_dispatch_work(channel, &linear, state, vibrato_reload)?;
                return Some(MusicStreamWork {
                    state,
                    note,
                    next_ptr: ptr,
                    return_ptr,
                    loop_counter,
                    tempo,
                    vibrato_reload,
                    linear_commands: linear,
                    control_cycles,
                });
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::audio_next_note_work::music_next_note_work;
    use super::*;

    #[test]
    fn counted_loops_match_original_instruction_path_and_native_remaining_counter() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("audio_loop_work_golden.json")).unwrap();
        for c in f["cases"].as_array().unwrap() {
            let channel = c["channel"].as_u64().unwrap() as u8;
            let count = c["count"].as_u64().unwrap() as u8;
            let remaining = c["native_remaining"].as_u64().unwrap() as u8;
            let state = MusicCommandState {
                speed: 1,
                volume: 0,
                duty: 0,
                octave: 4,
                flags1: if channel == 3 { 4 } else { 0 },
                vibrato_delay: 0,
            };
            let stream = music_stream_work(
                channel,
                &[0xfe, count, 6, 0, 0xc0, 0xc0, 0xc0],
                0,
                0,
                remaining,
                state,
                0,
            )
            .unwrap();
            assert_eq!(
                stream.control_cycles,
                c["T"].as_u64().unwrap() as u32,
                "channel{channel} count{count} left{remaining}"
            );
            assert_eq!(
                stream.next_ptr,
                if c["next_ptr"].as_u64().unwrap() == 0xd100 {
                    7
                } else {
                    5
                }
            );
            assert_eq!(
                stream.loop_counter,
                if count == 0 {
                    remaining
                } else {
                    (if remaining == 0 { count } else { remaining }) - 1
                }
            );
        }
    }

    #[test]
    fn real_original_stream_controls_match_pointer_state_and_full_work() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("audio_stream_work_golden.json")).unwrap();
        let cases = f["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 33);
        for c in cases {
            let input = &c["input"];
            let a = &c["initial"];
            let s = &a["state"];
            let b = |key: &str| s[key].as_u64().unwrap() as u8;
            let state = MusicCommandState {
                speed: b("speed"),
                volume: b("volume"),
                duty: b("duty"),
                octave: b("octave"),
                flags1: b("flags1"),
                vibrato_delay: b("vibrato_delay"),
            };
            let data: Vec<u8> = input["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let channel = input["channel"].as_u64().unwrap() as u8;
            let stream = music_stream_work(
                channel,
                &data,
                input["ptr"].as_u64().unwrap_or(0) as usize,
                input["return_ptr"].as_u64().unwrap_or(0) as usize,
                input["loop_counter"].as_u64().unwrap_or(0) as u8,
                state,
                4,
            )
            .unwrap();
            let result = music_next_note_work(
                channel,
                &stream.linear_commands,
                stream.state,
                stream.vibrato_reload,
                0,
                if stream.tempo.is_some() { 0 } else { 64 },
                stream.tempo.unwrap_or(124),
                0,
                crate::NOTE_FREQUENCIES
                    .get((stream.note >> 4) as usize)
                    .copied(),
            )
            .unwrap();
            let expected = &c["result"];
            assert_eq!(
                serde_json::json!({"cycles":result.cycles+stream.control_cycles,"note":stream.note,
                "state":{"speed":stream.state.speed,"volume":stream.state.volume,"duty":stream.state.duty,"octave":stream.state.octave,"flags1":stream.state.flags1,"vibrato_delay":stream.state.vibrato_delay,"vibrato_reload":stream.vibrato_reload},
                "next_ptr":stream.next_ptr,"return_ptr":stream.return_ptr,"delay":result.delay,"fraction":result.fraction,"tempo":stream.tempo.unwrap_or(124)}),
                // The original oracle additionally records the physical RET
                // stack boundary, which the native channel VM does not own.
                {
                    let mut value = expected.clone();
                    value.as_object_mut().unwrap().remove("SP");
                    value
                },
                "{}",
                input["label"]
            );
        }
    }

    #[test]
    fn malformed_or_unbounded_stream_does_not_invent_a_note() {
        let state = MusicCommandState {
            speed: 6,
            volume: 0xb3,
            duty: 0xc0,
            octave: 6,
            flags1: 0,
            vibrato_delay: 0,
        };
        assert!(music_stream_work(0, &[0xfe, 0, 0, 0], 0, 0, 0, state, 0).is_none());
        assert!(music_stream_work(0, &[0xfe, 1, 0, 0], 0, 0, 0, state, 0).is_none());
        assert!(music_stream_work(0, &[0xfd, 0x80, 0], 0, 0, 0, state, 0).is_none());
        assert!(music_stream_work(0, &[0xed, 0], 0, 0, 0, state, 0).is_none());
        assert!(music_stream_work(0, &[0xff], 0, 0, 0, state, 0).is_none());
    }
}
