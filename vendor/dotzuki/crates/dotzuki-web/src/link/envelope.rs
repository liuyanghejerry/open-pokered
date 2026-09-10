//! Re-exports of the shared link codec from `dotzuki-engine`.
//!
//! The canonical implementation lives in [`dotzuki_engine::link::codec`] —
//! pure serde, no I/O, so it sits in the zero-I/O engine layer where both the
//! native TCP transport (`dotzuki-app`) and this crate's wasm
//! `BroadcastChannel` transport can share byte-identical framing.

pub(crate) use dotzuki_engine::link::codec::{decode_line, encode_line};
pub use dotzuki_engine::link::codec::Frame;
