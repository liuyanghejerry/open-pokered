//! Audio playback for `dotzuki run`: [`RunnerAudio`].
//!
//! Backs the scene commands `PlayMusic` / `PlaySound` / `StopMusic` /
//! `FadeOutMusic`. Track ids are looked up in two libraries, in order:
//!
//! 1. dotzuki-audio [`TrackDef`](dotzuki_audio::format::TrackDef) JSON files
//!    under `<dataRoot>/audio/` (loaded recursively, so the `music/` +
//!    `sfx/` split is a convention, not a rule); the id is the track's
//!    `id` field;
//! 2. — with the `modern-audio` feature — real audio files (`*.wav`,
//!    `*.ogg`, `*.flac`, `*.mp3`) under the same tree; the id is the path
//!    relative to `audio/` without its extension, e.g. `playMusic("music/town")`
//!    plays `data/audio/music/town.ogg`. Such tracks stream through the
//!    dotzuki-audio `modern` mixer (BGM loops, SFX is one-shot), mixed with
//!    the chiptune APU output.
//!
//! Audio is **fully optional**:
//!
//! - no `data/audio/` dir (scaffolded projects) → empty library, every
//!   command is a silent no-op (debug-logged), cpal is never touched;
//! - the cpal output stream is initialised **lazily** — only when the
//!   library is non-empty *and* a play command actually arrives;
//! - no output device (CI / headless) or a stream failure → one warning,
//!   permanent silent mode, the game keeps running;
//! - headless runs pass `allow_device: false` and never init a device.
//!
//! Threading follows `pokered-app`/`wuxia-app`: the emulated APU is shared
//! with cpal's callback thread via a mutex; the callback advances it
//! (`tick_n`) and reads one stereo sample (`mix_sample`) per output frame.
//! The game thread only advances the *sequencer* once per video frame
//! ([`update_frame`](Self::update_frame)) and mutates playback via
//! `play_music`/`play_sound`/`stop_music`/`fade_out_music`.
//!
//! ## PCM render mode (WASM / callback-less hosts)
//!
//! On hosts where cpal has no real output (the browser's Null host), the
//! push model above never runs: there is no callback thread to drive the
//! APU. [`set_pcm_render`](Self::set_pcm_render) switches to a pull model:
//! play commands still create the shared engine (without touching cpal),
//! and the host pulls samples with [`render_samples`](Self::render_samples),
//! feeding them to e.g. a WebAudio `AudioBuffer`. Sample generation is the
//! same `tick_n` + `mix_sample` path the cpal callback uses (plus the
//! modern mixer when enabled), and `update_frame` still advances the
//! sequencer/fade once per video frame, so music, dedup, and fades behave
//! exactly as on native.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use dotzuki_audio::apu::Apu;
use dotzuki_audio::format::TrackDef;
use dotzuki_audio::library::AudioLibrary;
use dotzuki_audio::output::{render_apu_stereo, CpalOutput};
use dotzuki_audio::sequencer::Sequencer;

#[cfg(feature = "modern-audio")]
use dotzuki_audio::modern::{Bus as ModernBus, ModernAudio, PlayOptions as ModernPlayOptions};

use crate::vfs::{join_path, DiskFiles, ProjectFiles};

/// Output rate of the cpal stream (Hz). The GB APU is resampled to this by
/// ticking `CPU_CLOCK_HZ / SAMPLE_RATE` cycles per output sample
/// (`dotzuki_audio::output::render_apu_stereo`).
const SAMPLE_RATE: u32 = dotzuki_audio::SAMPLE_RATE;

/// Full master volume (NR50 per-side range is 0-7).
const FULL_VOLUME: u8 = 7;

/// Video frames between master-volume steps during a fade-out. A fade walks
/// volume 7→0 (8 audible levels), so total ≈ `FADE_STEP_FRAMES * 7` frames
/// (~1.2 s at 60 fps) before the music is cut.
const FADE_STEP_FRAMES: u8 = 10;

/// Music fade-out state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fade {
    None,
    /// Fading out: `counter` frames until the next volume step down.
    Out {
        counter: u8,
        reload: u8,
    },
}

/// Advance a fade by one frame.
///
/// Returns `(next_fade, next_master_volume, completed)`. When `completed` is
/// true the caller should stop the music; the returned volume is reset to
/// full so the next track starts at normal level. Pure, so the state machine
/// is unit-tested without a device.
fn step_fade(fade: Fade, master_volume: u8) -> (Fade, u8, bool) {
    match fade {
        Fade::None => (Fade::None, master_volume, false),
        // Time to step the volume down.
        Fade::Out { counter: 0, reload } => {
            let mv = master_volume.saturating_sub(1);
            if mv == 0 {
                (Fade::None, FULL_VOLUME, true) // faded out — stop, restore volume
            } else {
                (
                    Fade::Out {
                        counter: reload,
                        reload,
                    },
                    mv,
                    false,
                )
            }
        }
        // Still counting down to the next step.
        Fade::Out { counter, reload } => (
            Fade::Out {
                counter: counter - 1,
                reload,
            },
            master_volume,
            false,
        ),
    }
}

/// The APU + sequencer + playback state, shared between the game thread and
/// the audio callback thread behind a single mutex.
struct Engine {
    seq: Sequencer,
    apu: Apu,
    /// Current master volume (0-7), stamped onto NR50 each frame so a fade
    /// takes audible effect and in-stream volume writes don't fight it.
    master_volume: u8,
    fade: Fade,
    /// The music track currently requested, for dedup (don't restart BGM
    /// every frame / every map re-entry).
    current_music: Option<String>,
    /// Modern file-audio mixer (lazy; `None` until a file track plays).
    #[cfg(feature = "modern-audio")]
    modern: Option<ModernAudio>,
    /// The file-audio track currently requested (dedup like `current_music`).
    #[cfg(feature = "modern-audio")]
    modern_music: Option<String>,
    /// Reusable overlay buffer for the modern mixer (avoids per-callback
    /// allocation in `render_into`).
    #[cfg(feature = "modern-audio")]
    mix_buf: Vec<f32>,
}

/// Generate stereo samples from the engine into `data` (interleaved L/R,
/// `SAMPLE_RATE` Hz, normalised to `[-1.0, 1.0]`). Shared by the cpal output
/// callback and [`RunnerAudio::render_samples`] so both paths produce
/// byte-identical audio.
fn render_into(e: &mut Engine, data: &mut [f32]) {
    render_apu_stereo(&mut e.apu, data, SAMPLE_RATE);
    #[cfg(feature = "modern-audio")]
    if let Some(modern) = &mut e.modern {
        e.mix_buf.resize(data.len(), 0.0);
        e.mix_buf.fill(0.0);
        modern.render_into(&mut e.mix_buf);
        for (out, modern_s) in data.iter_mut().zip(e.mix_buf.iter()) {
            *out += *modern_s;
        }
    }
}

/// Advance the music/SFX sequencer one video frame and step any active fade.
/// Sample *generation* happens separately (cpal callback or `render_samples`).
fn update_engine_frame(e: &mut Engine) {
    // Advance the fade (if any) before the sequencer runs.
    let (fade, mv, completed) = step_fade(e.fade, e.master_volume);
    e.fade = fade;
    e.master_volume = mv;
    if completed {
        e.seq.stop_music();
        e.current_music = None;
    }

    #[cfg(feature = "modern-audio")]
    {
        // A completed chiptune fade also stops modern file music (fades are
        // issued per-kind by the runner, but a fade that completed here
        // means the caller wanted silence).
        if completed {
            if let Some(modern) = &mut e.modern {
                modern.stop_music(None);
            }
            e.modern_music = None;
        }
    }

    // Advance the sequencer, then stamp master volume onto NR50 (bits
    // 6-4 = left, 2-0 = right) so the fade is audible and outlives
    // in-stream writes.
    let Engine {
        seq,
        apu,
        master_volume,
        ..
    } = e;
    seq.update_frame(apu);
    let v = *master_volume & 0x07;
    apu.write_register(0xFF24, (v << 4) | v);
}

/// Create the shared engine: a powered-on APU plus a fresh sequencer.
fn new_engine() -> Arc<Mutex<Engine>> {
    let mut apu = Apu::new();
    apu.write_register(0xFF26, 0x80); // NR52: power on (else register writes are ignored)
    Arc::new(Mutex::new(Engine {
        seq: Sequencer::new(),
        apu,
        master_volume: FULL_VOLUME,
        fade: Fade::None,
        current_music: None,
        #[cfg(feature = "modern-audio")]
        modern: None,
        #[cfg(feature = "modern-audio")]
        modern_music: None,
        #[cfg(feature = "modern-audio")]
        mix_buf: Vec::new(),
    }))
}

/// Open the default output device and start streaming from `engine`. Returns
/// the live output (kept alive for the lifetime of playback; dropping it
/// stops the stream), or `None` when no device is available or the stream
/// cannot be built.
fn open_stream(engine: Arc<Mutex<Engine>>) -> Option<CpalOutput> {
    CpalOutput::new(move |data: &mut [f32], _sample_rate: u32| {
        let mut e = engine.lock().unwrap();
        render_into(&mut e, data);
    })
}

/// A file-backed audio track (loaded into memory as compressed bytes; the
/// decoder streams them, so PCM is never fully materialised).
#[cfg(feature = "modern-audio")]
struct FileTrack {
    bytes: Vec<u8>,
    ext: String,
}

/// Audio for a [`crate::game::RunnerGame`]: an [`AudioLibrary`] plus a lazily
/// initialised engine, driven either by a cpal output stream (native) or by
/// PCM pull-rendering (WASM). Silent by default; every method is a safe
/// no-op in silent mode.
pub struct RunnerAudio {
    library: AudioLibrary,
    /// File-audio tracks keyed by their extension-stripped `audio/`-relative
    /// path (e.g. `"music/town"`), only with the `modern-audio` feature.
    #[cfg(feature = "modern-audio")]
    file_tracks: BTreeMap<String, FileTrack>,
    /// `false` on headless runs — never open a device.
    allow_device: bool,
    /// PCM pull-render mode: play commands create the engine without a
    /// device, and the host pulls samples via [`render_samples`](Self::render_samples).
    pcm_render: bool,
    /// The shared engine; `None` until the first play command. Created
    /// together with the cpal stream on native, stand-alone in PCM mode.
    engine: Option<Arc<Mutex<Engine>>>,
    /// The live cpal output, kept alive for the lifetime of playback;
    /// dropping it stops the stream. Always `None` in PCM/headless mode.
    stream: Option<CpalOutput>,
    /// Set once an init attempt failed: don't retry, stay silent.
    init_failed: bool,
    /// Track ids already warned about (unknown ids warn once each).
    warned_ids: HashSet<String>,
}

impl RunnerAudio {
    /// Load every track under `<data_root>/audio/` (recursively) from disk.
    /// Convenience for [`from_files`](Self::from_files) over a [`DiskFiles`]
    /// rooted at `data_root`.
    pub fn new(data_root: &Path, allow_device: bool) -> Self {
        Self::from_files(&DiskFiles::new(data_root), "", allow_device)
    }

    /// VFS form of [`new`](Self::new): load every track under
    /// `<data_root_rel>/audio/` (recursively). A missing directory or an
    /// unloadable library yields an empty library — silent mode, not an
    /// error.
    pub fn from_files(files: &dyn ProjectFiles, data_root_rel: &str, allow_device: bool) -> Self {
        let prefix = join_path(data_root_rel, "audio");
        let library = load_library(files, &prefix).unwrap_or_else(|e| {
            log::warn!("audio: failed to load {prefix}: {e:#}");
            AudioLibrary::new()
        });
        if !library.is_empty() {
            log::info!("audio: loaded {} track(s) from {prefix}", library.len());
        }
        #[cfg(feature = "modern-audio")]
        let file_tracks = load_file_tracks(files, &prefix).unwrap_or_else(|e| {
            log::warn!("audio: failed to load file tracks from {prefix}: {e:#}");
            BTreeMap::new()
        });
        #[cfg(feature = "modern-audio")]
        if !file_tracks.is_empty() {
            log::info!(
                "audio: loaded {} file track(s) from {prefix}",
                file_tracks.len()
            );
        }
        Self {
            library,
            #[cfg(feature = "modern-audio")]
            file_tracks,
            allow_device,
            pcm_render: false,
            engine: None,
            stream: None,
            init_failed: false,
            warned_ids: HashSet::new(),
        }
    }

    /// Number of loaded JSON tracks (0 ⇒ every command is a silent no-op).
    pub fn track_count(&self) -> usize {
        self.library.len()
    }

    /// Whether a track with this id exists in either library (JSON first,
    /// then files, when the `modern-audio` feature is on).
    pub fn has_track(&self, id: &str) -> bool {
        self.library.get(id).is_some() || {
            #[cfg(feature = "modern-audio")]
            {
                self.file_tracks.contains_key(id)
            }
            #[cfg(not(feature = "modern-audio"))]
            {
                false
            }
        }
    }

    /// Whether the cpal output stream is live (test/debug introspection).
    pub fn has_output(&self) -> bool {
        self.stream.is_some()
    }

    /// Switch PCM pull-render mode on/off (see the module docs). In PCM mode
    /// play commands create the engine even with no usable output device —
    /// cpal is never touched — and the host pulls samples via
    /// [`render_samples`](Self::render_samples). Off by default (native).
    pub fn set_pcm_render(&mut self, on: bool) {
        self.pcm_render = on;
    }

    /// Render `frames` stereo PCM frames from the engine: interleaved L/R
    /// `f32` samples (length `2 * frames`) at 44100 Hz, normalised exactly
    /// like the cpal callback. Returns an empty `Vec` when no engine exists
    /// yet (silent mode, or no play command has arrived).
    ///
    /// Each call advances the APU — call this instead of (never in addition
    /// to) a live output stream, and advance the sequencer with
    /// [`update_frame`](Self::update_frame) once per video frame as usual.
    pub fn render_samples(&mut self, frames: usize) -> Vec<f32> {
        let Some(engine) = &self.engine else {
            return Vec::new();
        };
        let mut e = engine.lock().unwrap();
        let mut data = vec![0.0; frames * 2];
        render_into(&mut e, &mut data);
        data
    }

    /// The shared engine, lazily initialised on first use. Returns `None`
    /// (staying silent) when every library is empty or — outside PCM mode —
    /// the device is disallowed or init has failed before; an init failure
    /// is warned about once. On native the engine and the cpal stream are
    /// created together; in PCM mode the engine stands alone.
    fn engine(&mut self) -> Option<Arc<Mutex<Engine>>> {
        if let Some(engine) = &self.engine {
            return Some(Arc::clone(engine));
        }
        if self.library.is_empty() && {
            #[cfg(feature = "modern-audio")]
            {
                self.file_tracks.is_empty()
            }
            #[cfg(not(feature = "modern-audio"))]
            {
                true
            }
        } {
            return None;
        }
        if self.pcm_render {
            log::info!("audio: pcm render mode — engine started without an output device");
            let engine = new_engine();
            self.engine = Some(Arc::clone(&engine));
            return Some(engine);
        }
        if !self.allow_device || self.init_failed {
            return None;
        }
        let engine = new_engine();
        match open_stream(Arc::clone(&engine)) {
            Some(stream) => {
                log::info!("audio: output stream started");
                self.stream = Some(stream);
                self.engine = Some(Arc::clone(&engine));
                Some(engine)
            }
            None => {
                log::warn!("audio: no output device — sound disabled, continuing silent");
                self.init_failed = true;
                None
            }
        }
    }

    /// Warn about an unknown track id, once per id.
    fn warn_unknown(&mut self, kind: &str, id: &str) {
        if self.warned_ids.insert(id.to_string()) {
            log::warn!("audio: no {kind} track '{id}'");
        }
    }

    /// Advance the sequencer one video frame (called from
    /// [`RunnerGame::update`](crate::game::RunnerGame::update)); a no-op in
    /// silent mode. Runs against the engine, so the sequencer and fades
    /// advance in PCM render mode too.
    pub fn update_frame(&mut self) {
        if let Some(engine) = &self.engine {
            update_engine_frame(&mut engine.lock().unwrap());
        }
    }

    /// Start a background-music track by id. No-op if that track is already
    /// the active BGM (re-entering a map doesn't restart its theme). Cancels
    /// any in-progress fade and restores full volume.
    ///
    /// JSON [`TrackDef`] tracks take precedence; with the `modern-audio`
    /// feature, ids that match a file track (extension-stripped
    /// `audio/`-relative path) play as looping streamed audio instead.
    pub fn play_music(&mut self, id: &str) {
        if !self.has_track(id) {
            self.warn_unknown("music", id);
            return;
        }
        let Some(engine) = self.engine() else {
            return;
        };
        let mut e = engine.lock().unwrap();
        if self.library.get(id).is_some() {
            if e.current_music.as_deref() == Some(id) {
                return;
            }
            e.current_music = Some(id.to_string());
            e.fade = Fade::None;
            e.master_volume = FULL_VOLUME;
            // Existence was checked above; play() only fails on an unknown id.
            self.library.play(&mut e.seq, id);
            return;
        }
        #[cfg(feature = "modern-audio")]
        {
            if e.modern_music.as_deref() == Some(id) {
                return;
            }
            let Some(track) = self.file_tracks.get(id) else {
                return;
            };
            // Replacing a JSON BGM? Stop the chiptune side too, so a single
            // music command owns the music slot.
            e.seq.stop_music();
            e.current_music = None;
            let modern = e
                .modern
                .get_or_insert_with(|| ModernAudio::new(SAMPLE_RATE));
            if modern
                .play_music_bytes(
                    track.bytes.clone(),
                    Some(&track.ext),
                    ModernPlayOptions::default(),
                )
                .is_ok()
            {
                e.modern_music = Some(id.to_string());
            } else {
                log::warn!("audio: failed to decode music track '{id}'");
            }
        }
        #[cfg(not(feature = "modern-audio"))]
        {
            log::warn!("audio: music track '{id}' exists but modern-audio is not enabled");
        }
    }

    /// Play a one-shot sound effect by id (always retriggers). JSON tracks
    /// take precedence; file tracks play as one-shots on the SFX bus.
    pub fn play_sound(&mut self, id: &str) {
        if !self.has_track(id) {
            self.warn_unknown("sfx", id);
            return;
        }
        let Some(engine) = self.engine() else {
            return;
        };
        let mut e = engine.lock().unwrap();
        if self.library.get(id).is_some() {
            self.library.play(&mut e.seq, id);
            return;
        }
        #[cfg(feature = "modern-audio")]
        {
            let Some(track) = self.file_tracks.get(id) else {
                return;
            };
            let modern = e
                .modern
                .get_or_insert_with(|| ModernAudio::new(SAMPLE_RATE));
            if modern
                .play_sfx_bytes(
                    track.bytes.clone(),
                    Some(&track.ext),
                    ModernPlayOptions::default(),
                )
                .is_err()
            {
                log::warn!("audio: failed to decode sfx track '{id}'");
            }
        }
        #[cfg(not(feature = "modern-audio"))]
        {
            log::warn!("audio: sfx track '{id}' exists but modern-audio is not enabled");
        }
    }

    /// Stop all background music immediately (chiptune and file audio).
    pub fn stop_music(&mut self) {
        let Some(engine) = &self.engine else {
            return;
        };
        let mut e = engine.lock().unwrap();
        e.seq.stop_music();
        e.fade = Fade::None;
        e.master_volume = FULL_VOLUME;
        e.current_music = None;
        #[cfg(feature = "modern-audio")]
        {
            if let Some(modern) = &mut e.modern {
                modern.stop_music(None);
            }
            e.modern_music = None;
        }
    }

    /// Begin fading the current music out to silence, then stop it. No-op if
    /// no music is playing or a fade is already under way. Fades whichever
    /// kind of music is actually playing (chiptune fades through the APU
    /// master volume; file music fades through the modern mixer).
    pub fn fade_out_music(&mut self) {
        let Some(engine) = &self.engine else {
            return;
        };
        let mut e = engine.lock().unwrap();
        #[cfg(feature = "modern-audio")]
        if e.modern_music.is_some() {
            if let Some(modern) = &mut e.modern {
                modern.stop_music(Some(MODERN_FADE_SECS));
            }
            e.modern_music = None;
            return;
        }
        if e.current_music.is_none() || matches!(e.fade, Fade::Out { .. }) {
            return;
        }
        e.fade = Fade::Out {
            counter: FADE_STEP_FRAMES,
            reload: FADE_STEP_FRAMES,
        };
    }
}

/// Fade-out duration for modern file music (≈ the chiptune fade's ~1.2 s).
#[cfg(feature = "modern-audio")]
const MODERN_FADE_SECS: f32 = 1.2;

/// Load every `*.json` track under `prefix` (recursively) through the VFS
/// into an [`AudioLibrary`] — the [`ProjectFiles`] counterpart of
/// `AudioLibrary::load_dir`. A missing prefix yields an empty library; a
/// track that fails to parse aborts the whole load with an error naming the
/// file (same bar as the disk loader).
fn load_library(files: &dyn ProjectFiles, prefix: &str) -> anyhow::Result<AudioLibrary> {
    let mut lib = AudioLibrary::new();
    for path in files.list(prefix) {
        if path.rsplit('.').next() != Some("json") {
            continue;
        }
        let bytes = files.read(&path)?;
        let track: TrackDef =
            serde_json::from_slice(&bytes).map_err(|e| anyhow::anyhow!("{path}: {e}"))?;
        lib.insert(track);
    }
    Ok(lib)
}

/// File extensions the modern decoder can play.
#[cfg(feature = "modern-audio")]
const AUDIO_FILE_EXTS: [&str; 4] = ["wav", "ogg", "flac", "mp3"];

/// Load every playable audio file under `prefix` (recursively) through the
/// VFS. Track ids are the extension-stripped `prefix`-relative path, e.g.
/// `"data/audio/music/town.ogg"` → `"music/town"`.
#[cfg(feature = "modern-audio")]
fn load_file_tracks(
    files: &dyn ProjectFiles,
    prefix: &str,
) -> anyhow::Result<BTreeMap<String, FileTrack>> {
    let mut out = BTreeMap::new();
    for path in files.list(prefix) {
        let ext = match path.rsplit('.').next() {
            Some(e) if AUDIO_FILE_EXTS.contains(&e) => e.to_string(),
            _ => continue,
        };
        let bytes = files.read(&path)?;
        let id = path
            .strip_prefix(&format!("{prefix}/"))
            .unwrap_or(&path)
            .trim_end_matches(&format!(".{ext}"))
            .to_string();
        out.insert(id, FileTrack { bytes, ext });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::MemoryFiles;

    /// A minimal valid music track (dotzuki-audio `TrackDef` JSON).
    const THEME_JSON: &str = r#"{
      "id": "theme",
      "kind": "music",
      "tempo": 256,
      "channels": [
        {
          "hw": "pulse1",
          "commands": [
            { "type": "note_type", "speed": 12, "param": 197 },
            { "type": "octave", "value": 5 },
            { "type": "note", "pitch": 0, "length": 4 },
            { "type": "rest", "length": 4 },
            { "type": "sound_ret" }
          ]
        }
      ]
    }"#;

    /// A `RunnerAudio` in PCM render mode with a one-track library and no
    /// device access — the WASM shell setup.
    fn pcm_audio() -> RunnerAudio {
        let track: TrackDef = serde_json::from_str(THEME_JSON).unwrap();
        let mut library = AudioLibrary::new();
        library.insert(track);
        RunnerAudio {
            library,
            #[cfg(feature = "modern-audio")]
            file_tracks: BTreeMap::new(),
            allow_device: false,
            pcm_render: true,
            engine: None,
            stream: None,
            init_failed: false,
            warned_ids: HashSet::new(),
        }
    }

    #[test]
    fn fade_none_is_inert() {
        assert_eq!(step_fade(Fade::None, 5), (Fade::None, 5, false));
    }

    #[test]
    fn fade_counts_down_then_steps_volume() {
        // reload 2: counter 2 → 1 → 0 (holds volume), then the 0 tick steps it.
        let (f, v, done) = step_fade(
            Fade::Out {
                counter: 2,
                reload: 2,
            },
            7,
        );
        assert_eq!(
            (f, v, done),
            (
                Fade::Out {
                    counter: 1,
                    reload: 2
                },
                7,
                false
            )
        );
        let (f, v, done) = step_fade(f, v);
        assert_eq!(
            (f, v, done),
            (
                Fade::Out {
                    counter: 0,
                    reload: 2
                },
                7,
                false
            )
        );
        let (f, v, done) = step_fade(f, v);
        assert_eq!(
            (f, v, done),
            (
                Fade::Out {
                    counter: 2,
                    reload: 2
                },
                6,
                false
            )
        );
    }

    #[test]
    fn fade_completes_and_restores_full_volume() {
        // Drive a whole fade from full volume with the fastest reload.
        let mut fade = Fade::Out {
            counter: 0,
            reload: 0,
        };
        let mut vol = FULL_VOLUME;
        let mut completed = false;
        for _ in 0..64 {
            let (f, v, done) = step_fade(fade, vol);
            fade = f;
            vol = v;
            if done {
                completed = true;
                break;
            }
        }
        assert!(completed, "fade never completed");
        assert_eq!(fade, Fade::None);
        assert_eq!(vol, FULL_VOLUME, "volume should reset to full after a fade");
    }

    #[test]
    fn render_samples_is_empty_before_any_play() {
        let mut audio = pcm_audio();
        assert!(audio.render_samples(4410).is_empty());
        // Silent (non-PCM) mode never creates an engine either.
        let mut silent = RunnerAudio {
            pcm_render: false,
            ..pcm_audio()
        };
        silent.play_music("theme");
        assert!(silent.render_samples(4410).is_empty());
        assert!(!silent.has_output());
    }

    #[test]
    fn pcm_play_renders_nonzero_samples_without_a_device() {
        let mut audio = pcm_audio();
        audio.play_music("theme");
        assert!(!audio.has_output(), "pcm mode must not open a device");
        assert!(
            audio.engine.is_some(),
            "pcm mode creates the engine on play"
        );

        // A few video frames so the sequencer triggers the first note.
        for _ in 0..5 {
            audio.update_frame();
        }
        let pcm = audio.render_samples(4410);
        assert_eq!(pcm.len(), 8820, "stereo frames: 2 * frames");
        assert!(
            pcm.iter().any(|s| *s != 0.0),
            "playing music must render non-silent samples"
        );
    }

    #[test]
    fn pcm_play_music_dedups_like_native() {
        let mut audio = pcm_audio();
        audio.play_music("theme");
        audio.update_frame();
        {
            let e = audio.engine.as_ref().unwrap().lock().unwrap();
            assert_eq!(e.current_music.as_deref(), Some("theme"));
        }
        // Re-requesting the same track must not restart it…
        audio.play_music("theme");
        let e = audio.engine.as_ref().unwrap().lock().unwrap();
        assert_eq!(e.current_music.as_deref(), Some("theme"));
        assert_eq!(e.master_volume, FULL_VOLUME);
        assert_eq!(e.fade, Fade::None);
    }

    #[test]
    fn pcm_fade_out_completes_and_stops_music() {
        let mut audio = pcm_audio();
        audio.play_music("theme");
        for _ in 0..5 {
            audio.update_frame();
        }
        assert!(audio.render_samples(4410).iter().any(|s| *s != 0.0));

        audio.fade_out_music();
        // FADE_STEP_FRAMES * 7 volume steps plus slack to run the fade out.
        for _ in 0..200 {
            audio.update_frame();
        }
        let e = audio.engine.as_ref().unwrap().lock().unwrap();
        assert_eq!(e.fade, Fade::None, "fade state machine completed");
        assert!(e.current_music.is_none(), "music stopped after fade-out");
        assert_eq!(
            e.master_volume, FULL_VOLUME,
            "volume restored for next track"
        );
    }

    // ── Modern file audio (feature `modern-audio`) ───────────────────────

    /// A tiny 16-bit PCM mono WAV (the modern file-audio counterpart of
    /// `THEME_JSON`), so file-track tests never touch the real device.
    #[cfg(feature = "modern-audio")]
    fn test_wav_bytes() -> Vec<u8> {
        let rate = 8000u32;
        let seconds = 1u32;
        let samples = rate * seconds;
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
        wav
    }

    /// A `RunnerAudio` loaded from an in-memory project containing one file
    /// track at `data/audio/music/town.wav` (PCM render, no device).
    #[cfg(feature = "modern-audio")]
    fn file_audio() -> RunnerAudio {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert("data/audio/music/town.wav".to_string(), test_wav_bytes());
        let files = MemoryFiles::from(map);
        let mut audio = RunnerAudio::from_files(&files, "data", false);
        audio.set_pcm_render(true);
        audio
    }

    #[cfg(feature = "modern-audio")]
    #[test]
    fn file_tracks_load_with_path_ids() {
        let audio = file_audio();
        assert!(audio.has_track("music/town"), "extension-stripped path id");
        assert!(!audio.has_track("town"), "no bare filename ids");
    }

    #[cfg(feature = "modern-audio")]
    #[test]
    fn file_music_plays_and_renders_audio() {
        let mut audio = file_audio();
        audio.play_music("music/town");
        assert!(audio.engine.is_some(), "engine created on play");
        for _ in 0..3 {
            audio.update_frame();
        }
        let pcm = audio.render_samples(8000);
        assert_eq!(pcm.len(), 16_000, "stereo: 2 * frames");
        assert!(
            pcm.iter().any(|s| *s != 0.0),
            "file BGM must render non-silent samples"
        );
        // Still playing after one full second (loops).
        let e = audio.engine.as_ref().unwrap().lock().unwrap();
        assert_eq!(e.modern_music.as_deref(), Some("music/town"));
    }

    #[cfg(feature = "modern-audio")]
    #[test]
    fn file_music_dedups_and_stops() {
        let mut audio = file_audio();
        audio.play_music("music/town");
        audio.play_music("music/town"); // must not restart
        {
            let e = audio.engine.as_ref().unwrap().lock().unwrap();
            assert_eq!(e.modern_music.as_deref(), Some("music/town"));
            assert_eq!(e.modern.as_ref().unwrap().active_voices(), 1);
        } // drop the guard before mutating audio again

        audio.stop_music();
        let e = audio.engine.as_ref().unwrap().lock().unwrap();
        assert!(e.modern_music.is_none(), "file BGM slot freed");
    }

    #[cfg(feature = "modern-audio")]
    #[test]
    fn file_sfx_one_shot_finishes() {
        let mut audio = file_audio();
        audio.play_sound("music/town"); // reuse the wav as a one-shot sfx
        for _ in 0..5 {
            audio.update_frame();
        }
        let e = audio.engine.as_ref().unwrap().lock().unwrap();
        // SFX plays on its own bus; BGM slot stays empty.
        assert!(e.modern_music.is_none());
        assert_eq!(e.modern.as_ref().unwrap().active_voices(), 1);
    }

    #[cfg(feature = "modern-audio")]
    #[test]
    fn json_track_wins_over_file_track() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(
            "data/audio/theme.json".to_string(),
            THEME_JSON.as_bytes().to_vec(),
        );
        map.insert("data/audio/theme.wav".to_string(), test_wav_bytes());
        let files = MemoryFiles::from(map);
        let mut audio = RunnerAudio::from_files(&files, "data", false);
        audio.set_pcm_render(true);
        // Same id "theme" in both libraries → JSON wins.
        audio.play_music("theme");
        let e = audio.engine.as_ref().unwrap().lock().unwrap();
        assert_eq!(
            e.current_music.as_deref(),
            Some("theme"),
            "JSON track played"
        );
        assert!(e.modern_music.is_none(), "file track not used");
    }
}
