//! End-to-end link play tests over real loopback TCP.
//!
//! These drive the core battle state machine (and the app `LinkSession`
//! router + the CORE drivers wired through it) through two real sockets, up
//! to `Battling`. All waits are bounded deadline polls — no fixed sleeps.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use pokered_app::link::{LinkServer, LinkSession, TcpTransport, link_activity};
use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::link::link_battle::{LinkBattleManager, LinkBattlePollResult, LinkBattleState};
use pokered_core::link::protocol::{LinkAction, NetworkMessage, PartyExchangeData};
use pokered_core::link::transport::{NetworkTransport, TransportError};
use pokered_core::pokemon::party::Party;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

/// Poll `cond` until it returns true or `timeout` elapses (1 ms sleeps only
/// while waiting).
fn wait_until<F: FnMut() -> bool>(timeout: Duration, mut cond: F) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A connected `(client, server)` pair on loopback with a real socket.
fn tcp_pair() -> (TcpTransport, TcpTransport) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let client = TcpTransport::connect(addr).unwrap();
    let (raw_server, _peer) = listener.accept().unwrap();
    (client, TcpTransport::from_stream(raw_server).unwrap())
}

fn make_party_exchange_data(name: &str) -> PartyExchangeData {
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

    let mut party = Party::new();
    party.add(mon(Species::Pikachu, 25)).unwrap();
    party.add(mon(Species::Charizard, 36)).unwrap();
    PartyExchangeData {
        trainer_name: name.as_bytes().to_vec(),
        party,
        random_numbers: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    }
}

/// A two-mon party (used by the driver wiring in the session test).
fn party() -> Party {
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

    let mut p = Party::new();
    p.add(mon(Species::Pikachu, 25)).unwrap();
    p.add(mon(Species::Charizard, 36)).unwrap();
    p
}

/// Drive the core handshake (Hello/HelloAck) over two real transports.
fn handshake_over_tcp(
    client: &mut TcpTransport,
    server: &mut TcpTransport,
    mgr_client: &mut LinkBattleManager,
    mgr_server: &mut LinkBattleManager,
) {
    mgr_client.start_handshake(client).unwrap();
    assert_eq!(*mgr_client.state(), LinkBattleState::WaitingForHelloAck);

    // Server receives Hello, auto-acks, reaches Connected.
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_server.poll(server) == LinkBattlePollResult::HandshakeComplete
    }));
    assert_eq!(*mgr_server.state(), LinkBattleState::Connected);

    // Client receives HelloAck → Connected.
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_client.poll(client) == LinkBattlePollResult::HandshakeComplete
    }));
    assert_eq!(*mgr_client.state(), LinkBattleState::Connected);
}

#[test]
fn link_battle_over_tcp_to_battling() {
    let (mut client, mut server) = tcp_pair();
    let mut mgr_client = LinkBattleManager::new();
    let mut mgr_server = LinkBattleManager::new();

    handshake_over_tcp(&mut client, &mut server, &mut mgr_client, &mut mgr_server);

    // Server requests the battle; client accepts.
    mgr_server.request_battle(&mut server).unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_client.poll(&mut client) == LinkBattlePollResult::BattleRequested
    }));
    mgr_client.accept_battle(&mut client).unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_server.poll(&mut server) == LinkBattlePollResult::BattleAccepted
    }));

    // Both sides exchange party data → Battling.
    mgr_client
        .send_party_data(&mut client, make_party_exchange_data("RED"))
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_server.poll(&mut server) == LinkBattlePollResult::PartyDataReceived
    }));
    // Server still lacks its own party data — only its half is exchanged.
    assert_eq!(*mgr_server.state(), LinkBattleState::ExchangingParties);
    mgr_server
        .send_party_data(&mut server, make_party_exchange_data("BLUE"))
        .unwrap();
    // Sending the server's party completes the exchange on its side.
    assert_eq!(*mgr_server.state(), LinkBattleState::Battling);
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_client.poll(&mut client) == LinkBattlePollResult::PartyDataReceived
    }));
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_client.state() == &LinkBattleState::Battling
    }));

    // Both sides send a turn action → TurnReady on both sides.
    mgr_client
        .send_turn_action(&mut client, LinkAction::UseMove(0))
        .unwrap();
    mgr_server
        .send_turn_action(&mut server, LinkAction::Switch(1))
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_client.poll(&mut client)
            == LinkBattlePollResult::TurnReady {
                local_action: LinkAction::UseMove(0),
                remote_action: LinkAction::Switch(1),
            }
    }));
    assert!(wait_until(Duration::from_secs(5), || {
        mgr_server.poll(&mut server)
            == LinkBattlePollResult::TurnReady {
                local_action: LinkAction::Switch(1),
                remote_action: LinkAction::UseMove(0),
            }
    }));
}

#[test]
fn link_session_over_tcp_handshake_trade_battle_disconnect() {
    use pokered_core::battle::link_battle_driver::{
        LinkBattleDriver, LinkDriverEvent, LinkDriverPhase,
    };
    use pokered_core::link::link_trade::{LinkTradeDriver, LinkTradePollResult};

    let (client_transport, server_transport) = tcp_pair();
    let mut client = LinkSession::new(
        Box::new(client_transport),
        link_activity,
        NetworkMessage::Disconnect,
    );
    let mut server = LinkSession::new(
        Box::new(server_transport),
        link_activity,
        NetworkMessage::Disconnect,
    );

    // The CORE drivers wired through the session routers — the production
    // seam (`game.rs` does exactly this).
    let mut client_battle = LinkBattleDriver::new(client.battle_transport(), party(), "RED".into())
        .with_host_random_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let mut server_battle = LinkBattleDriver::new(server.battle_transport(), party(), "BLUE".into())
        .with_host_random_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let mut client_trade = LinkTradeDriver::new(party(), 1);
    let mut server_trade = LinkTradeDriver::new(party(), 2);

    // Handshake through the session: the client's driver starts the
    // asymmetric Hello/HelloAck exchange; the server's driver auto-acks from
    // its Idle state.
    client_battle.start_handshake().unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        client.poll();
        server.poll();
        server_battle
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::Connected))
    }));
    assert!(wait_until(Duration::from_secs(5), || {
        client.poll();
        server.poll();
        client_battle
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::Connected))
    }));
    assert_eq!(client_battle.phase(), &LinkDriverPhase::Connected);
    assert_eq!(server_battle.phase(), &LinkDriverPhase::Connected);

    // Trade request routed to the TRADE driver while the battle driver
    // stays untouched.
    client_trade
        .request_trade(&mut *client.trade_transport())
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        client.poll();
        server.poll();
        server_trade.poll(&mut *server.trade_transport()) == LinkTradePollResult::TradeRequested
    }));
    assert_eq!(server_battle.phase(), &LinkDriverPhase::Connected);
    server_trade
        .accept_trade(&mut *server.trade_transport())
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        client.poll();
        server.poll();
        client_trade.poll(&mut *client.trade_transport()) == LinkTradePollResult::TradeAccepted
    }));

    // Cancel back out of the trade flow, then battle instead.
    client_trade
        .cancel_trade(&mut *client.trade_transport())
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        client.poll();
        server.poll();
        server_trade.poll(&mut *server.trade_transport()) == LinkTradePollResult::PeerCancelled
    }));
    server_battle.request_battle().unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        client.poll();
        server.poll();
        client_battle
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::BattleRequested))
    }));
    // Accepting sends AcceptBattle AND the party data; both sides then
    // exchange and reach Battling on their own (the two sides' events land
    // on different frames — accumulate across iterations).
    client_battle.accept_battle().unwrap();
    let mut server_started = false;
    let mut client_started = false;
    let ok = wait_until(Duration::from_secs(5), || {
        client.poll();
        server.poll();
        let server_events = server_battle.poll();
        let client_events = client_battle.poll();
        server_started |= server_events
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::BattleStarted));
        client_started |= client_events
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::BattleStarted));
        server_started && client_started
    });
    assert!(ok, "BattleStarted never fired on both sides");
    assert_eq!(client_battle.phase(), &LinkDriverPhase::Battling);
    assert_eq!(server_battle.phase(), &LinkDriverPhase::Battling);

    // Peer drops the socket → the server's session reports the failure and
    // the drivers surface Disconnected (the game maps this to the visible
    // "Player2 disconnected" status). The client's drivers must be dropped
    // with the session — they share the transport's `Arc`, so the socket
    // only closes when the last holder is gone (the game drops session +
    // drivers together at teardown).
    drop(client);
    drop(client_battle);
    assert!(wait_until(Duration::from_secs(5), || {
        server.poll().is_some() && server.is_closed()
    }));
    assert!(wait_until(Duration::from_secs(5), || {
        server_battle
            .poll()
            .iter()
            .any(|e| matches!(e, LinkDriverEvent::Disconnected(_)))
    }));
}

#[test]
fn transport_handshake_through_link_server_accept() {
    // Listener side uses the real LinkServer (non-blocking accept), client
    // side uses TcpTransport::connect — the two entry points the CLI wires.
    let server = LinkServer::new("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = server.local_addr().unwrap();
    let mut client = TcpTransport::connect(addr).unwrap();

    // Wait for the accept to land (deadline poll), then wrap the peer.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut server_transport = loop {
        match server.accept() {
            Ok(Some(transport)) => break transport,
            Ok(None) => {
                assert!(Instant::now() < deadline, "timed out waiting for peer");
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => panic!("accept failed: {}", e),
        }
    };

    // Hello/HelloAck through the LinkServer-accepted socket.
    client.send(NetworkMessage::hello()).unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        matches!(
            server_transport.try_recv(),
            Ok(Some(NetworkMessage::Hello {
                version: NetworkMessage::PROTOCOL_VERSION
            }))
        )
    }));
    server_transport.send(NetworkMessage::hello_ack()).unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        matches!(
            client.try_recv(),
            Ok(Some(NetworkMessage::HelloAck {
                version: NetworkMessage::PROTOCOL_VERSION
            }))
        )
    }));

    // And the drop signal flows in the accepted direction too.
    drop(client);
    assert!(wait_until(Duration::from_secs(5), || {
        matches!(
            server_transport.try_recv(),
            Err(TransportError::Disconnected)
        )
    }));
}

#[test]
fn fragmented_write_reassembled_across_sockets() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let mut raw_client = TcpStream::connect(addr).unwrap();
    let (raw_server, _peer) = listener.accept().unwrap();
    let mut server = TcpTransport::from_stream(raw_server).unwrap();

    // One message's JSON written in three fragments with flushes between
    // them — as TCP segmentation may deliver it. The reader thread must
    // reassemble the line regardless of how the fragments interleave with
    // its blocking reads.
    let json = serde_json::to_string(&NetworkMessage::hello()).unwrap();
    let bytes = json.as_bytes();
    let chunk_size = bytes.len() / 3 + 1;
    for chunk in bytes.chunks(chunk_size) {
        raw_client.write_all(chunk).unwrap();
        raw_client.flush().unwrap();
    }
    raw_client.write_all(b"\n").unwrap();
    raw_client.flush().unwrap();

    let mut received = None;
    assert!(wait_until(Duration::from_secs(5), || match server.try_recv() {
        Ok(Some(msg)) => {
            received = Some(msg);
            true
        }
        _ => false,
    }));
    assert_eq!(received, Some(NetworkMessage::hello()));
}
