//! Instruction work for the finite pulse confirmation tone. The real native
//! sequencer remains the only playback owner; this model never ticks audio.
use super::audio_math_work::{multiply_add_work, next_music_byte_work_cycles};
use super::audio_output_work::{
    cry_check_work, duty_and_length_work_cycles, enable_channel_work_cycles,
    register_pointer_work_cycles, wave_frequency_work,
};

pub const SOURCE_PRESS_AB_ID: u8 = 144;

/// Canonical percussion SFX work. CHAN8 uses fixed tempo and one frequency
/// byte; CHAN5's current ID still selects the common non-cry tail.
pub fn noise_next_note(
    instrument: u8,
    data: &[u8],
    ptr: usize,
    fraction: u8,
    source_sfx_id: u8,
) -> Option<PressAbWork> {
    if !matches!(source_sfx_id, 0 | SOURCE_PRESS_AB_ID)
        || data != super::press_ab_instructions::PressAbInstructions::noise_data(instrument)?
        || ptr % 3 != 0
        || ptr >= data.len()
    {
        return None;
    }
    if data[ptr] == 0xff {
        return Some(PressAbWork {
            cycles: super::audio_work::noise_channel_end_work_with_sfx(source_sfx_id)?,
            next_ptr: ptr + 1,
            delay: 0,
            fraction,
            duty: 0,
            envelope: 0,
            frequency: 0,
            ended: true,
        });
    }
    let get = next_music_byte_work_cycles();
    let (length, first) = multiply_add_work(1, u16::from((data[ptr] & 15) + 1), 0);
    let (delay, second) = multiply_add_work(length as u8, 0x100, fraction);
    let entry = 12 + 8 + 8 + 12 + 8 + 8 + 12 + 8 + 16 + 16 + 24;
    let dispatch = 24
        + get
        + 4
        + 8
        + 16
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
        + 12
        + 8
        + 12
        + 7 * (8 + 12)
        + 8
        + 8
        + 12
        + 8
        + 8
        + 4
        + 8
        + 8
        + 8
        + 12
        + 8
        + 12
        + 8
        + 24;
    let length_prefix = 4 + 16 + 8 + 4 + 8 + 4 + 4 + 12 + 8 + 8 + 4 + 24;
    let noise_tempo = 4 + 8 + 12 + 8 + 8 + 8 + 12;
    let second_prefix = 4 + 8 + 12 + 8 + 8 + 24;
    let store = 4 + 4 + 12 + 8 + 8 + 4 + 12 + 8 + 8 + 12 + 8 + 12;
    let return_sfx = 8 + 12 + 8 + 12 + 8 + 12 + 16;
    let pointer = register_pointer_work_cycles();
    let frequency = u16::from(data[ptr + 2]);
    let output = 4
        + 8
        + 12
        + 8
        + 8
        + 4
        + 4
        + 8
        + 24
        + pointer
        + 8
        + 24
        + get
        + 4
        + 8
        + 24
        + pointer
        + 8
        + 24
        + get
        + 4
        + 4
        + 8
        + 8
        + 12
        + 4
        + 16
        + 24
        + duty_and_length_work_cycles(7)?
        + 24
        + enable_channel_work_cycles(7, false)?
        + 12
        + 24
        + wave_frequency_work(7, source_sfx_id, 0, frequency)?.1
        + 16
        + 16;
    Some(PressAbWork {
        cycles: entry
            + dispatch
            + length_prefix
            + first
            + noise_tempo
            + second_prefix
            + second
            + store
            + return_sfx
            + output,
        next_ptr: ptr + 3,
        delay: (delay >> 8) as u8,
        fraction: delay as u8,
        duty: 0,
        envelope: data[ptr + 1],
        frequency,
        ended: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressAbWork {
    pub cycles: u32,
    pub next_ptr: usize,
    pub delay: u8,
    pub fraction: u8,
    pub duty: u8,
    pub envelope: u8,
    pub frequency: u16,
    pub ended: bool,
}

fn note_length(note: u8, fraction: u8) -> (u8, u8, u32) {
    let (length, first) = multiply_add_work(1, u16::from((note & 15) + 1), 0);
    let (delay, second) = multiply_add_work(length as u8, 0x100, fraction);
    let prefix = 4 + 16 + 8 + 4 + 8 + 4 + 4 + 12 + 8 + 8 + 4 + 24;
    // Source SetSfxTempo checks the raw header-table ID, not the native enum.
    let set_tempo = 24 + cry_check_work(SOURCE_PRESS_AB_ID).1 + 12 + 4 + 16 + 8 + 16 + 16;
    let select_tempo = 4 + 8 + 12 + 8 + 8 + 8 + 8 + 24 + set_tempo + 16 + 4 + 16 + 4;
    let multiply = 4 + 8 + 12 + 8 + 8 + 24;
    let store = 4 + 4 + 12 + 8 + 8 + 4 + 12 + 8 + 8 + 12 + 8 + 12;
    let return_sfx = 8 + 12 + 8 + 12 + 8 + 12 + 16;
    (
        delay as u8,
        (delay >> 8) as u8,
        prefix + first + select_tempo + multiply + second + store + return_sfx,
    )
}

/// Only the known, finite PressAB stream is accepted. Other SFX, control
/// commands, channels and mutable tempo/flag variants remain unsupported.
pub fn press_ab_next_note(
    data: &[u8],
    mut ptr: usize,
    fraction: u8,
    mut duty: u8,
) -> Option<PressAbWork> {
    if data != crate::sfx_data::SFX_PRESS_AB_CH5 || ![0, 6, 10, 14, 18].contains(&ptr) {
        return None;
    }
    let get = next_music_byte_work_cycles();
    let entry = 12 + 8 + 8 + 12 + 8 + 8 + 12 + 8 + 16 + 16 + 24;
    let mut cycles = entry;
    if data[ptr] == 0xff {
        let read_return = 24 + get + 4 + 8 + 12 + 8 + 12 + 8 + 12 + 8 + 4 + 8 + 12;
        let flags = 16 + 12 + 8 + 16 + 8 + 12 + 12 + 12;
        let no_cry_and_stop = 16 + 8 + 12 + 16 + 8 + 8 + 8 + 12 + 12 + 8 + 8 + 16 + 16;
        return Some(PressAbWork {
            cycles: cycles + read_return + flags + no_cry_and_stop,
            next_ptr: ptr + 1,
            delay: 0,
            fraction,
            duty,
            envelope: 0,
            frequency: 0,
            ended: true,
        });
    }
    if data[ptr] == 0xec {
        // sound_ret -> call/loop/note-type/perfect-pitch/vibrato/slide/duty.
        cycles += 24
            + get
            + 4
            + 8
            + 16
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
            + 12
            + 8
            + 12
            + 8
            + 8
            + 24
            + get
            + 4
            + 4
            + 8
            + 8
            + 12
            + 8
            + 8
            + 16;
        duty = (data[ptr + 1] & 3) << 6;
        ptr += 2;
    }
    let note = data[ptr];
    cycles += 24 + get + 4 + 8 + 16 + 8 + 16 + 8 + 16 + 8 + 8 + 16;
    cycles += 4 + 8 + 12 + 8 + 12 + 8 + 12 + 7 * (8 + 12);
    cycles += 8 + 8 + 12 + 8 + 8 + 4 + 8 + 8 + 8 + 12 + 8 + 12 + 8 + 24;
    let (fraction, delay, length) = note_length(note, fraction);
    cycles += length;
    // Duty/length and envelope writes, then the two frequency bytes.
    let pointer = register_pointer_work_cycles();
    cycles += 4 + 8 + 12 + 8 + 8 + 4 + 4 + 8 + 24 + pointer + 8;
    cycles += 24 + get + 4 + 8 + 24 + pointer + 8;
    cycles += 24 + get + 4 + 4 + 8 + 8 + 8 + 16 + 24 + get + 12;
    let frequency = u16::from_le_bytes([data[ptr + 2], data[ptr + 3]]);
    cycles += 4
        + 16
        + 24
        + duty_and_length_work_cycles(4)?
        + 24
        + enable_channel_work_cycles(4, false)?
        + 12
        + 24
        + wave_frequency_work(4, SOURCE_PRESS_AB_ID, 0, frequency)?.1
        + 16
        + 16;
    Some(PressAbWork {
        cycles,
        next_ptr: ptr + 4,
        delay,
        fraction,
        duty,
        envelope: data[ptr + 1],
        frequency,
        ended: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_press_ab_paths_match_original_instructions_and_registers() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("press_ab_work_golden.json")).unwrap();
        assert_eq!(fixture["source_press_ab_ID"], SOURCE_PRESS_AB_ID);
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 20);
        for c in cases {
            let input = &c["input"];
            let work = press_ab_next_note(
                crate::sfx_data::SFX_PRESS_AB_CH5,
                input["ptr"].as_u64().unwrap() as usize,
                input["fraction"].as_u64().unwrap() as u8,
                input["duty"].as_u64().unwrap() as u8,
            )
            .unwrap();
            assert_eq!(
                work.cycles,
                c["T"].as_u64().unwrap() as u32,
                "ptr{}",
                input["ptr"]
            );
            assert_eq!(work.next_ptr, c["next_ptr"].as_u64().unwrap() as usize);
            assert_eq!(work.delay, c["delay"].as_u64().unwrap() as u8);
            assert_eq!(work.fraction, c["fraction"].as_u64().unwrap() as u8);
            assert_eq!(work.duty, c["duty"].as_u64().unwrap() as u8);
            if !work.ended {
                let hw = wave_frequency_work(4, SOURCE_PRESS_AB_ID, 0, work.frequency)
                    .unwrap()
                    .0;
                assert_eq!(
                    u64::from(hw as u8),
                    c["registers"]["0xff13"].as_u64().unwrap()
                );
                assert_eq!(
                    u64::from((hw >> 8) as u8),
                    c["registers"]["0xff14"].as_u64().unwrap()
                );
                assert_eq!(
                    u64::from(work.envelope),
                    c["registers"]["0xff12"].as_u64().unwrap()
                );
            }
        }
    }
}
