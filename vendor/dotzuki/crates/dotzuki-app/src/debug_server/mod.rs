//! Generic debug server for native games: a TCP JSON-line protocol for
//! driving and inspecting a running game from tests and tooling.
//!
//! - [`protocol`] — the wire types: [`CoreDebugCommand`](protocol::CoreDebugCommand)
//!   (the generic JRPG command set, extensible by games) and
//!   [`DebugResponse`](protocol::DebugResponse) (the ok/error/data envelope).
//! - [`server`] — the transport machinery: [`DebugServer`](server::DebugServer)
//!   (TCP listener, JSON-line parsing, 300s response timeout) and
//!   [`DebugServerHandle`](server::DebugServerHandle) (the game-loop side:
//!   non-blocking command poll + response send over `mpsc`).
//!
//! Both are generic over the game's command type `C: DeserializeOwned`, so a
//! game keeps full ownership of its debug protocol while reusing the server.
//! Native only (TCP); not compiled for wasm.

pub mod protocol;
pub mod server;

pub use protocol::{CoreDebugCommand, DebugResponse};
pub use server::{DebugServer, DebugServerHandle};
