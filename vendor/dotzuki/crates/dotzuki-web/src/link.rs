//! Browser-tab link transport over the Web `BroadcastChannel` API — no
//! server, no sockets. Two tabs on the same origin join a channel (the "link
//! room") and exchange whole messages of the game's protocol type `M`.
//!
//! Semantics mirror [`dotzuki_engine::link::ChannelTransport`]:
//! `recv`/`try_recv` drain an `mpsc` channel fed by the `onmessage` listener,
//! and `Drop` closes the channel so pending frames drain before `try_recv`
//! deterministically reports [`TransportError::Disconnected`].
//!
//! Framing is the JSON-line convention each game also uses on its native
//! socket transports: one serde-JSON document per message. Because
//! BroadcastChannel delivers every post to EVERY tab on the channel —
//! including the sender's own — each frame is wrapped in a [`Frame`] envelope
//! carrying a random per-session tag, and receivers drop frames whose tag is
//! their own ([`Frame::is_self`]).
//!
//! The channel name acts as the room: exactly two participants (tabs) should
//! use the same name. A third tab would receive (and be received by) both
//! sides' messages — the protocol has no addressing — so use a fresh random
//! name per session.
//!
//! The transport itself is wasm-only; the [`Frame`] envelope compiles (and is
//! tested) on every target so games can verify the wire contract natively.

mod envelope;

#[cfg(target_arch = "wasm32")]
mod transport;

#[cfg(target_arch = "wasm32")]
pub use transport::BroadcastChannelTransport;

pub use envelope::Frame;
