//! [`LinkSession`] — the app's link TRANSPORT OWNER + MESSAGE ROUTER.
//!
//! Owns the real network transport and routes every incoming message by type
//! into one of two per-activity sub-transports: a battle queue and a trade
//! queue (the two canonical JRPG link activities); a broadcast/disconnect
//! message goes to both. The sub-transports are handed to the game's link
//! drivers (battle/trade state machines). The session itself holds NO game
//! logic: [`LinkSession::poll`] only drains the real transport into the
//! queues.
//!
//! Why route at all, instead of handing the drivers the raw transport? Each
//! driver's state machine errors on messages that belong to the other
//! activity, and the peer's request can arrive while the other activity is
//! live (or before either driver is created). The session is the single
//! reader of the real transport; the drivers only ever see their own queue,
//! so battle and trade flows coexist on one connection without protocol
//! errors.
//!
//! The message type `M` is the game's wire protocol; the game supplies:
//! - a classification function `fn(&M) -> `[`Activity`] telling the router
//!   which queue each message belongs to, and
//! - the disconnect message `M` queued into both sub-queues when the link
//!   goes down (so both drivers surface their `Disconnected` event).
//!
//! Lifecycle (driven by the game loop, not by this module):
//! 1. `LinkSession::new(transport, classify, disconnect)` at connect.
//! 2. The game creates its drivers with [`Self::battle_transport`] /
//!    [`Self::trade_transport`] (cheap clones — the session keeps its own
//!    copies for routing).
//! 3. Every frame: [`LinkSession::poll`] (route) → the drivers poll their
//!    sub-transports → their events feed the game's link UI.
//! 4. A transport failure closes the session and queues the disconnect
//!    message into both queues; the drivers surface it as their
//!    `Disconnected` event.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use dotzuki_engine::link::{NetworkTransport, TransportError};

/// Which activity queue an incoming link message belongs to.
///
/// Battle and trade are the two canonical JRPG link activities; games with
/// more activities can multiplex them onto these two queues or run several
/// sessions. [`Activity::Both`] is the broadcast class — the wire disconnect
/// message — and closes the session: it is queued into EVERY sub-queue so
/// all drivers see the link go down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// Battle traffic (handshake, battle requests, party data, turns, …).
    Battle,
    /// Trade traffic (trade requests, selection, completion, …).
    Trade,
    /// Broadcast to every activity queue and close the session — the
    /// protocol's disconnect message.
    Both,
}

/// Transport handed to a link driver: serves routed messages from a queue
/// first, then delegates sends to the real transport.
///
/// `try_recv` never falls through to the underlying transport — the session
/// is the sole reader of the real transport and decides where each message
/// goes. `recv` falls through only when the queue is empty, which is safe
/// for standalone blocking use but not inside a session.
///
/// Cloning shares both the queue and the real transport, so the session
/// keeps one clone for routing while a driver consumes the same queue.
#[derive(Clone)]
pub struct QueueTransport<M> {
    inner: Arc<Mutex<Box<dyn NetworkTransport<M>>>>,
    queue: Arc<Mutex<VecDeque<M>>>,
}

impl<M> QueueTransport<M> {
    fn new(inner: Arc<Mutex<Box<dyn NetworkTransport<M>>>>) -> Self {
        QueueTransport {
            inner,
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn queue(&mut self, msg: M) {
        self.queue.lock().unwrap().push_back(msg);
    }
}

fn lock_err() -> TransportError {
    TransportError::IoError("link transport lock poisoned".into())
}

impl<M> NetworkTransport<M> for QueueTransport<M> {
    fn send(&mut self, msg: M) -> Result<(), TransportError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        inner.send(msg)
    }

    fn recv(&mut self) -> Result<M, TransportError> {
        if let Some(msg) = self.queue.lock().unwrap().pop_front() {
            return Ok(msg);
        }
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        inner.recv()
    }

    fn try_recv(&mut self) -> Result<Option<M>, TransportError> {
        Ok(self.queue.lock().unwrap().pop_front())
    }
}

/// App-level link session: owns the transport and routes messages into the
/// per-activity queues consumed by the game's link drivers. No game state.
pub struct LinkSession<M> {
    /// The real transport. The session is its only reader; the per-driver
    /// `QueueTransport`s share it (through the `Arc`) for sends.
    shared: Arc<Mutex<Box<dyn NetworkTransport<M>>>>,
    battle_queue: QueueTransport<M>,
    trade_queue: QueueTransport<M>,
    /// The game's classification table: which queue each message owns.
    classify: fn(&M) -> Activity,
    /// The protocol's disconnect message, cloned into both queues when the
    /// link goes down (transport failure or [`Self::disconnect`]).
    disconnect_msg: M,
    /// True after a disconnect was sent or received; `poll` then stops
    /// touching the (possibly dead) transport.
    closed: bool,
}

impl<M: Clone + 'static> LinkSession<M> {
    /// Create a session around a connected transport. `classify` routes
    /// incoming messages to their activity queue; `disconnect_msg` is the
    /// protocol's disconnect message, queued into both queues when the link
    /// goes down. The transport's reader thread (or channel) is live
    /// immediately; any handshake is started by the game on its drivers.
    pub fn new(
        transport: Box<dyn NetworkTransport<M>>,
        classify: fn(&M) -> Activity,
        disconnect_msg: M,
    ) -> Self {
        let shared = Arc::new(Mutex::new(transport));
        LinkSession {
            battle_queue: QueueTransport::new(Arc::clone(&shared)),
            trade_queue: QueueTransport::new(Arc::clone(&shared)),
            shared,
            classify,
            disconnect_msg,
            closed: false,
        }
    }

    /// A clone of the battle sub-transport: hand it to the game's battle
    /// driver. Cheap — the session keeps its own clone for routing.
    pub fn battle_transport(&self) -> Box<dyn NetworkTransport<M>> {
        Box::new(self.battle_queue.clone())
    }

    /// A clone of the trade sub-transport: hand it to the game's trade
    /// driver. Cheap — the session keeps its own clone for routing.
    pub fn trade_transport(&self) -> Box<dyn NetworkTransport<M>> {
        Box::new(self.trade_queue.clone())
    }

    /// Drive the routing: drain everything the transport has right now into
    /// the per-activity queues. Call once per frame, before polling the
    /// drivers. Messages that arrive during the driver polls below stay in
    /// the transport and are routed on the next frame, so routing is never
    /// racy.
    ///
    /// Returns `Some(reason)` on the frame the transport failed: the session
    /// is closed and the disconnect message was queued into BOTH sub-queues,
    /// so the drivers surface their `Disconnected` event. `None` while
    /// healthy.
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
                    // Wake both drivers so their states end terminal,
                    // exactly as if the peer had sent its disconnect.
                    self.battle_queue.queue(self.disconnect_msg.clone());
                    self.trade_queue.queue(self.disconnect_msg.clone());
                    return Some(e.to_string());
                }
            }
        }
        None
    }

    /// Send the disconnect message to the peer and mark the session closed
    /// (subsequent `poll`s are no-ops). The underlying socket stays open
    /// until the last transport holder (the session or a driver sharing its
    /// `Arc`) is dropped; dropping the last one shuts the socket down and
    /// stops the reader thread.
    pub fn disconnect(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let _ = self
            .shared
            .lock()
            .map_err(|_| lock_err())
            .and_then(|mut t| t.send(self.disconnect_msg.clone()));
        self.battle_queue.queue(self.disconnect_msg.clone());
        self.trade_queue.queue(self.disconnect_msg.clone());
    }

    /// True once the transport failed or `disconnect()` was called.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Route one incoming message to the queue that owns it.
    fn route(&mut self, msg: M) {
        match (self.classify)(&msg) {
            Activity::Battle => self.battle_queue.queue(msg),
            Activity::Trade => self.trade_queue.queue(msg),
            Activity::Both => {
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
    use dotzuki_engine::link::ChannelTransport;

    /// A stand-in wire protocol covering both activities plus the broadcast
    /// disconnect, mirroring the shape of a real link protocol enum.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestMessage {
        Hello,
        HelloAck,
        RequestBattle,
        AcceptBattle,
        RequestTrade,
        AcceptTrade,
        Disconnect,
    }

    fn classify(msg: &TestMessage) -> Activity {
        match msg {
            TestMessage::Hello
            | TestMessage::HelloAck
            | TestMessage::RequestBattle
            | TestMessage::AcceptBattle => Activity::Battle,
            TestMessage::RequestTrade | TestMessage::AcceptTrade => Activity::Trade,
            TestMessage::Disconnect => Activity::Both,
        }
    }

    fn session(transport: Box<dyn NetworkTransport<TestMessage>>) -> LinkSession<TestMessage> {
        LinkSession::new(transport, classify, TestMessage::Disconnect)
    }

    /// A connected pair of sessions wired through the routers — the
    /// production seam (the game loop does exactly this).
    fn session_pair() -> (LinkSession<TestMessage>, LinkSession<TestMessage>) {
        let (t_a, t_b) = ChannelTransport::new_pair();
        (session(Box::new(t_a)), session(Box::new(t_b)))
    }

    /// The seam test: a full handshake → request/accept flow end-to-end
    /// through BOTH routers. The session must not lose or misroute a single
    /// message.
    #[test]
    fn full_flow_through_session_router() {
        let (mut a, mut b) = session_pair();
        let mut battle_a = a.battle_transport();
        let mut battle_b = b.battle_transport();

        battle_a.send(TestMessage::Hello).unwrap();
        a.poll();
        b.poll();
        assert_eq!(battle_b.try_recv().unwrap(), Some(TestMessage::Hello));

        battle_b.send(TestMessage::HelloAck).unwrap();
        b.poll();
        a.poll();
        assert_eq!(battle_a.try_recv().unwrap(), Some(TestMessage::HelloAck));

        battle_a.send(TestMessage::RequestBattle).unwrap();
        a.poll();
        b.poll();
        assert_eq!(battle_b.try_recv().unwrap(), Some(TestMessage::RequestBattle));

        battle_b.send(TestMessage::AcceptBattle).unwrap();
        b.poll();
        a.poll();
        assert_eq!(battle_a.try_recv().unwrap(), Some(TestMessage::AcceptBattle));
    }

    /// Battle and trade traffic must be routed independently: a trade
    /// request arriving mid-battle-flow lands in the trade queue and leaves
    /// the battle queue untouched (and vice versa) — the two activities
    /// share one wire but never see each other's messages.
    #[test]
    fn routes_battle_and_trade_messages_independently() {
        let (mut a, mut b) = session_pair();
        let mut battle_b = b.battle_transport();
        let mut trade_b = b.trade_transport();

        a.battle_transport()
            .send(TestMessage::RequestBattle)
            .unwrap();
        a.trade_transport()
            .send(TestMessage::RequestTrade)
            .unwrap();
        a.poll();
        b.poll();

        assert_eq!(battle_b.try_recv().unwrap(), Some(TestMessage::RequestBattle));
        assert_eq!(battle_b.try_recv().unwrap(), None);
        assert_eq!(trade_b.try_recv().unwrap(), Some(TestMessage::RequestTrade));
        assert_eq!(trade_b.try_recv().unwrap(), None);
    }

    /// A full trade flow through the router: request → accept.
    #[test]
    fn full_trade_flow_through_session_router() {
        let (mut a, mut b) = session_pair();
        let mut trade_a = a.trade_transport();
        let mut trade_b = b.trade_transport();

        trade_a.send(TestMessage::RequestTrade).unwrap();
        a.poll();
        b.poll();
        assert_eq!(trade_b.try_recv().unwrap(), Some(TestMessage::RequestTrade));

        trade_b.send(TestMessage::AcceptTrade).unwrap();
        b.poll();
        a.poll();
        assert_eq!(trade_a.try_recv().unwrap(), Some(TestMessage::AcceptTrade));
    }

    /// The peer vanishing: the session's poll reports the failure and queues
    /// the disconnect message into BOTH queues — each driver surfaces it.
    #[test]
    fn transport_failure_queues_disconnect_to_both_drivers() {
        let (mut a, mut b) = session_pair();
        let mut battle_a = a.battle_transport();
        let mut battle_b = b.battle_transport();
        let mut trade_b = b.trade_transport();

        // Kill the channel underneath B's session: a transport whose sender
        // is gone reports `Disconnected` on `try_recv`, exactly like a
        // dropped peer socket.
        let (tx, rx) = ChannelTransport::<TestMessage>::new_pair();
        drop(tx);
        *b.shared.lock().unwrap() = Box::new(rx);

        let reason = b.poll().expect("transport failure reported");
        assert!(b.is_closed());
        assert!(!reason.is_empty());
        assert_eq!(battle_b.try_recv().unwrap(), Some(TestMessage::Disconnect));
        assert_eq!(trade_b.try_recv().unwrap(), Some(TestMessage::Disconnect));
        // The OTHER side still works: A sees B's channel close on its next
        // poll and queues its own disconnect wakeup.
        let _ = a.poll();
        assert_eq!(battle_a.try_recv().unwrap(), Some(TestMessage::Disconnect));
    }

    #[test]
    fn disconnect_closes_session_and_queues_disconnect() {
        let (mut a, mut b) = session_pair();
        let mut battle_a = a.battle_transport();
        let mut battle_b = b.battle_transport();

        a.disconnect();
        assert!(a.is_closed());
        // The local queue sees the queued disconnect; the peer's queue sees
        // the wire disconnect (classified `Both` → broadcast + close).
        assert_eq!(battle_a.try_recv().unwrap(), Some(TestMessage::Disconnect));
        let _ = b.poll();
        assert!(b.is_closed());
        assert_eq!(battle_b.try_recv().unwrap(), Some(TestMessage::Disconnect));
        // A closed session is inert: no panics, no further events.
        assert_eq!(a.poll(), None);
    }

    #[test]
    fn sub_transports_route_by_type_only() {
        // Battle traffic lands only in the battle sub-transport; the trade
        // sub-transport stays empty (and vice versa).
        let (t_a, mut t_b) = ChannelTransport::new_pair();
        let mut session = session(Box::new(t_a));
        let mut battle_t = session.battle_transport();
        let mut trade_t = session.trade_transport();

        t_b.send(TestMessage::Hello).unwrap();
        session.poll();
        assert!(matches!(battle_t.try_recv(), Ok(Some(TestMessage::Hello))));
        assert!(matches!(trade_t.try_recv(), Ok(None)));

        t_b.send(TestMessage::RequestTrade).unwrap();
        session.poll();
        assert!(matches!(trade_t.try_recv(), Ok(Some(TestMessage::RequestTrade))));
        assert!(matches!(battle_t.try_recv(), Ok(None)));
    }
}
