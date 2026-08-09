//! Game-agnostic link-transport seam, re-exported from the engine.
//!
//! The [`NetworkTransport`] trait, [`TransportError`], the in-memory
//! [`ChannelTransport`] pair, and [`LinkRole`] live in `jrpg_engine::link`
//! so ANY game on the engine can do link play; this module re-exports them
//! under the pokered link surface so existing imports keep working. The
//! game-specific wire protocol ([`super::protocol::NetworkMessage`]) stays
//! here.

pub use jrpg_engine::link::{ChannelTransport, NetworkTransport, TransportError};
