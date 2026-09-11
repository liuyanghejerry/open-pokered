//! Headless driver for [`RunnerGame`] — run frames without a window.
//!
//! Used by `dotzuki run --headless` (CI smoke tests, screenshot harnesses) and
//! by the integration tests. The driver synthesises an [`InputState`] per
//! frame (auto-pressing A on a configurable cadence so dialogue advances),
//! updates the game, then renders the final frame into a [`FrameBuffer`]
//! that can be dumped to a PNG.

use std::path::Path;

use anyhow::{Context, Result};
use dotzuki_engine::render::{FrameBuffer, Rgba};
use dotzuki_engine::render_config::RenderConfig;
use dotzuki_renderer::input::{GbButton, InputState};

use crate::game::{RunnerGame, SCREEN_H, SCREEN_W};

/// Options for [`run_headless`].
#[derive(Debug, Clone)]
pub struct HeadlessOptions {
    /// Frames to simulate (default 120 ≈ 2 s at the GB cadence).
    pub frames: u32,
    /// Press A every N frames (advances dialogue/choices); 0 disables.
    /// Ignored while `input_script` is non-empty.
    pub press_a_every: u32,
    /// Scripted input: exact-frame button presses `(frame, button)`. When
    /// non-empty it REPLACES the auto-A cadence — menu-driving harnesses use
    /// it to reach submenus a blind auto-A never would.
    pub input_script: Vec<(u32, GbButton)>,
    /// Optional PNG dump of the final framebuffer.
    pub screenshot: Option<std::path::PathBuf>,
}

impl Default for HeadlessOptions {
    fn default() -> Self {
        Self {
            frames: 120,
            press_a_every: 30,
            input_script: Vec::new(),
            screenshot: None,
        }
    }
}

/// Run `game` headless for `opts.frames` frames and return the final
/// framebuffer (also written to `opts.screenshot` when set).
///
/// # Errors
///
/// Fails only when the screenshot cannot be encoded/written.
pub fn run_headless(game: &mut RunnerGame, opts: &HeadlessOptions) -> Result<FrameBuffer> {
    let mut input = InputState::new();
    for frame in 0..opts.frames {
        let mask = if !opts.input_script.is_empty() {
            opts.input_script
                .iter()
                .filter(|(f, _)| *f == frame)
                .fold(0, |mask, (_, b)| mask | b.bit_mask())
        } else if opts.press_a_every > 0 && frame % opts.press_a_every == 0 {
            GbButton::A.bit_mask()
        } else {
            0
        };
        input.set_from_bitmask(mask);
        game.update(&input);
        input.begin_frame();
    }

    let mut fb = FrameBuffer::new(
        RenderConfig::new(SCREEN_W as u32, SCREEN_H as u32),
        Rgba::BLACK,
    );
    game.draw(&mut fb);

    if let Some(path) = &opts.screenshot {
        save_png(&fb, path)?;
    }
    Ok(fb)
}

/// Write a framebuffer out as an 8-bit RGBA PNG.
pub fn save_png(fb: &FrameBuffer, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }
    image::save_buffer(
        path,
        &fb.data,
        fb.width(),
        fb.height(),
        image::ColorType::Rgba8,
    )
    .with_context(|| format!("failed to write screenshot {}", path.display()))
}
