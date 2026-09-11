//! Tests for music sequencer, command decoding, frequency calculation,
//! effects, and channel state management.

use crate::apu::Apu;
use crate::commands::{self, Command};
use crate::effects;
use crate::sequencer::*;

// ═══════════════════════════════════════════════════════════════════════════
// Command Decoding Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_decode_note() {
    // Byte $37 = pitch 3 (D#), length 8 (nibble 7 + 1)
    let data = [0x37];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::Note {
            pitch: 3,
            length: 8
        }
    );
    assert_eq!(pos, 1);
}

#[test]
fn test_decode_note_c_shortest() {
    // $00 = pitch 0 (C), length 1
    let data = [0x00];
    let (cmd, _) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::Note {
            pitch: 0,
            length: 1
        }
    );
}

#[test]
fn test_decode_note_b_longest() {
    // $AF = pitch 10 (A#), length 16
    let data = [0xAF];
    let (cmd, _) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::Note {
            pitch: 10,
            length: 16
        }
    );
}

#[test]
fn test_decode_drum_note() {
    // $B3 on NOISE channel = drum note, length 4, next byte = instrument ID 5
    let data = [0xB3, 0x05];
    let (cmd, pos) = commands::decode_command(&data, 0, true, false, false);
    assert_eq!(
        cmd,
        Command::DrumNote {
            length: 4,
            instrument: 5
        }
    );
    assert_eq!(pos, 2);
}

#[test]
fn test_decode_b_note_on_non_noise_channel() {
    // $B3 on non-noise channel = note B natural (pitch 11), length 4
    let data = [0xB3, 0x05]; // second byte should NOT be consumed
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::Note {
            pitch: 11,
            length: 4
        }
    );
    assert_eq!(pos, 1); // only 1 byte consumed, NOT 2
}

#[test]
fn test_decode_b_note_range_on_non_noise() {
    // $B0 = B natural, length 1
    let (cmd, pos) = commands::decode_command(&[0xB0], 0, false, false, false);
    assert_eq!(
        cmd,
        Command::Note {
            pitch: 11,
            length: 1
        }
    );
    assert_eq!(pos, 1);

    // $BF = B natural, length 16
    let (cmd, pos) = commands::decode_command(&[0xBF], 0, false, false, false);
    assert_eq!(
        cmd,
        Command::Note {
            pitch: 11,
            length: 16
        }
    );
    assert_eq!(pos, 1);
}

#[test]
fn test_decode_rest() {
    // $C7 = rest, length 8
    let data = [0xC7];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd, Command::Rest { length: 8 });
    assert_eq!(pos, 1);
}

#[test]
fn test_decode_note_type() {
    // $D2 = note_type, speed = 2 (raw low nibble), next byte = volume/envelope
    let data = [0xD2, 0xA2];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::NoteType {
            speed: 2,
            param: 0xA2
        }
    );
    assert_eq!(pos, 2);
}

#[test]
fn test_decode_note_type_noise_channel() {
    // On noise channel, note_type has NO param byte
    let data = [0xD5, 0xA2]; // 0xA2 should NOT be consumed
    let (cmd, pos) = commands::decode_command(&data, 0, true, false, false);
    assert_eq!(cmd, Command::NoteType { speed: 5, param: 0 });
    assert_eq!(pos, 1); // only 1 byte consumed
}

#[test]
fn test_decode_octave() {
    // $E0 = octave 0 (raw), $E3 = octave 3 (raw), $E7 = octave 7 (raw)
    let (cmd, _) = commands::decode_command(&[0xE0], 0, false, false, false);
    assert_eq!(cmd, Command::Octave(0));

    let (cmd, _) = commands::decode_command(&[0xE3], 0, false, false, false);
    assert_eq!(cmd, Command::Octave(3));

    let (cmd, _) = commands::decode_command(&[0xE7], 0, false, false, false);
    assert_eq!(cmd, Command::Octave(7));
}

#[test]
fn test_decode_toggle_perfect_pitch() {
    let (cmd, pos) = commands::decode_command(&[0xE8], 0, false, false, false);
    assert_eq!(cmd, Command::TogglePerfectPitch);
    assert_eq!(pos, 1);
}

#[test]
fn test_decode_vibrato() {
    let data = [0xEA, 0x08, 0x34];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::Vibrato {
            delay: 8,
            depth_rate: 0x34
        }
    );
    assert_eq!(pos, 3);
}

#[test]
fn test_decode_pitch_slide() {
    let data = [0xEB, 0x02, 0x45];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::PitchSlide {
            length_modifier: 2,
            octave_pitch: 0x45,
        }
    );
    assert_eq!(pos, 3);
}

#[test]
fn test_decode_duty_cycle() {
    let data = [0xEC, 0x02];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd, Command::DutyCycle(2));
    assert_eq!(pos, 2);
}

#[test]
fn test_decode_tempo() {
    // Tempo is big-endian: $01 $00 = 256
    let data = [0xED, 0x01, 0x00];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd, Command::Tempo(0x0100));
    assert_eq!(pos, 3);
}

#[test]
fn test_decode_stereo_panning() {
    let data = [0xEE, 0x11];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd, Command::StereoPanning(0x11));
    assert_eq!(pos, 2);
}

#[test]
fn test_decode_volume() {
    let data = [0xF0, 0x77];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd, Command::Volume(0x77));
    assert_eq!(pos, 2);
}

#[test]
fn test_decode_execute_music() {
    let (cmd, _) = commands::decode_command(&[0xF8], 0, false, false, false);
    assert_eq!(cmd, Command::ExecuteMusic);
}

#[test]
fn test_decode_duty_cycle_pattern() {
    let data = [0xFC, 0xE4]; // pattern: 11 10 01 00
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd, Command::DutyCyclePattern(0xE4));
    assert_eq!(pos, 2);
}

#[test]
fn test_decode_sound_call() {
    // sound_call with 16-bit LE offset: $34 $12 = address 0x1234
    let data = [0xFD, 0x34, 0x12];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd, Command::SoundCall { offset: 0x1234 });
    assert_eq!(pos, 3);
}

#[test]
fn test_decode_sound_loop() {
    // sound_loop: count=3, LE offset $10 $00 = 0x0010
    let data = [0xFE, 0x03, 0x10, 0x00];
    let (cmd, pos) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::SoundLoop {
            count: 3,
            offset: 0x0010
        }
    );
    assert_eq!(pos, 4);
}

#[test]
fn test_decode_sound_loop_infinite() {
    let data = [0xFE, 0x00, 0x00, 0x00];
    let (cmd, _) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(
        cmd,
        Command::SoundLoop {
            count: 0,
            offset: 0x0000
        }
    );
}

#[test]
fn test_decode_sound_ret() {
    let (cmd, pos) = commands::decode_command(&[0xFF], 0, false, false, false);
    assert_eq!(cmd, Command::SoundRet);
    assert_eq!(pos, 1);
}

#[test]
fn test_decode_end_of_data() {
    let (cmd, _) = commands::decode_command(&[], 0, false, false, false);
    assert_eq!(cmd, Command::EndOfData);
}

#[test]
fn test_decode_sequence_of_commands() {
    // octave 4 ($E4), note_type speed=11 vol=$A0 ($DB $A0), C note len 4 ($03)
    let data = [0xE4, 0xDB, 0xA0, 0x03];
    let (cmd0, p0) = commands::decode_command(&data, 0, false, false, false);
    assert_eq!(cmd0, Command::Octave(4));

    let (cmd1, p1) = commands::decode_command(&data, p0, false, false, false);
    assert_eq!(
        cmd1,
        Command::NoteType {
            speed: 11,
            param: 0xA0
        }
    );

    let (cmd2, p2) = commands::decode_command(&data, p1, false, false, false);
    assert_eq!(
        cmd2,
        Command::Note {
            pitch: 0,
            length: 4
        }
    );
    assert_eq!(p2, 4);
}

// ═══════════════════════════════════════════════════════════════════════════
// Frequency Calculation Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_calculate_frequency_c_octave8() {
    // Octave 8 = no shift. C base = 0xF82C
    // 0xF82C + 0x0800 = 0x002C (only low 11 bits)
    let freq = commands::calculate_frequency(0, 8);
    // After adding 0x0800: 0xF82C as i16 = -2004, no shift, + 0x0800 = ...
    // Let's just verify it's a reasonable 11-bit value
    assert!(freq <= 0x07FF);
    assert!(freq > 0);
}

#[test]
fn test_calculate_frequency_increases_with_octave() {
    // In GB audio engine, LOWER octave number = HIGHER pitch (more shifts on negative base).
    // Octave 1 is the highest pitch, octave 8 is the lowest.
    // Higher freq_reg value = higher Hz on Game Boy.
    let f_oct1 = commands::calculate_frequency(0, 1);
    let f_oct4 = commands::calculate_frequency(0, 4);
    let f_oct8 = commands::calculate_frequency(0, 8);
    assert!(
        f_oct1 > f_oct4,
        "octave 1 freq ({}) should be > octave 4 ({})",
        f_oct1,
        f_oct4
    );
    assert!(
        f_oct4 > f_oct8,
        "octave 4 freq ({}) should be > octave 8 ({})",
        f_oct4,
        f_oct8
    );
}

#[test]
fn test_calculate_frequency_increases_with_note() {
    // Within same octave, higher notes have higher frequency registers
    let f_c = commands::calculate_frequency(0, 4); // C
    let f_e = commands::calculate_frequency(4, 4); // E
    let f_b = commands::calculate_frequency(11, 4); // B
    assert!(f_e > f_c, "E ({}) should be > C ({})", f_e, f_c);
    assert!(f_b > f_e, "B ({}) should be > E ({})", f_b, f_e);
}

#[test]
fn test_calculate_frequency_out_of_range() {
    // Note >= 12 should return 0
    assert_eq!(commands::calculate_frequency(12, 4), 0);
    assert_eq!(commands::calculate_frequency(255, 4), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Note Delay Calculation Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_calculate_delay_basic() {
    // length=1, speed=1, tempo=0x0100 (1.0), frac=0
    // raw = 1*1 = 1, int = 1*1 = 1, frac = 1*0 = 0
    let (delay, frac) = commands::calculate_delay(1, 1, 0x0100, 0);
    assert_eq!(delay, 1);
    assert_eq!(frac, 0);
}

#[test]
fn test_calculate_delay_longer_note() {
    // length=4, speed=6, tempo=0x0100
    // raw = 4*6 = 24, int = 24*1 = 24
    let (delay, _) = commands::calculate_delay(4, 6, 0x0100, 0);
    assert_eq!(delay, 24);
}

#[test]
fn test_calculate_delay_with_tempo_fraction() {
    // With tempo that has a fractional part, delays accumulate fractional bits
    let (delay1, frac1) = commands::calculate_delay(1, 1, 0x0180, 0);
    // raw=1, tempo_hi=1, tempo_lo=0x80
    // frac = 1*0x80 + 0 = 0x80, new_frac = 0x80, carry = 0
    // int = 1*1 + 0 = 1
    assert_eq!(delay1, 1);
    assert_eq!(frac1, 0x80);

    // Second note with accumulated fraction
    let (delay2, frac2) = commands::calculate_delay(1, 1, 0x0180, frac1);
    // frac = 1*0x80 + 0x80 = 0x100, new_frac = 0x00, carry = 1
    // int = 1*1 + 1 = 2
    assert_eq!(delay2, 2);
    assert_eq!(frac2, 0x00);
}

#[test]
fn test_calculate_delay_minimum_one() {
    // Even with very small values, delay is at least 1
    let (delay, _) = commands::calculate_delay(1, 1, 0x0001, 0);
    assert!(delay >= 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// Channel State Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_channel_state_new() {
    let ch = ChannelState::new();
    assert!(!ch.active);
    assert_eq!(ch.ptr, 0);
    assert_eq!(ch.octave, 4);
    assert_eq!(ch.note_speed, 1);
    assert_eq!(ch.delay_counter, 0);
    assert_eq!(ch.frequency, 0);
    assert!(ch.flags1.is_empty());
    assert!(ch.flags2.is_empty());
}

#[test]
fn test_channel_state_read_byte() {
    let mut ch = ChannelState::new();
    ch.data = vec![0x10, 0x20, 0x30];
    assert_eq!(ch.read_byte(), Some(0x10));
    assert_eq!(ch.read_byte(), Some(0x20));
    assert_eq!(ch.read_byte(), Some(0x30));
    assert_eq!(ch.read_byte(), None);
}

#[test]
fn test_channel_state_peek_byte() {
    let mut ch = ChannelState::new();
    ch.data = vec![0xAB];
    assert_eq!(ch.peek_byte(), Some(0xAB));
    assert_eq!(ch.peek_byte(), Some(0xAB)); // peek doesn't advance
    assert_eq!(ch.ptr, 0);
}

#[test]
fn test_channel_state_read_u16_le() {
    let mut ch = ChannelState::new();
    ch.data = vec![0x34, 0x12];
    assert_eq!(ch.read_u16_le(), Some(0x1234));
}

#[test]
fn test_channel_state_reset() {
    let mut ch = ChannelState::new();
    ch.data = vec![1, 2, 3];
    ch.active = true;
    ch.octave = 7;
    ch.frequency = 0x400;
    ch.reset();
    assert!(!ch.active);
    assert_eq!(ch.octave, 4);
    assert_eq!(ch.frequency, 0);
    assert!(ch.data.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// Channel Flags Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_channel_flags1() {
    let mut flags = ChannelFlags1::empty();
    assert!(!flags.contains(ChannelFlags1::PERFECT_PITCH));

    flags.insert(ChannelFlags1::PERFECT_PITCH);
    assert!(flags.contains(ChannelFlags1::PERFECT_PITCH));

    flags.toggle(ChannelFlags1::VIBRATO_DOWN);
    assert!(flags.contains(ChannelFlags1::VIBRATO_DOWN));
    flags.toggle(ChannelFlags1::VIBRATO_DOWN);
    assert!(!flags.contains(ChannelFlags1::VIBRATO_DOWN));
}

#[test]
fn test_channel_flags2_execute_music() {
    let mut flags = ChannelFlags2::empty();
    flags.toggle(ChannelFlags2::EXECUTE_MUSIC);
    assert!(flags.contains(ChannelFlags2::EXECUTE_MUSIC));
}

// ═══════════════════════════════════════════════════════════════════════════
// Sequencer Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sequencer_new() {
    let seq = Sequencer::new();
    assert!(!seq.music_playing);
    assert!(!seq.sfx_playing);
    assert!(!seq.is_playing());
    assert_eq!(seq.music_tempo, 0x0100);
}

#[test]
fn test_sequencer_play_music_activates_channels() {
    let mut seq = Sequencer::new();
    let ch1_data = vec![0xE4, 0xD0, 0xF0, 0x03, 0xFF]; // octave 4, note_type, note C, ret
    let ch2_data = vec![0xE4, 0x03, 0xFF]; // octave 4, note C, ret

    seq.play_music(1, &[ch1_data, ch2_data], 0x0100);

    assert!(seq.music_playing);
    assert!(seq.channels[CHAN1].active);
    assert!(seq.channels[CHAN2].active);
    assert!(!seq.channels[CHAN3].active);
    assert!(!seq.channels[CHAN4].active);
    assert_eq!(seq.current_music_id, 1);
}

#[test]
fn test_sequencer_stop_music() {
    let mut seq = Sequencer::new();
    seq.play_music(1, &[vec![0x03, 0xFF]], 0x0100);
    assert!(seq.music_playing);

    seq.stop_music();
    assert!(!seq.music_playing);
    assert!(!seq.channels[CHAN1].active);
}

#[test]
fn test_sequencer_play_sfx() {
    let mut seq = Sequencer::new();
    let sfx_data = vec![0xD0, 0xF0, 0x03, 0xFF]; // note_type, note C, ret
    seq.play_sfx(10, &[sfx_data], 0, 0x0100);

    assert!(seq.sfx_playing);
    assert!(seq.channels[CHAN5].active); // SFX channel 5 = HW channel 0
    assert_eq!(seq.current_sfx_id, 10);
}

#[test]
fn test_sequencer_sfx_overrides_music() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Start music on channel 1
    let music_data = vec![
        0xE4, // octave 4
        0xD0, 0xF0, // note_type speed=1, vol=0xF0
        0x03, // note C, length=4
        0xFE, 0x00, 0x03, 0x00, // sound_loop infinite to offset 3
    ];
    seq.play_music(1, &[music_data], 0x0100);

    // Start SFX on channel 5 (same HW channel 0)
    let sfx_data = vec![
        0xE2, // octave 6
        0xD0, 0xF0, // note_type
        0x47, // note E, length=8
        0xFF, // sound_ret (ends SFX)
    ];
    seq.play_sfx(20, &[sfx_data], 0, 0x0100);

    // Both should be active
    assert!(seq.channels[CHAN1].active);
    assert!(seq.channels[CHAN5].active);

    // Update frame — SFX takes priority over music for HW output
    seq.update_frame(&mut apu);

    // SFX channel 5 should have been the one driving HW channel 0
    assert!(seq.channels[CHAN5].active);
}

#[test]
fn test_sequencer_stop_all() {
    let mut seq = Sequencer::new();
    seq.play_music(1, &[vec![0x03, 0xFF]], 0x0100);
    seq.play_sfx(10, &[vec![0x03, 0xFF]], 0, 0x0100);

    seq.stop_all();
    assert!(!seq.music_playing);
    assert!(!seq.sfx_playing);
    assert!(!seq.is_playing());
}

#[test]
fn test_sequencer_update_processes_note() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Simple: octave 4, note_type speed=1 vol=0xF0, note C length=1, ret
    let data = vec![
        0xE4, // octave 4
        0xD0, 0xF0, // note_type speed=1, vol=0xF0
        0x00, // note C, length=1
        0xFF, // sound_ret
    ];
    seq.play_music(1, &[data], 0x0100);

    // First update: should process commands up to note
    seq.update_frame(&mut apu);
    assert!(
        seq.channels[CHAN1].frequency > 0,
        "frequency should be set after note"
    );
    assert_eq!(seq.channels[CHAN1].octave, 4);
    assert_eq!(seq.channels[CHAN1].note_speed, 0);
}

#[test]
fn test_sequencer_rest_sets_freq_zero() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    let data = vec![
        0xC0, // rest, length=1
        0xFF, // sound_ret
    ];
    seq.play_music(1, &[data], 0x0100);
    seq.update_frame(&mut apu);

    assert_eq!(seq.channels[CHAN1].frequency, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Effects Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_vibrato_inactive_when_extent_zero() {
    let mut ch = ChannelState::new();
    ch.vibrato.extent = 0;
    assert_eq!(effects::apply_vibrato(&mut ch), None);
}

#[test]
fn test_vibrato_delay_countdown() {
    let mut ch = ChannelState::new();
    ch.vibrato.extent = 0x22; // non-zero
    ch.vibrato.delay_counter = 3;
    ch.vibrato.rate = 0x11; // reload=1, counter=1
    ch.frequency = 0x100;
    ch.freq_lo_saved = 0x00;

    // Should count down delay and return None
    assert_eq!(effects::apply_vibrato(&mut ch), None);
    assert_eq!(ch.vibrato.delay_counter, 2);

    assert_eq!(effects::apply_vibrato(&mut ch), None);
    assert_eq!(ch.vibrato.delay_counter, 1);

    assert_eq!(effects::apply_vibrato(&mut ch), None);
    assert_eq!(ch.vibrato.delay_counter, 0);
}

#[test]
fn test_vibrato_applies_after_delay() {
    let mut ch = ChannelState::new();
    ch.vibrato.extent = 0x22; // up=2, down=2
    ch.vibrato.delay_counter = 0;
    ch.vibrato.rate = 0x00; // reload=0, counter=0 → fires immediately
    ch.frequency = 0x100;
    ch.freq_lo_saved = 0x00;
    ch.flags1.remove(ChannelFlags1::VIBRATO_DOWN);

    // Should apply vibrato (upward first)
    let result = effects::apply_vibrato(&mut ch);
    assert!(result.is_some());
}

#[test]
fn test_duty_cycle_rotation() {
    let mut ch = ChannelState::new();
    // Pattern: 11 10 01 00 = 0xE4
    ch.duty_cycle_pattern = 0xE4;

    // First rotation: 10 01 00 11 = 0x93, duty = bits 7-6 = 10 = 2
    let duty1 = effects::rotate_duty_cycle(&mut ch);
    assert_eq!(duty1, 2);
    assert_eq!(ch.duty_cycle_pattern, 0x93);

    // Second rotation: 01 00 11 10 = 0x4E, duty = 01 = 1
    let duty2 = effects::rotate_duty_cycle(&mut ch);
    assert_eq!(duty2, 1);

    // Third rotation: 00 11 10 01 = 0x39, duty = 00 = 0
    let duty3 = effects::rotate_duty_cycle(&mut ch);
    assert_eq!(duty3, 0);

    // Fourth rotation: 11 10 01 00 = 0xE4, back to start, duty = 11 = 3
    let duty4 = effects::rotate_duty_cycle(&mut ch);
    assert_eq!(duty4, 3);
    assert_eq!(ch.duty_cycle_pattern, 0xE4);
}

#[test]
fn test_pitch_slide_deactivates_when_not_set() {
    let mut ch = ChannelState::new();
    ch.flags1.remove(ChannelFlags1::PITCH_SLIDE_ON);
    assert_eq!(effects::apply_pitch_slide(&mut ch), None);
}

#[test]
fn test_pitch_slide_increasing() {
    let mut ch = ChannelState::new();
    ch.flags1.insert(ChannelFlags1::PITCH_SLIDE_ON);
    ch.flags1.remove(ChannelFlags1::PITCH_SLIDE_DEC);
    ch.frequency = 0x100;
    ch.pitch_slide.target_freq = 0x110;
    ch.pitch_slide.freq_step = 0x08;

    let result = effects::apply_pitch_slide(&mut ch);
    assert!(result.is_some());
    assert_eq!(ch.frequency, 0x108);
}

#[test]
fn test_pitch_slide_reaches_target() {
    let mut ch = ChannelState::new();
    ch.flags1.insert(ChannelFlags1::PITCH_SLIDE_ON);
    ch.flags1.remove(ChannelFlags1::PITCH_SLIDE_DEC);
    ch.frequency = 0x0FE;
    ch.pitch_slide.target_freq = 0x100;
    ch.pitch_slide.freq_step = 0x10; // step > remaining distance

    let result = effects::apply_pitch_slide(&mut ch);
    assert_eq!(result, Some(0x100)); // snaps to target
    assert!(!ch.flags1.contains(ChannelFlags1::PITCH_SLIDE_ON)); // deactivated
}

// ═══════════════════════════════════════════════════════════════════════════
// Sound Call / Return Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_sound_call_and_return() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Data layout:
    //  0: sound_call to offset 5
    //  3: note C length=1 (after return)
    //  4: sound_ret (end)
    //  5: octave 4
    //  6: sound_ret (return from call)
    let data = vec![
        0xFD, 0x05, 0x00, // sound_call offset=5
        0x00, // note C len=1 (offset 3)
        0xFF, // sound_ret (offset 4)
        0xE4, // octave 4 (offset 5)
        0xFF, // sound_ret (offset 6) — returns to offset 3
    ];
    seq.play_music(1, &[data], 0x0100);

    // First update: process sound_call → jump to 5, read octave 4, sound_ret → back to 3, note C
    seq.update_frame(&mut apu);
    assert_eq!(seq.channels[CHAN1].octave, 4);
    assert!(seq.channels[CHAN1].frequency > 0); // note was played
}

#[test]
fn test_sound_loop_counted() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Data:
    //  0: note C len=1
    //  1: sound_loop count=2, offset=0
    //  5: sound_ret
    let data = vec![
        0x00, // note C len=1 (offset 0)
        0xFE, 0x02, 0x00, 0x00, // sound_loop count=2, offset=0
        0xFF, // sound_ret (offset 5)
    ];
    seq.play_music(1, &[data], 0x0100);

    // First frame: note C played, delay = 1
    seq.update_frame(&mut apu);
    assert!(seq.channels[CHAN1].active);

    // Second frame: delay expires, loop back (loop_counter = 2→1)
    seq.update_frame(&mut apu);
    assert!(seq.channels[CHAN1].active);

    // Third frame: delay expires, loop back again (loop_counter = 1→0), but 0 means done
    seq.update_frame(&mut apu);
    // Now should fall through to sound_ret
}

#[test]
fn test_sound_loop_infinite() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Infinite loop: note C len=1, sound_loop count=0 offset=0
    let data = vec![
        0x00, // note C len=1 (offset 0)
        0xFE, 0x00, 0x00, 0x00, // sound_loop count=0 (infinite), offset=0
    ];
    seq.play_music(1, &[data], 0x0100);

    // Play 10 frames — should keep looping
    for _ in 0..10 {
        seq.update_frame(&mut apu);
        assert!(
            seq.channels[CHAN1].active,
            "channel should stay active in infinite loop"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Channel Mapping Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_hw_channel_mapping() {
    assert_eq!(hw_channel_for(CHAN1), 0);
    assert_eq!(hw_channel_for(CHAN2), 1);
    assert_eq!(hw_channel_for(CHAN3), 2);
    assert_eq!(hw_channel_for(CHAN4), 3);
    assert_eq!(hw_channel_for(CHAN5), 0); // SFX mirrors
    assert_eq!(hw_channel_for(CHAN6), 1);
    assert_eq!(hw_channel_for(CHAN7), 2);
    assert_eq!(hw_channel_for(CHAN8), 3);
}

#[test]
fn test_is_sfx_channel() {
    assert!(!is_sfx_channel(CHAN1));
    assert!(!is_sfx_channel(CHAN2));
    assert!(!is_sfx_channel(CHAN3));
    assert!(!is_sfx_channel(CHAN4));
    assert!(is_sfx_channel(CHAN5));
    assert!(is_sfx_channel(CHAN6));
    assert!(is_sfx_channel(CHAN7));
    assert!(is_sfx_channel(CHAN8));
}

// ═══════════════════════════════════════════════════════════════════════════
// Vibrato State Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_vibrato_state_extent_split() {
    let mut vib = VibratoState::default();
    vib.extent = 0x37; // up=3, down=7
    assert_eq!(vib.extent_up(), 3);
    assert_eq!(vib.extent_down(), 7);
}

#[test]
fn test_vibrato_state_rate_fields() {
    let mut vib = VibratoState::default();
    vib.rate = 0x53; // reload=5, counter=3
    assert_eq!(vib.rate_reload(), 5);
    assert_eq!(vib.rate_counter(), 3);

    vib.set_rate_counter(0x0A);
    assert_eq!(vib.rate_counter(), 0x0A);
    assert_eq!(vib.rate_reload(), 5); // reload unchanged
}

// ═══════════════════════════════════════════════════════════════════════════
// Integration: Multi-channel Music
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_multichannel_music_playback() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Channel 1: pulse - octave 4, note C, D, E then loop
    let ch1 = vec![
        0xE4, // octave 4
        0xD0, 0xF0, // note_type speed=1, vol=0xF0
        0x00, // note C len=1
        0x20, // note D len=1
        0x40, // note E len=1
        0xFE, 0x00, 0x03, 0x00, // infinite loop to offset 3
    ];

    // Channel 2: pulse - octave 3, note G, A then loop
    let ch2 = vec![
        0xE5, // octave 3
        0xD0, 0xA0, // note_type speed=1, vol=0xA0
        0x70, // note G len=1
        0x90, // note A len=1
        0xFE, 0x00, 0x03, 0x00, // infinite loop to offset 3
    ];

    seq.play_music(1, &[ch1, ch2], 0x0100);

    // Run several frames
    for _ in 0..6 {
        seq.update_frame(&mut apu);
    }

    // Both channels should still be active (infinite loops)
    assert!(seq.channels[CHAN1].active);
    assert!(seq.channels[CHAN2].active);
}

#[test]
fn test_sfx_priority_then_music_resumes() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Music: simple infinite loop
    let music = vec![0xE4, 0xD0, 0xF0, 0x00, 0xFE, 0x00, 0x03, 0x00];
    seq.play_music(1, &[music], 0x0100);

    // SFX: play one note then end
    let sfx = vec![
        0xE2, 0xD0, 0xF0, 0x00, // octave 6, note_type, note C len=1
        0xFF, // sound_ret — SFX ends
    ];
    seq.play_sfx(10, &[sfx], 0, 0x0100);

    // Frame 1: SFX active
    seq.update_frame(&mut apu);
    assert!(seq.channels[CHAN5].active);

    // Frame 2: SFX note delay expires, processes sound_ret → SFX ends
    seq.update_frame(&mut apu);
    assert!(!seq.channels[CHAN5].active);

    // Music should still be going
    assert!(seq.channels[CHAN1].active);
    assert!(seq.music_playing);
}

// ═══════════════════════════════════════════════════════════════════════════
// volume ($F0) command → NR50 (classic engine's volume routine)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_volume_command_writes_nr50() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;
    assert_eq!(apu.nr50, 0x77); // power-on default

    // Channel 1: volume $64, then a note so the channel stays active.
    let ch1 = vec![0xF0, 0x64, 0x37, 0xFF];
    seq.play_music(1, &[ch1], 0x0100);
    seq.update_frame(&mut apu);

    // The volume command executes mid-stream and hits NR50 on the same frame.
    assert_eq!(apu.nr50, 0x64);
    assert_eq!(seq.pending_nr50, None); // consumed
}

#[test]
fn test_volume_command_mid_song() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // note (len 1), then volume $31, then another note.
    let ch1 = vec![0x00, 0xF0, 0x31, 0x10, 0xFF];
    seq.play_music(1, &[ch1], 0x0100);

    seq.update_frame(&mut apu); // frame 1: note plays, no volume command yet
    assert_eq!(apu.nr50, 0x77);

    seq.update_frame(&mut apu); // frame 2: note delay expires → volume executes
    assert_eq!(apu.nr50, 0x31);
}

// ═══════════════════════════════════════════════════════════════════════════
// drum_note → noise-instrument SFX on CHAN8
// (classic engine's drum-note handler)
// ═══════════════════════════════════════════════════════════════════════════

/// Noise-instrument stream for instrument 1 (one-frame noise note with
/// envelope $C1 and noise params $33, then sound_ret → 0x20 0xC1 0x33 0xFF).
const TEST_INSTRUMENT1: [u8; 4] = [0x20, 0xC1, 0x33, 0xFF];

fn seq_with_drums() -> Sequencer {
    let mut seq = Sequencer::new();
    seq.register_noise_instruments(vec![TEST_INSTRUMENT1.to_vec()]);
    seq
}

#[test]
fn test_drum_note_triggers_noise_instrument_on_chan8() {
    let mut seq = seq_with_drums();
    let mut apu = Apu::new();
    apu.power = true;

    // CHAN4 (music noise): drum_note length 2, instrument 1.
    let ch4 = vec![0xB1, 0x01, 0xFF];
    seq.play_music(1, &[vec![0xC0], vec![0xC0], vec![0xC0], ch4], 0x0100);
    seq.update_frame(&mut apu);

    // The instrument's SFX stream is now running on CHAN8.
    assert!(seq.channels[CHAN8].active);
    assert_eq!(seq.channels[CHAN8].data, TEST_INSTRUMENT1.to_vec());
    assert_eq!(seq.channels[CHAN8].sound_id, 1);

    // Its first noise note ($20 vol=$C1 params=$33) drives the noise hardware,
    // hijacking the channel from the music stream.
    assert_eq!(apu.read_register(0xFF21), 0xC1); // NR42 volume envelope
    assert_eq!(apu.read_register(0xFF22), 0x33); // NR43 polynomial counter

    // The music noise channel only advances its delay — it must not write
    // stale noise params itself (the classic note-length routine's early
    // return for the noise-flagged channel).
    assert!(seq.channels[CHAN4].active);
    assert!(!seq.channels[CHAN4].trigger);
    assert_eq!(seq.channels[CHAN4].delay_counter, 2); // length 2 * speed 1

    // The one-shot instrument ends after its 1-frame note; music CHAN4
    // keeps running on its own timing afterwards.
    seq.update_frame(&mut apu);
    assert!(!seq.channels[CHAN8].active); // sound_ret reached
    assert!(seq.channels[CHAN4].active); // still counting its 2-frame delay
}

#[test]
fn test_drum_note_dropped_when_chan8_busy() {
    // The classic sound-start routine: a noise-instrument SFX never interrupts a
    // busy CHAN8 — the drum hit is dropped.
    let mut seq = seq_with_drums();
    let mut apu = Apu::new();
    apu.power = true;

    // Occupy CHAN8 with a long regular SFX noise note (length nibble $F = 16).
    let long_sfx = vec![0x2F, 0xF1, 0x00, 0xFF];
    seq.play_sfx(100, &[long_sfx.clone()], 3, 0x0100);
    seq.update_frame(&mut apu);
    assert!(seq.channels[CHAN8].active);
    assert_eq!(seq.channels[CHAN8].sound_id, 100);

    // Music drum_note fires while CHAN8 is busy → dropped.
    let ch4 = vec![0xB1, 0x01, 0xFF];
    seq.play_music(1, &[vec![0xC0], vec![0xC0], vec![0xC0], ch4], 0x0100);
    seq.update_frame(&mut apu);

    assert_eq!(seq.channels[CHAN8].sound_id, 100); // original SFX untouched
    assert_eq!(seq.channels[CHAN8].data, long_sfx);
}

#[test]
fn test_drum_note_retriggered_after_chan8_frees() {
    let mut seq = seq_with_drums();
    let mut apu = Apu::new();
    apu.power = true;

    // Two drum hits (length 2 each), instrument 1. Length-2 spacing lets the
    // 1-frame instrument finish before the next drum_note executes — with
    // back-to-back length-1 hits the second would be dropped because CHAN8
    // is still busy when CHAN4 executes (channels update CHAN1→CHAN8).
    let ch4 = vec![0xB1, 0x01, 0xB1, 0x01, 0xFF];
    seq.play_music(1, &[vec![0xC0], vec![0xC0], vec![0xC0], ch4], 0x0100);

    seq.update_frame(&mut apu); // frame 1: first drum triggers CHAN8
    assert!(seq.channels[CHAN8].active);

    seq.update_frame(&mut apu); // frame 2: instrument ends (1-frame note)
    assert!(!seq.channels[CHAN8].active);

    seq.update_frame(&mut apu); // frame 3: second drum re-triggers CHAN8
    assert!(seq.channels[CHAN8].active);
    assert_eq!(seq.channels[CHAN8].sound_id, 1);
}

#[test]
fn test_sfx_noise_note_uses_fixed_tempo() {
    // The classic note-length routine: CHAN8 noise notes always run at tempo $0100,
    // ignoring the SFX tempo field (e.g. a cry tempo of $0080 must not slow
    // drums).
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // noise_note length nibble 3 → delay (3+1) * speed 1 * tempo $0100 = 4.
    let sfx = vec![0x23, 0xC1, 0x33, 0xFF];
    seq.play_sfx(9, &[sfx], 3, 0x0080);
    seq.update_frame(&mut apu);

    assert_eq!(seq.channels[CHAN8].delay_counter, 4);
}

// ═══════════════════════════════════════════════════════════════════════════
// override_channel_stream (the classic overwrite-channel-pointer routine —
// the live command-pointer poke used by the flute battle-music routine)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_override_channel_stream_executes_new_stream_from_start() {
    // Real usage pattern: the sound-start routine initializes the channel,
    // then its pointer is hijacked before the next tick, so the original
    // stream never executes.
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    let caught_like = vec![0xC7, 0xFF]; // long rest, then sound_ret
    seq.play_sfx(50, &[caught_like], 0, 0x0100);

    // New stream: note_type 8/$F0, note C length 1, sound_ret.
    let new_stream = [0xD8, 0xF0, 0x00, 0xFF];
    seq.override_channel_stream(CHAN5, &new_stream);

    // Data + pointer replaced; every other channel state left as the
    // sound-start routine set it up (the classic routine only writes the
    // pointer).
    assert_eq!(seq.channels[CHAN5].data, new_stream.to_vec());
    assert_eq!(seq.channels[CHAN5].ptr, 0);
    assert!(seq.channels[CHAN5].active);
    assert_eq!(seq.channels[CHAN5].sound_id, 50);

    seq.update_frame(&mut apu);
    // The new stream executed from byte 0: note_type applied, then the note.
    assert_eq!(seq.channels[CHAN5].note_speed, 8);
    assert_eq!(seq.channels[CHAN5].volume_envelope, 0xF0);
    assert_eq!(
        seq.channels[CHAN5].frequency,
        commands::calculate_frequency(0, 4)
    );

    // The 1-length note at speed 8 ends after 8 frames; the channel then
    // reaches sound_ret and stops.
    for _ in 0..16 {
        seq.update_frame(&mut apu);
    }
    assert!(!seq.channels[CHAN5].active);
}

#[test]
fn test_override_channel_stream_music_resumes_after_end() {
    // Battle music keeps running on the music channels while the hijacked
    // SFX channel owns the hardware; when the override stream ends, the
    // music channel takes the hardware back (a sound_ret on an SFX channel
    // re-enables the music channel's output).
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // Music channel 1: rest 8 looping forever (battle music underneath).
    let music_ch1 = vec![0xC7, 0xFE, 0x00, 0x00];
    seq.play_music(1, &[music_ch1], 0x0100);

    let caught_like = vec![0xC7, 0xFF];
    seq.play_sfx(50, &[caught_like], 0, 0x0100);

    // Override stream: note_type 8/$80, note C length 1, sound_ret.
    seq.override_channel_stream(CHAN5, &[0xD8, 0x80, 0x00, 0xFF]);

    seq.update_frame(&mut apu);
    // SFX channel owns HW pulse-1: NR12 gets the SFX volume ($80), not the
    // music channel's default ($F0).
    assert_eq!(apu.read_register(0xFF12), 0x80);
    assert!(seq.channels[CHAN1].active, "music keeps running underneath");

    for _ in 0..16 {
        seq.update_frame(&mut apu);
    }
    assert!(!seq.channels[CHAN5].active);
    assert!(seq.channels[CHAN1].active, "music channel still active");
    // Music channel owns the hardware again: its next trigger writes NR12.
    assert_eq!(apu.read_register(0xFF12), 0xF0);
}

#[test]
fn test_override_channel_stream_leaves_other_channels_alone() {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    let ch5 = vec![0xC7, 0xFF];
    let ch6 = vec![0xC6, 0xFF];
    seq.play_sfx(50, &[ch5, ch6.clone()], 0, 0x0100);

    seq.override_channel_stream(CHAN5, &[0x00, 0xFF]);

    assert_eq!(seq.channels[CHAN6].data, ch6, "CHAN6 stream untouched");
    assert_eq!(seq.channels[CHAN6].ptr, 0, "CHAN6 pointer untouched");

    // Out-of-range index is a no-op (8 logical channels).
    seq.override_channel_stream(8, &[0xFF]);
    seq.update_frame(&mut apu);
}

#[test]
fn test_override_channel_stream_does_not_enable_inactive_channel() {
    // The classic routine only pokes a pointer into the channel
    // command-pointer table — a channel whose sound ID is 0 stays silent
    // (the update routine skips it).
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    seq.override_channel_stream(CHAN5, &[0x00, 0xFF]);
    assert!(!seq.channels[CHAN5].active);
    seq.update_frame(&mut apu);
    assert!(!seq.channels[CHAN5].active);
    assert_eq!(seq.channels[CHAN5].frequency, 0);
}

#[test]
fn test_override_music_channel_alternate_tempo_pattern() {
    // The alternate tempo/start mechanism (the classic alternate-tempo
    // routine):
    // Music start, then a live MUSIC channel's pointer is overwritten with a
    // stream whose `sound_loop 0, <start-of-normal-data>` pointed into the
    // normal channel data in the ROM. The converted override streams are the
    // alternate prefix followed by a copy of the normal channel data with
    // loop offsets rebased, so the loop must land inside the appended body.
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    apu.power = true;

    // "Normal" channel: tempo 112, note_type 12, note D_ len 1 (offset 5),
    // sound_loop 0 -> 5.
    let normal = vec![0xED, 0x00, 0x70, 0xDC, 0xB3, 0x20, 0xFE, 0x00, 0x05, 0x00];
    // Override: tempo 100, note_type 12, note C_ len 1, sound_loop 0 -> 15
    // (10-byte prefix + normal offset 5), then the normal data with its own
    // loop rebased to 15 as well.
    let override_stream = vec![
        0xED, 0x00, 0x64, 0xDC, 0xB3, 0x00, 0xFE, 0x00, 0x0F, 0x00, // alt prefix
        0xED, 0x00, 0x70, 0xDC, 0xB3, 0x20, 0xFE, 0x00, 0x0F, 0x00, // appended body
    ];

    seq.play_music(11, &[normal], 0x0070);
    assert_eq!(seq.music_tempo, 0x0070);

    // Hijack the channel before the first tick, exactly like the alternate
    // tempo routine (music start, then the pointer poke lands before the
    // next audio update, so the normal stream never executes).
    seq.override_channel_stream(CHAN1, &override_stream);
    seq.update_frame(&mut apu);
    // The alternate prefix executed: alternate tempo (100), different note.
    assert_eq!(seq.music_tempo, 0x0064);
    assert_eq!(
        seq.channels[CHAN1].frequency,
        commands::calculate_frequency(0, 4)
    );

    // After the prefix's note ends, the sound_loop must jump to offset 15 —
    // inside the appended normal body (note D_), not back into the prefix.
    for _ in 0..40 {
        seq.update_frame(&mut apu);
    }
    assert!(seq.channels[CHAN1].active, "channel keeps looping");
    assert_eq!(
        seq.channels[CHAN1].frequency,
        commands::calculate_frequency(2, 4),
        "loop landed in the appended normal body (note D_)"
    );
}
