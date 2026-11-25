//! Original audio arithmetic results and instruction work, separate from playback.

pub fn multiply_add_work(factor: u8, value: u16, low: u8) -> (u16, u32) {
    const CYCLES: [u32; 256] = {
        let mut table = [0; 256];
        let mut factor = 0;
        while factor < 256 {
            let mut remaining = factor;
            let mut cycles = 24;
            loop {
                let bit = remaining & 1;
                remaining >>= 1;
                cycles += if remaining == 0 {
                    52 + 4 * bit
                } else {
                    60 + 4 * bit
                };
                if remaining == 0 {
                    break;
                }
            }
            table[factor] = cycles as u32;
            factor += 1;
        }
        table
    };
    (
        u16::from(low).wrapping_add(u16::from(factor).wrapping_mul(value)),
        CYCLES[usize::from(factor)],
    )
}

pub fn calculate_frequency_work(pitch: u16, octave: u8) -> (u16, u32) {
    let shifts = 7u8.wrapping_sub(octave);
    let shifted = ((pitch as i16) >> shifts.min(15)) as u16;
    (shifted.wrapping_add(0x800), 128 + 48 * u32::from(shifts))
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
    use super::note_math_instructions::{NoteLengthInput, NoteLengthInstructions};
    let work = NoteLengthInstructions::new(NoteLengthInput {
        channel,
        note,
        speed,
        fraction,
        tempo,
        flags1,
        flags2,
    })?;
    let ((fraction, delay), cycles) = work.evaluate();
    Some((fraction, delay, cycles))
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
