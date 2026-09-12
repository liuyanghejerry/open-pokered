//! Shared device output for the pokered frontends: [`AudioOutput`].
//!
//! The actual device glue (cpal stream on native, Web Audio
//! `ScriptProcessorNode` in the browser) lives in the generic
//! [`dotzuki_audio::output`] backends; this module only wires the shared
//! [`AudioManager`] into them and re-exposes the manager's API behind the
//! frontend-facing mutex. Both pokered-app and pokered-tui use this instead
//! of carrying their own copies.
//!
//! Enabled per-target via the crate features: `cpal` on native targets,
//! `web-audio` on `wasm32`.

use std::sync::{Arc, Mutex};

use crate::audio_manager::AudioManager;
use crate::music_data::MusicId;
use crate::sfx_data::SfxId;

/// The shared, mutex-guarded audio manager plus the live device output.
/// Dropping `AudioOutput` stops the output stream.
pub struct AudioOutput {
    pub manager: Arc<Mutex<AudioManager>>,
    #[cfg(all(not(target_arch = "wasm32"), feature = "cpal"))]
    _output: Option<dotzuki_audio::output::CpalOutput>,
    #[cfg(all(target_arch = "wasm32", feature = "web-audio"))]
    _output: Option<dotzuki_audio::output::WebAudioOutput>,
}

impl AudioOutput {
    /// Create the shared manager (powering on the APU, NR52 bit 7) and open
    /// the device output. `None` when no output device/context is available —
    /// the caller continues silent.
    /// A powered-on game sound source without a platform device.
    pub fn new_pcm() -> Self {
        let mut manager = AudioManager::new();
        manager.apu.write_register(0xFF26, 0x80);
        Self {
            manager: Arc::new(Mutex::new(manager)),
            #[cfg(any(
                all(not(target_arch = "wasm32"), feature = "cpal"),
                all(target_arch = "wasm32", feature = "web-audio")
            ))]
            _output: None,
        }
    }
    pub fn new() -> Option<Self> {
        #[cfg(any(
            all(not(target_arch = "wasm32"), feature = "cpal"),
            all(target_arch = "wasm32", feature = "web-audio")
        ))]
        {
            let mut audio = Self::new_pcm();
            let manager = Arc::clone(&audio.manager);
            let source = move |out: &mut [f32], rate: u32| {
                let mut manager = manager.lock().unwrap();
                dotzuki_audio::output::render_apu_stereo(&mut manager.apu, out, rate);
            };
            #[cfg(not(target_arch = "wasm32"))]
            {
                audio._output = Some(dotzuki_audio::output::CpalOutput::new(source)?);
            }
            #[cfg(target_arch = "wasm32")]
            {
                audio._output = Some(dotzuki_audio::output::WebAudioOutput::new(source)?);
            }
            Some(audio)
        }
        #[cfg(not(any(
            all(not(target_arch = "wasm32"), feature = "cpal"),
            all(target_arch = "wasm32", feature = "web-audio")
        )))]
        {
            None
        }
    }
    /// Called by the embedding game thread, never alongside a live device output.
    pub fn render_pcm(&self, output: &mut [f32]) {
        let mut manager = self.manager.lock().unwrap();
        dotzuki_audio::output::render_apu_stereo(&mut manager.apu, output, 44100);
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

    pub fn play_flute_overworld(&self, resume_music: MusicId) {
        self.try_resume();
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.play_flute_overworld(resume_music);
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

    /// Resume the audio context if the browser suspended it (no-op on
    /// native — the stream is always running).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_resume(&self) {}

    /// Resume the AudioContext if suspended (browsers require a user
    /// gesture).
    #[cfg(target_arch = "wasm32")]
    pub fn try_resume(&self) {
        #[cfg(feature = "web-audio")]
        if let Some(ref output) = self._output {
            output.try_resume();
        }
    }
}
