//! Game Boy Audio Processing Unit (APU) — master controller.
//!
//! Combines the 4 hardware channels, drives the frame sequencer, and
//! provides register-level read/write access (NR10-NR52 + wave RAM).

use crate::channel::{NoiseChannel, PulseChannel, WaveChannel};
use crate::CYCLES_PER_FRAME_SEQ_TICK;

// ── APU ──────────────────────────────────────────────────────────────────

/// The Game Boy APU.
///
/// Tick this once per CPU cycle. The frame sequencer internally drives
/// length counters (256 Hz), volume envelopes (64 Hz), and frequency
/// sweep (128 Hz, channel 1 only).
#[derive(Debug, Clone)]
pub struct Apu {
    // ── Channels ──
    /// Channel 1: Pulse with sweep.
    pub ch1: PulseChannel,
    /// Channel 2: Pulse (no sweep).
    pub ch2: PulseChannel,
    /// Channel 3: Wave.
    pub ch3: WaveChannel,
    /// Channel 4: Noise.
    pub ch4: NoiseChannel,

    // ── Master control ──
    /// NR50: Master volume. Bits 6-4 = left volume (0-7), bits 2-0 = right volume (0-7).
    /// Bit 7 = Vin left enable, bit 3 = Vin right enable (unused in pokered).
    pub nr50: u8,
    /// NR51: Sound panning. Each bit enables a channel on left (bits 7-4) or right (bits 3-0).
    pub nr51: u8,
    /// NR52: Sound on/off. Bit 7 = master enable. Bits 3-0 = channel status (read-only).
    pub power: bool,

    // ── Frame sequencer ──
    /// CPU cycle counter for frame sequencer ticks.
    frame_seq_counter: u32,
    /// Current frame sequencer step (0-7).
    frame_seq_step: u8,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            ch1: PulseChannel::new(true),  // Channel 1 has sweep
            ch2: PulseChannel::new(false), // Channel 2 does not
            ch3: WaveChannel::new(),
            ch4: NoiseChannel::new(),
            nr50: 0x77, // default: both L/R at max volume
            nr51: 0xFF, // default: all channels to both outputs
            power: false,
            frame_seq_counter: 0,
            frame_seq_step: 0,
        }
    }

    /// Advance the APU by one CPU cycle.
    pub fn tick(&mut self) {
        if !self.power {
            return;
        }

        // Tick all channels
        self.ch1.tick();
        self.ch2.tick();
        self.ch3.tick();
        self.ch4.tick();

        // Frame sequencer
        self.frame_seq_counter += 1;
        if self.frame_seq_counter >= CYCLES_PER_FRAME_SEQ_TICK {
            self.frame_seq_counter = 0;
            self.clock_frame_sequencer();
        }
    }

    /// Advance the APU by `n` CPU cycles.
    pub fn tick_n(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
    }

    /// Clock the frame sequencer (called at 512 Hz).
    fn clock_frame_sequencer(&mut self) {
        match self.frame_seq_step {
            0 => {
                // Length counter (256 Hz)
                self.tick_lengths();
            }
            1 => {
                // Nothing
            }
            2 => {
                // Length counter + Sweep (128 Hz)
                self.tick_lengths();
                self.ch1.tick_sweep();
            }
            3 => {
                // Nothing
            }
            4 => {
                // Length counter
                self.tick_lengths();
            }
            5 => {
                // Nothing
            }
            6 => {
                // Length counter + Sweep
                self.tick_lengths();
                self.ch1.tick_sweep();
            }
            7 => {
                // Volume envelope (64 Hz)
                self.tick_envelopes();
            }
            _ => unreachable!(),
        }
        self.frame_seq_step = (self.frame_seq_step + 1) & 7;
    }

    /// Tick all length counters.
    fn tick_lengths(&mut self) {
        self.ch1.tick_length();
        self.ch2.tick_length();
        self.ch3.tick_length();
        self.ch4.tick_length();
    }

    /// Tick all volume envelopes.
    fn tick_envelopes(&mut self) {
        self.ch1.tick_envelope();
        self.ch2.tick_envelope();
        self.ch4.tick_envelope();
        // ch3 (wave) does not have a volume envelope
    }

    /// Mix all channel outputs into a stereo sample pair (left, right).
    /// Each output is in range 0..=60 (4 channels × 15 max sample).
    /// Master volume (0-7) is applied.
    pub fn mix_sample(&self) -> (i16, i16) {
        if !self.power {
            return (0, 0);
        }

        let ch1_out = self.ch1.sample() as i16;
        let ch2_out = self.ch2.sample() as i16;
        let ch3_out = self.ch3.sample() as i16;
        let ch4_out = self.ch4.sample() as i16;

        let mut left: i16 = 0;
        let mut right: i16 = 0;

        // NR51 panning
        if self.nr51 & 0x10 != 0 {
            left += ch1_out;
        }
        if self.nr51 & 0x20 != 0 {
            left += ch2_out;
        }
        if self.nr51 & 0x40 != 0 {
            left += ch3_out;
        }
        if self.nr51 & 0x80 != 0 {
            left += ch4_out;
        }

        if self.nr51 & 0x01 != 0 {
            right += ch1_out;
        }
        if self.nr51 & 0x02 != 0 {
            right += ch2_out;
        }
        if self.nr51 & 0x04 != 0 {
            right += ch3_out;
        }
        if self.nr51 & 0x08 != 0 {
            right += ch4_out;
        }

        // Apply master volume (NR50)
        let left_vol = ((self.nr50 >> 4) & 0x07) as i16 + 1;
        let right_vol = (self.nr50 & 0x07) as i16 + 1;

        left *= left_vol;
        right *= right_vol;

        (left, right)
    }

    /// Write to an APU register (address $FF10-$FF3F).
    pub fn write_register(&mut self, addr: u16, val: u8) {
        // If power is off, only NR52 and wave RAM writes are allowed
        if !self.power {
            match addr {
                0xFF26 => { /* NR52 handled below */ }
                0xFF30..=0xFF3F => { /* wave RAM allowed */ }
                _ => return,
            }
        }

        match addr {
            // ── Channel 1 (Pulse + Sweep) ──
            0xFF10 => {
                if let Some(ref mut sweep) = self.ch1.sweep {
                    sweep.write_nr10(val);
                }
            }
            0xFF11 => self.ch1.write_nrx1(val),
            0xFF12 => self.ch1.write_nrx2(val),
            0xFF13 => self.ch1.write_nrx3(val),
            0xFF14 => self.ch1.write_nrx4(val),

            // ── Channel 2 (Pulse) ──
            0xFF16 => self.ch2.write_nrx1(val),
            0xFF17 => self.ch2.write_nrx2(val),
            0xFF18 => self.ch2.write_nrx3(val),
            0xFF19 => self.ch2.write_nrx4(val),

            // ── Channel 3 (Wave) ──
            0xFF1A => self.ch3.write_nr30(val),
            0xFF1B => self.ch3.write_nr31(val),
            0xFF1C => self.ch3.write_nr32(val),
            0xFF1D => self.ch3.write_nr33(val),
            0xFF1E => self.ch3.write_nr34(val),

            // ── Channel 4 (Noise) ──
            0xFF20 => self.ch4.write_nr41(val),
            0xFF21 => self.ch4.write_nr42(val),
            0xFF22 => self.ch4.write_nr43(val),
            0xFF23 => self.ch4.write_nr44(val),

            // ── Master control ──
            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,
            0xFF26 => {
                let new_power = val & 0x80 != 0;
                if !new_power && self.power {
                    self.power_off();
                } else if new_power && !self.power {
                    self.power_on();
                }
            }

            // ── Wave RAM ──
            0xFF30..=0xFF3F => {
                let idx = (addr - 0xFF30) as usize;
                self.ch3.wave_ram[idx] = val;
            }

            _ => {} // unmapped
        }
    }

    /// Read from an APU register (address $FF10-$FF3F).
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            // ── Channel 1 ──
            0xFF10 => {
                if let Some(ref sweep) = self.ch1.sweep {
                    sweep.read_nr10() | 0x80 // bit 7 unused, reads 1
                } else {
                    0xFF
                }
            }
            0xFF11 => (self.ch1.duty as u8) << 6 | 0x3F, // only duty bits readable
            0xFF12 => self.ch1.envelope.read_nrx2(),
            0xFF13 => 0xFF, // write-only
            0xFF14 => {
                if self.ch1.length.enabled {
                    0xBF | 0x40
                } else {
                    0xBF
                }
            }

            // ── Channel 2 ──
            0xFF15 => 0xFF, // NR20 unused
            0xFF16 => (self.ch2.duty as u8) << 6 | 0x3F,
            0xFF17 => self.ch2.envelope.read_nrx2(),
            0xFF18 => 0xFF,
            0xFF19 => {
                if self.ch2.length.enabled {
                    0xBF | 0x40
                } else {
                    0xBF
                }
            }

            // ── Channel 3 ──
            0xFF1A => {
                if self.ch3.dac_enabled {
                    0x7F | 0x80
                } else {
                    0x7F
                }
            }
            0xFF1B => 0xFF, // write-only
            0xFF1C => (self.ch3.volume_code << 5) | 0x9F,
            0xFF1D => 0xFF,
            0xFF1E => {
                if self.ch3.length.enabled {
                    0xBF | 0x40
                } else {
                    0xBF
                }
            }

            // ── Channel 4 ──
            0xFF1F => 0xFF, // NR40 unused
            0xFF20 => 0xFF, // write-only
            0xFF21 => self.ch4.envelope.read_nrx2(),
            0xFF22 => {
                (self.ch4.clock_shift << 4)
                    | if self.ch4.width_mode { 0x08 } else { 0x00 }
                    | self.ch4.divisor_code
            }
            0xFF23 => {
                if self.ch4.length.enabled {
                    0xBF | 0x40
                } else {
                    0xBF
                }
            }

            // ── Master control ──
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                let mut val = 0x70; // bits 6-4 always read 1
                if self.power {
                    val |= 0x80;
                }
                if self.ch1.enabled {
                    val |= 0x01;
                }
                if self.ch2.enabled {
                    val |= 0x02;
                }
                if self.ch3.enabled {
                    val |= 0x04;
                }
                if self.ch4.enabled {
                    val |= 0x08;
                }
                val
            }

            // ── Wave RAM ──
            0xFF30..=0xFF3F => {
                let idx = (addr - 0xFF30) as usize;
                self.ch3.wave_ram[idx]
            }

            _ => 0xFF,
        }
    }

    /// Power off: clear all registers, disable all channels.
    fn power_off(&mut self) {
        self.ch1 = PulseChannel::new(true);
        self.ch2 = PulseChannel::new(false);
        // Wave RAM is NOT cleared on power off
        let wave_ram_backup = self.ch3.wave_ram;
        self.ch3 = WaveChannel::new();
        self.ch3.wave_ram = wave_ram_backup;
        self.ch4 = NoiseChannel::new();
        self.nr50 = 0;
        self.nr51 = 0;
        self.power = false;
        self.frame_seq_counter = 0;
        self.frame_seq_step = 0;
    }

    /// Power on: just set the flag, channels stay cleared.
    fn power_on(&mut self) {
        self.power = true;
        self.frame_seq_counter = 0;
        self.frame_seq_step = 0;
    }
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}
