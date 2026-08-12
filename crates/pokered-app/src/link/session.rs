//! [`LinkSession`] — the app's link TRANSPORT OWNER + MESSAGE ROUTER.
//!
//! Owns the real network transport and routes every incoming message by type
//! into one of two per-activity sub-transports: a battle queue (handshake,
//! battle requests, `PartyData`, `TurnAction`, `BattleResult`) and a trade
//! queue (trade requests, selection, `TradeComplete`); `Disconnect` goes to
//! both. The sub-transports are handed to the CORE drivers
//! ([`LinkBattleDriver`] / [`LinkTradeDriver`]) — they are the only battle
//! and trade state machines in the app. The session itself holds NO game
//! logic, NO managers and NO events: [`LinkSession::poll`] only drains the
//! real transport into the queues.
//!
//! Why route at all, instead of handing the drivers the raw transport? Each
//! driver's state machine errors on messages that belong to the other
//! activity (their catch-all `(state, msg)` arm transitions to `Error`), and
//! the peer's request can arrive while the other activity is live (or before
//! either driver is created). The session is the single reader of the real
//! transport; the drivers only ever see their own queue, so battle and trade
//! flows coexist on one connection without protocol errors.
//!
//! Lifecycle (driven by `game.rs`, not by this module):
//! 1. `LinkSession::new(transport)` at connect; the client (`--link-connect`)
//!    immediately starts the handshake on its battle driver (the server
//!    auto-acks from the driver's `Idle` state).
//! 2. The game creates both core drivers with [`Self::battle_transport`] /
//!    [`Self::trade_transport`] (cheap clones — the session keeps its own
//!    copies for routing) and refreshes their party snapshots at the
//!    cable-club table.
//! 3. Every frame: [`LinkSession::poll`] (route) → `driver.poll()` (battle)
//!    → `driver.poll(transport)` (trade) → the events feed `CableClubFlow`.
//! 4. A transport failure closes the session and queues `Disconnect` into
//!    both queues; the drivers surface it as their `Disconnected` event.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use pokered_core::link::protocol::NetworkMessage;
use pokered_core::link::transport::{NetworkTransport, TransportError};

/// Transport handed to a core driver: serves routed messages from a queue
/// first, then delegates sends to the real transport.
///
/// `try_recv` never falls through to the underlying transport — the session
/// is the sole reader of the real transport and decides where each message
/// goes. `recv` falls through only when the queue is empty, which is safe
/// for standalone blocking use (`poll_blocking` in tests) but not inside a
/// session.
///
/// Cloning shares both the queue and the real transport, so the session
/// keeps one clone for routing while a driver consumes the same queue.
#[derive(Clone)]
struct QueueTransport {
    inner: Arc<Mutex<Box<dyn NetworkTransport<NetworkMessage>>>>,
    queue: Arc<Mutex<VecDeque<NetworkMessage>>>,
}

impl QueueTransport {
    fn new(inner: Arc<Mutex<Box<dyn NetworkTransport<NetworkMessage>>>>) -> Self {
        QueueTransport {
            inner,
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn queue(&mut self, msg: NetworkMessage) {
        self.queue.lock().unwrap().push_back(msg);
    }
}

fn lock_err() -> TransportError {
    TransportError::IoError("link transport lock poisoned".into())
}

impl NetworkTransport<NetworkMessage> for QueueTransport {
    fn send(&mut self, msg: NetworkMessage) -> Result<(), TransportError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        inner.send(msg)
    }

    fn recv(&mut self) -> Result<NetworkMessage, TransportError> {
        if let Some(msg) = self.queue.lock().unwrap().pop_front() {
            return Ok(msg);
        }
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        inner.recv()
    }

    fn try_recv(&mut self) -> Result<Option<NetworkMessage>, TransportError> {
        Ok(self.queue.lock().unwrap().pop_front())
    }
}

/// App-level link session: owns the transport and routes messages into the
/// per-activity queues consumed by the core drivers. No game state.
pub struct LinkSession {
    /// The real transport. The session is its only reader; the per-driver
    /// `QueueTransport`s share it (through the `Arc`) for sends.
    shared: Arc<Mutex<Box<dyn NetworkTransport<NetworkMessage>>>>,
    battle_queue: QueueTransport,
    trade_queue: QueueTransport,
    /// True after a disconnect was sent or received; `poll` then stops
    /// touching the (possibly dead) transport.
    closed: bool,
}

impl LinkSession {
    /// Create a session around a connected transport. The transport's
    /// reader thread (or channel) is live immediately; the handshake is
    /// started by the game on its battle driver (`LinkBattleDriver`).
    pub fn new(transport: Box<dyn NetworkTransport<NetworkMessage>>) -> Self {
        let shared = Arc::new(Mutex::new(transport));
        LinkSession {
            battle_queue: QueueTransport::new(Arc::clone(&shared)),
            trade_queue: QueueTransport::new(Arc::clone(&shared)),
            shared,
            closed: false,
        }
    }

    /// A clone of the battle sub-transport: hand it to
    /// [`LinkBattleDriver::new`](pokered_core::battle::link_battle_driver::LinkBattleDriver::new).
    /// Cheap — the session keeps its own clone for routing.
    pub fn battle_transport(&self) -> Box<dyn NetworkTransport<NetworkMessage>> {
        Box::new(self.battle_queue.clone())
    }

    /// A clone of the trade sub-transport: pass it to the
    /// [`LinkTradeDriver`](pokered_core::link::link_trade::LinkTradeDriver)
    /// calls. Cheap — the session keeps its own clone for routing.
    pub fn trade_transport(&self) -> Box<dyn NetworkTransport<NetworkMessage>> {
        Box::new(self.trade_queue.clone())
    }

    /// Drive the routing: drain everything the transport has right now into
    /// the per-activity queues. Call once per frame, before polling the
    /// drivers. Messages that arrive during the driver polls below stay in
    /// the transport and are routed on the next frame, so routing is never
    /// racy.
    ///
    /// Returns `Some(reason)` on the frame the transport failed: the session
    /// is closed and `Disconnect` was queued into BOTH sub-queues, so the
    /// drivers surface their `Disconnected` event. `None` while healthy.
    pub fn poll(&mut self) -> Option<String> {
        if self.closed {
            return None;
        }
        loop {
            let mut transport = match self.shared.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    self.closed = true;
                    return Some("link transport lock poisoned".into());
                }
            };
            match transport.try_recv() {
                Ok(Some(msg)) => {
                    drop(transport);
                    self.route(msg);
                }
                Ok(None) => break,
                Err(e) => {
                    drop(transport);
                    self.closed = true;
                    // Wake both drivers so their states end terminal
                    // (Finished / Cancelled), exactly as if the peer had
                    // sent `Disconnect`.
                    self.battle_queue.queue(NetworkMessage::Disconnect);
                    self.trade_queue.queue(NetworkMessage::Disconnect);
                    return Some(e.to_string());
                }
            }
        }
        None
    }

    /// Send `Disconnect` to the peer and mark the session closed (subsequent
    /// `poll`s are no-ops). The underlying socket stays open until the last
    /// transport holder (the session or a core driver sharing its `Arc`) is
    /// dropped; dropping the last one shuts the socket down and stops the
    /// reader thread.
    // (The native bin itself doesn't call this — it is the lib's public API
    // for the link UI integration and tests.)
    #[allow(dead_code)]
    pub fn disconnect(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let _ = self
            .shared
            .lock()
            .map_err(|_| lock_err())
            .and_then(|mut t| t.send(NetworkMessage::Disconnect));
        self.battle_queue.queue(NetworkMessage::Disconnect);
        self.trade_queue.queue(NetworkMessage::Disconnect);
    }

    /// True once the transport failed or `disconnect()` was called.
    #[allow(dead_code)]
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Route one incoming message to the driver that owns it.
    fn route(&mut self, msg: NetworkMessage) {
        match &msg {
            NetworkMessage::Hello { .. }
            | NetworkMessage::HelloAck { .. }
            | NetworkMessage::RequestBattle
            | NetworkMessage::AcceptBattle
            | NetworkMessage::DeclineBattle
            | NetworkMessage::PartyData(_)
            | NetworkMessage::TurnAction(_)
            // Battle results (protocol v2+) belong to the battle driver.
            | NetworkMessage::BattleResult(_) => self.battle_queue.queue(msg),
            NetworkMessage::RequestTrade
            | NetworkMessage::AcceptTrade
            | NetworkMessage::DeclineTrade
            | NetworkMessage::SelectMon(_)
            | NetworkMessage::ConfirmTrade
            | NetworkMessage::CancelTrade
            | NetworkMessage::TradeComplete(_) => self.trade_queue.queue(msg),
            NetworkMessage::Disconnect => {
                self.closed = true;
                self.battle_queue.queue(msg.clone());
                self.trade_queue.queue(msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_core::battle::link_battle_driver::{LinkBattleDriver, LinkDriverEvent, LinkDriverPhase};
    use pokered_core::battle::state::{Pokemon, StatusCondition};
    use pokered_core::link::link_trade::{LinkTradeDriver, LinkTradePollResult};
    use pokered_core::link::transport::ChannelTransport;
    use pokered_core::pokemon::party::Party;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn mon(species: Species, level: u8) -> Pokemon {
        Pokemon {
            species,
            nickname: [0x50; 11],
            level,
            hp: 100,
            max_hp: 100,
            attack: 50,
            defense: 40,
            speed: 60,
            special: 55,
            type1: PokemonType::Normal,
            type2: PokemonType::Normal,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [35, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xAB, 0xCD],
            stat_exp: [0; 5],
            total_exp: 1000,
            is_traded: false,
            ot_id: 0,
            ot_name: [0x50; 11],
        }
    }

    fn party2() -> Party {
        let mut p = Party::new();
        p.add(mon(Species::Pikachu, 25)).unwrap();
        p.add(mon(Species::Charizard, 36)).unwrap();
        p
    }

    /// A connected pair of sessions with their battle drivers wired through
    /// the routers — the production seam (`game.rs` does exactly this).
    struct BattlePair {
        sessions: (LinkSession, LinkSession),
        drivers: (LinkBattleDriver, LinkBattleDriver),
    }

    fn battle_pair() -> BattlePair {
        let (t_a, t_b) = ChannelTransport::new_pair();
        let a = LinkSession::new(Box::new(t_a));
        let b = LinkSession::new(Box::new(t_b));
        let driver_a = LinkBattleDriver::new(a.battle_transport(), party2(), "ALICE".into())
            .with_host_random_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let driver_b = LinkBattleDriver::new(b.battle_transport(), party2(), "BOB".into())
            .with_host_random_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        BattlePair {
            sessions: (a, b),
            drivers: (driver_a, driver_b),
        }
    }

    /// One routing + driver-poll frame on both sides (the game loop's
    /// `poll_link`): route the session, then let each driver consume its
    /// queue. Returns the collected events.
    fn pump(pair: &mut BattlePair) -> (Vec<LinkDriverEvent>, Vec<LinkDriverEvent>) {
        let (sessions, drivers) = (&mut pair.sessions, &mut pair.drivers);
        sessions.0.poll();
        sessions.1.poll();
        (drivers.0.poll(), drivers.1.poll())
    }

    #[test]
    fn handshake_through_router_drives_battle_driver() {
        let mut pair = battle_pair();
        // The client (`--link-connect`, "friend" side) starts the asymmetric
        // Hello/HelloAck exchange; the server auto-acks from Idle.
        pair.drivers.0.start_handshake().unwrap();
        // Two frames: the Hello and its ack are two wire round-trips.
        let (ea, eb) = pump(&mut pair);
        let (ea2, eb2) = pump(&mut pair);
        assert!(ea.iter().chain(ea2.iter()).any(|e| matches!(e, LinkDriverEvent::Connected)));
        assert!(eb.iter().chain(eb2.iter()).any(|e| matches!(e, LinkDriverEvent::Connected)));
        assert_eq!(pair.drivers.0.phase(), &LinkDriverPhase::Connected);
        assert_eq!(pair.drivers.1.phase(), &LinkDriverPhase::Connected);
    }

    /// The seam test: a full battle flow — handshake → request/accept →
    /// party exchange — end-to-end through BOTH routers. The session must
    /// not lose or misroute a single message, and the drivers must never see
    /// each other's traffic.
    #[test]
    fn full_battle_flow_through_session_router() {
        let mut pair = battle_pair();
        pair.drivers.0.start_handshake().unwrap();
        let _ = pump(&mut pair);
        let _ = pump(&mut pair);

        pair.drivers.0.request_battle().unwrap();
        let (ea, eb) = pump(&mut pair);
        assert!(eb.iter().any(|e| matches!(e, LinkDriverEvent::BattleRequested)));
        assert!(!ea.iter().any(|e| matches!(e, LinkDriverEvent::BattleRequested)));

        pair.drivers.1.accept_battle().unwrap();
        let (ea, eb) = pump(&mut pair);
        let (ea2, eb2) = pump(&mut pair);
        // The accept + both party data messages flow through the router.
        let ea = ea.iter().chain(ea2.iter());
        let eb = eb.iter().chain(eb2.iter());
        assert!(ea.clone().any(|e| matches!(e, LinkDriverEvent::BattleAccepted)));
        assert!(ea.clone().any(|e| matches!(e, LinkDriverEvent::BattleStarted)));
        assert!(eb.clone().any(|e| matches!(e, LinkDriverEvent::BattleStarted)));
        assert_eq!(pair.drivers.0.phase(), &LinkDriverPhase::Battling);
        assert_eq!(pair.drivers.1.phase(), &LinkDriverPhase::Battling);
        assert!(pair.drivers.0.screen().is_some());
        assert!(pair.drivers.1.screen().is_some());
    }

    /// Battle and trade traffic must be routed independently: a trade
    /// request arriving mid-battle-flow leaves the battle driver untouched
    /// (and vice versa) — the two managers share one wire but never see
    /// each other's messages.
    #[test]
    fn routes_battle_and_trade_messages_independently() {
        let mut pair = battle_pair();
        pair.drivers.0.start_handshake().unwrap();
        let _ = pump(&mut pair);
        let _ = pump(&mut pair);

        // Battle activity on top of the connection.
        pair.drivers.0.request_battle().unwrap();
        let (_ea, eb) = pump(&mut pair);
        assert!(eb.iter().any(|e| matches!(e, LinkDriverEvent::BattleRequested)));

        // A trade request now: it must reach the TRADE driver (routed to the
        // trade queue), not the battle driver.
        let mut trade_a = LinkTradeDriver::new(party2(), 42);
        let mut trade_b = LinkTradeDriver::new(party2(), 42);
        trade_a.request_trade(&mut *pair.sessions.0.trade_transport()).unwrap();
        pair.sessions.0.poll();
        pair.sessions.1.poll();
        let result_b = trade_b.poll(&mut *pair.sessions.1.trade_transport());
        assert_eq!(result_b, LinkTradePollResult::TradeRequested);
        // The battle driver's state is untouched by the trade message.
        assert_eq!(pair.drivers.1.phase(), &LinkDriverPhase::PeerRequestedBattle);
    }

    /// A full trade flow through the router: request → accept → select →
    /// confirm → execute.
    #[test]
    fn full_trade_flow_through_session_router() {
        let (t_a, t_b) = ChannelTransport::new_pair();
        let mut session_a = LinkSession::new(Box::new(t_a));
        let mut session_b = LinkSession::new(Box::new(t_b));
        let mut trade_a = LinkTradeDriver::new(party2(), 42);
        let mut trade_b = LinkTradeDriver::new(party2(), 42);

        trade_a.request_trade(&mut *session_a.trade_transport()).unwrap();
        session_a.poll();
        session_b.poll();
        assert_eq!(
            trade_b.poll(&mut *session_b.trade_transport()),
            LinkTradePollResult::TradeRequested
        );
        trade_b.accept_trade(&mut *session_b.trade_transport()).unwrap();
        session_b.poll();
        session_a.poll();
        assert_eq!(
            trade_a.poll(&mut *session_a.trade_transport()),
            LinkTradePollResult::TradeAccepted
        );
        trade_a.select_mon(&mut *session_a.trade_transport(), 0).unwrap();
        session_a.poll();
        session_b.poll();
        assert_eq!(
            trade_b.poll(&mut *session_b.trade_transport()),
            LinkTradePollResult::PeerSelectedMon(0)
        );
        trade_b.select_mon(&mut *session_b.trade_transport(), 1).unwrap();
        session_b.poll();
        session_a.poll();
        assert_eq!(
            trade_a.poll(&mut *session_a.trade_transport()),
            LinkTradePollResult::BothSelected { local_index: 0, remote_index: 1 }
        );
        trade_a.confirm_trade(&mut *session_a.trade_transport()).unwrap();
        trade_b.confirm_trade(&mut *session_b.trade_transport()).unwrap();
        // Each side must process the peer's ConfirmTrade (PeerConfirmed) and
        // then its TradeComplete (TradeExecute) — two wire round-trips.
        let mut executed = None;
        for _ in 0..8 {
            session_a.poll();
            session_b.poll();
            session_a.poll();
            session_b.poll();
            let r = trade_a.poll(&mut *session_a.trade_transport());
            match r {
                LinkTradePollResult::TradeExecute {
                    local_index,
                    received_pokemon,
                    ..
                } => {
                    executed = Some((local_index, received_pokemon));
                    break;
                }
                _ => continue,
            }
        }
        let (local_index, received_pokemon) = executed.expect("trade executed");
        assert_eq!(local_index, 0);
        // A traded its Pikachu (index 0) for B's Charizard (index 1).
        assert_eq!(received_pokemon.species, Species::Charizard);
    }

    /// The peer vanishing: the session's poll reports the failure and queues
    /// `Disconnect` into BOTH queues — each driver surfaces it.
    #[test]
    fn transport_failure_queues_disconnect_to_both_drivers() {
        let mut pair = battle_pair();
        pair.drivers.0.start_handshake().unwrap();
        let _ = pump(&mut pair);

        // Kill the channel underneath B's session.
        let (sessions, drivers) = (&mut pair.sessions, &mut pair.drivers);
        {
            // A transport whose channel sender is gone: `try_recv` reports
            // `Disconnected`, exactly like a dropped peer socket.
            let (tx, rx) = ChannelTransport::new_pair();
            drop(tx);
            *sessions.1.shared.lock().unwrap() = Box::new(rx);
        }
        let reason = sessions.1.poll().expect("transport failure reported");
        assert!(sessions.1.is_closed());
        assert!(!reason.is_empty());
        assert!(drivers
            .1
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::Disconnected(_))));
        // The OTHER side still works: A's driver sees B's Disconnect.
        let _ = sessions.0.poll();
        assert!(drivers
            .0
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::Disconnected(_))));
    }

    #[test]
    fn disconnect_closes_session_and_queues_disconnect() {
        let mut pair = battle_pair();
        pair.drivers.0.start_handshake().unwrap();
        let _ = pump(&mut pair);

        pair.sessions.0.disconnect();
        assert!(pair.sessions.0.is_closed());
        // The local driver sees the queued Disconnect; the peer's driver
        // sees the wire Disconnect.
        assert!(pair
            .drivers
            .0
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::Disconnected(_))));
        let _ = pair.sessions.1.poll();
        assert!(pair
            .drivers
            .1
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::Disconnected(_))));
        // A closed session is inert: no panics, no further events.
        assert_eq!(pair.sessions.0.poll(), None);
    }

    #[test]
    fn sub_transports_route_by_type_only() {
        // Battle traffic lands only in the battle sub-transport; the trade
        // sub-transport stays empty (and vice versa).
        let (t_a, mut t_b) = ChannelTransport::new_pair();
        let mut session = LinkSession::new(Box::new(t_a));
        let mut battle_t = session.battle_transport();
        let mut trade_t = session.trade_transport();

        t_b.send(NetworkMessage::hello()).unwrap();
        session.poll();
        assert!(matches!(battle_t.try_recv(), Ok(Some(NetworkMessage::Hello { .. }))));
        assert!(matches!(trade_t.try_recv(), Ok(None)));

        t_b.send(NetworkMessage::RequestTrade).unwrap();
        session.poll();
        assert!(matches!(trade_t.try_recv(), Ok(Some(NetworkMessage::RequestTrade))));
        assert!(matches!(battle_t.try_recv(), Ok(None)));
    }
}
