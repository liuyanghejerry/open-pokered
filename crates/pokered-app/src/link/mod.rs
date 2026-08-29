//! Link play (Cable Club) — real network transport + app-level session.
//!
//! The session/router, the native TCP transport and the framing codec are
//! generic engine/platform-layer machinery, bound here to pokered's wire
//! protocol ([`NetworkMessage`]):
//! - [`LinkSession`] / [`Activity`] — `dotzuki_app::link`: owns the
//!   transport and routes incoming messages by type into per-activity
//!   sub-queues (battle / trade), classified by [`link_activity`].
//! - [`TcpTransport`] / [`LinkServer`] — `dotzuki_app::link` (native only):
//!   plain `std::net` TCP with newline-framed JSON (`--link-listen` /
//!   `--link-connect`).
//! - wasm: `BroadcastChannel` between two browser tabs on the same origin
//!   ([`broadcast_channel::BroadcastChannelTransport`], `?link=<channel>`
//!   in the URL).
//!
//! Both transports speak the same JSON-line protocol, encoded by the shared
//! engine codec (see [`codec`]). The sub-transports handed out by
//! [`LinkSession`] feed the CORE drivers
//! (`pokered_core::battle::link_battle_driver::LinkBattleDriver`
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
pub mod cable_club;
#[cfg(target_arch = "wasm32")]
pub mod broadcast_channel;

pub use cable_club::{CableClubFlow, CableClubPhase, FlowNeed, LinkKind};

use pokered_core::link::protocol::NetworkMessage;

pub use dotzuki_app::link::Activity;

/// App-level link session (transport owner + message router), bound to
/// pokered's [`NetworkMessage`] wire protocol. Construct it with
/// [`link_activity`] as the classification table and
/// `NetworkMessage::Disconnect` as the disconnect message.
pub type LinkSession = dotzuki_app::link::LinkSession<NetworkMessage>;

/// Native TCP link transport, bound to [`NetworkMessage`].
#[cfg(not(target_arch = "wasm32"))]
pub type TcpTransport = dotzuki_app::link::TcpTransport<NetworkMessage>;

/// Native TCP link listener (the `--link-listen` host), bound to
/// [`NetworkMessage`].
#[cfg(not(target_arch = "wasm32"))]
pub type LinkServer = dotzuki_app::link::LinkServer<NetworkMessage>;

/// The routing classification table for the generic session router: which
/// activity queue each [`NetworkMessage`] belongs to. Battle traffic
/// (handshake, battle requests, party data, turns, results) goes to the
/// battle queue; trade traffic (requests, selection, completion) goes to
/// the trade queue; `Disconnect` broadcasts to both and closes the session.
pub fn link_activity(msg: &NetworkMessage) -> Activity {
    match msg {
        NetworkMessage::Hello { .. }
        | NetworkMessage::HelloAck { .. }
        | NetworkMessage::RequestBattle
        | NetworkMessage::AcceptBattle
        | NetworkMessage::DeclineBattle
        | NetworkMessage::PartyData(_)
        | NetworkMessage::TurnAction(_)
        // Battle results (protocol v2+) belong to the battle driver.
        | NetworkMessage::BattleResult(_) => Activity::Battle,
        NetworkMessage::RequestTrade
        | NetworkMessage::AcceptTrade
        | NetworkMessage::DeclineTrade
        | NetworkMessage::SelectMon(_)
        | NetworkMessage::ConfirmTrade
        | NetworkMessage::CancelTrade
        | NetworkMessage::TradeComplete(_) => Activity::Trade,
        NetworkMessage::Disconnect => Activity::Both,
    }
}

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
