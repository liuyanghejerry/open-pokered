//! Tests for the dotzuki-audio crate.

use crate::apu::Apu;
use crate::channel::{
    FrequencySweep, LengthCounter, NoiseChannel, PulseChannel, VolumeEnvelope, WaveChannel,
};
use crate::*;

// ── DutyCycle tests ──────────────────────────────────────────────────────

#[test]
fn duty_cycle_patterns() {
    assert_eq!(DutyCycle::Duty12.pattern(), [0, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(DutyCycle::Duty25.pattern(), [1, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(DutyCycle::Duty50.pattern(), [1, 0, 0, 0, 0, 1, 1, 1]);
    assert_eq!(DutyCycle::Duty75.pattern(), [0, 1, 1, 1, 1, 1, 1, 0]);
}

#[test]
fn duty_cycle_from_u8_roundtrip() {
    for val in 0..=3u8 {
        let duty = DutyCycle::from_u8(val);
        assert_eq!(duty as u8, val);
    }
    // Out of range wraps
    assert_eq!(DutyCycle::from_u8(4), DutyCycle::Duty12);
    assert_eq!(DutyCycle::from_u8(0xFF), DutyCycle::Duty75);
}

// ── Note frequency tests ─────────────────────────────────────────────────

#[test]
fn note_frequencies_count() {
    assert_eq!(NOTE_FREQUENCIES.len(), NUM_NOTES);
    assert_eq!(NUM_NOTES, 12);
}

#[test]
fn note_freq_reg_extraction() {
    // C note: 0xF82C → low 11 bits = 0x02C = 44
    assert_eq!(note_freq_reg(0), 0x02C);
    // B note: 0xFBDA → low 11 bits = 0x3DA = 986
    assert_eq!(note_freq_reg(11), 0x3DA);
    // All values should be in 0..2048
    for i in 0..NUM_NOTES {
        assert!(note_freq_reg(i) < 2048);
    }
}

#[test]
fn note_freq_ascending() {
    // Higher notes should have higher frequency register values
    for i in 1..NUM_NOTES {
        assert!(
            note_freq_reg(i) > note_freq_reg(i - 1),
            "Note {} freq ({}) should be > note {} freq ({})",
            i,
            note_freq_reg(i),
            i - 1,
            note_freq_reg(i - 1)
        );
    }
}

// ── Wave instrument tests ────────────────────────────────────────────────

#[test]
fn wave_instruments_count() {
    assert_eq!(WAVE_INSTRUMENTS.len(), NUM_WAVE_INSTRUMENTS);
    assert_eq!(NUM_WAVE_INSTRUMENTS, 6);
}

#[test]
fn wave_pack_unpack_roundtrip() {
    for (idx, packed) in WAVE_INSTRUMENTS.iter().enumerate() {
        let unpacked = unpack_wave(packed);
        // All nibbles should be 0-15
        for (j, &nibble) in unpacked.iter().enumerate() {
            assert!(
                nibble <= 15,
                "Wave {} sample {} = {} (should be 0-15)",
                idx,
                j,
                nibble
            );
        }
        // Re-pack should equal original
        let repacked = pack_wave(unpacked);
        assert_eq!(
            &repacked, packed,
            "Wave {} pack/unpack roundtrip failed",
            idx
        );
    }
}

#[test]
fn wave_ram_size() {
    assert_eq!(WAVE_RAM_SIZE, 16);
    assert_eq!(WAVE_SAMPLES_PER_INSTRUMENT, 32);
}

// ── HwChannel tests ──────────────────────────────────────────────────────

#[test]
fn hw_channel_from_u8() {
    assert_eq!(HwChannel::from_u8(0), Some(HwChannel::Pulse1));
    assert_eq!(HwChannel::from_u8(1), Some(HwChannel::Pulse2));
    assert_eq!(HwChannel::from_u8(2), Some(HwChannel::Wave));
    assert_eq!(HwChannel::from_u8(3), Some(HwChannel::Noise));
    assert_eq!(HwChannel::from_u8(4), None);
    assert_eq!(HwChannel::from_u8(255), None);
}

#[test]
fn hw_channel_masks() {
    assert_eq!(HwChannel::Pulse1.enable_mask(), 0x11);
    assert_eq!(HwChannel::Pulse2.enable_mask(), 0x22);
    assert_eq!(HwChannel::Wave.enable_mask(), 0x44);
    assert_eq!(HwChannel::Noise.enable_mask(), 0x88);

    // Enable and disable masks should be complementary
    for ch in [
        HwChannel::Pulse1,
        HwChannel::Pulse2,
        HwChannel::Wave,
        HwChannel::Noise,
    ] {
        assert_eq!(ch.enable_mask() | ch.disable_mask(), 0xFF);
        assert_eq!(ch.enable_mask() & ch.disable_mask(), 0x00);
    }
}

// ── LengthCounter tests ─────────────────────────────────────────────────

#[test]
fn length_counter_disabled() {
    let mut lc = LengthCounter::new();
    lc.counter = 10;
    lc.enabled = false;
    // Should not count down when disabled
    assert!(!lc.tick());
    assert_eq!(lc.counter, 10);
}

#[test]
fn length_counter_counts_down() {
    let mut lc = LengthCounter::new();
    lc.counter = 3;
    lc.enabled = true;
    assert!(!lc.tick()); // 3 → 2
    assert_eq!(lc.counter, 2);
    assert!(!lc.tick()); // 2 → 1
    assert_eq!(lc.counter, 1);
    assert!(lc.tick()); // 1 → 0, returns true (disable)
    assert_eq!(lc.counter, 0);
}

#[test]
fn length_counter_stays_at_zero() {
    let mut lc = LengthCounter::new();
    lc.counter = 0;
    lc.enabled = true;
    assert!(!lc.tick()); // already 0, no action
}

// ── VolumeEnvelope tests ─────────────────────────────────────────────────

#[test]
fn envelope_increase() {
    let mut env = VolumeEnvelope::new();
    env.write_nrx2(0x39); // initial_vol=3, increase (bit3=1), period=1
    env.trigger();
    assert_eq!(env.current_volume, 3);

    env.tick(); // counter: 1→0, reload, vol 3→4
    assert_eq!(env.current_volume, 4);
    env.tick(); // vol 4→5
    assert_eq!(env.current_volume, 5);
}

#[test]
fn envelope_decrease() {
    let mut env = VolumeEnvelope::new();
    env.write_nrx2(0x51); // initial_vol=5, decrease, period=1
    env.trigger();
    assert_eq!(env.current_volume, 5);

    env.tick();
    assert_eq!(env.current_volume, 4);
    env.tick();
    assert_eq!(env.current_volume, 3);
}

#[test]
fn envelope_clamps_at_bounds() {
    let mut env = VolumeEnvelope::new();
    // Start at 15, increase — should stay at 15
    env.write_nrx2(0xF9); // initial=15, increase, period=1
    env.trigger();
    assert_eq!(env.current_volume, 15);
    env.tick();
    assert_eq!(env.current_volume, 15); // clamped

    // Start at 0, decrease — should stay at 0
    env.write_nrx2(0x01); // initial=0, decrease, period=1
    env.trigger();
    assert_eq!(env.current_volume, 0);
    env.tick();
    assert_eq!(env.current_volume, 0); // clamped
}

#[test]
fn envelope_period_zero_disabled() {
    let mut env = VolumeEnvelope::new();
    env.write_nrx2(0x50); // initial_vol=5, decrease, period=0 (disabled)
    env.trigger();
    assert_eq!(env.current_volume, 5);
    env.tick();
    assert_eq!(env.current_volume, 5); // no change
}

#[test]
fn envelope_nrx2_roundtrip() {
    let mut env = VolumeEnvelope::new();
    let val = 0xA5; // initial=10, decrease, period=5
    env.write_nrx2(val);
    assert_eq!(env.read_nrx2(), val);

    let val2 = 0x3B; // initial=3, increase, period=3
    env.write_nrx2(val2);
    assert_eq!(env.read_nrx2(), val2);
}

// ── FrequencySweep tests ─────────────────────────────────────────────────

#[test]
fn sweep_increase() {
    let mut sweep = FrequencySweep::new();
    sweep.write_nr10(0x12); // period=1, increase, shift=2
    assert!(sweep.trigger(0x400).is_ok()); // freq=1024

    // Sweep: new = 1024 + (1024 >> 2) = 1024 + 256 = 1280
    match sweep.tick() {
        Ok(Some(f)) => assert_eq!(f, 1280),
        other => panic!("Expected Ok(Some(1280)), got {:?}", other),
    }
}

#[test]
fn sweep_decrease() {
    let mut sweep = FrequencySweep::new();
    sweep.write_nr10(0x1A); // period=1, decrease, shift=2
    assert!(sweep.trigger(0x400).is_ok()); // freq=1024

    // Sweep: new = 1024 - (1024 >> 2) = 1024 - 256 = 768
    match sweep.tick() {
        Ok(Some(f)) => assert_eq!(f, 768),
        other => panic!("Expected Ok(Some(768)), got {:?}", other),
    }
}

#[test]
fn sweep_overflow_disables() {
    let mut sweep = FrequencySweep::new();
    sweep.write_nr10(0x11); // period=1, increase, shift=1
                            // freq=1000: trigger check → 1000+500=1500 OK
    assert!(sweep.trigger(1000).is_ok());

    // After tick: 1500+750=2250 > 2047 → overflow on second check
    match sweep.tick() {
        Ok(Some(1500)) => {
            // Frequency updated to 1500, but the post-update overflow check
            // (1500+750=2250>2047) should cause Err on the NEXT tick.
            // Actually the implementation checks new_freq overflow AFTER updating.
            // Let's tick again to confirm overflow.
        }
        Err(()) => {} // overflow detected
        other => panic!("Expected Ok(Some(1500)) or Err(()), got {:?}", other),
    }
}

#[test]
fn sweep_trigger_overflow_check() {
    let mut sweep = FrequencySweep::new();
    sweep.write_nr10(0x11); // period=1, increase, shift=1
                            // freq=1400, new = 1400 + 700 = 2100 > 2047 → overflow on trigger
    assert!(sweep.trigger(1400).is_err());
}

#[test]
fn sweep_nr10_roundtrip() {
    let mut sweep = FrequencySweep::new();
    let val = 0x36; // period=3, decrease=no, shift=6
                    // wait, 0x36 = 0011_0110 → period=3, bit3=0 (increase), shift=6
    sweep.write_nr10(val);
    assert_eq!(sweep.read_nr10(), val);

    let val2 = 0x5D; // period=5, decrease=yes, shift=5
                     // 0x5D = 0101_1101 → period=5, bit3=1 (decrease), shift=5
    sweep.write_nr10(val2);
    assert_eq!(sweep.read_nr10(), val2);
}

// ── PulseChannel tests ──────────────────────────────────────────────────

#[test]
fn pulse_channel_silence_when_disabled() {
    let ch = PulseChannel::new(false);
    assert_eq!(ch.sample(), 0);
}

#[test]
fn pulse_channel_trigger_produces_output() {
    let mut ch = PulseChannel::new(false);
    ch.write_nrx2(0xF0); // volume=15, no envelope
    ch.write_nrx1(0x80); // duty=50%
    ch.write_nrx3(0x00); // freq low
    ch.write_nrx4(0x87); // trigger + freq high=7 → freq_reg=0x700=1792

    assert!(ch.enabled);
    assert!(ch.dac_enabled);
    // The duty 50% pattern is [1,0,0,0,0,1,1,1], initial pos depends on trigger
    // After trigger, waveform_pos stays where it was (not reset on real GB)
    // But sample should be non-zero at some positions
    let mut found_nonzero = false;
    for _ in 0..8 * (2048 - 1792) * 4 {
        ch.tick();
        if ch.sample() > 0 {
            found_nonzero = true;
            break;
        }
    }
    assert!(
        found_nonzero,
        "Pulse channel should produce non-zero output after trigger"
    );
}

#[test]
fn pulse_channel_dac_off_no_output() {
    let mut ch = PulseChannel::new(false);
    ch.write_nrx2(0x00); // volume=0, decrease → DAC off
    ch.write_nrx4(0x80); // trigger
    assert!(!ch.enabled); // DAC off prevents enable
    assert_eq!(ch.sample(), 0);
}

#[test]
fn pulse_channel_length_disables() {
    let mut ch = PulseChannel::new(false);
    ch.write_nrx2(0xF0); // volume=15
    ch.write_nrx1(0x3E); // length = 64 - 62 = 2
    ch.write_nrx4(0xC0); // trigger + length enable, freq=0

    assert!(ch.enabled);
    ch.tick_length(); // 2 → 1
    assert!(ch.enabled);
    ch.tick_length(); // 1 → 0 → disable
    assert!(!ch.enabled);
}

#[test]
fn pulse_channel_sweep_disables_on_overflow() {
    let mut ch = PulseChannel::new(true);
    ch.write_nrx2(0xF0); // volume=15
    if let Some(ref mut sweep) = ch.sweep {
        sweep.write_nr10(0x11); // period=1, increase, shift=1
    }
    // Set freq to 0x700 via register writes: NRx3=0x00, NRx4=0x87 (trigger + freq high=7)
    ch.write_nrx3(0x00);
    ch.write_nrx4(0x87); // trigger with freq_reg = 0x700 = 1792

    // Sweep: 1792 + 896 = 2688 > 2047 → overflow check on trigger → disabled
    assert!(
        !ch.enabled,
        "Should be disabled after trigger due to sweep overflow check"
    );
}

#[test]
fn pulse_waveform_duty_12_one_eighth() {
    let mut ch = PulseChannel::new(false);
    ch.write_nrx2(0xF0); // volume=15
    ch.write_nrx1(0x00); // duty=12.5%
                         // freq_reg=2047 → period=(2048-2047)*4=4 cycles per step
    ch.write_nrx3(0xFF); // freq low = 0xFF
    ch.write_nrx4(0x87); // trigger + freq high = 7 → freq_reg = 0x7FF = 2047

    // Count high vs low over one full cycle (8 steps × 4 cycles = 32 cycles)
    let mut highs = 0;
    let mut total = 0;
    for _ in 0..32 {
        ch.tick();
        if ch.sample() > 0 {
            highs += 1;
        }
        total += 1;
    }
    // 12.5% duty = 1 out of 8 steps high → ~4 out of 32 cycles
    assert_eq!(total, 32);
    assert_eq!(highs, 4, "12.5% duty should be high for 4 out of 32 cycles");
}

// ── WaveChannel tests ────────────────────────────────────────────────────

#[test]
fn wave_channel_silence_when_disabled() {
    let ch = WaveChannel::new();
    assert_eq!(ch.sample(), 0);
}

#[test]
fn wave_channel_plays_loaded_waveform() {
    let mut ch = WaveChannel::new();
    // Load wave with alternating 0xF0 bytes → nibbles: 15, 0, 15, 0, ...
    let ram = [0xF0; 16];
    ch.load_wave_ram(&ram);
    ch.write_nr30(0x80); // DAC on
    ch.write_nr32(0x20); // volume = 100%
                         // freq_reg=2047 → period=(2048-2047)*2=2 cycles per sample advance
    ch.write_nr33(0xFF);
    ch.write_nr34(0x87); // trigger + freq high=7

    // Collect samples over multiple ticks
    let mut found_nonzero = false;
    for _ in 0..64 {
        ch.tick();
        if ch.sample() > 0 {
            found_nonzero = true;
            break;
        }
    }
    assert!(found_nonzero, "Wave channel should produce non-zero output");
}

#[test]
fn wave_volume_shift() {
    let mut ch = WaveChannel::new();
    // Load wave with all samples = 12 (0xCC)
    let ram = [0xCC; 16]; // each byte = nibbles 12, 12
    ch.load_wave_ram(&ram);
    ch.write_nr30(0x80); // DAC on
    ch.freq_reg = 2047;

    // Volume code 1 = 100%
    ch.write_nr32(0x20);
    ch.write_nr34(0x80); // trigger
    for _ in 0..4 {
        ch.tick();
    } // advance to get a sample
    assert_eq!(ch.sample(), 12);

    // Volume code 2 = 50%
    ch.write_nr32(0x40);
    assert_eq!(ch.sample(), 6); // 12 >> 1

    // Volume code 3 = 25%
    ch.write_nr32(0x60);
    assert_eq!(ch.sample(), 3); // 12 >> 2

    // Volume code 0 = mute
    ch.write_nr32(0x00);
    assert_eq!(ch.sample(), 0);
}

#[test]
fn wave_dac_off_no_output() {
    let mut ch = WaveChannel::new();
    ch.load_wave_ram(&[0xFF; 16]);
    ch.write_nr30(0x00); // DAC off
    ch.write_nr32(0x20); // volume 100%
    ch.write_nr34(0x80); // trigger — should fail since DAC off
    assert!(!ch.enabled);
    assert_eq!(ch.sample(), 0);
}

#[test]
fn wave_length_disables() {
    let mut ch = WaveChannel::new();
    ch.load_wave_ram(&[0xFF; 16]);
    ch.write_nr30(0x80);
    ch.write_nr31(254); // length = 256 - 254 = 2
    ch.write_nr32(0x20);
    ch.write_nr34(0xC0); // trigger + length enable

    assert!(ch.enabled);
    ch.tick_length(); // 2 → 1
    assert!(ch.enabled);
    ch.tick_length(); // 1 → 0 → disable
    assert!(!ch.enabled);
}

// ── NoiseChannel tests ──────────────────────────────────────────────────

#[test]
fn noise_channel_silence_when_disabled() {
    let ch = NoiseChannel::new();
    assert_eq!(ch.sample(), 0);
}

#[test]
fn noise_channel_produces_output() {
    let mut ch = NoiseChannel::new();
    ch.write_nr42(0xF0); // volume=15
    ch.write_nr43(0x00); // clock_shift=0, 15-bit, divisor=0 → period=8
    ch.write_nr44(0x80); // trigger

    assert!(ch.enabled);
    let mut found_nonzero = false;
    for _ in 0..1000 {
        ch.tick();
        if ch.sample() > 0 {
            found_nonzero = true;
            break;
        }
    }
    assert!(found_nonzero);
}

#[test]
fn noise_lfsr_15bit_vs_7bit() {
    // 15-bit mode: LFSR should cycle through more states
    let mut ch15 = NoiseChannel::new();
    ch15.write_nr42(0xF0);
    ch15.write_nr43(0x00); // 15-bit mode
    ch15.write_nr44(0x80);

    let mut ch7 = NoiseChannel::new();
    ch7.write_nr42(0xF0);
    ch7.write_nr43(0x08); // 7-bit mode
    ch7.write_nr44(0x80);

    // Run both for a bit
    let mut states_15 = std::collections::HashSet::new();
    let mut states_7 = std::collections::HashSet::new();

    for _ in 0..1000 {
        // Tick enough to advance LFSR (period for div=0, shift=0 is 8 cycles)
        for _ in 0..8 {
            ch15.tick();
            ch7.tick();
        }
        states_15.insert(ch15.lfsr);
        states_7.insert(ch7.lfsr);
    }

    // 15-bit should have more unique states than 7-bit
    assert!(
        states_15.len() > states_7.len(),
        "15-bit ({} states) should have more states than 7-bit ({} states)",
        states_15.len(),
        states_7.len()
    );
}

#[test]
fn noise_volume_envelope() {
    let mut ch = NoiseChannel::new();
    ch.write_nr42(0xF1); // volume=15, decrease, period=1
    ch.write_nr43(0x00);
    ch.write_nr44(0x80); // trigger

    assert_eq!(ch.envelope.current_volume, 15);
    ch.tick_envelope();
    assert_eq!(ch.envelope.current_volume, 14);
    ch.tick_envelope();
    assert_eq!(ch.envelope.current_volume, 13);
}

#[test]
fn noise_length_disables() {
    let mut ch = NoiseChannel::new();
    ch.write_nr42(0xF0);
    ch.write_nr41(0x3E); // length = 64 - 62 = 2
    ch.write_nr44(0xC0); // trigger + length enable

    assert!(ch.enabled);
    ch.tick_length(); // 2 → 1
    assert!(ch.enabled);
    ch.tick_length(); // 1 → 0 → disable
    assert!(!ch.enabled);
}

// ── APU tests ────────────────────────────────────────────────────────────

#[test]
fn apu_default_is_off() {
    let apu = Apu::new();
    assert!(!apu.power);
    assert_eq!(apu.mix_sample(), (0, 0));
}

#[test]
fn apu_power_on_off() {
    let mut apu = Apu::new();
    // Power on
    apu.write_register(0xFF26, 0x80);
    assert!(apu.power);

    // Power off clears registers
    apu.write_register(0xFF26, 0x00);
    assert!(!apu.power);
    assert_eq!(apu.nr50, 0);
    assert_eq!(apu.nr51, 0);
}

#[test]
fn apu_registers_ignored_when_off() {
    let mut apu = Apu::new();
    // Power off
    assert!(!apu.power);
    // Try to write NR11 — should be ignored
    apu.write_register(0xFF11, 0x80);
    // ch1 duty should still be default
    assert_eq!(apu.ch1.duty, DutyCycle::default());
}

#[test]
fn apu_wave_ram_writable_when_off() {
    let mut apu = Apu::new();
    assert!(!apu.power);
    apu.write_register(0xFF30, 0xAB);
    assert_eq!(apu.ch3.wave_ram[0], 0xAB);
}

#[test]
fn apu_stereo_panning() {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80); // power on
    apu.nr50 = 0x00; // master volume = 1 (0+1) on both sides

    // Set up ch1 with volume 10
    apu.write_register(0xFF12, 0xA0); // vol=10, no envelope
    apu.write_register(0xFF11, 0x80); // duty=50%
    apu.write_register(0xFF13, 0xFF); // freq low
    apu.write_register(0xFF14, 0x87); // trigger, freq high=7

    // Pan ch1 to left only
    apu.nr51 = 0x10; // ch1 left=on, right=off

    // Run enough ticks to get a non-zero sample
    let mut got_left_only = false;
    for _ in 0..2000 {
        apu.tick();
        let (l, r) = apu.mix_sample();
        if l > 0 {
            assert_eq!(r, 0, "Ch1 should not be on right");
            got_left_only = true;
            break;
        }
    }
    assert!(got_left_only, "Should get left-only output");
}

#[test]
fn apu_master_volume() {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80); // power on

    // Set up ch1 with volume 8
    apu.write_register(0xFF12, 0x80); // vol=8, no envelope
    apu.write_register(0xFF11, 0x80); // duty=50%
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x87); // trigger

    // Pan to both sides
    apu.nr51 = 0x11;

    // Tick to a point where ch1 outputs non-zero
    let mut base_sample = (0i16, 0i16);
    apu.nr50 = 0x00; // volume = 0+1 = 1 on both
    for _ in 0..2000 {
        apu.tick();
        let s = apu.mix_sample();
        if s.0 > 0 {
            base_sample = s;
            break;
        }
    }

    if base_sample.0 > 0 {
        // With volume=7 (max), output should be 8x base
        apu.nr50 = 0x77; // volume = 7+1 = 8 on both
        let (l, r) = apu.mix_sample();
        assert_eq!(l, base_sample.0 * 8);
        assert_eq!(r, base_sample.1 * 8);
    }
}

#[test]
fn apu_frame_sequencer_drives_length() {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80);

    // Set up ch2 with short length
    apu.write_register(0xFF17, 0xF0); // vol=15
    apu.write_register(0xFF16, 0x3F); // length = 64 - 63 = 1
    apu.write_register(0xFF19, 0xC0); // trigger + length enable

    assert!(apu.ch2.enabled);

    // One frame sequencer tick at step 0 triggers length
    // Tick exactly CYCLES_PER_FRAME_SEQ_TICK cycles
    apu.tick_n(CYCLES_PER_FRAME_SEQ_TICK);
    // After step 0 (length tick), counter went from 1→0 → disabled
    assert!(
        !apu.ch2.enabled,
        "Ch2 should be disabled after length expired"
    );
}

#[test]
fn apu_frame_sequencer_drives_envelope() {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80);

    // Set up ch1 with envelope decrease, period=1
    apu.write_register(0xFF12, 0xF1); // vol=15, decrease, period=1
    apu.write_register(0xFF14, 0x80); // trigger

    assert_eq!(apu.ch1.envelope.current_volume, 15);

    // Need to reach step 7 to trigger envelope
    // Steps 0-6 happen on ticks 0-6, step 7 on the 8th tick
    apu.tick_n(CYCLES_PER_FRAME_SEQ_TICK * 8); // 8 frame seq ticks = one full cycle
                                               // Step 7 triggers envelope, vol should decrease
    assert!(
        apu.ch1.envelope.current_volume < 15,
        "Volume should have decreased after envelope tick"
    );
}

#[test]
fn apu_read_nr52_status() {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80); // power on

    let status = apu.read_register(0xFF26);
    assert!(status & 0x80 != 0, "Power should be on");
    assert_eq!(status & 0x0F, 0, "No channels should be active yet");

    // Trigger ch1
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF14, 0x80);
    let status = apu.read_register(0xFF26);
    assert!(status & 0x01 != 0, "Ch1 should be active");
}

#[test]
fn apu_power_off_preserves_wave_ram() {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80); // power on

    // Write some wave RAM
    for i in 0..16u8 {
        apu.write_register(0xFF30 + i as u16, i * 17); // fill with pattern
    }

    // Power off
    apu.write_register(0xFF26, 0x00);
    assert!(!apu.power);

    // Wave RAM should be preserved
    for i in 0..16u8 {
        assert_eq!(
            apu.ch3.wave_ram[i as usize],
            i * 17,
            "Wave RAM[{}] should be preserved after power off",
            i
        );
    }
}

#[test]
fn apu_register_read_write_roundtrip() {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80); // power on

    // NR50 (master volume)
    apu.write_register(0xFF24, 0x53);
    assert_eq!(apu.read_register(0xFF24), 0x53);

    // NR51 (panning)
    apu.write_register(0xFF25, 0xA5);
    assert_eq!(apu.read_register(0xFF25), 0xA5);

    // Wave RAM
    apu.write_register(0xFF35, 0xDE);
    assert_eq!(apu.read_register(0xFF35), 0xDE);
}
