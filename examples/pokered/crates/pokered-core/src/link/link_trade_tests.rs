use super::link_trade::*;
use super::protocol::*;
use super::transport::*;
use crate::battle::state::{Pokemon, StatusCondition};
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_test_pokemon(species: Species, level: u8) -> Pokemon {
    Pokemon {
        species,
        nickname: None,
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
        is_traded: false, ot_id: 0, ot_name: None,
    }
}

#[test]
fn test_trade_request_accept_flow() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkTradeManager::new();
    let mut mgr_b = LinkTradeManager::new();

    assert_eq!(*mgr_a.state(), LinkTradeState::Idle);
    assert_eq!(*mgr_b.state(), LinkTradeState::Idle);

    mgr_a.request_trade(&mut t_a).unwrap();
    assert_eq!(*mgr_a.state(), LinkTradeState::WaitingForTradeResponse);

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkTradePollResult::TradeRequested);
    assert_eq!(*mgr_b.state(), LinkTradeState::PeerRequestedTrade);

    mgr_b.accept_trade(&mut t_b).unwrap();
    assert_eq!(*mgr_b.state(), LinkTradeState::SelectingMon);

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkTradePollResult::TradeAccepted);
    assert_eq!(*mgr_a.state(), LinkTradeState::SelectingMon);
}

#[test]
fn test_trade_request_decline_flow() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkTradeManager::new();
    let mut mgr_b = LinkTradeManager::new();

    mgr_a.request_trade(&mut t_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);

    mgr_b.decline_trade(&mut t_b).unwrap();
    assert_eq!(*mgr_b.state(), LinkTradeState::Idle);

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkTradePollResult::TradeDeclined);
    assert_eq!(*mgr_a.state(), LinkTradeState::Idle);
}

#[test]
fn test_request_trade_when_not_idle_fails() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkTradeManager::new();

    mgr_a.request_trade(&mut t_a).unwrap();
    let result = mgr_a.request_trade(&mut t_b);
    assert!(result.is_err());
}

#[test]
fn test_accept_trade_without_request_fails() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkTradeManager::new();

    let result = mgr.accept_trade(&mut t_a);
    assert!(result.is_err());
}

#[test]
fn test_decline_trade_without_request_fails() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkTradeManager::new();

    let result = mgr.decline_trade(&mut t_a);
    assert!(result.is_err());
}

fn setup_selecting_pair(
    t_a: &mut ChannelTransport<NetworkMessage>,
    t_b: &mut ChannelTransport<NetworkMessage>,
) -> (LinkTradeManager, LinkTradeManager) {
    let mut mgr_a = LinkTradeManager::new();
    let mut mgr_b = LinkTradeManager::new();

    mgr_a.request_trade(t_a).unwrap();
    mgr_b.poll_blocking(t_b);
    mgr_b.accept_trade(t_b).unwrap();
    mgr_a.poll_blocking(t_a);

    assert_eq!(*mgr_a.state(), LinkTradeState::SelectingMon);
    assert_eq!(*mgr_b.state(), LinkTradeState::SelectingMon);
    (mgr_a, mgr_b)
}

#[test]
fn test_mon_selection_both_select() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_selecting_pair(&mut t_a, &mut t_b);

    mgr_a.select_mon(&mut t_a, 0).unwrap();
    assert_eq!(*mgr_a.state(), LinkTradeState::WaitingForPeerSelection);

    mgr_b.select_mon(&mut t_b, 1).unwrap();
    assert_eq!(*mgr_b.state(), LinkTradeState::WaitingForPeerSelection);

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(
        result_b,
        LinkTradePollResult::BothSelected {
            local_index: 1,
            remote_index: 0,
        }
    );
    assert_eq!(
        *mgr_b.state(),
        LinkTradeState::BothSelected {
            local_index: 1,
            remote_index: 0,
        }
    );

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(
        result_a,
        LinkTradePollResult::BothSelected {
            local_index: 0,
            remote_index: 1,
        }
    );
}

#[test]
fn test_peer_selects_first_then_local() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_selecting_pair(&mut t_a, &mut t_b);

    mgr_b.select_mon(&mut t_b, 2).unwrap();

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkTradePollResult::PeerSelectedMon(2));
    assert_eq!(*mgr_a.state(), LinkTradeState::SelectingMon);

    mgr_a.select_mon(&mut t_a, 0).unwrap();
    assert_eq!(
        *mgr_a.state(),
        LinkTradeState::BothSelected {
            local_index: 0,
            remote_index: 2,
        }
    );
}

#[test]
fn test_select_mon_wrong_state_fails() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkTradeManager::new();

    let result = mgr.select_mon(&mut t_a, 0);
    assert!(result.is_err());
}

#[test]
fn test_full_trade_execute() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_selecting_pair(&mut t_a, &mut t_b);

    mgr_a.select_mon(&mut t_a, 0).unwrap();
    mgr_b.select_mon(&mut t_b, 1).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);

    let pokemon_a = make_test_pokemon(Species::Pikachu, 25);
    let pokemon_b = make_test_pokemon(Species::Charizard, 36);

    mgr_a.confirm_trade(&mut t_a, pokemon_a.clone()).unwrap();
    assert!(matches!(
        mgr_a.state(),
        LinkTradeState::WaitingForPeerConfirm { .. }
    ));

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkTradePollResult::PeerConfirmed);
    assert!(matches!(
        mgr_b.state(),
        LinkTradeState::PeerConfirmedWaitingLocal { .. }
    ));

    mgr_b.confirm_trade(&mut t_b, pokemon_b.clone()).unwrap();
    assert!(matches!(mgr_b.state(), LinkTradeState::Trading { .. }));

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert!(matches!(
        result_b,
        LinkTradePollResult::TradeExecute {
            local_index: 1,
            remote_index: 0,
            ..
        }
    ));
    assert_eq!(*mgr_b.state(), LinkTradeState::Completed);

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result_a, LinkTradePollResult::PeerConfirmed);

    let result_a = mgr_a.poll_blocking(&mut t_a);
    assert!(matches!(
        result_a,
        LinkTradePollResult::TradeExecute {
            local_index: 0,
            remote_index: 1,
            ..
        }
    ));
    assert_eq!(*mgr_a.state(), LinkTradeState::Completed);
}

#[test]
fn test_confirm_trade_wrong_state_fails() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkTradeManager::new();

    let pokemon = make_test_pokemon(Species::Pikachu, 25);
    let result = mgr.confirm_trade(&mut t_a, pokemon);
    assert!(result.is_err());
}

#[test]
fn test_cancel_trade_returns_to_selecting() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_selecting_pair(&mut t_a, &mut t_b);

    mgr_a.select_mon(&mut t_a, 0).unwrap();
    mgr_b.select_mon(&mut t_b, 1).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);

    mgr_a.cancel_trade(&mut t_a).unwrap();
    assert_eq!(*mgr_a.state(), LinkTradeState::SelectingMon);

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkTradePollResult::PeerCancelled);
    assert_eq!(*mgr_b.state(), LinkTradeState::SelectingMon);
}

#[test]
fn test_disconnect_during_trade() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (_mgr_a, mut mgr_b) = setup_selecting_pair(&mut t_a, &mut t_b);

    t_a.send(NetworkMessage::Disconnect).unwrap();

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkTradePollResult::Disconnected);
    assert_eq!(*mgr_b.state(), LinkTradeState::Cancelled);
}

#[test]
fn test_channel_drop_causes_disconnect() {
    let (mut t_a, t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkTradeManager::new();

    mgr_a.request_trade(&mut t_a).unwrap();
    drop(t_b);

    let result = mgr_a.poll_blocking(&mut t_a);
    assert_eq!(result, LinkTradePollResult::Disconnected);
    assert_eq!(*mgr_a.state(), LinkTradeState::Cancelled);
}

#[test]
fn test_poll_returns_pending_when_no_message() {
    let (mut t_a, _t_b) = ChannelTransport::new_pair();
    let mut mgr = LinkTradeManager::new();

    let result = mgr.poll(&mut t_a);
    assert_eq!(result, LinkTradePollResult::Pending);
}

#[test]
fn test_reset_for_new_trade() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_selecting_pair(&mut t_a, &mut t_b);

    mgr_a.select_mon(&mut t_a, 0).unwrap();
    mgr_b.select_mon(&mut t_b, 1).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);

    let pokemon_a = make_test_pokemon(Species::Pikachu, 25);
    let pokemon_b = make_test_pokemon(Species::Charizard, 36);
    mgr_a.confirm_trade(&mut t_a, pokemon_a).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_b.confirm_trade(&mut t_b, pokemon_b).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    assert_eq!(*mgr_b.state(), LinkTradeState::Completed);

    mgr_b.reset_for_new_trade();
    assert_eq!(*mgr_b.state(), LinkTradeState::Idle);
}

#[test]
fn test_unexpected_message_causes_error() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_a = LinkTradeManager::new();

    t_b.send(NetworkMessage::ConfirmTrade).unwrap();

    let result = mgr_a.poll_blocking(&mut t_a);
    assert!(matches!(result, LinkTradePollResult::Error(_)));
    assert!(matches!(mgr_a.state(), LinkTradeState::Error(_)));
}

#[test]
fn test_reselect_mon_resets_confirmation() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut mgr_a, mut mgr_b) = setup_selecting_pair(&mut t_a, &mut t_b);

    mgr_a.select_mon(&mut t_a, 0).unwrap();
    mgr_b.select_mon(&mut t_b, 1).unwrap();
    mgr_b.poll_blocking(&mut t_b);
    mgr_a.poll_blocking(&mut t_a);

    mgr_a.select_mon(&mut t_a, 2).unwrap();
    assert_eq!(
        *mgr_a.state(),
        LinkTradeState::BothSelected {
            local_index: 2,
            remote_index: 1,
        }
    );

    let result_b = mgr_b.poll_blocking(&mut t_b);
    assert_eq!(
        result_b,
        LinkTradePollResult::BothSelected {
            local_index: 1,
            remote_index: 2,
        }
    );
}

// ---------------------------------------------------------------------------
// LinkTradeDriver tests
// ---------------------------------------------------------------------------

use crate::evolution_screen::PendingEvolution;
use crate::link::link_trade::{LinkTradeDriver, LinkTradeError};
use crate::pokemon::party::Party;
use crate::pokemon::pokedex::Pokedex;

/// A mon with distinctive data so integrity through the wire is checkable.
fn trade_mon(species: Species, level: u8, ot_id: u16, nickname: Option<&str>) -> Pokemon {
    let mut mon = make_test_pokemon(species, level);
    mon.ot_id = ot_id;
    mon.ot_name = Some("REMOTE".to_string());
    mon.is_traded = false;
    mon.pp_ups = [2, 0, 1, 3];
    mon.total_exp = 123456;
    mon.dv_bytes = [0x12, 0x34];
    if let Some(nick) = nickname {
        mon.set_nickname(nick.to_string());
    }
    mon
}

fn driver_pair(
    t_a: &mut ChannelTransport<NetworkMessage>,
    t_b: &mut ChannelTransport<NetworkMessage>,
    party_a: Party,
    party_b: Party,
) -> (LinkTradeDriver, LinkTradeDriver) {
    let mut d_a = LinkTradeDriver::new(party_a, 0x1111);
    let mut d_b = LinkTradeDriver::new(party_b, 0x2222);
    d_a.request_trade(t_a).unwrap();
    d_b.poll_blocking(t_b);
    d_b.accept_trade(t_b).unwrap();
    d_a.poll_blocking(t_a);
    assert_eq!(*d_a.state(), LinkTradeState::SelectingMon);
    assert_eq!(*d_b.state(), LinkTradeState::SelectingMon);
    (d_a, d_b)
}

/// Drive both sides through selection + confirmation until each has seen a
/// TradeExecute.
fn run_to_trade_execute(
    d_a: &mut LinkTradeDriver,
    d_b: &mut LinkTradeDriver,
    t_a: &mut ChannelTransport<NetworkMessage>,
    t_b: &mut ChannelTransport<NetworkMessage>,
    idx_a: u8,
    idx_b: u8,
) -> (LinkTradePollResult, LinkTradePollResult) {
    d_a.select_mon(t_a, idx_a).unwrap();
    assert_eq!(
        d_b.poll_blocking(t_b),
        LinkTradePollResult::PeerSelectedMon(idx_a)
    );
    d_b.select_mon(t_b, idx_b).unwrap();
    assert!(matches!(
        d_a.poll_blocking(t_a),
        LinkTradePollResult::BothSelected { .. }
    ));
    d_a.confirm_trade(t_a).unwrap();
    assert_eq!(d_b.poll_blocking(t_b), LinkTradePollResult::PeerConfirmed);
    d_b.confirm_trade(t_b).unwrap();
    let r_b = d_b.poll_blocking(t_b);
    assert!(matches!(r_b, LinkTradePollResult::TradeExecute { .. }));
    assert_eq!(d_a.poll_blocking(t_a), LinkTradePollResult::PeerConfirmed);
    let r_a = d_a.poll_blocking(t_a);
    assert!(matches!(r_a, LinkTradePollResult::TradeExecute { .. }));
    (r_a, r_b)
}

/// Both sides trade mon 0 for mon 0; the exchange applies on both sides and
/// every data field survives the wire, the traded flag flips, and the dex
/// records only the received species.
#[test]
fn driver_full_trade_both_directions_with_integrity() {
    let pikachu = trade_mon(Species::Pikachu, 25, 0x1111, Some("SPARKY"));
    let bulbasaur = trade_mon(Species::Bulbasaur, 5, 0x1111, None);
    let charmander = trade_mon(Species::Charmander, 36, 0x2222, Some("BLAZE"));
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![pikachu.clone(), bulbasaur.clone()]),
        Party::from(vec![charmander.clone()]),
    );

    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);

    // The given mon is still in the party until apply_exchange.
    assert_eq!(d_a.given_mon(), Some(&pikachu));
    assert_eq!(d_a.party().count(), 2);
    assert_eq!(
        d_a.received_mon(),
        Some(&charmander),
        "wire copy available before apply"
    );

    let mut dex_a = Pokedex::new();
    let mut dex_b = Pokedex::new();
    let evo_a = d_a.apply_exchange(&mut dex_a).unwrap();
    let evo_b = d_b.apply_exchange(&mut dex_b).unwrap();
    assert!(evo_a.is_none());
    assert!(evo_b.is_none());

    // A: Pikachu is gone, Charmander joined with ALL its data.
    let a_party = d_a.party().to_vec();
    assert_eq!(a_party.len(), 2);
    assert_eq!(a_party[0], bulbasaur);
    let recv = &a_party[1];
    assert_eq!(recv.species, Species::Charmander);
    assert_eq!(recv.nickname.as_deref(), Some("BLAZE"));
    assert_eq!(recv.level, 36);
    assert_eq!(recv.dv_bytes, [0x12, 0x34]);
    assert_eq!(recv.pp_ups, [2, 0, 1, 3]);
    assert_eq!(recv.total_exp, 123456);
    assert_eq!(recv.ot_id, 0x2222, "OT stays the remote trainer's");
    assert_eq!(recv.ot_name.as_deref(), Some("REMOTE"));
    assert!(recv.is_traded, "remote OT != local player ID → traded");
    assert_eq!(d_a.given_mon(), Some(&pikachu), "removed mon still exposed");

    // B: party is just the received Pikachu, data intact, now traded.
    let b_party = d_b.party().to_vec();
    assert_eq!(b_party.len(), 1);
    let recv_b = &b_party[0];
    assert_eq!(recv_b.species, Species::Pikachu);
    assert_eq!(recv_b.nickname.as_deref(), Some("SPARKY"));
    assert_eq!(recv_b.dv_bytes, [0x12, 0x34]);
    assert_eq!(recv_b.pp_ups, [2, 0, 1, 3]);
    assert_eq!(recv_b.total_exp, 123456);
    assert_eq!(recv_b.ot_id, 0x1111);
    assert!(recv_b.is_traded);

    // Dex: only the RECEIVED species is flagged (AddEnemyMonToPlayerParty,
    // add_mon.asm:325-337) — never the given one.
    assert!(dex_a.is_owned(Species::Charmander));
    assert!(dex_a.is_seen(Species::Charmander));
    assert!(!dex_a.is_owned(Species::Pikachu));
    assert!(!dex_a.is_seen(Species::Pikachu));
    assert!(dex_b.is_owned(Species::Pikachu));
    assert!(dex_b.is_seen(Species::Pikachu));
    assert!(!dex_b.is_owned(Species::Charmander));
}

/// Gen 1 allows trading the LAST party member (`RemovePokemon` has no
/// party-count guard in the link path) — the received mon replaces it.
#[test]
fn driver_trade_last_remaining_mon_allowed() {
    let pikachu = trade_mon(Species::Pikachu, 25, 0x1111, None);
    let charmander = trade_mon(Species::Charmander, 36, 0x2222, None);
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![pikachu]),
        Party::from(vec![charmander]),
    );

    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);

    let mut dex = Pokedex::new();
    d_a.apply_exchange(&mut dex).unwrap();
    let a_party = d_a.party().to_vec();
    assert_eq!(a_party.len(), 1, "party emptied by the trade, then refilled");
    assert_eq!(a_party[0].species, Species::Charmander);
    d_b.apply_exchange(&mut dex).unwrap();
    assert_eq!(d_b.party().count(), 1);
}

/// Selection is bounds-checked; the manager's state guards still apply.
#[test]
fn driver_select_out_of_bounds_rejected() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, _d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 5, 1, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 5, 2, None)]),
    );
    assert_eq!(
        d_a.select_mon(&mut t_a, 1),
        Err(LinkTradeError::InvalidIndex(1))
    );
    assert_eq!(*d_a.state(), LinkTradeState::SelectingMon, "state untouched");
}

/// Confirm requires a selection.
#[test]
fn driver_confirm_without_selection_rejected() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, _d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 5, 1, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 5, 2, None)]),
    );
    assert!(matches!(
        d_a.confirm_trade(&mut t_a),
        Err(LinkTradeError::WrongState(_))
    ));
}

/// Cancel at every stage returns both sides to selection with the party
/// untouched (cable_club.asm .tradeCancelled → TradeCenter_SelectMon).
#[test]
fn driver_cancel_at_each_step() {
    // Cancel before any selection.
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 5, 1, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 5, 2, None)]),
    );
    d_a.cancel_trade(&mut t_a).unwrap();
    assert_eq!(*d_a.state(), LinkTradeState::SelectingMon);
    let result_b = d_b.poll_blocking(&mut t_b);
    assert_eq!(result_b, LinkTradePollResult::PeerCancelled);
    assert_eq!(*d_b.state(), LinkTradeState::SelectingMon);

    // Cancel after both sides selected.
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 5, 1, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 5, 2, None)]),
    );
    d_a.select_mon(&mut t_a, 0).unwrap();
    d_b.poll_blocking(&mut t_b);
    d_b.select_mon(&mut t_b, 0).unwrap();
    d_a.poll_blocking(&mut t_a);

    let party_before = d_a.party().to_vec();
    d_a.cancel_trade(&mut t_a).unwrap();
    assert_eq!(*d_a.state(), LinkTradeState::SelectingMon);
    assert_eq!(d_a.local_index(), None, "selection cleared");
    assert_eq!(d_b.poll_blocking(&mut t_b), LinkTradePollResult::PeerCancelled);
    assert_eq!(*d_b.state(), LinkTradeState::SelectingMon);
    assert_eq!(d_a.party().to_vec(), party_before, "party untouched");

    // After a peer cancel the local side must re-select before confirming.
    assert!(matches!(
        d_a.confirm_trade(&mut t_a),
        Err(LinkTradeError::WrongState(_))
    ));
}

/// Disconnect mid-trade (after our confirm, before the peer's mon arrives)
/// aborts with nothing applied.
#[test]
fn driver_disconnect_mid_trade_party_untouched() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![
            trade_mon(Species::Pikachu, 25, 1, None),
            trade_mon(Species::Bulbasaur, 5, 1, None),
        ]),
        Party::from(vec![trade_mon(Species::Charmander, 36, 2, None)]),
    );
    d_a.select_mon(&mut t_a, 0).unwrap();
    d_b.poll_blocking(&mut t_b);
    d_b.select_mon(&mut t_b, 0).unwrap();
    d_a.poll_blocking(&mut t_a);
    d_a.confirm_trade(&mut t_a).unwrap();

    // The cable breaks before B confirms.
    t_b.send(NetworkMessage::Disconnect).unwrap();
    assert_eq!(
        d_a.poll_blocking(&mut t_a),
        LinkTradePollResult::Disconnected
    );
    assert_eq!(*d_a.state(), LinkTradeState::Cancelled);
    assert!(d_a.is_completed());
    assert_eq!(d_a.received_mon(), None);
    assert_eq!(d_a.party().count(), 2, "no mon left the party");
}

/// The 4 link-evolution species trigger a FORCED pending evolution for the
/// received mon (TryEvolvingMon after the anim, cable_club.asm:851, with
/// wForceEvolution set at :822).
#[test]
fn driver_trade_evolution_four_species() {
    let cases = [
        (Species::Graveler, Species::Golem),
        (Species::Haunter, Species::Gengar),
        (Species::Machoke, Species::Machamp),
        (Species::Kadabra, Species::Alakazam),
    ];
    for (from, to) in cases {
        let (mut t_a, mut t_b) = ChannelTransport::new_pair();
        let (mut d_a, mut d_b) = driver_pair(
            &mut t_a,
            &mut t_b,
            Party::from(vec![trade_mon(Species::Abra, 30, 0x1111, Some("TOTORO"))]),
            Party::from(vec![trade_mon(from, 33, 0x2222, None)]),
        );
        run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);
        let mut dex = Pokedex::new();
        let pending = d_a.apply_exchange(&mut dex).unwrap();
        assert_eq!(
            pending,
            Some(PendingEvolution {
                party_index: 0, // received mon is the only party member left
                from,
                to,
                name: format!("{:?}", from).to_uppercase(),
                force: true, // wForceEvolution → cutscene cannot B-cancel
            }),
            "{:?} should evolve into {:?} on receipt",
            from,
            to
        );
        // The driver only DETECTS: the mon is still the pre-evolution species
        // until the frontend applies finalize_evolution.
        assert_eq!(d_a.received_mon().map(|m| m.species), Some(from));
        assert_eq!(d_a.party().count(), 1);
        // Dex already flags the received species.
        assert!(dex.is_owned(from));
    }
}

/// A mon whose evolutions are all level-up does NOT evolve on trade
/// (EVOLVE_TRADE entries only, evos_moves.asm:70-94) — even at max level.
#[test]
fn driver_no_trade_evolution_for_level_only_species() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 50, 0x1111, None)]),
        Party::from(vec![trade_mon(Species::Bulbasaur, 99, 0x2222, None)]),
    );
    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);
    let mut dex = Pokedex::new();
    let pending = d_a.apply_exchange(&mut dex).unwrap();
    assert_eq!(pending, None, "level-only species must not evolve on trade");
    assert_eq!(d_a.received_mon().map(|m| m.species), Some(Species::Bulbasaur));
}

/// The traded flag follows the obedience rule (`ot_id != 0 && != player_id`):
/// a freak OT-ID match with the local player counts as NOT traded.
#[test]
fn driver_traded_flag_freak_matching_id() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    // B's mon carries the SAME trainer ID as A (0x1111).
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 25, 0x1111, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 36, 0x1111, None)]),
    );
    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);
    let mut dex = Pokedex::new();
    d_a.apply_exchange(&mut dex).unwrap();
    assert_eq!(
        d_a.received_mon().map(|m| m.is_traded),
        Some(false),
        "matching OT ID → own mon (freak roll, faithful to obedience rule)"
    );
}

/// A full 6-mon party still completes the trade: removal first makes room
/// (cable_club.asm:800-817).
#[test]
fn driver_party_full_still_trades() {
    let six = vec![
        trade_mon(Species::Pikachu, 25, 0x1111, None),
        trade_mon(Species::Bulbasaur, 5, 0x1111, None),
        trade_mon(Species::Charmander, 5, 0x1111, None),
        trade_mon(Species::Squirtle, 5, 0x1111, None),
        trade_mon(Species::Jigglypuff, 5, 0x1111, None),
        trade_mon(Species::Spearow, 5, 0x1111, None),
    ];
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(six),
        Party::from(vec![trade_mon(Species::Kadabra, 40, 0x2222, None)]),
    );
    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);
    let mut dex = Pokedex::new();
    let pending = d_a.apply_exchange(&mut dex).unwrap();
    assert!(pending.is_some(), "Kadabra evolves on receipt");
    assert_eq!(d_a.party().count(), 6, "6 → remove 1 → add 1 = 6");
    assert_eq!(
        d_a.party().get(5).map(|m| m.species),
        Some(Species::Kadabra),
        "received mon lands in the last slot (wPartyCount - 1)"
    );
}

/// apply_exchange is single-shot per TradeExecute.
#[test]
fn driver_apply_exchange_twice_errors() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 25, 0x1111, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 36, 0x2222, None)]),
    );
    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);
    let mut dex = Pokedex::new();
    d_a.apply_exchange(&mut dex).unwrap();
    assert_eq!(
        d_a.apply_exchange(&mut dex),
        Err(LinkTradeError::NoExchange)
    );
    // And with no trade at all:
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, _d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 25, 0x1111, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 36, 0x2222, None)]),
    );
    assert_eq!(
        d_a.apply_exchange(&mut dex),
        Err(LinkTradeError::NoExchange)
    );
}

/// received_mon accessors span both phases: the wire copy before
/// apply_exchange, the party member afterwards (mutations persist).
#[test]
fn driver_received_mon_accessors() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![trade_mon(Species::Pikachu, 25, 0x1111, None)]),
        Party::from(vec![trade_mon(Species::Charmander, 36, 0x2222, None)]),
    );
    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);
    assert_eq!(
        d_a.received_mon().map(|m| m.species),
        Some(Species::Charmander)
    );
    let mut dex = Pokedex::new();
    d_a.apply_exchange(&mut dex).unwrap();
    // Mutating through received_mon_mut lands in the party.
    d_a.received_mon_mut().unwrap().set_nickname("NEWOWNER".to_string());
    assert_eq!(
        d_a.received_mon().unwrap().nickname.as_deref(),
        Some("NEWOWNER")
    );
    assert_eq!(
        d_a.party().get(0).unwrap().nickname.as_deref(),
        Some("NEWOWNER")
    );
}

/// After completion + apply, reset_for_new_trade allows a second trade on the
/// same transport pair.
#[test]
fn driver_reset_and_trade_again() {
    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let (mut d_a, mut d_b) = driver_pair(
        &mut t_a,
        &mut t_b,
        Party::from(vec![
            trade_mon(Species::Pikachu, 25, 0x1111, None),
            trade_mon(Species::Bulbasaur, 5, 0x1111, None),
        ]),
        Party::from(vec![trade_mon(Species::Charmander, 36, 0x2222, None)]),
    );
    run_to_trade_execute(&mut d_a, &mut d_b, &mut t_a, &mut t_b, 0, 0);
    let mut dex = Pokedex::new();
    d_a.apply_exchange(&mut dex).unwrap();
    d_b.apply_exchange(&mut dex).unwrap();
    assert_eq!(*d_a.state(), LinkTradeState::Completed);

    d_a.reset_for_new_trade();
    d_b.reset_for_new_trade();
    assert_eq!(*d_a.state(), LinkTradeState::Idle);
    assert_eq!(d_a.local_index(), None);
    assert_eq!(d_a.received_mon(), None);
    assert_eq!(d_a.given_mon(), None);

    // Second trade: A's Charmander (index 1) for B's Pikachu (index 0).
    d_a.request_trade(&mut t_a).unwrap();
    d_b.poll_blocking(&mut t_b);
    d_b.accept_trade(&mut t_b).unwrap();
    d_a.poll_blocking(&mut t_a);
    d_a.select_mon(&mut t_a, 1).unwrap();
    d_b.poll_blocking(&mut t_b);
    d_b.select_mon(&mut t_b, 0).unwrap();
    d_a.poll_blocking(&mut t_a);
    d_a.confirm_trade(&mut t_a).unwrap();
    d_b.poll_blocking(&mut t_b);
    d_b.confirm_trade(&mut t_b).unwrap();
    d_b.poll_blocking(&mut t_b);
    d_a.poll_blocking(&mut t_a);
    d_a.poll_blocking(&mut t_a);
    let mut dex2 = Pokedex::new();
    d_a.apply_exchange(&mut dex2).unwrap();
    d_b.apply_exchange(&mut dex2).unwrap();
    assert_eq!(d_a.party().count(), 2);
    assert_eq!(d_a.party().get(1).map(|m| m.species), Some(Species::Pikachu));
    assert_eq!(d_b.party().count(), 1);
    assert_eq!(d_b.party().get(0).map(|m| m.species), Some(Species::Charmander));
}
