//! Original audio arithmetic results and instruction work, separate from playback.

pub fn multiply_add_work(mut factor: u8, mut value: u16, low: u8) -> (u16, u32) {
    let mut result = u16::from(low);
    let mut cycles = 8;
    loop {
        let bit = factor & 1;
        factor >>= 1;
        cycles += 8;
        if bit == 0 {
            cycles += 12;
        } else {
            cycles += 8 + 8;
            result = result.wrapping_add(value);
        }
        value = value.wrapping_shl(1);
        cycles += 8 + 8 + 4;
        if factor == 0 {
            cycles += 12;
            break;
        } else {
            cycles += 8 + 12;
        }
    }
    (result, cycles + 16)
}

pub fn calculate_frequency_work(mut pitch: u16, mut octave: u8) -> (u16, u32) {
    let mut cycles = 8 + 4 + 8 + 4 + 4 + 12 + 8 + 8 + 8 + 8 + 4;
    loop {
        cycles += 8;
        if octave == 7 {
            cycles += 12;
            break;
        }
        cycles += 8 + 8 + 8 + 4 + 12;
        pitch = ((pitch as i16) >> 1) as u16;
        octave = octave.wrapping_add(1);
    }
    (pitch.wrapping_add(0x0800), cycles + 8 + 4 + 4 + 16)
}

pub fn next_music_byte_work_cycles() -> u32 {
    8 + 4 + 4 + 4 + 12 + 8 + 8 + 4 + 8 + 4 + 8 + 8 + 8 + 8 + 8 + 16
}

/// Music note-length computation through note-pitch entry, or the noise
/// music channel's early return. SFX tempo paths remain separate.
pub fn music_note_length_work(
    channel: u8,
    note: u8,
    speed: u8,
    fraction: u8,
    tempo: u16,
    flags1: u8,
    flags2: u8,
) -> Option<(u8, u8, u32)> {
    if channel >= 4
        || (channel == 3 && (flags1 & 4 == 0 || flags2 & 1 != 0))
        || (channel != 3 && flags2 & 1 == 0 && flags1 & 4 != 0)
    {
        return None;
    }
    let (length, first) = multiply_add_work(speed, u16::from((note & 15) + 1), 0);
    let (delay, second) = multiply_add_work(length as u8, tempo, fraction);
    let prefix = 4 + 16 + 8 + 4 + 8 + 4 + 4 + 12 + 8 + 8 + 4 + 24;
    let middle = 4 + 8 + 8 + 16 + 4 + 16 + 4 + 12 + 4 + 8 + 12 + 8 + 8 + 24;
    let mut suffix = 4 + 4 + 12 + 8 + 8 + 4 + 12 + 8 + 8 + 12 + 8 + 12;
    if flags2 & 1 != 0 {
        suffix += 12;
    } else if flags1 & 4 != 0 {
        suffix += 8 + 12 + 8 + 12 + 8 + 12 + 16;
    } else {
        suffix += 8 + 12 + 8 + 12 + 12;
    }
    Some((
        delay as u8,
        (delay >> 8) as u8,
        prefix + first + middle + second + suffix,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic_matches_original_results_and_instruction_work() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("audio_math_work_golden.json")).unwrap();
        let multiply = fixture["multiply"].as_array().unwrap();
        let frequency = fixture["frequency"].as_array().unwrap();
        assert_eq!(multiply.len(), 522);
        assert_eq!(frequency.len(), 104);
        for (index, case) in multiply.iter().enumerate() {
            assert_eq!(
                multiply_add_work(
                    case["a"].as_u64().unwrap() as u8,
                    case["de"].as_u64().unwrap() as u16,
                    case["low"].as_u64().unwrap() as u8
                ),
                (
                    case["result"].as_u64().unwrap() as u16,
                    case["cycles"].as_u64().unwrap() as u32
                ),
                "multiply case {index}"
            );
        }
        for (index, case) in frequency.iter().enumerate() {
            assert_eq!(
                calculate_frequency_work(
                    case["pitch"].as_u64().unwrap() as u16,
                    case["octave"].as_u64().unwrap() as u8
                ),
                (
                    case["result"].as_u64().unwrap() as u16,
                    case["cycles"].as_u64().unwrap() as u32
                ),
                "frequency case {index}"
            );
        }
    }

    #[test]
    fn music_note_length_matches_original_live_state_and_work() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("audio_note_length_work_golden.json")).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 389);
        for (index, c) in cases.iter().enumerate() {
            let byte = |key: &str| c[key].as_u64().unwrap() as u8;
            let result = music_note_length_work(
                byte("channel"),
                byte("note"),
                byte("speed"),
                byte("fraction"),
                c["tempo"].as_u64().unwrap() as u16,
                byte("flags1"),
                byte("flags2"),
            );
            assert_eq!(
                result,
                Some((
                    byte("after_fraction"),
                    byte("after_delay"),
                    c["cycles"].as_u64().unwrap() as u32
                )),
                "case {index}"
            );
        }
        assert_eq!(music_note_length_work(3, 0, 6, 0, 124, 0, 0), None);
        assert_eq!(music_note_length_work(0, 0, 6, 0, 124, 4, 0), None);
    }
}
