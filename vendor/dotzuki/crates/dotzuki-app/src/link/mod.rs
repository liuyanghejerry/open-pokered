//! Link play — real network transport + app-level session router.
//!
//! The engine (`dotzuki_engine::link`) owns the transport SEAM: the
//! [`NetworkTransport`](dotzuki_engine::link::NetworkTransport) trait,
//! [`TransportError`](dotzuki_engine::link::TransportError), the in-memory
//! [`ChannelTransport`](dotzuki_engine::link::ChannelTransport), and the
//! shared JSON-line [`codec`](dotzuki_engine::link::codec). This module
//! provides the platform-layer pieces any native game on the engine needs
//! for link play:
//!
//! - [`transport::TcpTransport`] / [`transport::LinkServer`] — the real
//!   native transport: plain `std::net` TCP with newline-framed JSON
//!   (native only; wasm games plug in a `BroadcastChannel` transport
//!   instead).
//! - [`session::LinkSession`] — the transport owner + message router:
//!   drains the real transport and routes each message by type into
//!   per-activity sub-queues (battle / trade) consumed by the game's link
//!   drivers, so the two activities coexist on one connection.
//!
//! Both are generic over the game's wire message type `M` (a serde enum);
//! the game also supplies a classification function
//! (`fn(&M) -> `[`session::Activity`]) and its disconnect message.

pub mod session;
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;

pub use session::{Activity, LinkSession, QueueTransport};
#[cfg(not(target_arch = "wasm32"))]
pub use transport::{LinkServer, TcpTransport};
