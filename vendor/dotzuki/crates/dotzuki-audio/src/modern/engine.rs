//! High-level facade for the `modern-audio` subsystem: BGM / SFX playback
//! on top of the [`Mixer`](crate::modern::mixer::Mixer).
//!
//! Owns the mixer and the current-music handle. `play_music` replaces any
//! previous BGM (cross-fading through its `fade_out`), `play_sfx` fires
//! one-shots, and bus/master levels + DSP are exposed directly.

use std::io::Cursor;

use symphonia::core::io::MediaSource;

use crate::modern::decode::{self, DecodeError, StreamInfo};
use crate::modern::mixer::{Bus, Mixer};

pub use crate::modern::mixer::PlayOptions;

/// The background music currently playing.
#[derive(Debug, Clone)]
pub struct MusicHandle {
    /// The mixer voice id (for stop/fade bookkeeping).
    voice_id: u64,
    /// Static facts about the opened stream.
    pub info: StreamInfo,
}

impl MusicHandle {
    /// Approximate duration in seconds, when the source exposes it.
    pub fn duration_secs(&self) -> Option<f64> {
        self.info.duration_secs()
    }
}

/// Modern audio: one mixer plus the BGM handle.
pub struct ModernAudio {
    mixer: Mixer,
    music: Option<MusicHandle>,
}

impl ModernAudio {
    /// Create the engine at `sample_rate` Hz output (44_100 or 48_000).
    pub fn new(sample_rate: u32) -> Self {
        Self {
            mixer: Mixer::new(sample_rate),
            music: None,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.mixer.sample_rate()
    }

    // ── BGM ──

    /// Start (or replace) the background music from a decoded source.
    ///
    /// The track always loops; any previous BGM is stopped with
    /// `opts.fade_out` (when given) while the new one fades in.
    pub fn play_music(
        &mut self,
        source: Box<dyn MediaSource>,
        ext_hint: Option<&str>,
        opts: PlayOptions,
    ) -> Result<(), DecodeError> {
        let dec = decode::open(source, ext_hint)?;
        let info = dec.info();
        if let Some(m) = &self.music {
            self.mixer.stop_voice(m.voice_id, opts.fade_out);
        }
        let opts = PlayOptions {
            loop_audio: true,
            ..opts
        };
        let voice_id = self.mixer.play(dec, Bus::Bgm, &opts);
        self.music = Some(MusicHandle { voice_id, info });
        Ok(())
    }

    /// Convenience: play BGM from raw file bytes (no I/O, VFS-friendly).
    pub fn play_music_bytes(
        &mut self,
        bytes: Vec<u8>,
        ext_hint: Option<&str>,
        opts: PlayOptions,
    ) -> Result<(), DecodeError> {
        self.play_music(Box::new(Cursor::new(bytes)), ext_hint, opts)
    }

    /// The handle of the current BGM, if any.
    pub fn current_music(&self) -> Option<&MusicHandle> {
        self.music.as_ref()
    }

    /// Stop the BGM — immediately, or with a fade-out when `fade_seconds`
    /// is given. The BGM slot is freed right away (a fading voice keeps
    /// playing out on the bus until it ends).
    pub fn stop_music(&mut self, fade_seconds: Option<f32>) {
        if let Some(m) = &self.music {
            self.mixer.stop_voice(m.voice_id, fade_seconds);
        }
        self.music = None;
    }

    // ── SFX ──

    /// Fire a one-shot sound effect. Returns the new voice id.
    pub fn play_sfx(
        &mut self,
        source: Box<dyn MediaSource>,
        ext_hint: Option<&str>,
        opts: PlayOptions,
    ) -> Result<u64, DecodeError> {
        let dec = decode::open(source, ext_hint)?;
        let opts = PlayOptions {
            loop_audio: false,
            ..opts
        };
        Ok(self.mixer.play(dec, Bus::Sfx, &opts))
    }

    /// Convenience: fire a one-shot from raw file bytes.
    pub fn play_sfx_bytes(
        &mut self,
        bytes: Vec<u8>,
        ext_hint: Option<&str>,
        opts: PlayOptions,
    ) -> Result<u64, DecodeError> {
        self.play_sfx(Box::new(Cursor::new(bytes)), ext_hint, opts)
    }

    /// Stop a specific voice (e.g. a lingering SFX) with an optional fade.
    pub fn stop_voice(&mut self, id: u64, fade_seconds: Option<f32>) {
        self.mixer.stop_voice(id, fade_seconds);
    }

    // ── Levels & DSP ──

    pub fn set_master_volume(&mut self, volume: f32) {
        self.mixer.set_master_volume(volume);
    }

    pub fn master_volume(&self) -> f32 {
        self.mixer.master_volume()
    }

    pub fn set_bus_volume(&mut self, bus: Bus, volume: f32) {
        self.mixer.set_bus_volume(bus, volume);
    }

    pub fn bus_volume(&self, bus: Bus) -> f32 {
        self.mixer.bus_volume(bus)
    }

    /// Ramp a bus volume towards `target` over `seconds`.
    pub fn fade_bus(&mut self, bus: Bus, target: f32, seconds: f32) {
        self.mixer.fade_bus(bus, target, seconds);
    }

    /// Attach/configure DSP (lowpass/reverb) on a bus.
    pub fn bus_dsp_mut(&mut self, bus: Bus) -> &mut crate::modern::dsp::DspChain {
        self.mixer.bus_dsp_mut(bus)
    }

    // ── Rendering ──

    /// Render `out.len() / 2` stereo frames (interleaved L/R f32).
    pub fn render_into(&mut self, out: &mut [f32]) {
        self.mixer.render_into(out);
    }

    /// Number of currently sounding voices (for introspection/tests).
    pub fn active_voices(&self) -> usize {
        self.mixer.active_voices()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_bytes_fail_cleanly() {
        let mut a = ModernAudio::new(44_100);
        a.play_music_bytes(vec![1, 2, 3], None, PlayOptions::default())
            .unwrap_err(); // garbage bytes must fail decode, not panic
        assert!(a.current_music().is_none());
    }

    #[test]
    fn music_handle_tracks_playback() {
        let mut a = ModernAudio::new(44_100);
        // Feed real WAV bytes so decode succeeds.
        let wav = crate::modern::decode::tests::make_wav(8000, 1);
        a.play_music_bytes(wav, Some("wav"), PlayOptions::default())
            .unwrap();
        let h = a.current_music().expect("music playing");
        assert_eq!(h.info.sample_rate, 8000);
        assert_eq!(a.active_voices(), 1);

        // Replacing stops the old voice (instant here) and starts a new one.
        let wav2 = crate::modern::decode::tests::make_wav(8000, 1);
        a.play_music_bytes(wav2, Some("wav"), PlayOptions::default())
            .unwrap();
        assert_eq!(a.active_voices(), 1, "old BGM voice replaced");

        a.stop_music(None);
        assert!(a.current_music().is_none());
        assert_eq!(a.active_voices(), 0);
    }

    #[test]
    fn sfx_one_shot_plays_and_ends() {
        let mut a = ModernAudio::new(44_100);
        let wav = crate::modern::decode::tests::make_wav(8000, 1);
        let id = a
            .play_sfx_bytes(wav, Some("wav"), PlayOptions::default())
            .unwrap();
        assert!(a.mixer.voice_playing(id));
        let mut out = vec![0.0f32; 44_100 * 2];
        a.render_into(&mut out);
        assert!(out.iter().any(|&s| s.abs() > 0.1), "sfx audible");
        assert_eq!(a.active_voices(), 0, "one-shot ended at EOF");
    }

    #[test]
    fn bus_fade_and_volumes_flow_through() {
        let mut a = ModernAudio::new(44_100);
        a.set_master_volume(0.5);
        a.set_bus_volume(Bus::Bgm, 0.5);
        let wav = crate::modern::decode::tests::make_wav(8000, 1);
        a.play_music_bytes(wav, Some("wav"), PlayOptions::default())
            .unwrap();
        let mut out = vec![0.0f32; 8000 * 2];
        // BGM looped: never EOFs within one second.
        a.render_into(&mut out);
        assert!(
            out.iter().any(|&s| s.abs() > 0.1),
            "still audible at 0.25 gain"
        );
        a.fade_bus(Bus::Bgm, 0.0, 0.05);
        let mut out2 = vec![0.0f32; 8000];
        a.render_into(&mut out2);
        assert!(out2[out2.len() - 2].abs() < 1e-3, "faded bus is silent");
    }
}
