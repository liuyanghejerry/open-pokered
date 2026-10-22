//! Source sound-engine dispatch work. Channel effect bodies remain separate.

/// Audio3_UpdateMusic outer loop, including mute-state transition. Independent
/// effect costs are consumed only for actually dispatched channels. This is
/// instruction work, not audio synthesis or next-note parsing.
pub fn audio_dispatch_work(sound_ids: &[u8; 8], mut mute: u8, effects: &[u32; 8]) -> (u32, u8) {
    let mut cycles = 8;
    for channel in 0..8 {
        cycles += 8 + 12 + 8 + 8 + 4;
        if sound_ids[channel] == 0 {
            cycles += 12;
        } else {
            cycles += 8 + 4 + 8;
            let dispatch;
            if channel >= 4 {
                cycles += 12;
                dispatch = true;
            } else {
                cycles += 8 + 16 + 4;
                if mute == 0 {
                    cycles += 12;
                    dispatch = true;
                } else {
                    cycles += 8 + 8;
                    if mute & 128 != 0 {
                        cycles += 12;
                    } else {
                        cycles += 8 + 8 + 16 + 4 + 12 + 12 + 8 + 12 + 12;
                        mute |= 128;
                    }
                    dispatch = false;
                }
            }
            if dispatch {
                cycles += 24 + effects[channel];
            }
        }
        cycles += 4 + 4 + 8 + if channel == 7 { 8 } else { 12 };
    }
    (cycles + 16, mute)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_dispatch_matches_original_live_and_mute_cases() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("audio_dispatch_work_golden.json")).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 24);
        for (index, case) in cases.iter().enumerate() {
            let mut ids = [0; 8];
            let mut effects = [0; 8];
            for i in 0..8 {
                ids[i] = case["sound_ids"][i].as_u64().unwrap() as u8;
                effects[i] = case["effect_cycles"][i].as_u64().unwrap() as u32;
            }
            let result = audio_dispatch_work(&ids, case["mute"].as_u64().unwrap() as u8, &effects);
            assert_eq!(
                result,
                (
                    case["cycles"].as_u64().unwrap() as u32,
                    case["mute_after"].as_u64().unwrap() as u8
                ),
                "case {index}"
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct ChannelEffectWorkInput {
    pub channel: u8,
    pub paired_sfx_active: bool,
    pub note_delay: u8,
    pub flags1: u8,
    pub flags2: u8,
    pub vibrato_delay: u8,
    pub vibrato_extent: u8,
    pub vibrato_rate: u8,
    pub frequency_low: u8,
}

/// Source timer/vibrato work. Next-note parsing, rotating duty and pitch slide
/// are intentionally unsupported until their instruction work is composed.
pub fn channel_effect_work_cycles(input: ChannelEffectWorkInput) -> Option<u32> {
    Some(
        super::timer_work::EffectInstructionWork::new(input)?
            .map(u32::from)
            .sum(),
    )
}

#[cfg(test)]
mod effect_tests {
    use super::*;
    fn byte(value: &serde_json::Value) -> u8 {
        value.as_u64().unwrap() as u8
    }
    #[test]
    fn channel_effect_work_matches_original_supported_scene_calls() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("audio_effect_work_golden.json")).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 48);
        let mut supported = 0;
        let mut unknown = 0;
        for (index, case) in cases.iter().enumerate() {
            let s = &case["state"];
            let input = ChannelEffectWorkInput {
                channel: byte(&case["channel"]),
                paired_sfx_active: case["paired_sfx_active"].as_bool().unwrap(),
                note_delay: byte(&s["wChannelNoteDelayCounters"]),
                flags1: byte(&s["wChannelFlags1"]),
                flags2: byte(&s["wChannelFlags2"]),
                vibrato_delay: byte(&s["wChannelVibratoDelayCounters"]),
                vibrato_extent: byte(&s["wChannelVibratoExtents"]),
                vibrato_rate: byte(&s["wChannelVibratoRates"]),
                frequency_low: byte(&s["wChannelFrequencyLowBytes"]),
            };
            if input.note_delay == 1 {
                assert_eq!(channel_effect_work_cycles(input), None);
                unknown += 1;
            } else {
                assert_eq!(
                    channel_effect_work_cycles(input),
                    Some(case["cycles"].as_u64().unwrap() as u32),
                    "case {index}"
                );
                supported += 1;
            }
        }
        assert_eq!((supported, unknown), (43, 5));
    }
}

/// PlayNextNote -> sound_ret terminal CH4 path, with no paired SFX/cry.
/// State is the ordinary noise music channel, not a SOUND_CALL return.
/// Each block is the original instruction path; no measured service total
/// is used as a prediction.
pub fn noise_channel_end_work() -> u32 {
    use super::audio_math_work::next_music_byte_work_cycles;
    let entry = 12 + 8 + 8 + 12 + 8 + 8 + 12 + 8 + 16 + 16 + 24;
    let read_and_return_test =
        24 + next_music_byte_work_cycles() + 4 + 8 + 12 + 8 + 12 + 8 + 12 + 8 + 4 + 8 + 12;
    let clear_noise_flags = 16 + 12 + 8 + 16 + 8 + 12 + 12 + 12;
    let no_cry_and_stop = 16 + 8 + 8 + 12 + 12 + 8 + 8 + 16 + 16;
    entry + read_and_return_test + clear_noise_flags + no_cry_and_stop
}

#[cfg(test)]
mod noise_tests {
    use super::super::{
        audio_next_note_work::music_next_note_work, audio_note_dispatch_work::MusicCommandState,
    };
    use super::*;
    #[test]
    fn noise_rests_and_terminal_return_match_independent_rom_instructions() {
        let f: serde_json::Value =
            serde_json::from_str(include_str!("audio_noise_work_golden.json")).unwrap();
        assert_eq!(f["cases"].as_array().unwrap().len(), 82);
        for c in f["cases"].as_array().unwrap() {
            let i = &c["input"];
            if i["end"].as_bool() == Some(true) {
                assert_eq!(noise_channel_end_work(), c["T"].as_u64().unwrap() as u32);
                assert_eq!(c["sound_id"].as_u64(), Some(0));
            } else {
                let state = MusicCommandState {
                    speed: i["speed"].as_u64().unwrap() as u8,
                    volume: 0,
                    duty: 0,
                    octave: 4,
                    flags1: 4,
                    vibrato_delay: 0,
                };
                let result = music_next_note_work(
                    3,
                    &[i["note"].as_u64().unwrap() as u8],
                    state,
                    0,
                    0,
                    i["fraction"].as_u64().unwrap() as u8,
                    i["tempo"].as_u64().unwrap() as u16,
                    0,
                    None,
                )
                .unwrap();
                assert_eq!(
                    (result.cycles, result.delay, result.fraction),
                    (
                        c["T"].as_u64().unwrap() as u32,
                        c["delay"].as_u64().unwrap() as u8,
                        c["fraction"].as_u64().unwrap() as u8
                    ),
                    "{i}"
                );
            }
        }
    }
}
