//! Game-agnostic link-transport seam, re-exported from the engine.
//!
//! The [`NetworkTransport`] trait, [`TransportError`], the in-memory
//! [`ChannelTransport`] pair, and [`LinkRole`] live in `dotzuki_engine::link`
//! so ANY game on the engine can do link play; this module re-exports them
//! under the pokered link surface so existing imports keep working. The
//! game-specific wire protocol ([`super::protocol::NetworkMessage`]) stays
//! here.

#[cfg(not(target_os = "none"))]
pub use dotzuki_engine::link::ChannelTransport;
pub use dotzuki_engine::link::{NetworkTransport, TransportError};
