//! Modern file-audio subsystem (requires the `modern-audio` feature).
//!
//! The GB-sequencer path (`apu`/`sequencer`) synthesises chiptune from byte
//! streams; this module is the file-audio counterpart for games that want
//! real recordings:
//!
//! - [`decode`] — streaming WAV / OGG-Vorbis / FLAC / MP3 decoding via
//!   symphonia (caller-supplied `MediaSource`, no I/O in this crate);
//! - [`mixer`] — voices → BGM/SFX buses → master, with volume, equal-power
//!   pan, per-voice and per-bus fades, linear-interpolation resampling and
//!   a per-bus DSP chain;
//! - [`dsp`] — lowpass + reverb effect processors;
//! - [`engine`] — [`ModernAudio`](engine::ModernAudio) facade: play BGM
//!   (looping, cross-fading) and one-shot SFX, control bus levels.
//!
//! Everything here is gated behind `modern-audio`: with the feature off,
//! none of this code is compiled and symphonia is not built.

pub mod decode;
pub mod dsp;
pub mod engine;
pub mod mixer;

pub use decode::{DecodeError, SourceDecoder, StreamInfo};
pub use engine::{ModernAudio, MusicHandle, PlayOptions};
pub use mixer::Bus;
