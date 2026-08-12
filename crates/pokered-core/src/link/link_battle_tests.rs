use super::link_battle::*;
use super::protocol::*;
use super::transport::*;
use crate::battle::state::{Pokemon, StatusCondition};
use crate::pokemon::party::Party;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_test_pokemon(species: Species, level: u8) -> Pokemon {
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
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

fn make_party_exchange_data(name: &str) -> PartyExchangeData {
    let mut party = Party::new();
    party.add(make_test_pokemon(Species::Pikachu, 25)).unwrap();
    party
        .add(make_test_pokemon(Species::Charizard, 36))
        .unwrap();
    PartyExchangeData {
        trainer_name: name.as_bytes().to_vec(),
        party,
        random_numbers: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    }
}

#[test]
fn test_handshake_initiator_flow() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkBattleManager::new();
    let mut mgr_b = LinkBattleManager::new();

    assert_eq!(*mgr_a.state(), LinkBattleState::Idle);

    mgr_a.start_handshake(&mut t_a).unwrap();
    assert_eq!(*mgr_a.state(), LinkBattleState::WaitingForHelloAck);

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkBattlePollResult::HandshakeComplete);
    assert_eq!(*mgr_b.state(), LinkBattleState::Connected);

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkBattlePollResult::HandshakeComplete);
    assert_eq!(*mgr_a.state(), LinkBattleState::Connected);
}

#[test]
fn test_battle_request_accept() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkBattleManager::new();
    let mut mgr_b = LinkBattleManager::new();

    mgr_a.start_handshake(&mut t_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);

    mgr_a.request_battle(&mut t_a).unwrap();
    assert_eq!(*mgr_a.state(), LinkBattleState::WaitingForBattleResponse);

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkBattlePollResult::BattleRequested);
    assert_eq!(*mgr_b.state(), LinkBattleState::PeerRequestedBattle);

    mgr_b.accept_battle(&mut t_b).unwrap();
    assert_eq!(*mgr_b.state(), LinkBattleState::ExchangingParties);

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkBattlePollResult::BattleAccepted);
    assert_eq!(*mgr_a.state(), LinkBattleState::ExchangingParties);
}

#[test]
fn test_battle_request_decline() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkBattleManager::new();
    let mut mgr_b = LinkBattleManager::new();

    mgr_a.start_handshake(&mut t_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);

    mgr_a.request_battle(&mut t_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);

    mgr_b.decline_battle(&mut t_b).unwrap();
    assert_eq!(*mgr_b.state(), LinkBattleState::Connected);

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkBattlePollResult::BattleDeclined);
    assert_eq!(*mgr_a.state(), LinkBattleState::Connected);
}

#[test]
fn test_party_data_exchange() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkBattleManager::new();
    let mut mgr_b = LinkBattleManager::new();

    mgr_a.start_handshake(&mut t_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);
    mgr_a.request_battle(&mut t_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_b.accept_battle(&mut t_b).unwrap();
    mgr_a.poll_blocking(&mut t_a);

    let data_a = make_party_exchange_data("RED");
    let data_b = make_party_exchange_data("BLUE");

    mgr_a.send_party_data(&mut t_a, data_a).unwrap();
    assert_eq!(*mgr_a.state(), LinkBattleState::ExchangingParties);

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkBattlePollResult::PartyDataReceived);

    mgr_b.send_party_data(&mut t_b, data_b).unwrap();
    assert!(mgr_b.is_battling());

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkBattlePollResult::PartyDataReceived);
    assert!(mgr_a.is_battling());

    assert!(mgr_a.remote_party_data().is_some());
    assert_eq!(
        mgr_a.remote_party_data().unwrap().trainer_name,
        b"BLUE".to_vec()
    );
}

#[test]
fn test_turn_action_exchange() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_battling_pair(&mut t_a, &mut t_b);

    mgr_a
        .send_turn_action(&mut t_a, LinkAction::UseMove(0))
        .unwrap();
    mgr_b
        .send_turn_action(&mut t_b, LinkAction::UseMove(1))
        .unwrap();

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(
        result_b,
        LinkBattlePollResult::TurnReady {
            local_action: LinkAction::UseMove(1),
            remote_action: LinkAction::UseMove(0),
        }
    );

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(
        result_a,
        LinkBattlePollResult::TurnReady {
            local_action: LinkAction::UseMove(0),
            remote_action: LinkAction::UseMove(1),
        }
    );
}

fn setup_battling_pair(
    t_a: &mut ChannelTransport<NetworkMessage>,
    t_b: &mut ChannelTransport<NetworkMessage>,
) -> (LinkBattleManager, LinkBattleManager) {
    let mut mgr_a = LinkBattleManager::new();
    let mut mgr_b = LinkBattleManager::new();

    mgr_a.start_handshake(t_a).unwrap();
    mgr_b.poll_blocking(t_b);
    mgr_a.poll_blocking(t_a);
    mgr_a.request_battle(t_a).unwrap();
    mgr_b.poll_blocking(t_b);
    mgr_b.accept_battle(t_b).unwrap();
    mgr_a.poll_blocking(t_a);

    mgr_a
        .send_party_data(t_a, make_party_exchange_data("RED"))
        .unwrap();
    mgr_b
        .send_party_data(t_b, make_party_exchange_data("BLUE"))
        .unwrap();
    mgr_b.poll_blocking(t_b);
    mgr_a.poll_blocking(t_a);

    assert!(mgr_a.is_battling());
    assert!(mgr_b.is_battling());
    (mgr_a, mgr_b)
}

#[test]
fn test_multiple_turns() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_battling_pair(&mut t_a, &mut t_b);

    for turn in 0..3u8 {
        mgr_a
            .send_turn_action(&mut t_a, LinkAction::UseMove(turn % 4))
            .unwrap();
        mgr_b
            .send_turn_action(&mut t_b, LinkAction::Switch(turn))
            .unwrap();

        let result_a = mgr_a.poll_blocking(&mut t_a);
        let result_b = mgr_b.poll_blocking(&mut t_b);

        assert!(matches!(result_a, LinkBattlePollResult::TurnReady { .. }));
        assert!(matches!(result_b, LinkBattlePollResult::TurnReady { .. }));
        assert!(mgr_a.is_battling());
    }
}

#[test]
fn test_disconnect_during_battle() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_battling_pair(&mut t_a, &mut t_b);

    mgr_a.disconnect(&mut t_a).unwrap();
    assert!(mgr_a.is_finished());

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkBattlePollResult::Disconnected);
    assert!(mgr_b.is_finished());
}

#[test]
fn test_run_action() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_battling_pair(&mut t_a, &mut t_b);

    mgr_a.send_turn_action(&mut t_a, LinkAction::Run).unwrap();
    mgr_b
        .send_turn_action(&mut t_b, LinkAction::Struggle)
        .unwrap();

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(
        result_b,
        LinkBattlePollResult::TurnReady {
            local_action: LinkAction::Struggle,
            remote_action: LinkAction::Run,
        }
    );
}

#[test]
fn test_poll_returns_pending_when_no_message() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkBattleManager::new();

    let result = mgr.poll(&mut t_a);
    assert_eq!(result, LinkBattlePollResult::Pending);
}

#[test]
fn test_request_battle_before_connected_fails() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkBattleManager::new();

    let result = mgr.request_battle(&mut t_a);
    assert!(result.is_err());
}

#[test]
fn test_accept_battle_without_request_fails() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkBattleManager::new();
    let mut mgr_b = LinkBattleManager::new();

    mgr_a.start_handshake(&mut t_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);

    let result = mgr_a.accept_battle(&mut t_a);
    assert!(result.is_err());
}

#[test]
fn test_send_party_data_wrong_state_fails() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkBattleManager::new();

    let data = make_party_exchange_data("RED");
    let result = mgr.send_party_data(&mut t_a, data);
    assert!(result.is_err());
}

#[test]
fn test_send_turn_action_wrong_state_fails() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkBattleManager::new();

    let result = mgr.send_turn_action(&mut t_a, LinkAction::UseMove(0));
    assert!(result.is_err());
}

#[test]
fn test_reset_for_new_battle() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, _mgr_b) = setup_battling_pair(&mut t_a, &mut t_b);

    mgr_a.reset_for_new_battle();
    assert_eq!(*mgr_a.state(), LinkBattleState::Connected);
    assert!(mgr_a.remote_party_data().is_none());
}

#[test]
fn test_link_action_wire_roundtrip() {
    let actions = vec![
        LinkAction::UseMove(0),
        LinkAction::UseMove(1),
        LinkAction::UseMove(2),
        LinkAction::UseMove(3),
        LinkAction::Switch(0),
        LinkAction::Switch(5),
        LinkAction::Run,
        LinkAction::Struggle,
        LinkAction::NoAction,
    ];

    for action in actions {
        let wire = action.to_wire_byte();
        let decoded = LinkAction::from_wire_byte(wire);
        assert_eq!(
            action, decoded,
            "roundtrip failed for {:?} (wire=0x{:02X})",
            action, wire
        );
    }
}

#[test]
fn test_peer_disconnect_on_channel_drop() {
    let (mut t_a, t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkBattleManager::new();

    mgr_a.start_handshake(&mut t_a).unwrap();
    drop(t_b);

    let result = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result, LinkBattlePollResult::Disconnected);
    assert!(mgr_a.is_finished());
}
