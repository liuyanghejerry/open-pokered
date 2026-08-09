//! Link play (Cable Club) — real network transport + app-level session.
//!
//! The session/router, the Cable Club flow and the framing codec are
//! platform-neutral (pure mpsc/serde); only the transports differ per
//! target:
//! - Native: plain `std::net` TCP with newline-framed JSON
//!   ([`TcpTransport`]/[`LinkServer`], `--link-listen` / `--link-connect`).
//! - wasm: `BroadcastChannel` between two browser tabs on the same origin
//!   ([`broadcast_channel::BroadcastChannelTransport`], `?link=<channel>`
//!   in the URL).
//!
//! Both transports speak the same JSON-line protocol (`NetworkMessage`),
//! encoded by the shared [`codec`]. [`LinkSession`] owns the transport and
//! routes messages into the per-activity sub-transports consumed by the
//! CORE drivers (`pokered_core::battle::link_battle_driver::LinkBattleDriver`
//! / `pokered_core::link::link_trade::LinkTradeDriver`), which are the
//! canonical battle/trade state machines. The game (`game.rs`) owns the
//! drivers, refreshes their party snapshots at the cable-club table and
//! feeds their events into [`CableClubFlow`].
//!
//! Wiring: `--link-listen <port>` hosts (the game waits for one peer),
//! `--link-connect <host:port>` joins; `?link=<channel>[&linkHost=1]` joins
//! a BroadcastChannel session in the browser. The game holds the
//! server/session and exposes a [`LinkStatus`] for the Cable Club UI to
//! render.

mod codec;
mod session;
#[cfg(not(target_arch = "wasm32"))]
mod transport;
#[cfg(target_arch = "wasm32")]
pub mod broadcast_channel;

pub use cable_club::{CableClubFlow, CableClubPhase, FlowNeed, LinkKind};
pub use session::LinkSession;
#[cfg(not(target_arch = "wasm32"))]
pub use transport::{LinkServer, TcpTransport};

#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;

/// High-level link status for the game UI (including the visible "Player2
/// disconnected" state). The Cable Club screen can render this without
/// touching the session internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStatus {
    /// No link flags given — link play disabled.
    Disabled,
    /// `--link-listen` server bound; waiting for one peer.
    WaitingForPeer,
    /// Transport established (client or accepted server); handshake in flight.
    Connecting,
    /// Handshake complete — link session active.
    Connected,
    /// Peer gone, or a link error. The string carries the human-readable
    /// reason (e.g. "Player2 disconnected").
    Disconnected(String),
}

/// Parse a `host:port` link address (numeric or hostname) into a
/// `SocketAddr`. IPv6 literals must be bracketed (`[::1]:5000`).
/// Native only — the wasm transport (BroadcastChannel) has no addresses.
#[cfg(not(target_arch = "wasm32"))]
pub fn parse_link_addr(s: &str) -> Result<SocketAddr, String> {
    use std::net::ToSocketAddrs;

    if let Ok(addr) = s.parse::<SocketAddr>() {
        return Ok(addr);
    }
    let (host, port_str) = s
        .rsplit_once(':')
        .ok_or_else(|| format!("invalid link address '{}' (expected host:port)", s))?;
    let port: u16 = port_str
        .parse()
        .map_err(|_| format!("invalid port in link address '{}'", s))?;
    let mut addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve link address '{}': {}", s, e))?;
    addrs
        .next()
        .ok_or_else(|| format!("no addresses found for '{}'", s))
}
pub mod cable_club;
