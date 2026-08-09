use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use pokered_audio::audio_manager::AudioManager;
use pokered_audio::music_data::MusicId;
use pokered_audio::sfx_data::SfxId;
use pokered_audio::CPU_CLOCK_HZ;
use pokered_data::species::Species;

/// Play a species' cry with its pitch/length modifiers (`PlayCry` in
/// home/pokemon.asm: `GetCryData` + `PlaySound`).
pub fn play_species_cry(audio: &AudioOutput, species: Species) {
    let c = pokered_data::cries::cry_data(species);
    if let Some(id) = SfxId::from_u8(c.sfx) {
        audio.play_cry(id, c.pitch, c.length);
    }
}

// ── Native audio output (cpal) ──────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub struct AudioOutput {
    pub manager: Arc<Mutex<AudioManager>>,
    pub _stream: cpal::Stream,
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioOutput {
    pub fn new() -> Option<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(44_100),
            buffer_size: cpal::BufferSize::Default,
        };

        let manager = Arc::new(Mutex::new(AudioManager::new()));
        // Enable APU power (NR52 bit 7). Without this, all APU register writes
        // are silently ignored and no sound is produced.
        {
            let mut mgr = manager.lock().unwrap();
            mgr.apu.write_register(0xFF26, 0x80);
        }
        let mgr_clone = Arc::clone(&manager);

        let cycles_per_sample = CPU_CLOCK_HZ / 44_100;
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut mgr = mgr_clone.lock().unwrap();
                    let max_amplitude = 480.0_f32;
                    for frame in data.chunks_mut(2) {
                        mgr.apu.tick_n(cycles_per_sample);
                        let (left, right) = mgr.apu.mix_sample();
                        frame[0] = left as f32 / max_amplitude;
                        frame[1] = right as f32 / max_amplitude;
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .ok()?;

        stream.play().ok()?;

        Some(Self {
            manager,
            _stream: stream,
        })
    }

    pub fn play_music(&self, id: MusicId) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_music(id);
        }
    }

    pub fn play_music_with_fade(&self, id: MusicId, fade_speed: u8) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_music_with_fade(id, fade_speed);
        }
    }

    pub fn clear_saved_music_states(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.clear_saved_music_states();
        }
    }

    pub fn fade_out(&self, fade_speed: u8) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.fade_out(fade_speed);
        }
    }

    pub fn play_sfx(&self, id: SfxId) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_sfx(id);
        }
    }

    /// Play a species cry with pitch/length modifiers (`PlayCry`).
    pub fn play_cry(&self, id: SfxId, pitch_mod: u8, tempo_mod: u8) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_cry(id, pitch_mod, tempo_mod);
        }
    }

    /// In-battle POKé FLUTE jingle (`Music_PokeFluteInBattle`,
    /// audio/poke_flute.asm).
    pub fn play_flute_in_battle(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_flute_in_battle();
        }
    }

    /// Alternate tempo/start music variants (audio/alternate_tempo.asm);
    /// see `AudioManager::play_script_music`.
    pub fn play_script_music(&self, name: &str) -> bool {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_script_music(name)
        } else {
            false
        }
    }

    pub fn is_sfx_playing(&self) -> bool {
        if let Ok(mgr) = self.manager.lock() {
            mgr.is_sfx_playing()
        } else {
            false
        }
    }

    /// Drive the low-health alarm (`wLowHealthAlarm`) from the battle UI.
    pub fn set_low_health_alarm(&self, enable: bool) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.set_low_health_alarm(enable);
        }
    }

    /// While set, `WaitForSoundToFinish` returns immediately in the
    /// original (home/delay.asm:15-18) — frontends mirror that.
    pub fn low_health_alarm_active(&self) -> bool {
        if let Ok(mgr) = self.manager.lock() {
            mgr.low_health_alarm_active()
        } else {
            false
        }
    }

    pub fn stop_music(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.stop_music();
        }
    }

    pub fn stop_all(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.stop_all();
        }
    }

    pub fn last_music_id(&self) -> Option<MusicId> {
        if let Ok(mgr) = self.manager.lock() {
            mgr.last_music_id()
        } else {
            None
        }
    }

    pub fn update_frame(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.update_frame();
        }
    }

    /// Try to resume audio (no-op on native — always running).
    pub fn try_resume(&self) {}
}

// ── WASM audio output (Web Audio API) ───────────────────────────────────

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
pub struct AudioOutput {
    pub manager: Arc<Mutex<AudioManager>>,
    _ctx: web_sys::AudioContext,
    _processor: web_sys::ScriptProcessorNode,
    /// Prevent the onaudioprocess closure from being dropped.
    _closure: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::AudioProcessingEvent)>,
}

#[cfg(target_arch = "wasm32")]
impl AudioOutput {
    pub fn new() -> Option<Self> {
        let opts = web_sys::AudioContextOptions::new();
        opts.set_sample_rate(44_100.0);
        let ctx = web_sys::AudioContext::new_with_context_options(&opts)
            .or_else(|_| web_sys::AudioContext::new())
            .ok()?;

        let sample_rate = ctx.sample_rate() as u32;

        let manager = Arc::new(Mutex::new(AudioManager::new()));
        // Enable APU power (NR52 bit 7).
        {
            let mut mgr = manager.lock().unwrap();
            mgr.apu.write_register(0xFF26, 0x80);
        }

        // ScriptProcessorNode: buffer size 2048 (≈46ms at 44100Hz), 0 inputs, 2 outputs.
        let processor = ctx
            .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
                2048, 0, 2,
            )
            .ok()?;

        let mgr_clone = Arc::clone(&manager);
        let cycles_per_sample = CPU_CLOCK_HZ / sample_rate;
        let max_amplitude = 480.0_f32;

        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(
            move |event: web_sys::AudioProcessingEvent| {
                let output = match event.output_buffer() {
                    Ok(buf) => buf,
                    Err(_) => return,
                };
                let length = output.length() as usize;

                let mut left_buf = vec![0.0_f32; length];
                let mut right_buf = vec![0.0_f32; length];

                if let Ok(mut mgr) = mgr_clone.lock() {
                    for i in 0..length {
                        mgr.apu.tick_n(cycles_per_sample);
                        let (left, right) = mgr.apu.mix_sample();
                        left_buf[i] = left as f32 / max_amplitude;
                        right_buf[i] = right as f32 / max_amplitude;
                    }
                }

                let _ = output.copy_to_channel(&left_buf, 0);
                let _ = output.copy_to_channel(&right_buf, 1);
            },
        )
            as Box<dyn FnMut(web_sys::AudioProcessingEvent)>);

        processor.set_onaudioprocess(Some(closure.as_ref().unchecked_ref()));

        // Connect processor → destination. The ScriptProcessorNode needs a
        // connected source (even a silent one) to fire onaudioprocess.
        // Connecting processor directly to destination is sufficient since we
        // have 0 input channels and generate audio in the callback.
        processor.connect_with_audio_node(&ctx.destination()).ok()?;

        log::info!(
            "Web Audio initialized (sample_rate={}, buffer=2048)",
            sample_rate
        );

        Some(Self {
            manager,
            _ctx: ctx,
            _processor: processor,
            _closure: closure,
        })
    }

    pub fn play_music(&self, id: MusicId) {
        self.try_resume();
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_music(id);
        }
    }

    pub fn play_music_with_fade(&self, id: MusicId, fade_speed: u8) {
        self.try_resume();
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_music_with_fade(id, fade_speed);
        }
    }

    pub fn play_sfx(&self, id: SfxId) {
        self.try_resume();
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_sfx(id);
        }
    }

    /// Play a species cry with pitch/length modifiers (`PlayCry`).
    pub fn play_cry(&self, id: SfxId, pitch_mod: u8, tempo_mod: u8) {
        self.try_resume();
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_cry(id, pitch_mod, tempo_mod);
        }
    }

    /// In-battle POKé FLUTE jingle (`Music_PokeFluteInBattle`,
    /// audio/poke_flute.asm).
    pub fn play_flute_in_battle(&self) {
        self.try_resume();
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_flute_in_battle();
        }
    }

    /// Alternate tempo/start music variants (audio/alternate_tempo.asm);
    /// see `AudioManager::play_script_music`.
    pub fn play_script_music(&self, name: &str) -> bool {
        self.try_resume();
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_script_music(name)
        } else {
            false
        }
    }

    pub fn clear_saved_music_states(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.clear_saved_music_states();
        }
    }

    pub fn is_sfx_playing(&self) -> bool {
        if let Ok(mgr) = self.manager.lock() {
            mgr.is_sfx_playing()
        } else {
            false
        }
    }

    /// Drive the low-health alarm (`wLowHealthAlarm`) from the battle UI.
    pub fn set_low_health_alarm(&self, enable: bool) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.set_low_health_alarm(enable);
        }
    }

    /// While set, `WaitForSoundToFinish` returns immediately in the
    /// original (home/delay.asm:15-18) — frontends mirror that.
    pub fn low_health_alarm_active(&self) -> bool {
        if let Ok(mgr) = self.manager.lock() {
            mgr.low_health_alarm_active()
        } else {
            false
        }
    }

    pub fn stop_music(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.stop_music();
        }
    }

    pub fn fade_out(&self, fade_speed: u8) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.fade_out(fade_speed);
        }
    }

    pub fn stop_all(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.stop_all();
        }
    }

    pub fn last_music_id(&self) -> Option<MusicId> {
        if let Ok(mgr) = self.manager.lock() {
            mgr.last_music_id()
        } else {
            None
        }
    }

    pub fn update_frame(&self) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.update_frame();
        }
    }

    /// Resume the AudioContext if suspended (browsers require user gesture).
    pub fn try_resume(&self) {
        if self._ctx.state() == web_sys::AudioContextState::Suspended {
            let _ = self._ctx.resume();
        }
    }
}
