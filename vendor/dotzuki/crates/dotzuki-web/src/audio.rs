//! Real-engine audio playback bridge for editors.
//!
//! Renders a file-based [`TrackDef`](dotzuki_audio::format::TrackDef) (the JSON the
//! dotzuki-editor audio activity edits) to PCM using the *actual* `dotzuki-audio`
//! sequencer + APU, so what the editor plays is exactly what the game plays —
//! no reimplementation, no drift. The editor feeds the returned samples into a
//! WebAudio `AudioBuffer`.

use wasm_bindgen::prelude::*;

use dotzuki_audio::apu::Apu;
use dotzuki_audio::format::TrackDef;
use dotzuki_audio::sequencer::Sequencer;
use dotzuki_audio::CPU_CLOCK_HZ;

/// Output sample rate for rendered PCM. Matches the reference native/iOS bridge.
pub const OUTPUT_SAMPLE_RATE: u32 = 48_000;

/// The sample rate (Hz) of the buffers returned by [`render_audio_pcm`].
#[wasm_bindgen]
pub fn audio_sample_rate() -> u32 {
    OUTPUT_SAMPLE_RATE
}

/// Render a track to interleaved **stereo `f32`** PCM at [`OUTPUT_SAMPLE_RATE`].
///
/// * `track_json` — a JSON [`TrackDef`] (music or SFX).
/// * `max_seconds` — hard cap on render length; music loops forever, so this
///   bounds it (SFX and non-looping tracks stop on their own, sooner).
///
/// Returns the raw little-endian bytes of the `f32` sample buffer (JS
/// reinterprets as a `Float32Array`: `[L0, R0, L1, R1, …]`). Returns an empty
/// buffer if the JSON fails to parse.
#[wasm_bindgen]
pub fn render_audio_pcm(track_json: &str, max_seconds: f32) -> Vec<u8> {
    let track: TrackDef = match serde_json::from_str(track_json) {
        Ok(t) => t,
        Err(e) => {
            crate::log_error(&format!("render_audio_pcm: bad track JSON: {e}"));
            return Vec::new();
        }
    };
    let samples = render_track_samples(&track, max_seconds);

    // Pack interleaved f32 as little-endian bytes for the wasm-bindgen boundary.
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// Render a real audio file (WAV / OGG-Vorbis / FLAC / MP3) to interleaved
/// stereo `f32` PCM at [`OUTPUT_SAMPLE_RATE`], using the actual
/// dotzuki-audio modern decoder + mixer. Requires the `modern-audio`
/// feature; without it the export is absent (the editor hides the file
/// preview UI accordingly).
///
/// * `bytes` — the raw file bytes (as fetched from the project VFS).
/// * `ext_hint` — file extension without the dot (e.g. `"ogg"`), used only
///   to speed up container detection.
/// * `max_seconds` — hard cap on render length (looping music is bounded).
///
/// Returns raw little-endian bytes of the `f32` buffer (`[L0, R0, L1, R1, …]`),
/// or an empty buffer when decoding fails.
#[cfg(feature = "modern-audio")]
#[wasm_bindgen]
pub fn render_file_audio(bytes: &[u8], ext_hint: &str, max_seconds: f32) -> Vec<u8> {
    use dotzuki_audio::modern::{ModernAudio, PlayOptions};

    let mut audio = ModernAudio::new(OUTPUT_SAMPLE_RATE);
    let opts = PlayOptions {
        volume: 1.0,
        pan: 0.0,
        fade_in: None,
        fade_out: None,
        // Preview a finite slice: no looping, cap the length.
        loop_audio: false,
    };
    let ext = if ext_hint.is_empty() {
        None
    } else {
        Some(ext_hint)
    };
    if audio.play_sfx_bytes(bytes.to_vec(), ext, opts).is_err() {
        crate::log_error("render_file_audio: failed to decode audio file");
        return Vec::new();
    }
    let frames = (max_seconds.max(0.0) * OUTPUT_SAMPLE_RATE as f32) as usize;
    let mut pcm = vec![0.0f32; frames * 2];
    audio.render_into(&mut pcm);
    // Trim trailing silence so the preview buffer matches the real length.
    if let Some(last) = pcm.iter().rposition(|&s| s.abs() > 1e-5) {
        pcm.truncate(((last / 2) + 1) * 2);
    } else {
        pcm.clear();
    }
    let mut bytes_out = Vec::with_capacity(pcm.len() * 4);
    for s in pcm {
        bytes_out.extend_from_slice(&s.to_le_bytes());
    }
    bytes_out
}

/// Core render loop, shared by the wasm export and native tests.
///
/// Mirrors the shipping playback path (`pokered-ios`): power on the APU, start
/// the track on the sequencer, then per 1/60 s frame advance the sequencer and
/// pull `OUTPUT_SAMPLE_RATE / 60` stereo samples from the APU.
fn render_track_samples(track: &TrackDef, max_seconds: f32) -> Vec<f32> {
    let mut seq = Sequencer::new();
    let mut apu = Apu::new();
    // Power on. new() leaves NR50=0x77 / NR51=0xFF, and power_on() preserves
    // them, so master volume and panning are audible; the sequencer rewrites
    // NR51 each frame from the active channels.
    apu.write_register(0xFF26, 0x80);

    track.play_on(&mut seq);

    let cycles_per_sample = CPU_CLOCK_HZ / OUTPUT_SAMPLE_RATE;
    let samples_per_frame = (OUTPUT_SAMPLE_RATE / 60) as usize;
    let max_frames = (max_seconds.max(0.0) * 60.0).ceil() as usize;

    let mut out = Vec::with_capacity(samples_per_frame * max_frames * 2);
    let mut frame = 0;
    while seq.is_playing() && frame < max_frames {
        seq.update_frame(&mut apu);
        for _ in 0..samples_per_frame {
            apu.tick_n(cycles_per_sample);
            let (l, r) = apu.mix_sample();
            // mix_sample returns non-negative i16 in ~0..=480; scale to f32.
            out.push(l as f32 / 480.0);
            out.push(r as f32 / 480.0);
        }
        frame += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_audio::format::{AudioCommand, ChannelDef, TrackDef, TrackKind};
    use dotzuki_audio::HwChannel;

    fn demo_track() -> TrackDef {
        TrackDef {
            id: "Demo".into(),
            kind: TrackKind::Music,
            name: None,
            tempo: 0x0100,
            channels: vec![ChannelDef {
                hw: HwChannel::Pulse1,
                commands: vec![
                    AudioCommand::DutyCycle { value: 2 },
                    AudioCommand::NoteType {
                        speed: 8,
                        param: 0xF0,
                    }, // vol 15, no fade
                    AudioCommand::Octave { value: 4 },
                    AudioCommand::Note {
                        pitch: 0,
                        length: 16,
                    },
                ],
            }],
        }
    }

    #[test]
    fn renders_non_silent_pcm() {
        let samples = render_track_samples(&demo_track(), 0.25);
        assert!(!samples.is_empty(), "expected some samples");
        // Interleaved stereo → even length.
        assert_eq!(samples.len() % 2, 0);
        // A triggered note must produce at least one non-zero sample.
        assert!(
            samples.iter().any(|&s| s.abs() > 0.0),
            "rendered audio was completely silent"
        );
    }

    #[test]
    fn pcm_byte_export_is_f32_sized() {
        let json = serde_json::to_string(&demo_track()).unwrap();
        let bytes = render_audio_pcm(&json, 0.1);
        assert!(!bytes.is_empty());
        assert_eq!(bytes.len() % 4, 0, "f32 byte buffer must be 4-aligned");
    }

    #[test]
    fn bad_json_returns_empty() {
        assert!(render_audio_pcm("not json", 1.0).is_empty());
    }

    #[cfg(feature = "modern-audio")]
    #[test]
    fn renders_file_audio_to_pcm() {
        // Minimal 16-bit PCM mono WAV: 1 s, 8 kHz, 440 Hz sine.
        let rate = 8000u32;
        let samples = rate;
        let data_len = samples * 2;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..samples {
            let v = (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / rate as f32).sin();
            wav.extend_from_slice(&((v * 32000.0) as i16).to_le_bytes());
        }

        let bytes = render_file_audio(&wav, "wav", 2.0);
        assert!(!bytes.is_empty(), "WAV must render");
        assert_eq!(bytes.len() % 4, 0, "f32 buffer is 4-aligned");
        let pcm: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        // Trimmed to the real ~1 s length at 48 kHz (plus tiny slack).
        assert!(pcm.len() / 2 <= 48_000 + 512, "no trailing silence");
        assert!(
            pcm.iter().any(|&s| s.abs() > 0.1),
            "rendered file audio must be audible"
        );
    }

    #[cfg(feature = "modern-audio")]
    #[test]
    fn render_file_audio_rejects_garbage() {
        assert!(render_file_audio(b"not audio at all", "wav", 1.0).is_empty());
    }
}
