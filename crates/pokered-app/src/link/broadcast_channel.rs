//! BroadcastChannel link transport (wasm-only): two browser tabs on the same
//! origin can link over the Web `BroadcastChannel` API — no server, no
//! sockets.
//!
//! The transport itself is engine-generic and lives in
//! [`dotzuki_web::link`]; this module binds it to the pokered wire protocol
//! ([`NetworkMessage`]). See the engine module for the semantics: the
//! JSON-line framing with a self-echo-filtering
//! [`Frame`](dotzuki_web::link::Frame) envelope, the `mpsc`-backed
//! `recv`/`try_recv`, and the `Drop` behavior (frames drain before
//! `try_recv` deterministically reports `Disconnected`).

use pokered_core::link::protocol::NetworkMessage;

/// A link transport over `BroadcastChannel`, bound to the pokered link
/// protocol. The channel name acts as the "link room": exactly two tabs
/// should use the same name; the Hello/HelloAck handshake is started by the
/// game on its battle driver (via
/// [`PokemonGame::attach_link_transport`](crate::game::PokemonGame::attach_link_transport)
/// with `LinkRole::Guest`).
pub type BroadcastChannelTransport =
    dotzuki_web::link::BroadcastChannelTransport<NetworkMessage>;
