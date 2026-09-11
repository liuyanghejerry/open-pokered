//! Audio engine for pokered — re-exports the generic `dotzuki-audio` crate and
//! provides pokered-specific music/SFX data and audio manager.
//!
//! Dual-target: hosted builds keep the full `dotzuki-audio` re-export and the
//! shared device output (`output::AudioOutput`); bare-metal GBA builds
//! (target_os = "none") compile only the music/SFX data tables plus a no-op
//! `output::AudioOutput` with the same method surface.

#![no_std]

#[cfg(not(target_os = "none"))]
#[macro_use]
extern crate std;
extern crate alloc;

#[cfg(not(target_os = "none"))]
pub use dotzuki_audio::*;

#[cfg(not(target_os = "none"))]
pub mod audio_manager;
pub mod music_data;
pub mod sfx_data;

/// Shared device output (`AudioOutput`) for the native (`cpal` feature) and
/// WASM (`web-audio` feature) frontends.
#[cfg(all(
    not(target_os = "none"),
    any(
        all(not(target_arch = "wasm32"), feature = "cpal"),
        all(target_arch = "wasm32", feature = "web-audio"),
    ),
))]
pub mod output;

/// Bare-metal (GBA) stand-in for [`output`]: the same `AudioOutput` surface
/// with empty bodies — there is no audio device on the cartridge target.
#[cfg(target_os = "none")]
#[path = "output_none.rs"]
pub mod output;

// With `#![no_std]` the Vec/String/Box/vec!/format! family leaves the prelude
// on BOTH targets. Modules glob-import this to keep using them unqualified.
#[allow(unused_imports)]
pub(crate) mod alloc_prelude {
    pub use alloc::borrow::{Cow, ToOwned};
    pub use alloc::boxed::Box;
    pub use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}

#[cfg(test)]
mod music_data_tests;

#[cfg(test)]
mod sfx_data_tests;

#[cfg(all(test, not(target_os = "none")))]
mod audio_manager_tests;

// Validates the generic `dotzuki-audio` file-based format against every real
// pokered track (needs the `serde` feature, active via dev-dependencies).
#[cfg(all(test, not(target_os = "none")))]
mod audio_format_tests;
