//! Audio engine for pokered — re-exports the generic `dotzuki-audio` crate and
//! provides pokered-specific music/SFX data and audio manager.
//!
//! Hosted builds use PCM device output; bare-metal GBA builds send the same
//! sequencer register writes to the four hardware PSG channels.

#![no_std]

#[cfg(not(target_os = "none"))]
#[macro_use]
extern crate std;
extern crate alloc;

pub use dotzuki_audio::*;

pub mod audio_manager;
#[cfg(any(test, target_os = "none"))]
mod gba_psg;
pub mod music_data;
pub mod sfx_data;

/// Shared device output (`AudioOutput`) for the native (`cpal` feature) and
/// WASM (`web-audio` feature) frontends.
#[cfg(not(target_os = "none"))]
pub mod output;

/// Bare-metal GBA PSG output, sharing the music/SFX/cry sequencer.
#[cfg(target_os = "none")]
#[path = "output_gba.rs"]
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
