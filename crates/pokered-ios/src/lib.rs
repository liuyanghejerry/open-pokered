//! Pokémon Red/Blue iOS static library (C FFI surface).
//!
//! This crate exposes a C-compatible FFI surface for Swift to call into
//! the game engine. All FFI functions use only C-compatible types
//! (raw pointers, u8, f32, i8, usize, u32).

use std::ffi::CStr;
use std::path::PathBuf;
use std::ptr;
use std::sync::OnceLock;

mod audio_bridge;

use audio_bridge::AudioRingBuffer;
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_renderer::input::InputState;
use pokered_renderer::{FrameBuffer, RenderConfig, Rgba};

/// Singleton guard: ensures `pokered_init` is called at most once per process lifetime.
static INIT_ONCE: OnceLock<()> = OnceLock::new();

/// Initialize iOS-specific logging.
#[cfg(target_os = "ios")]
fn init_logging() {
    // TODO: Configure oslog logger when oslog crate dependency is added.
}

// ── Opaque Game Context ──────────────────────────────────────────────────

/// Opaque game context, heap-allocated and accessed via raw pointer from Swift.
pub struct GameContext {
    /// Core game engine instance.
    pub game: PokemonGame,
    /// Indexed framebuffer (160×144, packed 2bpp + display palette).
    pub fb: FrameBuffer,
    /// Scratch RGBA expansion of the framebuffer for the C export contract
    /// (160×144×4 = 92160 bytes, refreshed each `pokered_draw`).
    pub fb_rgba: Vec<u8>,
    /// Current input state (8 buttons as bitmask).
    pub input: InputState,
    /// Optional audio manager for sound generation.
    pub audio: Option<pokered_audio::audio_manager::AudioManager>,
    /// Optional save directory path.
    pub save_dir: Option<String>,
    /// Lock-free ring buffer for real-time audio bridge.
    /// Game thread pushes samples; audio callback thread pops them.
    pub ring_buffer: AudioRingBuffer,
    sample_buf: Vec<f32>,
    sample_idx: usize,
    pub frame: u64,
}

// ── C FFI Functions ──────────────────────────────────────────────────────

/// Initialize the game engine and return an opaque context pointer.
///
/// `version` must be 0 (Red) or 1 (Blue).
///
/// # Safety
///
/// Caller must call `pokered_destroy` to free the returned pointer.
/// This function must not be called while a previous context is still live
/// unless that context has been destroyed first.
#[no_mangle]
pub extern "C" fn pokered_init(version: u8) -> *mut GameContext {
    // Guard against double-initialization.
    if INIT_ONCE.set(()).is_err() {
        return ptr::null_mut();
    }

    let version = match version {
        0 => GameVersion::Red,
        _ => GameVersion::Blue,
    };
    let ctx = GameContext {
        game: PokemonGame::new(version),
        fb: FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE),
        fb_rgba: vec![0u8; 160 * 144 * 4],
        input: InputState::new(),
        audio: None,
        save_dir: None,
        ring_buffer: AudioRingBuffer::new(),
        sample_buf: vec![0.0f32; 1600],
        sample_idx: 0,
        frame: 0,
    };
    Box::into_raw(Box::new(ctx))
}

/// Destroy a game context previously created by `pokered_init`.
///
/// # Safety
///
/// `ctx` must be a valid pointer returned by `pokered_init`, or null
/// (in which case this is a no-op). After calling this function, the
/// pointer must not be used again.
#[no_mangle]
pub extern "C" fn pokered_destroy(ctx: *mut GameContext) {
    if !ctx.is_null() {
        // Safety: ctx is a valid, non-null pointer returned by pokered_init.
        // Box::from_raw reconstructs ownership of the heap allocation; drop frees it.
        unsafe { drop(Box::from_raw(ctx)); }
    }
}

/// Advance the game state by one frame.
///
/// `input_bits` is a bitfield encoding the 8 buttons:
/// bit 0 = A, 1 = B, 2 = Select, 3 = Start,
/// bit 4 = Right, 5 = Left, 6 = Up, 7 = Down.
/// A set bit indicates the button is pressed.
///
/// # Safety
///
/// `ctx` must be a valid, non-null pointer returned by `pokered_init`.
#[no_mangle]
pub extern "C" fn pokered_update(ctx: *mut GameContext, input_bits: u8) {
    let ctx = unsafe { &mut *ctx };

    ctx.input.begin_frame();
    ctx.input.set_from_bitmask(input_bits);
    ctx.game.update(&ctx.input);
    ctx.frame += 1;

    if let Some(ref mut audio_mgr) = ctx.audio {
        audio_mgr.update_frame();
        let cycles_per_sample = pokered_audio::CPU_CLOCK_HZ / 48_000;
        const SAMPLES_PER_FRAME: usize = 800;
        for _ in 0..SAMPLES_PER_FRAME {
            audio_mgr.apu.tick_n(cycles_per_sample);
            let (left, right) = audio_mgr.apu.mix_sample();
            ctx.sample_buf[ctx.sample_idx] = left as f32 / 480.0;
            ctx.sample_buf[ctx.sample_idx + 1] = right as f32 / 480.0;
            ctx.sample_idx += 2;
        }
        ctx.ring_buffer.push(&ctx.sample_buf[..ctx.sample_idx]);
        ctx.sample_idx = 0;
    }
}

/// Copy the current framebuffer into caller-provided RGBA buffer.
///
/// The framebuffer is 160×144 RGBA pixels (92160 bytes).
/// `buffer` must have capacity for at least `len` bytes.
///
/// # Safety
///
/// `ctx` must be a valid, non-null pointer. `buffer` must point to a
/// valid, writable region of at least `len` bytes.
#[no_mangle]
pub extern "C" fn pokered_draw(ctx: *mut GameContext, buffer: *mut u8, len: usize) {
    // Safety: ctx must be a valid, non-null pointer returned by pokered_init.
    let ctx = unsafe { &mut *ctx };
    ctx.game.draw(&mut ctx.fb);

    assert!(
        len >= 92160,
        "pokered_draw: buffer too small (need 92160 bytes, got {})",
        len
    );

    // Safety: buffer is writable with at least 92160 bytes (asserted above).
    // ctx.fb_rgba is the RGBA expansion of the indexed framebuffer (160×144×4).
    // Regions are non-overlapping: fb_rgba is owned by ctx, buffer is caller-provided.
    ctx.fb.to_rgba(&mut ctx.fb_rgba);
    unsafe {
        ptr::copy_nonoverlapping(ctx.fb_rgba.as_ptr(), buffer, 92160);
    }
}

/// Fill an audio buffer with the next frames of game audio.
///
/// `buffer` is an interleaved stereo (L, R, L, R, ...) `f32` sample buffer.
/// `frames` is the number of stereo frames (pairs of samples) requested.
/// Returns the number of frames actually written (may be less than requested).
///
/// # Safety
///
/// `ctx` must be a valid, non-null pointer. `buffer` must point to a
/// valid, writable region of at least `frames * 2 * sizeof(f32)` bytes.
#[no_mangle]
pub extern "C" fn pokered_audio_fill(
    ctx: *mut GameContext,
    buffer: *mut f32,
    frames: u32,
) -> u32 {
    let ctx = unsafe { &mut *ctx };
    let count = (frames as usize) * 2;

    let buf = unsafe { std::slice::from_raw_parts_mut(buffer, count) };
    let popped = ctx.ring_buffer.pop(buf, count);

    (popped / 2) as u32
}

/// Save the current game state to the given file path.
///
/// If `path` is non-null, writes to that exact path (atomic rename:
/// write to `{path}.tmp`, then `fs::rename` to `{path}`).
/// If `path` is null, uses `{save_dir}/pokered.sav`.
/// Returns `false` if the game is in battle (original Pokémon does not
/// support mid-battle saving), or if I/O fails.
///
/// # Safety
///
/// `ctx` must be a valid, non-null pointer. `path` must point to a
/// null-terminated UTF-8 string, or be null (in which case the save
/// directory from `pokered_set_save_dir` is used if set).
#[no_mangle]
pub extern "C" fn pokered_save(ctx: *mut GameContext, path: *const i8) -> bool {
    // Safety: validated below before dereferencing.
    if ctx.is_null() {
        return false;
    }
    let ctx = unsafe { &mut *ctx };

    // Original Pokémon does not support saving during battle.
    if matches!(ctx.game.state.screen, GameScreen::Battle) {
        return false;
    }

    // Resolve the save path: explicit path, or save_dir/pokered.sav.
    let save_path = if !path.is_null() {
        // Safety: caller guarantees a valid null-terminated UTF-8 string.
        let cstr = unsafe { CStr::from_ptr(path) };
        match cstr.to_str() {
            Ok(s) => PathBuf::from(s),
            Err(_) => return false,
        }
    } else if let Some(ref dir) = ctx.save_dir {
        PathBuf::from(dir).join("pokered.sav")
    } else {
        return false;
    };

    // Atomic write: write to .tmp, then rename.
    let tmp_path = {
        let mut p = save_path.clone().into_os_string();
        p.push(".tmp");
        PathBuf::from(p)
    };

    if let Err(e) = ctx.game.save_to_path(&tmp_path) {
        log::error!("pokered_save: failed to write tmp file: {e}");
        return false;
    }

    if let Err(e) = std::fs::rename(&tmp_path, &save_path) {
        log::error!("pokered_save: failed to rename tmp -> save: {e}");
        // Clean up the orphaned temp file.
        let _ = std::fs::remove_file(&tmp_path);
        return false;
    }

    true
}

/// Load a game state from the given file path.
///
/// If `path` is non-null, loads from that exact path.
/// If `path` is null, loads from `{save_dir}/pokered.sav`.
/// Returns `true` on success, `false` if the file cannot be read or
/// parsed as a valid SRAM save.
///
/// # Safety
///
/// `ctx` must be a valid, non-null pointer. `path` must point to a
/// null-terminated UTF-8 string, or be null (in which case the save
/// directory from `pokered_set_save_dir` is used if set).
#[no_mangle]
pub extern "C" fn pokered_load(ctx: *mut GameContext, path: *const i8) -> bool {
    if ctx.is_null() {
        return false;
    }
    let ctx = unsafe { &mut *ctx };

    let save_path = if !path.is_null() {
        // Safety: caller guarantees a valid null-terminated UTF-8 string.
        let cstr = unsafe { CStr::from_ptr(path) };
        match cstr.to_str() {
            Ok(s) => PathBuf::from(s),
            Err(_) => return false,
        }
    } else if let Some(ref dir) = ctx.save_dir {
        PathBuf::from(dir).join("pokered.sav")
    } else {
        return false;
    };

    match ctx.game.load_from_path(&save_path) {
        Ok(()) => true,
        Err(e) => {
            log::error!("pokered_load: failed to load from {:?}: {e}", save_path);
            false
        }
    }
}

/// Clear any cached (non-saved) game state without destroying the context.
///
/// # Safety
///
/// `ctx` must be a valid, non-null pointer returned by `pokered_init`.
#[no_mangle]
pub extern "C" fn pokered_clear_cache(_ctx: *mut GameContext) {
    // No-op stub — compiles, can be extended later.
}

#[no_mangle]
pub extern "C" fn pokered_frame_count(ctx: *mut GameContext) -> u64 {
    if ctx.is_null() { return 0; }
    unsafe { (*ctx).frame }
}

/// Set the save directory for future save/load operations.
///
/// When `path` is non-null, the string is copied into `ctx.save_dir`.
/// When `path` is null, the save directory is cleared.
///
/// # Safety
///
/// `ctx` must be a valid, non-null pointer. `path` must point to a
/// null-terminated UTF-8 string, or be null to clear the save dir.
#[no_mangle]
pub extern "C" fn pokered_set_save_dir(ctx: *mut GameContext, path: *const i8) {
    let ctx = unsafe { &mut *ctx };
    if path.is_null() {
        ctx.save_dir = None;
    } else {
        let cstr = unsafe { CStr::from_ptr(path) };
        match cstr.to_str() {
            Ok(s) => ctx.save_dir = Some(s.to_owned()),
            Err(_) => ctx.save_dir = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_renderer::input::GbButton;

    fn make_ctx() -> *mut GameContext {
        let version = GameVersion::Red;
        let ctx = GameContext {
            game: PokemonGame::new(version),
            fb: FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE),
            fb_rgba: vec![0u8; 160 * 144 * 4],
            input: InputState::new(),
            audio: None,
            save_dir: None,
            ring_buffer: AudioRingBuffer::new(),
            sample_buf: vec![0.0f32; 1600],
            sample_idx: 0,
            frame: 0,
        };
        Box::into_raw(Box::new(ctx))
    }

    #[test]
    fn draw_writes_to_buffer() {
        let ctx = make_ctx();
        let mut buf = vec![0u8; 92160];
        pokered_draw(ctx, buf.as_mut_ptr(), buf.len());
        pokered_destroy(ctx);
        let non_zero = buf.iter().any(|&b| b != 0);
        assert!(non_zero, "FrameBuffer should contain non-zero pixel data");
    }

    #[test]
    fn update_changes_frame_counter() {
        let ctx = make_ctx();
        let f0 = pokered_frame_count(ctx);
        pokered_update(ctx, 0);
        let f1 = pokered_frame_count(ctx);
        assert!(f1 > f0);
        pokered_destroy(ctx);
    }

    #[test]
    fn frame_count_null_returns_zero() {
        assert_eq!(pokered_frame_count(std::ptr::null_mut()), 0);
    }

    #[test]
    fn input_bitmask_mapping() {
        let ctx = make_ctx();
        pokered_update(ctx, 0b0000_1000);
        unsafe {
            assert!((*ctx).input.is_held(GbButton::Start));
        }
        pokered_update(ctx, 0);
        unsafe {
            assert!(!(*ctx).input.is_held(GbButton::Start));
        }
        pokered_destroy(ctx);
    }

    #[test]
    fn destroy_null_is_noop() {
        pokered_destroy(std::ptr::null_mut());
    }

    #[test]
    fn clear_cache_is_noop() {
        let ctx = make_ctx();
        pokered_clear_cache(ctx);
        pokered_destroy(ctx);
    }
}
