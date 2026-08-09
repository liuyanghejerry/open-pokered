//! Cable Club in-room flow tests: two `LinkSession`s (routers) over an
//! in-memory channel pair with the CORE `LinkBattleDriver` / `LinkTradeDriver`
//! wired through them, driven through the app's `CableClubFlow` exactly like
//! the game loop does (poll → on_battle_event / on_trade_event → execute
//! `FlowNeed`).
//!
//! Covers: the gameboy request flow (battle + trade), the peer yes/no
//! prompt, the simultaneous-gameboy tie-break (host wins), the party
//! exchange into `BattleSetup`, the trade selection → confirm → exchange,
//! and the disconnect error screen.

use pokered_app::link::cable_club::{
    CableClubFlow, CableClubPhase, FlowNeed, LinkKind, TEXT_LINK_CANCELED, TEXT_TRADE_CANCELED,
};
use pokered_app::link::LinkSession;
use pokered_core::battle::link_battle_driver::{
    LinkBattleDriver, LinkDriverEvent, LinkDriverPhase,
};
use pokered_core::battle::state::{Pokemon, StatusCondition};
use pokered_core::link::LinkRole;
use pokered_core::link::link_trade::{LinkTradeDriver, LinkTradePollResult};
use pokered_core::link::transport::ChannelTransport;
use pokered_core::party_screen::PartyScreenInput;
use pokered_core::pokemon::party::Party;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

const HOST_LIST: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

fn mon(species: Species, level: u8) -> Pokemon {
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
        is_traded: false,
        ot_id: 0,
        ot_name: None,
    }
}

fn party2() -> Vec<Pokemon> {
    vec![mon(Species::Pikachu, 25), mon(Species::Charizard, 36)]
}

fn party() -> Party {
    let mut p = Party::new();
    for m in party2() {
        p.add(m).unwrap();
    }
    p
}

/// A connected pair of sessions + core drivers + flows, host (Player role)
/// and guest (Friend role), exactly like `--link-listen` + `--link-connect`.
struct Pair {
    host_session: LinkSession,
    host_flow: CableClubFlow,
    host_battle: LinkBattleDriver,
    host_trade: LinkTradeDriver,
    guest_session: LinkSession,
    guest_flow: CableClubFlow,
    guest_battle: LinkBattleDriver,
    guest_trade: LinkTradeDriver,
}

fn pair() -> Pair {
    let (t_a, t_b) = ChannelTransport::new_pair();
    let mut host_session = LinkSession::new(Box::new(t_a));
    let mut guest_session = LinkSession::new(Box::new(t_b));
    let host_battle = LinkBattleDriver::new(host_session.battle_transport(), party(), "HOST".into())
        .with_role(LinkRole::Host)
        .with_host_random_list(HOST_LIST);
    let guest_battle =
        LinkBattleDriver::new(guest_session.battle_transport(), party(), "GUEST".into())
            .with_role(LinkRole::Guest)
            .with_host_random_list(HOST_LIST);
    let host_trade = LinkTradeDriver::new(party(), 1).with_role(LinkRole::Host);
    let guest_trade = LinkTradeDriver::new(party(), 2).with_role(LinkRole::Guest);
    // The host (`--link-listen`) never starts the handshake — the guest's
    // Hello auto-acks from its driver's Idle state.
    let mut pair = Pair {
        host_session,
        host_flow: CableClubFlow::new(),
        host_battle,
        host_trade,
        guest_session,
        guest_flow: CableClubFlow::new(),
        guest_battle,
        guest_trade,
    };
    pair.guest_battle.start_handshake().unwrap();
    // Handshake both sides (route + poll).
    pump_battle(&mut pair.host_session, &mut pair.host_battle, &mut pair.host_flow);
    pump_battle(&mut pair.guest_session, &mut pair.guest_battle, &mut pair.guest_flow);
    pump_battle(&mut pair.host_session, &mut pair.host_battle, &mut pair.host_flow);
    pump_battle(&mut pair.guest_session, &mut pair.guest_battle, &mut pair.guest_flow);
    pair
}

/// One routing + driver-poll frame on ONE side (the game loop's `poll_link`
/// half): route the session, poll the battle driver, feed its events into
/// the flow.
fn pump_battle(
    session: &mut LinkSession,
    driver: &mut LinkBattleDriver,
    flow: &mut CableClubFlow,
) {
    session.poll();
    for ev in driver.poll() {
        flow.on_battle_event(&ev);
    }
}

/// One routing + driver-poll frame on ONE side for the trade driver.
fn pump_trade(
    session: &mut LinkSession,
    driver: &mut LinkTradeDriver,
    flow: &mut CableClubFlow,
) {
    session.poll();
    let result = driver.poll(&mut *session.trade_transport());
    flow.on_trade_event(&result);
}

fn a_input() -> PartyScreenInput {
    PartyScreenInput {
        up: false,
        down: false,
        a: true,
        b: false,
    }
}

fn b_input() -> PartyScreenInput {
    PartyScreenInput {
        up: false,
        down: false,
        a: false,
        b: true,
    }
}

fn no_input() -> PartyScreenInput {
    PartyScreenInput {
        up: false,
        down: false,
        a: false,
        b: false,
    }
}

/// Normalize the trade driver's errors onto the transport error type (the
/// game's `handle_flow_need` does the same).
fn trade_err_to_transport(
    e: pokered_core::link::link_trade::LinkTradeError,
) -> pokered_core::link::transport::TransportError {
    match e {
        pokered_core::link::link_trade::LinkTradeError::Transport(t) => t,
        other => pokered_core::link::transport::TransportError::IoError(other.to_string()),
    }
}

/// Execute a `FlowNeed` like the game loop's `handle_flow_need` (driver call
/// + `on_need_done` bookkeeping; the party snapshot is refreshed at the
/// table exactly like the game does).
fn execute(
    session: &mut LinkSession,
    flow: &mut CableClubFlow,
    battle: &mut LinkBattleDriver,
    trade: &mut LinkTradeDriver,
    need: FlowNeed,
) {
    let result = match &need {
        FlowNeed::None => return,
        FlowNeed::RequestLink(LinkKind::Battle) => {
            battle.set_local_party(party());
            battle.request_battle()
        }
        FlowNeed::RequestLink(LinkKind::Trade) => {
            trade.set_party(party());
            trade
                .request_trade(&mut *session.trade_transport())
                .map_err(trade_err_to_transport)
        }
        FlowNeed::ReplyRequest { kind, accept } => match (*kind, *accept) {
            (LinkKind::Battle, true) => battle.accept_battle(),
            (LinkKind::Battle, false) => battle.decline_battle(),
            (LinkKind::Trade, true) => trade
                .accept_trade(&mut *session.trade_transport())
                .map_err(trade_err_to_transport),
            (LinkKind::Trade, false) => trade
                .decline_trade(&mut *session.trade_transport())
                .map_err(trade_err_to_transport),
        },
        FlowNeed::SelectMon(idx) => trade
            .select_mon(&mut *session.trade_transport(), *idx)
            .map_err(trade_err_to_transport),
        FlowNeed::CancelTrade => trade
            .cancel_trade(&mut *session.trade_transport())
            .map_err(trade_err_to_transport),
        FlowNeed::ConfirmTrade => trade
            .confirm_trade(&mut *session.trade_transport())
            .map_err(trade_err_to_transport),
    };
    result.unwrap();
    flow.on_need_done(&need);
}

// ── Battle flow ────────────────────────────────────────────────────

#[test]
fn battle_flow_gameboy_to_battle_setup() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);
    p.guest_flow.note_presence(true, true);

    // Host uses the gameboy in the Colosseum → LINK BATTLE request.
    let need = p.host_flow.on_gameboy_used(MapId::Colosseum);
    assert_eq!(need, FlowNeed::RequestLink(LinkKind::Battle));
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);
    assert_eq!(p.host_flow.text_box().as_deref(), Some("Just a moment."));

    // Host dismisses the box; the guest gets the request prompt.
    let need = p.host_flow.update(a_input(), &party2());
    assert_eq!(need, FlowNeed::None);
    pump_battle(&mut p.guest_session, &mut p.guest_battle, &mut p.guest_flow);
    assert_eq!(
        *p.guest_flow.phase(),
        CableClubPhase::PeerPrompt {
            kind: LinkKind::Battle,
            selected: 0
        }
    );
    assert_eq!(
        p.guest_flow.prompt().map(|(t, _)| t),
        Some("Start a link\nbattle?".to_string())
    );

    // Guest accepts → the driver sends both AcceptBattle AND its party data
    // → both sides exchange → BattleSetup.
    let need = p.guest_flow.update(a_input(), &party2());
    assert_eq!(
        need,
        FlowNeed::ReplyRequest {
            kind: LinkKind::Battle,
            accept: true
        }
    );
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);
    pump_battle(&mut p.host_session, &mut p.host_battle, &mut p.host_flow);
    pump_battle(&mut p.host_session, &mut p.host_battle, &mut p.host_flow);
    pump_battle(&mut p.guest_session, &mut p.guest_battle, &mut p.guest_flow);
    pump_battle(&mut p.guest_session, &mut p.guest_battle, &mut p.guest_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::BattleSetup);
    assert_eq!(*p.guest_flow.phase(), CableClubPhase::BattleSetup);
    assert_eq!(p.host_battle.phase(), &LinkDriverPhase::Battling);
    assert_eq!(p.guest_battle.phase(), &LinkDriverPhase::Battling);
}

/// Both players use the gameboy at (nearly) the same time: the clocking
/// (hosting) side's request wins — the guest silently accepts — instead of
/// both sides erroring (engine/menus/main_menu.asm:216-220).
#[test]
fn simultaneous_gameboy_host_wins() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);
    p.guest_flow.note_presence(true, true);

    let need_host = p.host_flow.on_gameboy_used(MapId::Colosseum);
    let need_guest = p.guest_flow.on_gameboy_used(MapId::Colosseum);
    assert_eq!(need_host, FlowNeed::RequestLink(LinkKind::Battle));
    assert_eq!(need_guest, FlowNeed::RequestLink(LinkKind::Battle));
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need_host);
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need_guest);

    // The guest's duplicate request reaches the host first: ignored (host
    // wins). The host's request reaches the guest: auto-accepted (the guest
    // driver yields by accepting and sending its party data).
    pump_battle(&mut p.host_session, &mut p.host_battle, &mut p.host_flow);
    pump_battle(&mut p.guest_session, &mut p.guest_battle, &mut p.guest_flow);
    pump_battle(&mut p.host_session, &mut p.host_battle, &mut p.host_flow);
    pump_battle(&mut p.guest_session, &mut p.guest_battle, &mut p.guest_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::BattleSetup);
    assert_eq!(*p.guest_flow.phase(), CableClubPhase::BattleSetup);
    assert_eq!(p.host_battle.phase(), &LinkDriverPhase::Battling);
    assert_eq!(p.guest_battle.phase(), &LinkDriverPhase::Battling);
}

/// The peer declines: the requester returns to the room with the original's
/// "The link was canceled." box (data/text/text_2.asm:1691-1694).
#[test]
fn battle_declined_shows_link_canceled() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);
    p.guest_flow.note_presence(true, true);

    let need = p.host_flow.on_gameboy_used(MapId::Colosseum);
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);
    let _ = p.host_flow.update(a_input(), &party2());
    pump_battle(&mut p.guest_session, &mut p.guest_battle, &mut p.guest_flow);
    // Guest declines (B).
    let need = p.guest_flow.update(b_input(), &party2());
    assert_eq!(
        need,
        FlowNeed::ReplyRequest {
            kind: LinkKind::Battle,
            accept: false
        }
    );
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);
    pump_battle(&mut p.host_session, &mut p.host_battle, &mut p.host_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::InRoom);
    assert_eq!(p.host_flow.text_box().as_deref(), Some(TEXT_LINK_CANCELED));
    // A dismisses the box; the room stays idle.
    let need = p.host_flow.update(a_input(), &party2());
    assert_eq!(need, FlowNeed::None);
    assert_eq!(p.host_flow.text_box(), None);
}

// ── Trade flow ─────────────────────────────────────────────────────

#[test]
fn trade_flow_select_confirm_execute() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);
    p.guest_flow.note_presence(true, true);

    // Host uses the gameboy in the Trade Center → LINK TRADE request.
    let need = p.host_flow.on_gameboy_used(MapId::TradeCenter);
    assert_eq!(need, FlowNeed::RequestLink(LinkKind::Trade));
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);
    let _ = p.host_flow.update(a_input(), &party2()); // dismiss "Just a moment."

    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    assert_eq!(
        *p.guest_flow.phase(),
        CableClubPhase::PeerPrompt {
            kind: LinkKind::Trade,
            selected: 0
        }
    );
    let need = p.guest_flow.update(a_input(), &party2());
    assert_eq!(
        need,
        FlowNeed::ReplyRequest {
            kind: LinkKind::Trade,
            accept: true
        }
    );
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);

    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::TradeSelect);

    // Both sides pick a mon (host picks Pikachu at index 0).
    let need = p.host_flow.update(no_input(), &party2()); // builds the selector
    assert_eq!(need, FlowNeed::None);
    assert!(p.host_flow.party_select().is_some());
    let need = p.host_flow.update(a_input(), &party2());
    assert_eq!(need, FlowNeed::SelectMon(0));
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);

    // Guest sees the host's pick (PeerSelectedMon) and picks its own mon.
    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    assert_eq!(*p.guest_flow.phase(), CableClubPhase::TradeSelect);
    let _ = p.guest_flow.update(no_input(), &party2());
    let need = p.guest_flow.update(a_input(), &party2());
    assert_eq!(need, FlowNeed::SelectMon(0));
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);

    // Both sides land on the confirm box.
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(
        *p.host_flow.phase(),
        CableClubPhase::TradeConfirm {
            local_index: 0,
            remote_index: 0,
            selected: 0
        }
    );
    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    assert_eq!(
        *p.guest_flow.phase(),
        CableClubPhase::TradeConfirm {
            local_index: 0,
            remote_index: 0,
            selected: 0
        }
    );
    // Guest confirms first (each side confirms at its own pace, like the
    // original's TRADE_CANCEL_MENU).
    let need = p.guest_flow.update(a_input(), &party2());
    assert_eq!(need, FlowNeed::ConfirmTrade);
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);

    // Host sees the guest's confirm (the box stays up for its own confirm).
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(
        *p.host_flow.phase(),
        CableClubPhase::TradeConfirm {
            local_index: 0,
            remote_index: 0,
            selected: 0
        }
    );
    let need = p.host_flow.update(a_input(), &party2());
    assert_eq!(need, FlowNeed::ConfirmTrade);
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);

    // Both sides receive the exchanged mon → cutscene pending on each side.
    // (The confirm + the mon arrive as two wire messages, so each side may
    // need two polls — the game loop polls every frame.)
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::TradeAnim);
    assert_eq!(
        p.host_trade.received_mon().map(|m| m.species),
        Some(Species::Pikachu)
    );

    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    assert_eq!(*p.guest_flow.phase(), CableClubPhase::TradeAnim);
    assert_eq!(
        p.guest_trade.received_mon().map(|m| m.species),
        Some(Species::Pikachu)
    );
}

/// Cancelling the trade selection returns both sides to selection with the
/// original's "Too bad! The trade was canceled!" text.
#[test]
fn trade_cancel_shows_canceled_text() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);
    p.guest_flow.note_presence(true, true);

    let need = p.host_flow.on_gameboy_used(MapId::TradeCenter);
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);
    let _ = p.host_flow.update(a_input(), &party2());
    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    let need = p.guest_flow.update(a_input(), &party2());
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::TradeSelect);

    // Host picks a mon; the guest cancels → host sees the canceled text and
    // returns to selection.
    let _ = p.host_flow.update(no_input(), &party2());
    let need = p.host_flow.update(a_input(), &party2());
    assert_eq!(need, FlowNeed::SelectMon(0));
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);

    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    // Guest backs out of the confirm with B.
    let need = p.guest_flow.update(b_input(), &party2());
    assert_eq!(need, FlowNeed::CancelTrade);
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);

    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::TradeSelect);
    assert_eq!(
        p.host_flow.text_box().as_deref(),
        Some(TEXT_TRADE_CANCELED)
    );
}

// ── Disconnect ─────────────────────────────────────────────────────

#[test]
fn disconnect_mid_trade_shows_error_then_inactive() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);

    let need = p.host_flow.on_gameboy_used(MapId::TradeCenter);
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);
    let _ = p.host_flow.update(a_input(), &party2());

    // The peer vanishes mid-request.
    p.host_session.disconnect();
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(
        *p.host_flow.phase(),
        CableClubPhase::Error {
            text: TEXT_LINK_CANCELED.to_string()
        }
    );
    // A dismisses → Inactive (the presence hook clears on disconnect).
    let _ = p.host_flow.update(a_input(), &party2());
    assert_eq!(*p.host_flow.phase(), CableClubPhase::Inactive);
}

/// Leaving the room while connected returns the flow to Inactive (the room
/// then behaves like the placeholder map).
#[test]
fn leaving_room_deactivates_flow() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);
    assert!(p.host_flow.is_active());
    p.host_flow.note_presence(true, false);
    assert!(!p.host_flow.is_active());
    p.host_flow.note_presence(true, true);
    assert!(p.host_flow.is_active());
}

/// Both sides back out of the trade: the original returns to the room
/// (`ReturnToCableClubRoom` after the both-cancel nybble,
/// engine/link/cable_club.asm:571-580) — no "trade was canceled" loop.
#[test]
fn both_cancel_returns_to_room() {
    let mut p = pair();
    p.host_flow.note_presence(true, true);
    p.guest_flow.note_presence(true, true);

    let need = p.host_flow.on_gameboy_used(MapId::TradeCenter);
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);
    let _ = p.host_flow.update(a_input(), &party2());
    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    let need = p.guest_flow.update(a_input(), &party2());
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::TradeSelect);

    // Both sides cancel from the selection screen.
    let _ = p.host_flow.update(no_input(), &party2());
    let need = p.host_flow.update(b_input(), &party2());
    assert_eq!(need, FlowNeed::CancelTrade);
    execute(&mut p.host_session, &mut p.host_flow, &mut p.host_battle, &mut p.host_trade, need);
    let _ = p.guest_flow.update(no_input(), &party2());
    let need = p.guest_flow.update(b_input(), &party2());
    assert_eq!(need, FlowNeed::CancelTrade);
    execute(&mut p.guest_session, &mut p.guest_flow, &mut p.guest_battle, &mut p.guest_trade, need);

    // Each side sees the peer's cancel while its own cancel is pending:
    // both cancelled → back to the room, no error text.
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    pump_trade(&mut p.host_session, &mut p.host_trade, &mut p.host_flow);
    pump_trade(&mut p.guest_session, &mut p.guest_trade, &mut p.guest_flow);
    assert_eq!(*p.host_flow.phase(), CableClubPhase::InRoom);
    assert_eq!(*p.guest_flow.phase(), CableClubPhase::InRoom);
    assert_eq!(p.host_flow.text_box(), None);
}
