//! Thin shim over the engine's shared JSON-line codec
//! ([`dotzuki_engine::link::codec`]), binding the generic [`Frame`] envelope
//! to pokered's [`NetworkMessage`].
//!
//! The codec itself (encode/decode + the `Frame` self-echo envelope) moved
//! to the engine so the native TCP transport and the wasm BroadcastChannel
//! transport share one byte-identical framing layer; only the message-type
//! binding stays here. Referenced by the wasm
//! [`super::broadcast_channel::BroadcastChannelTransport`]; it stays
//! compiled on the host so the binding is checked natively.

use pokered_core::link::protocol::NetworkMessage;

/// A BroadcastChannel frame: the sender's per-session tag plus the link
/// protocol message.
#[allow(dead_code)]
pub type Frame = dotzuki_engine::link::codec::Frame<NetworkMessage>;

#[allow(unused_imports)]
pub(crate) use dotzuki_engine::link::codec::{decode_line, encode_line};
