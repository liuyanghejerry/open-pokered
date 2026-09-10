//! Game Boy APU (Audio Processing Unit) emulation — generic audio engine.
//!
//! Models the 4-channel Game Boy sound hardware:
//! - Channel 1: Pulse wave with frequency sweep
//! - Channel 2: Pulse wave (no sweep)
//! - Channel 3: Programmable wave
//! - Channel 4: Noise (LFSR-based)
//!
//! Also provides a music/SFX sequencer, command decoder, and audio effects
//! (vibrato, pitch slide, duty cycle rotation).
//!
//! On top of that: [`manager`] orchestrates the per-VBlank work (master
//! volume, music fade-out, cross-track resume states, SFX/cry playback) and
//! [`output`] glues the APU to real device backends (cpal native, Web Audio
//! in the browser — both feature-gated off by default).
//!
//! This crate has no pokered-specific dependencies — it is a standalone
//! Game Boy audio emulator suitable for any GB emulation project.

pub mod apu;
pub mod channel;
pub mod commands;
pub mod effects;
pub mod manager;
pub mod output;
pub mod sequencer;

/// Declarative, file-based audio format + directory loader (requires `serde`).
#[cfg(feature = "serde")]
pub mod format;
#[cfg(feature = "serde")]
pub mod library;

/// Modern file-audio subsystem: streaming WAV/OGG decoding, mixing buses,
/// DSP effects (requires `modern-audio`; compiled out otherwise).
#[cfg(feature = "modern-audio")]
pub mod modern;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod sequencer_tests;

// ── Constants ────────────────────────────────────────────────────────────

/// Game Boy CPU clock frequency in Hz (≈4.194304 MHz).
pub const CPU_CLOCK_HZ: u32 = 4_194_304;

/// The APU's frame sequencer runs at 512 Hz (CPU_CLOCK / 8192).
pub const FRAME_SEQUENCER_HZ: u32 = 512;

/// Number of CPU cycles per frame sequencer tick.
pub const CYCLES_PER_FRAME_SEQ_TICK: u32 = CPU_CLOCK_HZ / FRAME_SEQUENCER_HZ; // 8192

/// Standard output sample rate (can be resampled from GB rate).
pub const SAMPLE_RATE: u32 = 44_100;

/// Number of music channels in the engine.
pub const NUM_MUSIC_CHANNELS: usize = 4;

/// Number of SFX channels (mirrors of the 4 music channels).
pub const NUM_SFX_CHANNELS: usize = 4;

/// Total logical channels (4 music + 4 SFX).
pub const NUM_CHANNELS: usize = NUM_MUSIC_CHANNELS + NUM_SFX_CHANNELS;

/// Number of notes in one octave.
pub const NUM_NOTES: usize = 12;

// ── Duty Cycle ───────────────────────────────────────────────────────────

/// Pulse wave duty cycle patterns.
/// Each pattern is 8 steps; 1 = high, 0 = low.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DutyCycle {
    /// 12.5% — waveform: 00000001
    Duty12 = 0,
    /// 25% — waveform: 10000001
    Duty25 = 1,
    /// 50% — waveform: 10000111
    Duty50 = 2,
    /// 75% — waveform: 01111110
    Duty75 = 3,
}

impl DutyCycle {
    /// The 8-step waveform pattern for this duty cycle.
    /// Returns the output level (0 or 1) for each of the 8 positions.
    pub const fn pattern(self) -> [u8; 8] {
        match self {
            DutyCycle::Duty12 => [0, 0, 0, 0, 0, 0, 0, 1],
            DutyCycle::Duty25 => [1, 0, 0, 0, 0, 0, 0, 1],
            DutyCycle::Duty50 => [1, 0, 0, 0, 0, 1, 1, 1],
            DutyCycle::Duty75 => [0, 1, 1, 1, 1, 1, 1, 0],
        }
    }

    /// Convert from a 2-bit value.
    pub const fn from_u8(val: u8) -> Self {
        match val & 0x03 {
            0 => DutyCycle::Duty12,
            1 => DutyCycle::Duty25,
            2 => DutyCycle::Duty50,
            _ => DutyCycle::Duty75,
        }
    }
}

impl Default for DutyCycle {
    fn default() -> Self {
        DutyCycle::Duty50
    }
}

// ── Note Pitch ───────────────────────────────────────────────────────────

/// Musical note names (within an octave).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NoteName {
    C = 0,
    Cs = 1, // C#
    D = 2,
    Ds = 3, // D#
    E = 4,
    F = 5,
    Fs = 6, // F#
    G = 7,
    Gs = 8, // G#
    A = 9,
    As = 10, // A#
    B = 11,
}

/// Frequency register values for base octave notes.
/// These are the raw 11-bit values written to NR13/NR14 (or NR23/NR24, NR33/NR34).
/// Stored as big-endian words (the low 11 bits are the frequency).
/// The GB frequency formula: freq_hz = 131072 / (2048 - freq_reg)
pub const NOTE_FREQUENCIES: [u16; NUM_NOTES] = [
    0xF82C, // C
    0xF89D, // C#
    0xF907, // D
    0xF96B, // D#
    0xF9CA, // E
    0xFA23, // F
    0xFA77, // F#
    0xFAC7, // G
    0xFB12, // G#
    0xFB58, // A
    0xFB9B, // A#
    0xFBDA, // B
];

/// Extract the 11-bit frequency register value from a note table entry.
/// The table stores big-endian 16-bit words; the low 11 bits are the frequency.
pub const fn note_freq_reg(note: usize) -> u16 {
    NOTE_FREQUENCIES[note] & 0x07FF
}

// ── Wave Samples ─────────────────────────────────────────────────────────

/// Number of wave instruments available.
pub const NUM_WAVE_INSTRUMENTS: usize = 6; // wave0..wave5 (wave5 reused for 6,7,8)

/// Wave RAM is 16 bytes = 32 4-bit samples.
pub const WAVE_RAM_SIZE: usize = 16;
pub const WAVE_SAMPLES_PER_INSTRUMENT: usize = 32;

/// Wave instrument data (32 nibbles per instrument).
/// Each instrument is 32 nibbles (stored as 16 bytes, 2 nibbles per byte).
/// These represent the waveform loaded into FF30-FF3F.
pub const WAVE_INSTRUMENTS: [[u8; WAVE_RAM_SIZE]; NUM_WAVE_INSTRUMENTS] = [
    // wave0: sawtooth-like
    pack_wave([
        0, 2, 4, 6, 8, 10, 12, 14, 15, 15, 15, 14, 14, 13, 13, 12, 12, 11, 10, 9, 8, 7, 6, 5, 4, 4,
        3, 3, 2, 2, 1, 1,
    ]),
    // wave1: slightly different sawtooth
    pack_wave([
        0, 2, 4, 6, 8, 10, 12, 14, 14, 15, 15, 15, 15, 14, 14, 14, 13, 13, 12, 11, 10, 9, 8, 7, 6,
        5, 4, 3, 2, 2, 1, 1,
    ]),
    // wave2: triangle-like
    pack_wave([
        1, 3, 6, 9, 11, 13, 14, 14, 14, 14, 15, 15, 15, 15, 14, 13, 13, 14, 15, 15, 15, 15, 14, 14,
        14, 14, 13, 11, 9, 6, 3, 1,
    ]),
    // wave3: modified sawtooth
    pack_wave([
        0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 15, 14, 13, 14, 15, 15, 14, 14, 13, 12, 11, 10, 9, 8, 7,
        6, 5, 4, 3, 2, 1, 0,
    ]),
    // wave4: complex wave
    pack_wave([
        0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 13, 14, 14, 15, 7, 7, 15, 14, 14, 13, 12, 10, 8, 7, 6,
        5, 4, 3, 2, 1, 0,
    ]),
    // wave5: used in the tower themes (actual data is from sfx stream)
    // The base definition; actual data varies by audio engine context.
    pack_wave([
        2, 1, 14, 2, 3, 3, 2, 8, 14, 1, 2, 2, 15, 15, 14, 10, 1, 0, 1, 4, 13, 12, 1, 0, 14, 3, 4,
        1, 5, 1, 7, 3,
    ]),
];

/// Pack 32 nibbles into 16 bytes (2 nibbles per byte, high nibble first).
const fn pack_wave(nibbles: [u8; 32]) -> [u8; 16] {
    let mut result = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        result[i] = (nibbles[i * 2] << 4) | (nibbles[i * 2 + 1] & 0x0F);
        i += 1;
    }
    result
}

/// Unpack a 16-byte wave RAM into 32 nibble samples (0-15).
pub const fn unpack_wave(packed: &[u8; 16]) -> [u8; 32] {
    let mut result = [0u8; 32];
    let mut i = 0;
    while i < 16 {
        result[i * 2] = packed[i] >> 4;
        result[i * 2 + 1] = packed[i] & 0x0F;
        i += 1;
    }
    result
}

// ── Hardware Channel Mapping ─────────────────────────────────────────────

/// Hardware channel index (0-3 for the 4 physical GB sound channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[repr(u8)]
pub enum HwChannel {
    Pulse1 = 0,
    Pulse2 = 1,
    Wave = 2,
    Noise = 3,
}

impl HwChannel {
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(HwChannel::Pulse1),
            1 => Some(HwChannel::Pulse2),
            2 => Some(HwChannel::Wave),
            3 => Some(HwChannel::Noise),
            _ => None,
        }
    }

    /// The NR5x enable/disable bitmask for this channel (for rAUDTERM / NR51).
    /// Bit layout: bit 7-4 = left output, bit 3-0 = right output.
    pub const fn enable_mask(self) -> u8 {
        match self {
            HwChannel::Pulse1 => 0x11, // bit 0 + bit 4
            HwChannel::Pulse2 => 0x22, // bit 1 + bit 5
            HwChannel::Wave => 0x44,   // bit 2 + bit 6
            HwChannel::Noise => 0x88,  // bit 3 + bit 7
        }
    }

    /// The disable mask (complement of enable_mask).
    pub const fn disable_mask(self) -> u8 {
        !self.enable_mask()
    }
}

// ── Volume Envelope Direction ────────────────────────────────────────────

/// Direction of volume envelope change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvelopeDirection {
    Decrease,
    Increase,
}

impl Default for EnvelopeDirection {
    fn default() -> Self {
        EnvelopeDirection::Decrease
    }
}

// ── Sweep Direction ──────────────────────────────────────────────────────

/// Direction of frequency sweep (Channel 1 only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SweepDirection {
    Increase,
    Decrease,
}

impl Default for SweepDirection {
    fn default() -> Self {
        SweepDirection::Increase
    }
}

// ── Bitflags for channel state (moved from sequencer.rs) ─────────────────

bitflags::bitflags! {
    /// Per-channel flags (the classic GB engine's per-channel flag byte).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ChannelFlags1: u8 {
        /// Bit 0: perfect pitch — add 1 to frequency register.
        const PERFECT_PITCH     = 0x01;
        /// Bit 1: channel is in a sound_call subroutine.
        const SOUND_CALL_ACTIVE = 0x02;
        /// Bit 2: this is a noise or SFX channel.
        const NOISE_OR_SFX      = 0x04;
        /// Bit 3: vibrato direction (0=up, 1=down).
        const VIBRATO_DOWN      = 0x08;
        /// Bit 4: pitch slide is active.
        const PITCH_SLIDE_ON    = 0x10;
        /// Bit 5: pitch slide direction (0=increasing, 1=decreasing).
        const PITCH_SLIDE_DEC   = 0x20;
        /// Bit 6: rotate duty cycle pattern each note.
        const ROTATE_DUTY       = 0x40;
    }
}

bitflags::bitflags! {
    /// Per-channel flags2 (second byte of classic GB engine flags).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ChannelFlags2: u8 {
        /// Bit 0: execute_music — SFX channel interprets data like music
        ///        (enables vibrato/pitch slide processing on SFX).
        const EXECUTE_MUSIC = 0x01;
    }
}

impl Default for ChannelFlags1 {
    fn default() -> Self {
        Self::empty()
    }
}

impl Default for ChannelFlags2 {
    fn default() -> Self {
        Self::empty()
    }
}

// ── Vibrato State (moved from sequencer.rs) ──────────────────────────────

/// Per-channel vibrato parameters and running state.
#[derive(Debug, Clone, Default)]
pub struct VibratoState {
    /// Delay before vibrato starts (reload value in frames).
    pub delay_reload: u8,
    /// Current delay countdown.
    pub delay_counter: u8,
    /// Vibrato extent (upper nibble = up amount, lower nibble = down amount).
    pub extent: u8,
    /// Vibrato rate (upper nibble = reload, lower nibble = counter).
    pub rate: u8,
}

impl VibratoState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Upper extent (pitch increase amount).
    pub fn extent_up(&self) -> u8 {
        (self.extent >> 4) & 0x0F
    }

    /// Lower extent (pitch decrease amount).
    pub fn extent_down(&self) -> u8 {
        self.extent & 0x0F
    }

    /// Rate reload value.
    pub fn rate_reload(&self) -> u8 {
        (self.rate >> 4) & 0x0F
    }

    /// Rate counter value.
    pub fn rate_counter(&self) -> u8 {
        self.rate & 0x0F
    }

    /// Set the rate counter (low nibble).
    pub fn set_rate_counter(&mut self, val: u8) {
        self.rate = (self.rate & 0xF0) | (val & 0x0F);
    }
}

// ── Pitch Slide State (moved from sequencer.rs) ──────────────────────────

/// Per-channel pitch slide (portamento) parameters and running state.
#[derive(Debug, Clone, Default)]
pub struct PitchSlideState {
    /// Target frequency (11-bit).
    pub target_freq: u16,
    /// Current frequency (full 16-bit for fractional precision).
    pub current_freq: u16,
    /// Frequency step per tick (how much to add/subtract each frame).
    pub freq_step: u16,
    /// Fractional accumulator for sub-frame precision.
    pub freq_frac: u8,
    /// Length modifier for slide duration.
    pub length_modifier: u8,
}

impl PitchSlideState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

// ── Channel State (moved from sequencer.rs) ──────────────────────────────

/// Full state for one logical audio channel (music or SFX).
///
/// Models the classic GB engine's per-channel variables (one set per
/// channel, stored in the channel struct array).
#[derive(Debug, Clone)]
pub struct ChannelState {
    // ── Command stream ──
    /// The sound data (byte stream) for this channel.
    pub data: Vec<u8>,
    /// Current read position in `data`.
    pub ptr: usize,
    /// Saved position for sound_call return.
    pub return_ptr: usize,

    // ── Timing ──
    /// Frames remaining before the next command is read.
    pub delay_counter: u8,
    /// Fractional delay accumulator (sub-frame precision for tempo).
    pub delay_frac: u8,
    /// Note speed (from note_type command).
    pub note_speed: u8,
    /// Note length (from the current note/rest command).
    pub note_length: u8,

    // ── Pitch ──
    /// Current octave (0-7).
    pub octave: u8,
    /// Current note frequency (11-bit register value for HW).
    pub frequency: u16,
    /// Saved frequency low byte (for vibrato base).
    pub freq_lo_saved: u8,

    // ── Volume / Envelope ──
    /// Volume and fade packed as in note_type: high nibble = volume, low nibble = fade.
    pub volume_envelope: u8,

    // ── Duty cycle ──
    /// Current duty cycle (0-3).
    pub duty_cycle: u8,
    /// Packed duty cycle rotation pattern (4 x 2-bit, rotated left each note).
    pub duty_cycle_pattern: u8,

    // ── Effects ──
    pub vibrato: VibratoState,
    pub pitch_slide: PitchSlideState,

    // ── Flags ──
    pub flags1: ChannelFlags1,
    pub flags2: ChannelFlags2,

    // ── Loop ──
    /// Loop counter for sound_loop command.
    pub loop_counter: u8,

    // ── Identity ──
    /// The sound ID currently playing on this channel.
    pub sound_id: u8,
    /// Whether this channel is currently active.
    pub active: bool,
    /// Wave instrument index (for ch3/ch7).
    pub wave_instrument: u8,

    // ── Stereo ──
    /// Panning enable mask for this channel (bits in NR51 format).
    pub stereo_panning: u8,

    /// Set when a new note starts; cleared after APU trigger write.
    pub trigger: bool,

    /// Vibrato-modified frequency low byte for APU write (None = no vibrato active).
    pub vibrato_freq_lo: Option<u8>,

    /// Pitch sweep value for NR10 (channel 1 only). Set by PitchSweep command.
    /// Written to 0xFF10 when a note triggers on HW channel 0.
    pub pitch_sweep_value: u8,
}

impl ChannelState {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            ptr: 0,
            return_ptr: 0,
            delay_counter: 0,
            delay_frac: 0,
            note_speed: 1,
            note_length: 0,
            octave: 4,
            frequency: 0,
            freq_lo_saved: 0,
            volume_envelope: 0xF0,
            duty_cycle: 0,
            duty_cycle_pattern: 0,
            vibrato: VibratoState::default(),
            pitch_slide: PitchSlideState::default(),
            flags1: ChannelFlags1::default(),
            flags2: ChannelFlags2::default(),
            loop_counter: 0,
            sound_id: 0,
            active: false,
            wave_instrument: 0,
            stereo_panning: 0xFF,
            trigger: false,
            vibrato_freq_lo: None,
            pitch_sweep_value: 0,
        }
    }

    /// Reset channel to inactive state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read the next byte from the command stream, advancing the pointer.
    /// Returns None if at end of data.
    pub fn read_byte(&mut self) -> Option<u8> {
        if self.ptr < self.data.len() {
            let b = self.data[self.ptr];
            self.ptr += 1;
            Some(b)
        } else {
            None
        }
    }

    /// Peek at the next byte without advancing.
    pub fn peek_byte(&self) -> Option<u8> {
        if self.ptr < self.data.len() {
            Some(self.data[self.ptr])
        } else {
            None
        }
    }

    /// Read a 16-bit little-endian value (low byte first, as in Z80 convention).
    pub fn read_u16_le(&mut self) -> Option<u16> {
        let lo = self.read_byte()? as u16;
        let hi = self.read_byte()? as u16;
        Some((hi << 8) | lo)
    }
}

impl Default for ChannelState {
    fn default() -> Self {
        Self::new()
    }
}
