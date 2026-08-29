//! Audio engine for pokered — re-exports the generic `dotzuki-audio` crate and
//! provides pokered-specific music/SFX data and audio manager.

pub use dotzuki_audio::*;

pub mod audio_manager;
pub mod music_data;
pub mod sfx_data;

/// Shared device output (`AudioOutput`) for the native (`cpal` feature) and
/// WASM (`web-audio` feature) frontends.
#[cfg(any(
    all(not(target_arch = "wasm32"), feature = "cpal"),
    all(target_arch = "wasm32", feature = "web-audio"),
))]
pub mod output;

#[cfg(test)]
mod music_data_tests;

#[cfg(test)]
mod sfx_data_tests;

#[cfg(test)]
mod audio_manager_tests;

// Validates the generic `dotzuki-audio` file-based format against every real
// pokered track (needs the `serde` feature, active via dev-dependencies).
#[cfg(test)]
mod audio_format_tests;
