//! Source hardware-channel setup work, separate from audio synthesis.

/// GetRegisterPointer's carry branch costs the same with or without carry.
pub fn register_pointer_work_cycles() -> u32 {
    4 + 12 + 4 + 12 + 4 + 8 + 4 + 4 + 8 + 16
}

pub fn duty_and_length_work_cycles(channel: u8) -> Option<u32> {
    if channel >= 8 {
        return None;
    }
    let mut cycles = 8 + 12 + 8 + 8 + 4 + 8;
    if channel == 2 {
        cycles += 12;
    } else {
        cycles += 8 + 8;
        if channel == 6 {
            cycles += 12;
        } else {
            cycles += 8 + 4 + 8 + 4 + 12 + 8 + 8 + 4 + 4;
        }
    }
    Some(cycles + 8 + 24 + register_pointer_work_cycles() + 8 + 16)
}

pub fn enable_channel_work_cycles(channel: u8, paired_sfx_active: bool) -> Option<u32> {
    if channel >= 8 {
        return None;
    }
    let mut cycles = 8 + 12 + 8 + 12 + 8 + 4 + 4 + 8;
    let apply_panning;
    if channel == 7 {
        cycles += 12;
        apply_panning = true;
    } else {
        cycles += 8 + 8;
        if channel >= 4 {
            cycles += 12;
            apply_panning = false;
        } else {
            cycles += 8 + 12 + 8 + 8 + 4;
            if paired_sfx_active {
                cycles += 12;
                apply_panning = false;
            } else {
                cycles += 8;
                apply_panning = true;
            }
        }
    }
    if apply_panning {
        cycles += 16 + 12 + 8 + 8 + 4 + 12 + 12 + 8 + 8 + 4 + 4;
    }
    Some(cycles + 4 + 12 + 16)
}

/// Original cry-ID interval in the shared SFX header table (20..134).
pub fn cry_check_work(sound_id: u8) -> (bool, u32) {
    let mut cycles = 16 + 8;
    if sound_id < 20 {
        return (false, cycles + 8 + 12 + 4 + 4 + 16);
    }
    cycles += 12 + 8;
    if sound_id == 134 {
        return (false, cycles + 12 + 4 + 4 + 16);
    }
    cycles += 8;
    if sound_id < 134 {
        (true, cycles + 12 + 4 + 16)
    } else {
        (false, cycles + 8 + 4 + 4 + 16)
    }
}

pub fn frequency_modifier_work(sound_id: u8, modifier: u8, frequency: u16) -> (u16, u32) {
    let (is_cry, check) = cry_check_work(sound_id);
    let mut cycles = 24 + check;
    if !is_cry {
        return (frequency, cycles + 12 + 16);
    }
    cycles += 8 + 16 + 4 + 12 + 8 + 4 + 8 + 8 + 8 + 16;
    (frequency.wrapping_add(u16::from(modifier)), cycles)
}

pub fn wave_frequency_work(
    channel: u8,
    sound_id: u8,
    modifier: u8,
    frequency: u16,
) -> Option<(u16, u32)> {
    if channel >= 8 {
        return None;
    }
    let mut cycles = 4 + 8;
    let wave;
    if channel == 2 {
        cycles += 12;
        wave = true;
    } else {
        cycles += 8 + 8;
        if channel == 6 {
            cycles += 8;
            wave = true;
        } else {
            cycles += 12;
            wave = false;
        }
    }
    if wave {
        cycles += 16 + 12 + 8;
        if channel == 2 {
            cycles += 12;
        } else {
            cycles += 8 + 12;
        }
        cycles += 8 + 4 + 8 + 4 + 12 + 8 + 8 + 8 + 8 + 12 + 8 + 8 + 12;
        for tile_byte in 0..16 {
            cycles += 8 + 8 + 8 + 4 + 4 + 4 + if tile_byte == 15 { 8 } else { 12 };
        }
        cycles += 8 + 12 + 12;
    }
    let hardware_frequency = ((frequency >> 8) as u8 | 0x80) & 0xc7;
    let frequency = (u16::from(hardware_frequency) << 8) | (frequency & 255);
    let (result, modifier_cycles) = frequency_modifier_work(sound_id, modifier, frequency);
    cycles += 4
        + 8
        + 8
        + 4
        + 8
        + 24
        + register_pointer_work_cycles()
        + 8
        + 8
        + 8
        + 24
        + modifier_cycles
        + 16;
    Some((result, cycles))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_setup_matches_original_live_and_isolated_calls() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("audio_output_work_golden.json")).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 59);
        for (index, case) in cases.iter().enumerate() {
            let channel = case["channel"].as_u64().unwrap() as u8;
            let paired = case["paired"].as_bool().unwrap();
            let cycles = match case["name"].as_str().unwrap() {
                "Audio3_GetRegisterPointer" => Some(register_pointer_work_cycles()),
                "Audio3_ApplyDutyCycleAndSoundLength" => duty_and_length_work_cycles(channel),
                "Audio3_EnableChannelOutput" => enable_channel_work_cycles(channel, paired),
                _ => panic!("unknown fixture routine"),
            };
            assert_eq!(
                cycles,
                Some(case["cycles"].as_u64().unwrap() as u32),
                "case {index}"
            );
        }
        assert_eq!(duty_and_length_work_cycles(8), None);
        assert_eq!(enable_channel_work_cycles(255, false), None);
    }

    #[test]
    fn wave_and_cry_work_matches_original_boundaries_and_carry() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("audio_wave_cry_work_golden.json")).unwrap();
        assert_eq!(fixture["cry_start"].as_u64(), Some(20));
        assert_eq!(fixture["cry_end"].as_u64(), Some(134));
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 288);
        for (index, c) in cases.iter().enumerate() {
            let byte = |key: &str| c[key].as_u64().unwrap() as u8;
            let frequency = c["frequency"].as_u64().unwrap() as u16;
            let expected_cycles = c["cycles"].as_u64().unwrap() as u32;
            let expected_frequency = c["DE"].as_u64().unwrap() as u16;
            match c["name"].as_str().unwrap() {
                "Audio3_IsCry" => {
                    assert_eq!(
                        cry_check_work(byte("sound_id")),
                        (byte("flags") & 16 != 0, expected_cycles),
                        "cry case {index}"
                    );
                    assert_eq!(expected_frequency, frequency);
                }
                "Audio3_ApplyFrequencyModifier" => assert_eq!(
                    frequency_modifier_work(byte("sound_id"), byte("modifier"), frequency),
                    (expected_frequency, expected_cycles),
                    "modifier case {index}"
                ),
                "Audio3_ApplyWavePatternAndFrequency" => assert_eq!(
                    wave_frequency_work(
                        byte("channel"),
                        byte("sound_id"),
                        byte("modifier"),
                        frequency
                    ),
                    Some((expected_frequency, expected_cycles)),
                    "wave case {index}"
                ),
                _ => panic!("unknown fixture routine"),
            }
        }
        assert_eq!(wave_frequency_work(8, 0, 0, 0), None);
    }
}
