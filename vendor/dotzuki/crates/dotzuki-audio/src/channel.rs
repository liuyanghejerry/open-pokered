//! Game Boy APU channel implementations.
//!
//! Each hardware channel (Pulse1, Pulse2, Wave, Noise) is modelled as a struct
//! with `tick()` (advance one CPU cycle) and `sample()` (current output 0-15).

use crate::{DutyCycle, EnvelopeDirection, SweepDirection, WAVE_RAM_SIZE};

// ── Length Counter ────────────────────────────────────────────────────────

/// Shared length counter component (all 4 channels have one).
/// When enabled and the counter reaches zero the channel is disabled.
#[derive(Debug, Clone)]
pub struct LengthCounter {
    /// Whether the length counter is active.
    pub enabled: bool,
    /// Current countdown value (ticks at 256 Hz via frame sequencer).
    pub counter: u16,
}

impl LengthCounter {
    pub fn new() -> Self {
        Self {
            enabled: false,
            counter: 0,
        }
    }

    /// Called at 256 Hz (frame sequencer steps 0, 2, 4, 6).
    /// Returns `true` if the channel should be disabled (counter expired).
    pub fn tick(&mut self) -> bool {
        if self.enabled && self.counter > 0 {
            self.counter -= 1;
            if self.counter == 0 {
                return true; // disable channel
            }
        }
        false
    }
}

impl Default for LengthCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Volume Envelope ──────────────────────────────────────────────────────

/// Volume envelope (Channels 1, 2, 4).
/// Adjusts volume at 64 Hz (frame sequencer step 7).
#[derive(Debug, Clone)]
pub struct VolumeEnvelope {
    /// Initial volume (0-15), loaded on trigger.
    pub initial_volume: u8,
    /// Current volume (0-15).
    pub current_volume: u8,
    /// Envelope direction.
    pub direction: EnvelopeDirection,
    /// Envelope period (0 = disabled, 1-7).
    pub period: u8,
    /// Internal period counter.
    counter: u8,
}

impl VolumeEnvelope {
    pub fn new() -> Self {
        Self {
            initial_volume: 0,
            current_volume: 0,
            direction: EnvelopeDirection::Decrease,
            period: 0,
            counter: 0,
        }
    }

    /// Called at 64 Hz (frame sequencer step 7).
    pub fn tick(&mut self) {
        if self.period == 0 {
            return;
        }
        if self.counter > 0 {
            self.counter -= 1;
        }
        if self.counter == 0 {
            self.counter = self.period;
            match self.direction {
                EnvelopeDirection::Increase => {
                    if self.current_volume < 15 {
                        self.current_volume += 1;
                    }
                }
                EnvelopeDirection::Decrease => {
                    if self.current_volume > 0 {
                        self.current_volume -= 1;
                    }
                }
            }
        }
    }

    /// Reset on channel trigger.
    pub fn trigger(&mut self) {
        self.current_volume = self.initial_volume;
        self.counter = self.period;
    }

    /// Write from NRx2 register value.
    pub fn write_nrx2(&mut self, val: u8) {
        self.initial_volume = (val >> 4) & 0x0F;
        self.direction = if val & 0x08 != 0 {
            EnvelopeDirection::Increase
        } else {
            EnvelopeDirection::Decrease
        };
        self.period = val & 0x07;
    }

    /// Read back as NRx2 value.
    pub fn read_nrx2(&self) -> u8 {
        (self.initial_volume << 4)
            | if self.direction == EnvelopeDirection::Increase {
                0x08
            } else {
                0x00
            }
            | self.period
    }
}

impl Default for VolumeEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

// ── Frequency Sweep ──────────────────────────────────────────────────────

/// Frequency sweep (Channel 1 only).
/// Adjusts frequency at 128 Hz (frame sequencer steps 2, 6).
#[derive(Debug, Clone)]
pub struct FrequencySweep {
    /// Sweep period (0 = disabled, 1-7).
    pub period: u8,
    /// Sweep direction.
    pub direction: SweepDirection,
    /// Sweep shift (0-7).
    pub shift: u8,
    /// Shadow frequency register (11-bit).
    pub shadow_freq: u16,
    /// Internal period counter.
    counter: u8,
    /// Whether sweep is currently active.
    pub enabled: bool,
}

impl FrequencySweep {
    pub fn new() -> Self {
        Self {
            period: 0,
            direction: SweepDirection::Increase,
            shift: 0,
            shadow_freq: 0,
            counter: 0,
            enabled: false,
        }
    }

    /// Calculate the new frequency after one sweep step.
    /// Returns `None` if the result overflows (> 2047), which disables the channel.
    pub fn calc_new_freq(&self) -> Option<u16> {
        let delta = self.shadow_freq >> self.shift;
        let new_freq = match self.direction {
            SweepDirection::Increase => self.shadow_freq.wrapping_add(delta),
            SweepDirection::Decrease => self.shadow_freq.wrapping_sub(delta),
        };
        if new_freq > 2047 {
            None // overflow — disable channel
        } else {
            Some(new_freq)
        }
    }

    /// Called at 128 Hz (frame sequencer steps 2, 6).
    /// Returns `Some(new_freq)` if frequency should be updated,
    /// `None` if sweep overflowed (channel should be disabled).
    /// Returns `Some(current)` unchanged if sweep is inactive.
    pub fn tick(&mut self) -> Result<Option<u16>, ()> {
        if !self.enabled || self.period == 0 {
            return Ok(None); // no update
        }
        if self.counter > 0 {
            self.counter -= 1;
        }
        if self.counter == 0 {
            self.counter = self.period;
            if self.shift != 0 {
                match self.calc_new_freq() {
                    Some(new_freq) => {
                        self.shadow_freq = new_freq;
                        // Overflow check on the NEW frequency too
                        if self.calc_new_freq().is_none() {
                            return Err(()); // disable channel
                        }
                        return Ok(Some(new_freq));
                    }
                    None => return Err(()), // disable channel
                }
            }
        }
        Ok(None)
    }

    /// Trigger: load shadow frequency from current freq_reg, reset counter, run overflow check.
    /// Returns Err(()) if overflow check fails (channel should be disabled).
    pub fn trigger(&mut self, freq_reg: u16) -> Result<(), ()> {
        self.shadow_freq = freq_reg;
        self.counter = if self.period != 0 { self.period } else { 8 };
        self.enabled = self.period != 0 || self.shift != 0;
        // If shift != 0, do an immediate overflow check
        if self.shift != 0 {
            if self.calc_new_freq().is_none() {
                return Err(());
            }
        }
        Ok(())
    }

    /// Write from NR10 register value.
    pub fn write_nr10(&mut self, val: u8) {
        self.period = (val >> 4) & 0x07;
        self.direction = if val & 0x08 != 0 {
            SweepDirection::Decrease
        } else {
            SweepDirection::Increase
        };
        self.shift = val & 0x07;
    }

    /// Read back as NR10 value.
    pub fn read_nr10(&self) -> u8 {
        (self.period << 4)
            | if self.direction == SweepDirection::Decrease {
                0x08
            } else {
                0x00
            }
            | self.shift
    }
}

impl Default for FrequencySweep {
    fn default() -> Self {
        Self::new()
    }
}

// ── Pulse Channel ────────────────────────────────────────────────────────

/// Pulse wave channel (Channels 1 and 2).
/// Channel 1 has a frequency sweep; Channel 2 does not.
#[derive(Debug, Clone)]
pub struct PulseChannel {
    /// Whether this channel is currently active.
    pub enabled: bool,
    /// DAC enabled (volume envelope initial_vol > 0 or direction = increase).
    pub dac_enabled: bool,
    /// Duty cycle selector.
    pub duty: DutyCycle,
    /// Current position in the 8-step waveform (0-7).
    pub waveform_pos: u8,
    /// 11-bit frequency register value.
    pub freq_reg: u16,
    /// Internal frequency timer (counts down from period).
    freq_timer: u32,
    /// Length counter.
    pub length: LengthCounter,
    /// Volume envelope.
    pub envelope: VolumeEnvelope,
    /// Frequency sweep (Some for Channel 1, None for Channel 2).
    pub sweep: Option<FrequencySweep>,
}

impl PulseChannel {
    /// Create a new pulse channel. `has_sweep` = true for Channel 1.
    pub fn new(has_sweep: bool) -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            duty: DutyCycle::default(),
            waveform_pos: 0,
            freq_reg: 0,
            freq_timer: 0,
            length: LengthCounter::new(),
            envelope: VolumeEnvelope::new(),
            sweep: if has_sweep {
                Some(FrequencySweep::new())
            } else {
                None
            },
        }
    }

    /// Period in CPU cycles = (2048 - freq_reg) * 4.
    fn period(&self) -> u32 {
        (2048 - self.freq_reg as u32) * 4
    }

    /// Advance by one CPU cycle.
    pub fn tick(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = self.period();
            self.waveform_pos = (self.waveform_pos + 1) & 7;
        }
    }

    /// Current output sample (0-15).
    pub fn sample(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }
        let wave_bit = self.duty.pattern()[self.waveform_pos as usize];
        if wave_bit != 0 {
            self.envelope.current_volume
        } else {
            0
        }
    }

    /// Trigger the channel (write 1 to NRx4 bit 7).
    pub fn trigger(&mut self) {
        self.enabled = true;
        if self.length.counter == 0 {
            self.length.counter = 64;
        }
        self.freq_timer = self.period();
        self.envelope.trigger();

        if let Some(ref mut sweep) = self.sweep {
            if sweep.trigger(self.freq_reg).is_err() {
                self.enabled = false;
            }
        }

        // Check DAC
        self.dac_enabled = self.envelope.initial_volume != 0
            || self.envelope.direction == EnvelopeDirection::Increase;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Tick the length counter (256 Hz). Disables channel if counter expires.
    pub fn tick_length(&mut self) {
        if self.length.tick() {
            self.enabled = false;
        }
    }

    /// Tick the volume envelope (64 Hz).
    pub fn tick_envelope(&mut self) {
        self.envelope.tick();
    }

    /// Tick the frequency sweep (128 Hz). Channel 1 only.
    pub fn tick_sweep(&mut self) {
        if let Some(ref mut sweep) = self.sweep {
            match sweep.tick() {
                Ok(Some(new_freq)) => {
                    self.freq_reg = new_freq;
                }
                Ok(None) => {}
                Err(()) => {
                    self.enabled = false;
                }
            }
        }
    }

    /// Write NRx1: duty (bits 7-6) + length load (bits 5-0).
    pub fn write_nrx1(&mut self, val: u8) {
        self.duty = DutyCycle::from_u8((val >> 6) & 0x03);
        self.length.counter = 64 - (val & 0x3F) as u16;
    }

    /// Write NRx2: volume envelope parameters.
    pub fn write_nrx2(&mut self, val: u8) {
        self.envelope.write_nrx2(val);
        self.dac_enabled = (val & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Write NRx3: frequency low 8 bits.
    pub fn write_nrx3(&mut self, val: u8) {
        self.freq_reg = (self.freq_reg & 0x0700) | val as u16;
    }

    /// Write NRx4: trigger (bit 7), length enable (bit 6), frequency high 3 bits (bits 2-0).
    pub fn write_nrx4(&mut self, val: u8) {
        self.length.enabled = val & 0x40 != 0;
        self.freq_reg = (self.freq_reg & 0x00FF) | ((val as u16 & 0x07) << 8);
        if val & 0x80 != 0 {
            self.trigger();
        }
    }
}

// ── Wave Channel ─────────────────────────────────────────────────────────

/// Programmable wave channel (Channel 3).
/// Plays a custom 32-sample (4-bit each) waveform from wave RAM.
#[derive(Debug, Clone)]
pub struct WaveChannel {
    /// Whether this channel is currently active.
    pub enabled: bool,
    /// DAC enabled (NR30 bit 7).
    pub dac_enabled: bool,
    /// 11-bit frequency register.
    pub freq_reg: u16,
    /// Internal frequency timer.
    freq_timer: u32,
    /// Current position in the 32-sample waveform (0-31).
    pub sample_pos: u8,
    /// Wave RAM (16 bytes = 32 4-bit samples).
    pub wave_ram: [u8; WAVE_RAM_SIZE],
    /// Volume shift: 0=mute, 1=100%, 2=50%, 3=25%.
    pub volume_code: u8,
    /// Length counter.
    pub length: LengthCounter,
}

impl WaveChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            freq_reg: 0,
            freq_timer: 0,
            sample_pos: 0,
            wave_ram: [0; WAVE_RAM_SIZE],
            volume_code: 0,
            length: LengthCounter::new(),
        }
    }

    /// Period in CPU cycles = (2048 - freq_reg) * 2.
    fn period(&self) -> u32 {
        (2048 - self.freq_reg as u32) * 2
    }

    /// Advance by one CPU cycle.
    pub fn tick(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = self.period();
            self.sample_pos = (self.sample_pos + 1) & 31;
        }
    }

    /// Read the current 4-bit sample from wave RAM.
    fn current_wave_sample(&self) -> u8 {
        let byte = self.wave_ram[(self.sample_pos / 2) as usize];
        if self.sample_pos & 1 == 0 {
            byte >> 4 // high nibble first
        } else {
            byte & 0x0F
        }
    }

    /// Current output sample (0-15).
    pub fn sample(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }
        let raw = self.current_wave_sample();
        match self.volume_code {
            0 => 0,        // mute
            1 => raw,      // 100%
            2 => raw >> 1, // 50%
            3 => raw >> 2, // 25%
            _ => 0,
        }
    }

    /// Trigger the channel.
    pub fn trigger(&mut self) {
        self.enabled = true;
        if self.length.counter == 0 {
            self.length.counter = 256;
        }
        self.freq_timer = self.period();
        self.sample_pos = 0;

        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Tick the length counter (256 Hz).
    pub fn tick_length(&mut self) {
        if self.length.tick() {
            self.enabled = false;
        }
    }

    /// Load wave RAM from a slice of 16 bytes.
    pub fn load_wave_ram(&mut self, data: &[u8; WAVE_RAM_SIZE]) {
        self.wave_ram.copy_from_slice(data);
    }

    /// Write NR30: DAC enable (bit 7).
    pub fn write_nr30(&mut self, val: u8) {
        self.dac_enabled = val & 0x80 != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Write NR31: length load (full 8 bits).
    pub fn write_nr31(&mut self, val: u8) {
        self.length.counter = 256 - val as u16;
    }

    /// Write NR32: volume code (bits 6-5).
    pub fn write_nr32(&mut self, val: u8) {
        self.volume_code = (val >> 5) & 0x03;
    }

    /// Write NR33: frequency low 8 bits.
    pub fn write_nr33(&mut self, val: u8) {
        self.freq_reg = (self.freq_reg & 0x0700) | val as u16;
    }

    /// Write NR34: trigger (bit 7), length enable (bit 6), frequency high 3 bits.
    pub fn write_nr34(&mut self, val: u8) {
        self.length.enabled = val & 0x40 != 0;
        self.freq_reg = (self.freq_reg & 0x00FF) | ((val as u16 & 0x07) << 8);
        if val & 0x80 != 0 {
            self.trigger();
        }
    }
}

impl Default for WaveChannel {
    fn default() -> Self {
        Self::new()
    }
}

// ── Noise Channel ────────────────────────────────────────────────────────

/// Noise channel (Channel 4) using a Linear Feedback Shift Register (LFSR).
#[derive(Debug, Clone)]
pub struct NoiseChannel {
    /// Whether this channel is currently active.
    pub enabled: bool,
    /// DAC enabled.
    pub dac_enabled: bool,
    /// LFSR (15-bit).
    pub lfsr: u16,
    /// Clock shift (0-15).
    pub clock_shift: u8,
    /// Width mode: false = 15-bit, true = 7-bit.
    pub width_mode: bool,
    /// Divisor code (0-7).
    pub divisor_code: u8,
    /// Internal frequency timer.
    freq_timer: u32,
    /// Volume envelope.
    pub envelope: VolumeEnvelope,
    /// Length counter.
    pub length: LengthCounter,
}

impl NoiseChannel {
    pub fn new() -> Self {
        Self {
            enabled: false,
            dac_enabled: false,
            lfsr: 0x7FFF, // all bits set
            clock_shift: 0,
            width_mode: false,
            divisor_code: 0,
            freq_timer: 0,
            envelope: VolumeEnvelope::new(),
            length: LengthCounter::new(),
        }
    }

    /// Base divisor from divisor code.
    fn divisor(&self) -> u32 {
        match self.divisor_code {
            0 => 8,
            n => (n as u32) * 16,
        }
    }

    /// Period in CPU cycles = divisor << clock_shift.
    fn period(&self) -> u32 {
        self.divisor() << self.clock_shift
    }

    /// Advance by one CPU cycle.
    pub fn tick(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = self.period();
            // XOR bits 0 and 1
            let xor_bit = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            // Shift right
            self.lfsr >>= 1;
            // Put XOR result into bit 14
            self.lfsr |= xor_bit << 14;
            // If 7-bit mode, also put into bit 6
            if self.width_mode {
                self.lfsr &= !(1 << 6);
                self.lfsr |= xor_bit << 6;
            }
        }
    }

    /// Current output sample (0-15).
    /// Output is inverted bit 0 of LFSR, multiplied by volume.
    pub fn sample(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }
        // Inverted bit 0: when bit 0 is 0, output is high
        if self.lfsr & 1 == 0 {
            self.envelope.current_volume
        } else {
            0
        }
    }

    /// Trigger the channel.
    pub fn trigger(&mut self) {
        self.enabled = true;
        if self.length.counter == 0 {
            self.length.counter = 64;
        }
        self.freq_timer = self.period();
        self.lfsr = 0x7FFF; // reset all bits
        self.envelope.trigger();

        self.dac_enabled = self.envelope.initial_volume != 0
            || self.envelope.direction == EnvelopeDirection::Increase;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Tick the length counter (256 Hz).
    pub fn tick_length(&mut self) {
        if self.length.tick() {
            self.enabled = false;
        }
    }

    /// Tick the volume envelope (64 Hz).
    pub fn tick_envelope(&mut self) {
        self.envelope.tick();
    }

    /// Write NR41: length load (bits 5-0).
    pub fn write_nr41(&mut self, val: u8) {
        self.length.counter = 64 - (val & 0x3F) as u16;
    }

    /// Write NR42: volume envelope parameters.
    pub fn write_nr42(&mut self, val: u8) {
        self.envelope.write_nrx2(val);
        self.dac_enabled = (val & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    /// Write NR43: clock shift (bits 7-4), width mode (bit 3), divisor code (bits 2-0).
    pub fn write_nr43(&mut self, val: u8) {
        self.clock_shift = (val >> 4) & 0x0F;
        self.width_mode = val & 0x08 != 0;
        self.divisor_code = val & 0x07;
    }

    /// Write NR44: trigger (bit 7), length enable (bit 6).
    pub fn write_nr44(&mut self, val: u8) {
        self.length.enabled = val & 0x40 != 0;
        if val & 0x80 != 0 {
            self.trigger();
        }
    }
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self::new()
    }
}
