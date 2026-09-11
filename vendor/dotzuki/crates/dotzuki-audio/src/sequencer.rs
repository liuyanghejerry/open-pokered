//! Music/SFX sequencer — reads command streams and drives APU channels.
//!
//! Models the classic GB audio engine. The engine has
//! 8 logical channels: 4 music (CHAN1-4) and 4 SFX (CHAN5-8). SFX channels
//! override the corresponding music channel on the same hardware output.
//!
//! Each logical channel has its own command pointer, note delay, volume,
//! vibrato state, pitch slide state, etc.

use crate::apu::Apu;
use crate::commands::{self, Command};
use crate::{HwChannel, NUM_CHANNELS, NUM_MUSIC_CHANNELS, NUM_NOTES, WAVE_INSTRUMENTS};

// Re-export shared types from crate for backward compatibility
pub use crate::{ChannelFlags1, ChannelFlags2, ChannelState, PitchSlideState, VibratoState};

// ── Channel index constants ──────────────────────────────────────────────

/// Music channel indices (0-3).
pub const CHAN1: usize = 0;
pub const CHAN2: usize = 1;
pub const CHAN3: usize = 2;
pub const CHAN4: usize = 3;

/// SFX channel indices (4-7) — mirror HW channels 0-3.
pub const CHAN5: usize = 4;
pub const CHAN6: usize = 5;
pub const CHAN7: usize = 6;
pub const CHAN8: usize = 7;

/// Map logical channel index (0-7) to hardware channel (0-3).
pub const fn hw_channel_for(ch: usize) -> usize {
    ch & 3
}

/// Is this a SFX channel? (index 4-7)
pub const fn is_sfx_channel(ch: usize) -> bool {
    ch >= NUM_MUSIC_CHANNELS
}

// ── Sequencer ────────────────────────────────────────────────────────────

/// The music/SFX sequencer.
///
/// Drives 8 logical channels (4 music + 4 SFX), reads command streams,
/// calculates note timing, applies effects, and writes to the APU.
///
/// Call `update_frame()` once per VBlank (~60 Hz).
#[derive(Debug, Clone)]
pub struct Sequencer {
    /// 8 logical channels: [0-3] = music, [4-7] = SFX.
    pub channels: [ChannelState; NUM_CHANNELS],

    /// Music tempo (16-bit, big-endian format: high byte = integer, low byte = fraction).
    pub music_tempo: u16,
    /// SFX tempo.
    pub sfx_tempo: u16,

    /// Global stereo panning (NR51 value).
    pub stereo_panning: u8,

    /// Whether music is currently playing.
    pub music_playing: bool,
    /// Whether SFX is currently playing.
    pub sfx_playing: bool,

    /// The music sound ID currently playing.
    pub current_music_id: u8,
    /// The SFX sound ID currently playing.
    pub current_sfx_id: u8,

    /// Frequency modifier for cries (added to base frequency).
    pub frequency_modifier: i16,
    /// Tempo modifier for cries (added to base tempo).
    pub tempo_modifier: i16,

    /// Fade counter for music fade out (0 = no fade pending this frame).
    /// Owned by the sequencer (it is part of the sound engine's live state);
    /// driven by [`crate::manager::AudioManager`]'s fade state machine.
    pub fade_counter: u8,

    /// Pending master-volume (NR50) write from a `volume` ($F0) command.
    /// Applied to the APU at the end of `update_frame`, so it takes effect
    /// on the same frame the command executes.
    pub pending_nr50: Option<u8>,

    /// Noise-instrument (drum) SFX streams, indexed by instrument number - 1
    /// (instrument 1 → slot 0). Registered by the game layer; `drum_note`
    /// plays these on CHAN8 (the SFX noise channel). Empty = drums disabled.
    pub noise_instruments: Vec<Vec<u8>>,
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            channels: std::array::from_fn(|_| ChannelState::new()),
            music_tempo: 0x0100, // default: 1.0 (256)
            sfx_tempo: 0x0100,
            stereo_panning: 0xFF,
            music_playing: false,
            sfx_playing: false,
            current_music_id: 0,
            current_sfx_id: 0,
            frequency_modifier: 0,
            tempo_modifier: 0,
            fade_counter: 0,
            pending_nr50: None,
            noise_instruments: Vec::new(),
        }
    }

    /// Register the noise-instrument (drum) SFX streams used by `drum_note`.
    /// `streams[i]` is the CHAN8 byte stream for instrument `i + 1`.
    pub fn register_noise_instruments(&mut self, streams: Vec<Vec<u8>>) {
        self.noise_instruments = streams;
    }

    /// Start playing music on channels 0-3.
    ///
    /// `channel_data` is a slice of up to 4 byte streams, one per channel.
    /// `sound_id` is the music ID.
    /// `tempo` is the initial tempo.
    pub fn play_music(&mut self, sound_id: u8, channel_data: &[Vec<u8>], tempo: u16) {
        // Stop any existing music
        self.stop_music();

        self.current_music_id = sound_id;
        self.music_tempo = tempo;
        self.music_playing = true;

        for (i, data) in channel_data.iter().enumerate() {
            if i >= NUM_MUSIC_CHANNELS {
                break;
            }
            let ch = &mut self.channels[i];
            ch.reset();
            ch.data = data.clone();
            ch.active = true;
            ch.sound_id = sound_id;
            ch.note_speed = 1;
            ch.octave = 4;
            ch.volume_envelope = 0xF0;
            ch.duty_cycle = 0;
            ch.stereo_panning = 0xFF;

            // Channel 3/4 get special flags
            if i == CHAN3 {
                ch.wave_instrument = 0;
            }
            if i == CHAN4 {
                ch.flags1.insert(ChannelFlags1::NOISE_OR_SFX);
            }
        }
    }

    /// Start playing a SFX on channels 4-7.
    ///
    /// `channel_data` maps SFX channels to their byte streams.
    /// `start_channel` is the first channel (0-3) the SFX uses.
    pub fn play_sfx(
        &mut self,
        sound_id: u8,
        channel_data: &[Vec<u8>],
        start_channel: usize,
        tempo: u16,
    ) {
        // Match the original game's behavior: if this SFX is already playing,
        // don't restart it. The original checks the SFX sound-ID field for
        // CHAN5 and skips the sound init if it matches (e.g. the collision
        // SFX dedup in the overworld sound-trigger routine).
        if self.sfx_playing && self.current_sfx_id == sound_id {
            return;
        }

        self.current_sfx_id = sound_id;
        self.sfx_tempo = tempo;
        self.sfx_playing = true;

        for (i, data) in channel_data.iter().enumerate() {
            let ch_idx = NUM_MUSIC_CHANNELS + start_channel + i;
            if ch_idx >= NUM_CHANNELS {
                break;
            }
            let ch = &mut self.channels[ch_idx];
            ch.reset();
            ch.data = data.clone();
            ch.active = true;
            ch.sound_id = sound_id;
            ch.note_speed = 1;
            ch.octave = 4;
            ch.volume_envelope = 0xF0;
            ch.duty_cycle = 0;
            ch.stereo_panning = 0xFF;
            ch.flags1.insert(ChannelFlags1::NOISE_OR_SFX);

            if hw_channel_for(ch_idx) == 2 {
                ch.wave_instrument = 0;
            }
        }
    }

    /// Overwrite a live channel's command stream, keeping every other piece
    /// of channel state (delay counter, flags, enabled state) untouched.
    ///
    /// Mirrors the classic overwrite-channel-pointer routine, which pokes a
    /// new pointer straight into the channel command-pointer table while the
    /// channel is live. The flute battle-music routine uses this to hijack
    /// the SFX channels right after the sound-start routine has initialized
    /// them, so the new stream starts executing on the next frame.
    ///
    /// The channel must already be active (e.g. via `play_sfx`) — like the
    /// original pointer poke, this does not enable a stopped channel.
    pub fn override_channel_stream(&mut self, ch_idx: usize, data: &[u8]) {
        if ch_idx >= NUM_CHANNELS {
            return;
        }
        let ch = &mut self.channels[ch_idx];
        ch.data = data.to_vec();
        ch.ptr = 0;
    }

    /// Stop all music channels.
    pub fn stop_music(&mut self) {
        for i in 0..NUM_MUSIC_CHANNELS {
            self.channels[i].reset();
        }
        self.music_playing = false;
        self.current_music_id = 0;
    }

    /// Stop all SFX channels.
    pub fn stop_sfx(&mut self) {
        for i in NUM_MUSIC_CHANNELS..NUM_CHANNELS {
            self.channels[i].reset();
        }
        self.sfx_playing = false;
        self.current_sfx_id = 0;
    }

    /// Stop everything.
    pub fn stop_all(&mut self) {
        self.stop_music();
        self.stop_sfx();
    }

    /// Update one frame (~60 Hz). Call this once per VBlank.
    ///
    /// For each active channel:
    /// 1. Decrement delay counter
    /// 2. If delay expired, execute commands until next note/rest
    /// 3. Apply per-frame effects (vibrato, pitch slide, duty rotation)
    /// 4. Write results to APU
    pub fn update_frame(&mut self, apu: &mut Apu) {
        for ch_idx in 0..NUM_CHANNELS {
            if !self.channels[ch_idx].active {
                continue;
            }

            self.update_channel(ch_idx);
        }

        // Apply channel states to APU
        self.apply_to_apu(apu);

        // Apply a pending master-volume write from a `volume` ($F0) command.
        // The classic engine's volume routine writes the param byte straight
        // to NR50 in the middle of command processing, so it takes effect on
        // the same frame the command executes.
        // Written directly (like the fade machinery's NR50 writes) rather
        // than through the power-gated write_register.
        if let Some(vol) = self.pending_nr50.take() {
            apu.nr50 = vol;
        }
    }

    /// Update a single channel for one frame.
    fn update_channel(&mut self, ch_idx: usize) {
        let ch = &mut self.channels[ch_idx];

        // Decrement delay
        if ch.delay_counter > 1 {
            ch.delay_counter -= 1;
            // Apply effects while waiting
            self.apply_effects(ch_idx);
            return;
        }

        // Delay expired (or first frame) — execute commands
        self.execute_commands(ch_idx);
    }

    /// Execute commands from the stream until a note, rest, or end is encountered.
    fn execute_commands(&mut self, ch_idx: usize) {
        // Safety: limit iterations to prevent infinite loops on bad data
        let mut max_commands = 256;

        loop {
            if max_commands == 0 {
                self.channels[ch_idx].active = false;
                return;
            }
            max_commands -= 1;

            let ch = &self.channels[ch_idx];
            if !ch.active || ch.ptr >= ch.data.len() {
                self.channels[ch_idx].active = false;
                return;
            }

            let pos = ch.ptr;
            let is_noise = hw_channel_for(ch_idx) == 3;
            let is_sfx = is_sfx_channel(ch_idx);
            let exec_music = ch.flags2.contains(ChannelFlags2::EXECUTE_MUSIC);
            let data_clone = ch.data.clone(); // Clone to avoid borrow conflict
            let (cmd, new_pos) =
                commands::decode_command(&data_clone, pos, is_noise, is_sfx, exec_music);
            self.channels[ch_idx].ptr = new_pos;

            match cmd {
                Command::Note { pitch, length } => {
                    self.handle_note(ch_idx, pitch, length);
                    return; // Note starts playing — wait for delay
                }
                Command::DrumNote { length, instrument } => {
                    self.handle_drum_note(ch_idx, length, instrument);
                    return;
                }
                Command::Rest { length } => {
                    self.handle_rest(ch_idx, length);
                    return;
                }
                Command::NoteType { speed, param } => {
                    self.handle_note_type(ch_idx, speed, param);
                }
                Command::Octave(oct) => {
                    self.channels[ch_idx].octave = oct;
                }
                Command::TogglePerfectPitch => {
                    self.channels[ch_idx]
                        .flags1
                        .toggle(ChannelFlags1::PERFECT_PITCH);
                }
                Command::Vibrato { delay, depth_rate } => {
                    self.handle_vibrato(ch_idx, delay, depth_rate);
                }
                Command::PitchSlide {
                    length_modifier,
                    octave_pitch,
                } => {
                    self.handle_pitch_slide(ch_idx, length_modifier, octave_pitch);
                }
                Command::DutyCycle(duty) => {
                    self.channels[ch_idx].duty_cycle = duty & 0x03;
                }
                Command::Tempo(tempo) => {
                    if is_sfx_channel(ch_idx) {
                        self.sfx_tempo = tempo;
                    } else {
                        self.music_tempo = tempo;
                    }
                }
                Command::StereoPanning(pan) => {
                    self.channels[ch_idx].stereo_panning = pan;
                }
                Command::UnknownEF(_) => {
                    // Mostly unused — ignore
                }
                Command::Volume(vol) => {
                    // The classic engine's volume routine: the param byte is
                    // written directly to NR50, effective immediately
                    // mid-song (not deferred to the next note). The write
                    // lands at the end of this update_frame.
                    self.pending_nr50 = Some(vol);
                }
                Command::ExecuteMusic => {
                    self.channels[ch_idx]
                        .flags2
                        .toggle(ChannelFlags2::EXECUTE_MUSIC);
                }
                Command::DutyCyclePattern(pattern) => {
                    self.channels[ch_idx].duty_cycle_pattern = pattern;
                    self.channels[ch_idx]
                        .flags1
                        .insert(ChannelFlags1::ROTATE_DUTY);
                }
                Command::SoundCall { offset } => {
                    let ch = &mut self.channels[ch_idx];
                    ch.return_ptr = ch.ptr;
                    ch.ptr = offset as usize;
                    ch.flags1.insert(ChannelFlags1::SOUND_CALL_ACTIVE);
                }
                Command::SoundLoop { count, offset } => {
                    self.handle_sound_loop(ch_idx, count, offset);
                }
                Command::SoundRet => {
                    let ch = &mut self.channels[ch_idx];
                    if ch.flags1.contains(ChannelFlags1::SOUND_CALL_ACTIVE) {
                        ch.ptr = ch.return_ptr;
                        ch.flags1.remove(ChannelFlags1::SOUND_CALL_ACTIVE);
                    } else {
                        // End of channel data
                        ch.active = false;
                        self.check_sfx_end(ch_idx);
                        return;
                    }
                }
                Command::PitchSweep { param } => {
                    self.handle_pitch_sweep_sfx(ch_idx, param);
                }
                Command::SfxSquareNote {
                    length,
                    volume_envelope,
                    frequency,
                } => {
                    self.handle_sfx_square_note(ch_idx, length, volume_envelope, frequency);
                    return; // Note starts playing — wait for delay
                }
                Command::SfxNoiseNote {
                    length,
                    volume_envelope,
                    noise_params,
                } => {
                    self.handle_sfx_noise_note(ch_idx, length, volume_envelope, noise_params);
                    return; // Note starts playing — wait for delay
                }
                Command::EndOfData => {
                    self.channels[ch_idx].active = false;
                    self.check_sfx_end(ch_idx);
                    return;
                }
            }
        }
    }

    // ── Command Handlers ─────────────────────────────────────────────────

    /// Handle a note command: set frequency, calculate delay, apply effects setup.
    fn handle_note(&mut self, ch_idx: usize, pitch: u8, length: u8) {
        let ch = &mut self.channels[ch_idx];

        // If duty cycle rotation is enabled, rotate and use new duty
        if ch.flags1.contains(ChannelFlags1::ROTATE_DUTY) {
            ch.duty_cycle = crate::effects::rotate_duty_cycle(ch);
        }

        // Calculate frequency for this note + octave
        let freq = commands::calculate_frequency(pitch, ch.octave);
        ch.frequency = freq;
        ch.freq_lo_saved = (freq & 0xFF) as u8;
        ch.note_length = length;

        // Apply perfect pitch
        if ch.flags1.contains(ChannelFlags1::PERFECT_PITCH) {
            ch.frequency = ch.frequency.wrapping_add(1) & 0x07FF;
        }

        // Reset vibrato delay for this note
        ch.vibrato.delay_counter = ch.vibrato.delay_reload;

        // Calculate note delay from length, speed, and tempo
        let tempo = if is_sfx_channel(ch_idx) {
            self.sfx_tempo
        } else {
            self.music_tempo
        };
        let (delay, new_frac) =
            commands::calculate_delay(length, ch.note_speed, tempo, ch.delay_frac);
        ch.delay_counter = delay;
        ch.delay_frac = new_frac;
        ch.trigger = true;
    }

    /// Handle a drum note: trigger the noise-instrument SFX on CHAN8.
    ///
    /// In the classic engine, the instrument byte is a SFX ID
    /// (noise-instrument IDs 1-19) played through the sound-start routine.
    /// The music noise channel (CHAN4) itself only advances its note delay —
    /// the note-length routine returns without any hardware write because
    /// CHAN4 has the noise/SFX flag set. All drum hardware output comes from
    /// the one-shot SFX running on CHAN8.
    ///
    /// The delay is computed from the length nibble with the channel's
    /// note_speed and the music tempo (CHAN4 is a music channel) — unchanged
    /// from before.
    fn handle_drum_note(&mut self, ch_idx: usize, length: u8, instrument: u8) {
        let ch = &mut self.channels[ch_idx];
        ch.note_length = length;

        // Calculate delay
        let tempo = if is_sfx_channel(ch_idx) {
            self.sfx_tempo
        } else {
            self.music_tempo
        };
        let (delay, new_frac) =
            commands::calculate_delay(length, ch.note_speed, tempo, ch.delay_frac);
        ch.delay_counter = delay;
        ch.delay_frac = new_frac;

        self.trigger_drum_instrument(instrument);
    }

    /// Trigger a noise-instrument (drum) SFX on CHAN8.
    ///
    /// Priority rule from the classic engine's sound-start routine: if CHAN8
    /// is still busy, a noise-instrument SFX is dropped entirely — drums
    /// never interrupt a playing SFX, not even another drum. (Real SFX with
    /// IDs past the noise-instrument range *do* override a playing drum;
    /// those go through the normal `play_sfx` path.)
    ///
    /// The original also gates on the disable-output-when-SFX-ends flag, but
    /// that flag is only ever set by the unused unknown-music $EF command
    /// ($EF, "appears to never be used"), so it is always clear in practice.
    fn trigger_drum_instrument(&mut self, instrument: u8) {
        if instrument == 0 || instrument as usize > self.noise_instruments.len() {
            return;
        }
        if self.channels[CHAN8].active {
            // CHAN8 busy: drum hit dropped.
            return;
        }
        let stream = self.noise_instruments[instrument as usize - 1].clone();
        let ch = &mut self.channels[CHAN8];
        ch.reset();
        ch.data = stream;
        ch.active = true;
        ch.sound_id = instrument;
        ch.note_speed = 1;
        ch.stereo_panning = 0xFF;
        ch.flags1.insert(ChannelFlags1::NOISE_OR_SFX);
        self.sfx_playing = true;
    }

    /// Handle a rest command: silence the channel for the given duration.
    fn handle_rest(&mut self, ch_idx: usize, length: u8) {
        let ch = &mut self.channels[ch_idx];
        ch.note_length = length;

        let tempo = if is_sfx_channel(ch_idx) {
            self.sfx_tempo
        } else {
            self.music_tempo
        };
        let (delay, new_frac) =
            commands::calculate_delay(length, ch.note_speed, tempo, ch.delay_frac);
        ch.delay_counter = delay;
        ch.delay_frac = new_frac;

        // Set frequency to 0 to indicate silence
        ch.frequency = 0;
        ch.trigger = true;
    }

    /// Handle note_type command.
    fn handle_note_type(&mut self, ch_idx: usize, speed: u8, param: u8) {
        let ch = &mut self.channels[ch_idx];
        ch.note_speed = speed;

        let hw = hw_channel_for(ch_idx);
        if hw == 2 {
            // Wave channel: low nibble = wave instrument index,
            // bits 5-4 = volume code (already shifted for NR32)
            ch.wave_instrument = param & 0x0F;
            ch.volume_envelope = param; // Store raw for APU write
        } else {
            // Pulse/noise: high nibble = volume, low nibble = fade
            ch.volume_envelope = param;
        }
    }

    /// Handle vibrato command.
    fn handle_vibrato(&mut self, ch_idx: usize, delay: u8, depth_rate: u8) {
        let ch = &mut self.channels[ch_idx];
        ch.vibrato.delay_reload = delay;
        ch.vibrato.delay_counter = delay;
        ch.vibrato.extent = depth_rate >> 4;

        // Pack extent: upper nibble = ceil(extent/2), lower nibble = floor(extent/2)
        let raw_extent = depth_rate >> 4;
        let up = (raw_extent + 1) / 2;
        let down = raw_extent / 2;
        ch.vibrato.extent = (up << 4) | down;

        ch.vibrato.rate = depth_rate & 0x0F;
        // Set rate as reload|counter — reload in upper nibble, counter in lower
        let rate_val = depth_rate & 0x0F;
        ch.vibrato.rate = (rate_val << 4) | rate_val;

        // Clear vibrato direction
        ch.flags1.remove(ChannelFlags1::VIBRATO_DOWN);
    }

    /// Handle pitch slide command.
    fn handle_pitch_slide(&mut self, ch_idx: usize, length_modifier: u8, octave_pitch: u8) {
        let ch = &mut self.channels[ch_idx];

        let target_octave = (octave_pitch >> 4) & 0x0F;
        let target_pitch = octave_pitch & 0x0F;

        // Calculate target frequency
        let target_freq = if (target_pitch as usize) < NUM_NOTES {
            commands::calculate_frequency(target_pitch, target_octave)
        } else {
            0
        };

        ch.pitch_slide.target_freq = target_freq;
        ch.pitch_slide.length_modifier = length_modifier;
        ch.flags1.insert(ChannelFlags1::PITCH_SLIDE_ON);

        // Determine direction
        if target_freq < ch.frequency {
            ch.flags1.insert(ChannelFlags1::PITCH_SLIDE_DEC);
        } else {
            ch.flags1.remove(ChannelFlags1::PITCH_SLIDE_DEC);
        }

        // The next command should be a note — we need to read it to get the
        // starting frequency and calculate the step. But in our decode-execute loop,
        // the note will be processed naturally. We pre-calculate the step here
        // based on current info.

        // For now, calculate step as simple linear interpolation
        let current = ch.frequency;
        let diff = if target_freq > current {
            target_freq - current
        } else {
            current - target_freq
        };

        // Step per frame — the original divides by (delay - length_modifier)
        // Since we don't know the delay yet (it depends on the next note),
        // we store the modifier and calculate the step when the note plays.
        ch.pitch_slide.freq_step = if diff > 0 { diff.max(1) } else { 0 };
        ch.pitch_slide.current_freq = current;
    }

    /// Handle sound_loop command.
    fn handle_sound_loop(&mut self, ch_idx: usize, count: u8, offset: u16) {
        let ch = &mut self.channels[ch_idx];

        if count == 0 {
            // Infinite loop
            ch.ptr = offset as usize;
        } else {
            // Counted loop
            if ch.loop_counter == 0 {
                // First time: set counter
                ch.loop_counter = count;
            }
            ch.loop_counter -= 1;
            if ch.loop_counter > 0 {
                ch.ptr = offset as usize;
            }
            // else: counter exhausted, continue past the loop
        }
    }

    /// Handle SFX pitch sweep — writes param directly to NR10 (0xFF10).
    /// Replicates the classic SFX pitch-sweep handling.
    fn handle_pitch_sweep_sfx(&mut self, ch_idx: usize, param: u8) {
        self.channels[ch_idx].pitch_sweep_value = param;
    }

    /// Handle SFX square note — directly sets volume, frequency, and triggers.
    /// Replicates the classic SFX square-note path.
    fn handle_sfx_square_note(
        &mut self,
        ch_idx: usize,
        length: u8,
        volume_envelope: u8,
        frequency: u16,
    ) {
        let ch = &mut self.channels[ch_idx];

        let tempo = self.sfx_tempo;
        let (delay, new_frac) =
            commands::calculate_delay(length + 1, ch.note_speed, tempo, ch.delay_frac);
        ch.delay_counter = delay;
        ch.delay_frac = new_frac;

        ch.volume_envelope = volume_envelope;
        ch.frequency = frequency;
        ch.trigger = true;
    }

    /// Handle SFX noise note — directly sets volume, noise params, and triggers.
    /// Replicates the classic SFX noise-note path.
    fn handle_sfx_noise_note(
        &mut self,
        ch_idx: usize,
        length: u8,
        volume_envelope: u8,
        noise_params: u8,
    ) {
        let ch = &mut self.channels[ch_idx];

        // CHAN8 noise notes always use the fixed tempo $0100, never
        // the SFX tempo field (the classic note-length routine hardcodes
        // tempo 1.0 for the CHAN8 branch).
        let tempo = 0x0100;
        let (delay, new_frac) =
            commands::calculate_delay(length + 1, ch.note_speed, tempo, ch.delay_frac);
        ch.delay_counter = delay;
        ch.delay_frac = new_frac;

        ch.volume_envelope = volume_envelope;
        ch.frequency = noise_params as u16;
        ch.trigger = true;
    }

    /// Check if a SFX channel has ended and resume corresponding music channel.
    fn check_sfx_end(&mut self, ch_idx: usize) {
        if !is_sfx_channel(ch_idx) {
            return;
        }

        // Check if all SFX channels are done
        let any_sfx_active = (NUM_MUSIC_CHANNELS..NUM_CHANNELS).any(|i| self.channels[i].active);

        if !any_sfx_active {
            self.sfx_playing = false;
            self.current_sfx_id = 0;
        }
    }

    // ── Effects ──────────────────────────────────────────────────────────

    /// Apply per-frame effects to a channel (vibrato, pitch slide).
    fn apply_effects(&mut self, ch_idx: usize) {
        let is_music = !is_sfx_channel(ch_idx);
        let has_execute_music = self.channels[ch_idx]
            .flags2
            .contains(ChannelFlags2::EXECUTE_MUSIC);

        // Effects only apply to music channels, or SFX with execute_music flag
        if !is_music && !has_execute_music {
            return;
        }

        if let Some(vibrato_lo) = crate::effects::apply_vibrato(&mut self.channels[ch_idx]) {
            self.channels[ch_idx].vibrato_freq_lo = Some(vibrato_lo);
        } else {
            self.channels[ch_idx].vibrato_freq_lo = None;
        }

        // Apply pitch slide
        if let Some(new_freq) = crate::effects::apply_pitch_slide(&mut self.channels[ch_idx]) {
            self.channels[ch_idx].frequency = new_freq;
        }
    }

    // ── APU Interface ────────────────────────────────────────────────────

    /// Apply all channel states to the APU hardware registers.
    ///
    /// For each HW channel, determine which logical channel takes priority
    /// (SFX over music), and write its frequency/volume/duty to the APU.
    fn apply_to_apu(&mut self, apu: &mut Apu) {
        let mut panning = 0u8;

        for hw in 0..4usize {
            let sfx_idx = hw + NUM_MUSIC_CHANNELS;
            let music_idx = hw;

            let active_idx = if self.channels[sfx_idx].active {
                sfx_idx
            } else if self.channels[music_idx].active {
                music_idx
            } else {
                self.silence_hw_channel(apu, hw);
                continue;
            };

            let hw_mask = HwChannel::from_u8(hw as u8)
                .map(|h| h.enable_mask())
                .unwrap_or(0);
            panning |= self.channels[active_idx].stereo_panning & hw_mask;

            let ch = &mut self.channels[active_idx];
            match hw {
                0 => Self::apply_pulse_channel(apu, ch, true),
                1 => Self::apply_pulse_channel(apu, ch, false),
                2 => Self::apply_wave_channel(apu, ch),
                3 => Self::apply_noise_channel(apu, ch),
                _ => {}
            }
        }

        apu.nr51 = panning;
    }

    /// Write pulse channel state to APU.
    /// On new note (trigger=true): writes all registers and clears trigger.
    /// On sustain: only updates frequency low byte (for vibrato/pitch slide).
    fn apply_pulse_channel(apu: &mut Apu, ch: &mut ChannelState, is_ch1: bool) {
        let freq = ch.frequency;
        let nrx3 = ch.vibrato_freq_lo.unwrap_or((freq & 0xFF) as u8);

        if ch.trigger {
            let duty = ch.duty_cycle;
            let vol_env = ch.volume_envelope;

            let nrx1 = (duty & 0x03) << 6;
            let nrx2 = vol_env;
            let nrx4 = 0x80 | ((freq >> 8) & 0x07) as u8;

            if is_ch1 {
                apu.write_register(0xFF10, ch.pitch_sweep_value);
                apu.write_register(0xFF11, nrx1);
                apu.write_register(0xFF12, nrx2);
                apu.write_register(0xFF13, nrx3);
                apu.write_register(0xFF14, nrx4);
            } else {
                apu.write_register(0xFF16, nrx1);
                apu.write_register(0xFF17, nrx2);
                apu.write_register(0xFF18, nrx3);
                apu.write_register(0xFF19, nrx4);
            }

            ch.trigger = false;
        } else {
            if is_ch1 {
                apu.write_register(0xFF13, nrx3);
            } else {
                apu.write_register(0xFF18, nrx3);
            }
        }
    }

    /// Write wave channel state to APU.
    fn apply_wave_channel(apu: &mut Apu, ch: &mut ChannelState) {
        let freq = ch.frequency;

        if ch.trigger {
            let wave_idx = ch.wave_instrument as usize;

            if wave_idx < WAVE_INSTRUMENTS.len() {
                apu.write_register(0xFF1A, 0x00);
                let wave_data = &WAVE_INSTRUMENTS[wave_idx];
                for (i, &byte) in wave_data.iter().enumerate() {
                    apu.write_register(0xFF30 + i as u16, byte);
                }
                apu.write_register(0xFF1A, 0x80);
            }

            let volume_code = (ch.volume_envelope >> 4) & 0x03;
            apu.write_register(0xFF1C, volume_code << 5);
            apu.write_register(0xFF1D, (freq & 0xFF) as u8);
            apu.write_register(0xFF1E, 0x80 | ((freq >> 8) & 0x07) as u8);

            ch.trigger = false;
        } else {
            let nrx3 = ch.vibrato_freq_lo.unwrap_or((freq & 0xFF) as u8);
            apu.write_register(0xFF1D, nrx3);
        }
    }

    /// Write noise channel state to APU.
    fn apply_noise_channel(apu: &mut Apu, ch: &mut ChannelState) {
        if ch.trigger {
            apu.write_register(0xFF21, ch.volume_envelope);

            let freq = ch.frequency;
            let shift = ((freq >> 4) & 0x0F) as u8;
            let divisor = (freq & 0x07) as u8;
            apu.write_register(0xFF22, (shift << 4) | divisor);
            apu.write_register(0xFF23, 0x80);

            ch.trigger = false;
        }
    }

    /// Silence a hardware channel.
    fn silence_hw_channel(&self, apu: &mut Apu, hw: usize) {
        match hw {
            0 => {
                apu.write_register(0xFF12, 0x00); // volume 0
            }
            1 => {
                apu.write_register(0xFF17, 0x00);
            }
            2 => {
                apu.write_register(0xFF1A, 0x00); // DAC off
            }
            3 => {
                apu.write_register(0xFF21, 0x00);
            }
            _ => {}
        }
    }

    // ── Query Methods ────────────────────────────────────────────────────

    /// Check if any channel is actively playing.
    pub fn is_playing(&self) -> bool {
        self.channels.iter().any(|ch| ch.active)
    }

    /// Check if a specific music channel is active.
    pub fn is_music_channel_active(&self, ch: usize) -> bool {
        ch < NUM_MUSIC_CHANNELS && self.channels[ch].active
    }

    /// Check if a specific SFX channel is active.
    pub fn is_sfx_channel_active(&self, ch: usize) -> bool {
        let idx = ch + NUM_MUSIC_CHANNELS;
        idx < NUM_CHANNELS && self.channels[idx].active
    }

    /// Get the current frequency of a logical channel.
    pub fn channel_frequency(&self, ch: usize) -> u16 {
        if ch < NUM_CHANNELS {
            self.channels[ch].frequency
        } else {
            0
        }
    }

    /// Get the current octave of a logical channel.
    pub fn channel_octave(&self, ch: usize) -> u8 {
        if ch < NUM_CHANNELS {
            self.channels[ch].octave
        } else {
            0
        }
    }

    /// Restore music channel state from a saved snapshot.
    ///
    /// Copies music channel states (0-3), tempo, playing flag, and music ID
    /// from `snapshot`. SFX channels (4-7) and fade state are left untouched.
    pub fn restore_music_from(&mut self, snapshot: &Sequencer) {
        for i in 0..NUM_MUSIC_CHANNELS {
            self.channels[i] = snapshot.channels[i].clone();
        }
        self.music_tempo = snapshot.music_tempo;
        self.music_playing = snapshot.music_playing;
        self.current_music_id = snapshot.current_music_id;
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}
