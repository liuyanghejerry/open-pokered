//! Browser playtest shell for `dotzuki-runner`: [`WasmRunner`].
//!
//! The editor (or any web page) ships a whole zero-Rust game project either
//! as a `.dzpk` binary pack ([`WasmRunner::from_pack`] — what
//! `dotzuki export --web` produces) or as a JSON object of `path → base64`
//! file contents (the constructor — what the editor's Play activity posts);
//! this crate boots a [`RunnerGame`] over an in-memory file system
//! ([`MemoryFiles`] / [`PackFiles`](dotzuki_runner::pack::PackFiles)) and
//! exposes a frame-driven `tick(input_bitmask) → RGBA bytes` loop suitable
//! for a `<canvas>` `ImageData` blit. Save persistence is the caller's job:
//! [`WasmRunner::export_save`] hands out save JSON (e.g. for `localStorage`)
//! and [`WasmRunner::import_save`] / the boot `save_json` restore it.
//!
//! ## Input bitmask
//!
//! `tick` takes a `u8` of currently-held buttons, one bit per
//! [`GbButton`](dotzuki_renderer::input::GbButton):
//!
//! | bit | button |
//! |-----|--------|
//! | 0   | A      |
//! | 1   | B      |
//! | 2   | Select |
//! | 3   | Start  |
//! | 4   | Right  |
//! | 5   | Left   |
//! | 6   | Up     |
//! | 7   | Down   |
//!
//! A bit set for exactly one `tick` is a "just pressed" edge; the wrapper
//! keeps the previous frame's mask internally.
//!
//! ## Typical JS loop
//!
//! ```js
//! const runner = new WasmRunner(filesJson, localStorage.getItem("save"));
//! const ctx = canvas.getContext("2d");
//! const image = ctx.createImageData(runner.width(), runner.height());
//! function frame() {
//!     image.data.set(runner.tick(inputBitmask()));
//!     ctx.putImageData(image, 0, 0);
//!     const pcm = runner.take_audio(); // stereo f32 @ 44.1 kHz (may be empty)
//!     if (pcm.length) audioQueue.push(pcm);
//!     const save = runner.export_save();
//!     if (save) localStorage.setItem("save", save);
//!     requestAnimationFrame(frame);
//! }
//! requestAnimationFrame(frame);
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use dotzuki_engine::render::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::InputState;
use dotzuki_runner::pack::PackFiles;
use dotzuki_runner::vfs::{MemoryFiles, ProjectFiles};
use dotzuki_runner::{LoadedProject, RunnerGame, RunnerOptions, SCREEN_H, SCREEN_W};
use wasm_bindgen::prelude::*;

/// Game Boy frame rate the runner is tuned for (matches dotzuki-app's window
/// loop); audio PCM is generated at `SAMPLE_RATE / GB_FRAME_RATE` per tick.
const GB_FRAME_RATE: f32 = 59.7275;

/// Audio sample rate of the rendered PCM (matches dotzuki-runner's `SAMPLE_RATE`).
const SAMPLE_RATE: f32 = 44_100.0;

/// Cap on the undrained audio buffer: 1s of stereo f32. A caller that never
/// takes audio must not grow memory without bound.
const MAX_AUDIO_BUF: usize = SAMPLE_RATE as usize * 2;

/// A booted zero-Rust game project, ready to be driven frame by frame.
///
/// The framebuffer is a fixed [`SCREEN_W`]×[`SCREEN_H`] RGBA surface
/// (320×240, from `dotzuki_runner`'s `game.rs` constants); `tick` returns it as
/// a flat `width * height * 4` byte vector.
#[wasm_bindgen]
pub struct WasmRunner {
    game: RunnerGame,
    input: InputState,
    fb: FrameBuffer,
    /// Interleaved stereo f32 @ 44.1 kHz produced since the last
    /// [`take_audio`](Self::take_audio) call.
    audio_buf: Vec<f32>,
    /// Fractional-sample accumulator: each tick generates
    /// `SAMPLE_RATE / GB_FRAME_RATE` (≈738.4) audio frames.
    audio_frac: f32,
}

#[wasm_bindgen]
impl WasmRunner {
    /// Boot a project shipped as `files_json`: a JSON object mapping
    /// project-relative POSIX paths (`"data/maps/Town/map.tmx.json"`) to
    /// base64-encoded file contents. `save_json`, when given, is a save
    /// previously produced by [`export_save`](Self::export_save) and is
    /// restored after boot (a corrupt save is ignored, same as the runner's
    /// disk behaviour).
    ///
    /// Fails with a human-readable message naming the offending file/step.
    #[wasm_bindgen(constructor)]
    pub fn new(files_json: &str, save_json: Option<String>) -> Result<WasmRunner, JsValue> {
        #[cfg(feature = "debug-panic-hook")]
        console_error_panic_hook::set_once();
        let files = decode_files(files_json).map_err(|e| JsValue::from_str(&e))?;
        Self::boot_with_files(Arc::new(MemoryFiles::new(files)), save_json.as_deref())
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Boot a project shipped as a `.dzpk` binary pack (the format
    /// `dotzuki export --web` writes as `game.dzpk`): no base64, no JSON —
    /// the pack's raw bytes are read in place via
    /// [`PackFiles`](dotzuki_runner::pack::PackFiles). `save_json` behaves as
    /// in the constructor.
    ///
    /// Fails with a human-readable message naming the offending file/step.
    #[wasm_bindgen(js_name = fromPack)]
    pub fn from_pack(pack: Vec<u8>, save_json: Option<String>) -> Result<WasmRunner, JsValue> {
        #[cfg(feature = "debug-panic-hook")]
        console_error_panic_hook::set_once();
        let files = PackFiles::from_bytes(pack)
            .map_err(|e| JsValue::from_str(&format!("invalid game pack: {e:#}")))?;
        Self::boot_with_files(Arc::new(files), save_json.as_deref())
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Advance one frame: feed `input_bitmask` (see crate docs) to the game,
    /// update, draw, and return the 320×240×4 RGBA framebuffer.
    ///
    /// Also renders this frame's audio into the internal PCM buffer; drain it
    /// with [`take_audio`](Self::take_audio).
    pub fn tick(&mut self, input_bitmask: u8) -> Vec<u8> {
        self.input.set_from_bitmask(input_bitmask);
        self.game.update(&self.input);
        self.input.begin_frame();
        self.game.draw(&mut self.fb);

        // Render this video frame's worth of audio (fractional accumulator so
        // long runs don't drift), then cap the backlog.
        self.audio_frac += SAMPLE_RATE / GB_FRAME_RATE;
        let frames = self.audio_frac.floor() as usize;
        self.audio_frac -= frames as f32;
        let pcm = self.game.render_audio(frames);
        if !pcm.is_empty() {
            self.audio_buf.extend_from_slice(&pcm);
            if self.audio_buf.len() > MAX_AUDIO_BUF {
                let excess = self.audio_buf.len() - MAX_AUDIO_BUF;
                self.audio_buf.drain(..excess);
            }
        }

        self.fb.data.clone()
    }

    /// Drain the audio PCM rendered since the last call: interleaved stereo
    /// f32 (LRLR…) at 44.1 kHz, ready for a WebAudio queue. Empty while the
    /// game is silent (no `data/audio/` tracks, or nothing playing yet).
    pub fn take_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.audio_buf)
    }

    /// Framebuffer width in pixels.
    pub fn width(&self) -> u32 {
        SCREEN_W as u32
    }

    /// Framebuffer height in pixels.
    pub fn height(&self) -> u32 {
        SCREEN_H as u32
    }

    /// The current state as save JSON for the caller to persist
    /// (`localStorage`); `None` while the game is in a transient state that
    /// cannot round-trip (mid-scene, mid-warp, battle, shop).
    pub fn export_save(&self) -> Option<String> {
        self.game.export_save()
    }

    /// Restore a save produced by [`export_save`](Self::export_save).
    /// Returns `false` — the game keeps its current state — when the JSON is
    /// unparseable, from a newer save version, or names a map that no longer
    /// loads.
    pub fn import_save(&mut self, json: &str) -> bool {
        self.game.import_save(json)
    }
}

impl WasmRunner {
    /// Native/testable boot path (no `JsValue`): every failure is a plain
    /// `String` naming the file or step that failed.
    fn boot_with_files(files: Arc<dyn ProjectFiles>, save_json: Option<&str>) -> Result<Self, String> {
        let project = LoadedProject::load_with_files(files)
            .map_err(|e| format!("project load failed: {e:#}"))?;
        let opts = RunnerOptions {
            // WASM shell: no hot-reload watching (no disk), never open an
            // audio device — PCM is rendered per tick and drained via
            // `take_audio` instead — and ignore any disk save: saves arrive
            // via `save_json`/`import_save`.
            watch: false,
            headless: true,
            pcm_audio: true,
            fresh: true,
            ..RunnerOptions::default()
        };
        let mut game =
            RunnerGame::new(project, opts).map_err(|e| format!("game boot failed: {e:#}"))?;
        if let Some(json) = save_json {
            game.import_save(json);
        }
        let fb = FrameBuffer::new(
            RenderConfig::new(SCREEN_W as u32, SCREEN_H as u32),
            Rgba::BLACK,
        );
        Ok(Self {
            game,
            input: InputState::new(),
            fb,
            audio_buf: Vec::new(),
            audio_frac: 0.0,
        })
    }
}

/// Decode the `{ path: base64 }` files object into a `MemoryFiles` map.
fn decode_files(files_json: &str) -> Result<HashMap<String, Vec<u8>>, String> {
    let encoded: HashMap<String, String> = serde_json::from_str(files_json)
        .map_err(|e| format!("files_json is not a JSON object of path→base64 strings: {e}"))?;
    let mut out = HashMap::with_capacity(encoded.len());
    for (path, b64) in encoded {
        let bytes = BASE64
            .decode(&b64)
            .map_err(|e| format!("file '{path}': invalid base64: {e}"))?;
        out.insert(path, bytes);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_files_round_trips() {
        let json = format!(
            r#"{{"a/b.txt": "{}", "c.bin": "{}"}}"#,
            BASE64.encode(b"hello"),
            BASE64.encode([0u8, 1, 2, 255])
        );
        let files = decode_files(&json).unwrap();
        assert_eq!(files["a/b.txt"], b"hello");
        assert_eq!(files["c.bin"], vec![0, 1, 2, 255]);
    }

    #[test]
    fn decode_files_reports_the_bad_file() {
        let err = decode_files(r#"{"data/x.png": "!!! not base64 !!!"}"#).unwrap_err();
        assert!(err.contains("data/x.png"), "{err}");
    }

    #[test]
    fn decode_files_rejects_non_object_json() {
        let err = decode_files(r#"["not", "an", "object"]"#).unwrap_err();
        assert!(err.contains("files_json"), "{err}");
    }

    #[test]
    fn boot_fails_with_named_step() {
        // An empty project: the manifest read fails inside project load.
        let err = WasmRunner::boot_with_files(Arc::new(MemoryFiles::new(HashMap::new())), None)
            .err()
            .expect("boot should fail");
        assert!(err.contains("project load failed"), "{err}");
        assert!(err.contains(".dotzuki-editor.json"), "{err}");
    }
}
