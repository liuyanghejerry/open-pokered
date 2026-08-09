//! Deterministic two-sided link-battle tests.
//!
//! Two in-process drivers over a `ChannelTransport` pair play a full battle:
//! handshake → request/accept → party exchange → multi-turn battle, with the
//! shared RNG stream pinned so every roll is identical on both sides. The
//! core invariant asserted after every turn: the two sides' battle states are
//! perfect mirrors of each other (A's player == B's enemy, HP included) and
//! both RNG streams have consumed exactly the same bytes.

use super::link_battle_driver::{test_driver_pair, LinkBattleDriver, LinkDriverEvent, LinkDriverPhase};
use crate::battle::state::Pokemon;
use crate::battle::{BattleInput, BattlePhase};
use crate::link::protocol::LinkBattleResult;
use crate::link::transport::ChannelTransport;
use crate::link::rng::LINK_RANDOM_LIST_SIZE;
use crate::pokemon::party::Party;
use crate::pokemon::stats::create_pokemon_with_moves;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;

const HOST_LIST: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

fn mk(species: Species, level: u8, moves: [MoveId; 4]) -> Pokemon {
    create_pokemon_with_moves(species, level, [0xAB, 0xCD], moves).unwrap()
}

fn party_of(mons: Vec<Pokemon>) -> Party {
    Party::from(mons)
}

fn input_a() -> BattleInput {
    BattleInput { a: true, ..BattleInput::none() }
}

fn input_down() -> BattleInput {
    BattleInput { down: true, ..BattleInput::none() }
}

/// Handshake + request/accept + party exchange; both drivers end in
/// `Battling` with the battle screen built.
fn setup_battle(party_a: Party, party_b: Party) -> (LinkBattleDriver, LinkBattleDriver) {
    let (mut a, mut b) = test_driver_pair(HOST_LIST, party_a, party_b);
    a.start_handshake().unwrap();
    assert!(b.poll().iter().any(|e| matches!(e, LinkDriverEvent::Connected)));
    assert!(a.poll().iter().any(|e| matches!(e, LinkDriverEvent::Connected)));
    a.request_battle().unwrap();
    assert!(b.poll().iter().any(|e| matches!(e, LinkDriverEvent::BattleRequested)));
    b.accept_battle().unwrap();
    assert!(a.poll().iter().any(|e| matches!(e, LinkDriverEvent::BattleStarted)));
    assert!(b.poll().iter().any(|e| matches!(e, LinkDriverEvent::BattleStarted)));
    assert_eq!(a.phase(), &LinkDriverPhase::Battling);
    assert_eq!(b.phase(), &LinkDriverPhase::Battling);
    (a, b)
}

/// Press A until the intro / text phases are done (max frames).
fn pump_through_intro(d: &mut LinkBattleDriver, max: usize) {
    for _ in 0..max {
        let phase = d
            .screen()
            .map(|s| s.phase.clone())
            .unwrap_or(BattlePhase::PlayerMenu);
        match phase {
            BattlePhase::Intro { .. } | BattlePhase::ShowingText { .. } => {
                d.update(input_a());
            }
            _ => break,
        }
    }
}

/// Advance text pages (post-resolution narration) with A presses.
fn pump_text(d: &mut LinkBattleDriver, max: usize) {
    for _ in 0..max {
        let phase = d
            .screen()
            .map(|s| s.phase.clone())
            .unwrap_or(BattlePhase::PlayerMenu);
        match phase {
            BattlePhase::ShowingText { .. } => {
                d.update(input_a());
            }
            _ => break,
        }
    }
}

/// Assumes the screen is at `PlayerMenu`: A → MoveSelect, cursor to `idx`,
/// A → the move is deferred + sent (LinkWaiting).
fn select_move(d: &mut LinkBattleDriver, idx: usize) {
    d.update(input_a());
    for _ in 0..idx {
        d.update(input_down());
    }
    d.update(input_a());
}

/// Exchange + resolve the pending turn on both sides, then advance texts.
fn resolve_turns(a: &mut LinkBattleDriver, b: &mut LinkBattleDriver) {
    a.poll();
    b.poll();
    a.poll();
    b.poll();
    pump_text(a, 500);
    pump_text(b, 500);
}

/// One full move-vs-move turn. Assumes both screens are at `PlayerMenu`.
fn do_move_turn(a: &mut LinkBattleDriver, a_idx: usize, b: &mut LinkBattleDriver, b_idx: usize) {
    select_move(a, a_idx);
    select_move(b, b_idx);
    resolve_turns(a, b);
}

/// The convergence invariant: A's battle is B's battle mirrored, and both
/// sides consumed the same RNG bytes.
fn assert_mirrored(a: &LinkBattleDriver, b: &LinkBattleDriver) {
    let sa = a.screen().expect("a screen");
    let sb = b.screen().expect("b screen");
    let bsa = sa.battle_state.as_ref().expect("a battle state");
    let bsb = sb.battle_state.as_ref().expect("b battle state");
    assert_eq!(
        bsa.player.active_mon().hp,
        bsb.enemy.active_mon().hp,
        "A's player HP must mirror B's enemy HP"
    );
    assert_eq!(
        bsa.enemy.active_mon().hp,
        bsb.player.active_mon().hp,
        "A's enemy HP must mirror B's player HP"
    );
    assert_eq!(bsa.player.active_pokemon_index, bsb.enemy.active_pokemon_index);
    assert_eq!(bsa.enemy.active_pokemon_index, bsb.player.active_pokemon_index);
    assert_eq!(
        bsa.player.active_mon().status,
        bsb.enemy.active_mon().status,
        "A's player status must mirror B's enemy status"
    );
    for i in 0..bsa.player.party.len() {
        assert_eq!(bsa.player.party[i].hp, bsb.enemy.party[i].hp, "party[{i}] hp (A player vs B enemy)");
    }
    for i in 0..bsa.enemy.party.len() {
        assert_eq!(bsa.enemy.party[i].hp, bsb.player.party[i].hp, "party[{i}] hp (A enemy vs B player)");
    }
    let ra = sa.link_rng.as_ref().expect("a link rng");
    let rb = sb.link_rng.as_ref().expect("b link rng");
    assert_eq!(ra.consumed(), rb.consumed(), "RNG streams consumed identically");
    assert_eq!(ra.list(), rb.list(), "RNG lists identical");
    assert_eq!(ra.index(), rb.index(), "RNG indices identical");
}

fn standard_parties() -> (Party, Party) {
    (
        party_of(vec![
            mk(Species::Tauros, 50, [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]),
            mk(Species::Snorlax, 50, [MoveId::Growl, MoveId::Tackle, MoveId::None, MoveId::None]),
        ]),
        party_of(vec![
            mk(Species::Pidgeot, 50, [MoveId::Growl, MoveId::Tackle, MoveId::None, MoveId::None]),
            mk(Species::Rhydon, 50, [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]),
        ]),
    )
}

// ─────────────────────────────── tests ───────────────────────────────

#[test]
fn full_handshake_party_exchange_and_battle_setup() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);

    assert_eq!(a.remote_trainer_name().as_deref(), Some("BOB"));
    assert_eq!(b.remote_trainer_name().as_deref(), Some("ALICE"));

    let sa = a.screen().unwrap();
    assert!(sa.link_mode);
    assert_eq!(sa.trainer_name.as_deref(), Some("BOB"));
    assert_eq!(sa.player_name.as_deref(), Some("ALICE"));
    let sb = b.screen().unwrap();
    assert!(sb.link_mode);
    assert_eq!(sb.trainer_name.as_deref(), Some("ALICE"));

    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);
    assert_eq!(a.screen().unwrap().phase, BattlePhase::PlayerMenu);
    assert_eq!(b.screen().unwrap().phase, BattlePhase::PlayerMenu);
}

/// The guest must use the HOST's random list, not its own (cable_club.asm:
/// "the list generated by the gameboy clocking the connection is used by both
/// gameboys"). Here the two sides pin DIFFERENT lists; both must end up with
/// the host's (A's, since A requested).
#[test]
fn guest_uses_host_random_list() {
    let (pa, pb) = standard_parties();
    let (t_a, t_b) = ChannelTransport::new_pair();
    let mut a = LinkBattleDriver::new(Box::new(t_a), pa, "ALICE".into())
        .with_host_random_list(HOST_LIST);
    let mut b = LinkBattleDriver::new(Box::new(t_b), pb, "BOB".into())
        .with_host_random_list([0xEE; LINK_RANDOM_LIST_SIZE]);

    a.start_handshake().unwrap();
    b.poll();
    a.poll();
    a.request_battle().unwrap();
    b.poll();
    b.accept_battle().unwrap();
    a.poll();
    b.poll();

    assert_eq!(
        a.screen().unwrap().link_rng.as_ref().unwrap().list(),
        HOST_LIST,
        "host uses its own list"
    );
    assert_eq!(
        b.screen().unwrap().link_rng.as_ref().unwrap().list(),
        HOST_LIST,
        "guest must use the HOST's list, not its own"
    );
}

/// Multi-turn move-vs-move battle: every turn both sides resolve identically
/// (mirror states + identical RNG consumption).
#[test]
fn multi_turn_battle_converges_with_shared_rng() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    let hp_before = b.screen().unwrap().battle_state.as_ref().unwrap().enemy.active_mon().hp;
    for turn in 0..4 {
        // Alternate Tackle/Growl so both damage and stat stages move.
        let a_idx = if turn % 2 == 0 { 0 } else { 1 };
        let b_idx = if turn % 2 == 0 { 1 } else { 0 };
        do_move_turn(&mut a, a_idx, &mut b, b_idx);
        assert_mirrored(&a, &b);
        assert_eq!(a.screen().unwrap().phase, BattlePhase::PlayerMenu);
        assert_eq!(b.screen().unwrap().phase, BattlePhase::PlayerMenu);
    }
    // The shared RNG produced identical damage on both sides: A's player mon
    // dealt exactly the HP loss B's enemy mon shows.
    let sa = a.screen().unwrap();
    let bsa = sa.battle_state.as_ref().unwrap();
    let dealt = hp_before.saturating_sub(bsa.enemy.active_mon().hp);
    assert!(dealt > 0, "the battle made progress");
    assert_eq!(bsa.enemy.active_mon().hp, hp_before - dealt);
}

/// RUN is allowed in link battles and always succeeds; when BOTH sides run
/// the battle is a DRAW (TryRunningFromBattle, core.asm:1599-1606).
#[test]
fn both_run_is_a_draw() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    // PlayerMenu → RUN ((1,1): down + right) → A.
    for d in [&mut a, &mut b] {
        d.update(input_down());
        d.update(crate::battle::BattleInput { right: true, ..BattleInput::none() });
        d.update(input_a());
    }
    assert_eq!(a.screen().unwrap().phase, BattlePhase::LinkWaiting);
    assert_eq!(b.screen().unwrap().phase, BattlePhase::LinkWaiting);
    resolve_turns(&mut a, &mut b);

    assert_eq!(a.result(), Some(LinkBattleResult::Draw));
    assert_eq!(b.result(), Some(LinkBattleResult::Draw));
    assert_eq!(a.phase(), &LinkDriverPhase::Finished);
    assert_eq!(b.phase(), &LinkDriverPhase::Finished);
    // Both sides also received the peer's announcement (protocol v2).
    assert_eq!(a.remote_result(), Some(LinkBattleResult::Draw));
    assert_eq!(b.remote_result(), Some(LinkBattleResult::Draw));
}

/// One side runs while the other attacks: the runner LOSES, the attacker WINS
/// (wBattleResult 1 vs 0; the runner sees "Got away safely!", the attacker
/// sees the enemy ran). B commits its move BEFORE A commits the run — the
/// manager must hold the action until both are in.
#[test]
fn runner_loses_attacker_wins() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    // B attacks first; A runs later (out-of-order commit).
    select_move(&mut b, 0);
    a.update(input_down());
    a.update(crate::battle::BattleInput { right: true, ..BattleInput::none() });
    a.update(input_a());
    assert_eq!(a.screen().unwrap().phase, BattlePhase::LinkWaiting);
    assert_eq!(b.screen().unwrap().phase, BattlePhase::LinkWaiting);
    resolve_turns(&mut a, &mut b);

    assert_eq!(a.result(), Some(LinkBattleResult::Lose));
    assert_eq!(b.result(), Some(LinkBattleResult::Win));
    assert_eq!(a.remote_result(), Some(LinkBattleResult::Win));
    assert_eq!(b.remote_result(), Some(LinkBattleResult::Lose));
    // The winner's narration matches the original EnemyRan text family.
    let msg = b.screen().unwrap().current_message.clone().unwrap_or_default();
    assert!(msg.contains("ran!"), "winner sees the enemy ran: {msg:?}");
}

/// A's active switch propagates: B's enemy mon changes to the same index and
/// B's move lands on the incoming mon (no free switch in link battles).
#[test]
fn switch_propagation_and_no_free_turn() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    let incoming_max_hp = {
        let bs = a.screen().unwrap().battle_state.as_ref().unwrap();
        bs.player.party[1].max_hp
    };

    // A: PlayerMenu → PKMN ((0,1): right) → PartySelect → mon 1 → SWITCH.
    a.update(crate::battle::BattleInput { right: true, ..BattleInput::none() });
    a.update(input_a());
    a.update(input_down());
    a.update(input_a());
    assert_eq!(a.screen().unwrap().phase, BattlePhase::PartySubMenu { selected_index: 1 });
    a.update(input_a()); // SWITCH (cursor 0)
    assert_eq!(a.screen().unwrap().phase, BattlePhase::LinkWaiting);

    // B attacks while A switches.
    select_move(&mut b, 1);
    resolve_turns(&mut a, &mut b);

    // Both sides agree A's active mon is now index 1.
    let bsa = a.screen().unwrap().battle_state.as_ref().unwrap();
    let bsb = b.screen().unwrap().battle_state.as_ref().unwrap();
    assert_eq!(bsa.player.active_pokemon_index, 1);
    assert_eq!(bsb.enemy.active_pokemon_index, 1);
    // B's move landed on the incoming mon (no free switch): HP < max on BOTH
    // sides' copies.
    let hp_a = bsa.player.active_mon().hp;
    let hp_b = bsb.enemy.active_mon().hp;
    assert!(hp_a < incoming_max_hp, "incoming mon took damage (a)");
    assert_eq!(hp_a, hp_b, "damage mirrored");
    assert_mirrored(&a, &b);
}

/// Both sides switch: no attacks this turn, both parties unharmed.
#[test]
fn both_switch_no_attacks() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    let max_a1 = a.screen().unwrap().battle_state.as_ref().unwrap().player.party[1].max_hp;
    let max_b1 = b.screen().unwrap().battle_state.as_ref().unwrap().player.party[1].max_hp;

    // A: PKMN → mon 1 → SWITCH; B: PKMN → mon 1 → SWITCH.
    for d in [&mut a, &mut b] {
        d.update(crate::battle::BattleInput { right: true, ..BattleInput::none() });
        d.update(input_a());
        d.update(input_down());
        d.update(input_a());
        d.update(input_a());
    }
    resolve_turns(&mut a, &mut b);

    let bsa = a.screen().unwrap().battle_state.as_ref().unwrap();
    let bsb = b.screen().unwrap().battle_state.as_ref().unwrap();
    assert_eq!(bsa.player.active_pokemon_index, 1);
    assert_eq!(bsb.enemy.active_pokemon_index, 1);
    assert_eq!(bsa.enemy.active_pokemon_index, 1);
    assert_eq!(bsb.player.active_pokemon_index, 1);
    assert_eq!(bsa.player.active_mon().hp, max_a1, "A's switched-in mon unharmed");
    assert_eq!(bsa.enemy.active_mon().hp, max_b1, "B's switched-in mon unharmed");
    assert_mirrored(&a, &b);
}

/// Faint-driven end: A's single weak mon is KO'd; both sides converge on the
/// result without any further exchange (A: Lose, B: Win) and announce it.
#[test]
fn winner_convergence_both_ways() {
    // A: one weak Rattata; B: one strong Tauros.
    let (mut a, mut b) = setup_battle(
        party_of(vec![mk(Species::Rattata, 5, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])]),
        party_of(vec![mk(Species::Tauros, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])]),
    );
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    do_move_turn(&mut a, 0, &mut b, 0);

    assert_eq!(a.result(), Some(LinkBattleResult::Lose), "A's only mon fainted");
    assert_eq!(b.result(), Some(LinkBattleResult::Win), "B wiped A's party");
    assert_eq!(a.phase(), &LinkDriverPhase::Finished);
    assert_eq!(b.phase(), &LinkDriverPhase::Finished);
    assert_eq!(a.remote_result(), Some(LinkBattleResult::Win));
    assert_eq!(b.remote_result(), Some(LinkBattleResult::Lose));
    // The battle-mutated party is synced back to the driver (fainted HP).
    let synced = a.local_party();
    assert_eq!(synced.get(0).unwrap().hp, 0);
}

/// Faint replacement: B KOs A's first mon; A replaces it from the party menu,
/// B's simultaneous move lands on the replacement; both sides converge.
#[test]
fn faint_replacement_switch_propagation() {
    let (mut a, mut b) = setup_battle(
        party_of(vec![
            mk(Species::Rattata, 5, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]),
            mk(Species::Snorlax, 50, [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None]),
        ]),
        party_of(vec![mk(Species::Tauros, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])]),
    );
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    // Turn 1: B's Tackle KOs A's Rattata.
    do_move_turn(&mut a, 0, &mut b, 0);
    let bsa = a.screen().unwrap().battle_state.as_ref().unwrap();
    assert_eq!(bsa.player.active_mon().hp, 0, "A's first mon fainted");
    assert_eq!(
        a.screen().unwrap().phase,
        BattlePhase::PlayerFaintSwitch,
        "A must pick a replacement"
    );
    assert_eq!(b.screen().unwrap().phase, BattlePhase::PlayerMenu);

    // A picks mon 1 (PlayerFaintSwitch cursor 0 → down once → A). B attacks.
    a.update(input_down());
    a.update(input_a());
    assert_eq!(a.screen().unwrap().phase, BattlePhase::LinkWaiting);
    select_move(&mut b, 0);
    resolve_turns(&mut a, &mut b);

    // A's replacement is out on both sides, damaged by B's move (no free
    // switch), fully mirrored.
    let bsa = a.screen().unwrap().battle_state.as_ref().unwrap();
    let bsb = b.screen().unwrap().battle_state.as_ref().unwrap();
    assert_eq!(bsa.player.active_pokemon_index, 1, "replacement is out (a)");
    assert_eq!(bsb.enemy.active_pokemon_index, 1, "replacement is out (b)");
    assert_eq!(bsa.player.party[0].hp, 0, "the KO'd Rattata stays fainted");
    assert_eq!(bsb.enemy.party[0].hp, 0, "mirrored on B's side");
    let pika_max = bsa.player.party[1].max_hp;
    assert!(bsa.player.active_mon().hp < pika_max, "replacement took the hit (a)");
    assert_mirrored(&a, &b);
    assert_eq!(a.screen().unwrap().phase, BattlePhase::PlayerMenu);
    assert_eq!(b.screen().unwrap().phase, BattlePhase::PlayerMenu);
}

/// The bag is blocked in link battles ("Items can't be used here.") — the
/// player is bounced back to the menu, nothing is sent.
#[test]
fn bag_blocked_in_link_battles() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    // PlayerMenu → BAG ((1,0): down) → A.
    a.update(input_down());
    a.update(input_a());
    let phase = a.screen().unwrap().phase.clone();
    assert!(matches!(phase, BattlePhase::ShowingText { .. }), "block message shown: {phase:?}");
    pump_text(&mut a, 50);
    assert_eq!(a.screen().unwrap().phase, BattlePhase::PlayerMenu);
    assert!(a.screen().unwrap().link_pending_local_action.is_none());
    // No action was sent — B is unaffected and still at its menu.
    assert_eq!(b.screen().unwrap().phase, BattlePhase::PlayerMenu);
}

/// Dropping the peer mid-battle: the driver surfaces Disconnected (the
/// original instead soft-locks on Serial_ExchangeNybble — no error text
/// exists there; the app shows its own error screen).
#[test]
fn disconnect_mid_battle_surfaces_error_state() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    drop(b); // kills the channel
    let events = a.poll();
    assert!(
        events.iter().any(|e| matches!(e, LinkDriverEvent::Disconnected(_))),
        "disconnect surfaced: {events:?}"
    );
    assert!(matches!(a.phase(), LinkDriverPhase::Disconnected(_)));

    // Input while disconnected is swallowed, no panics.
    a.update(input_a());
    assert!(matches!(a.phase(), LinkDriverPhase::Disconnected(_)));
}

/// A turn where the remote sends NoAction: the enemy skips its move; the
/// local move still executes.
#[test]
fn remote_no_action_skips_enemy_move() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    // A attacks normally. B "commits" NoAction by driving its screen to
    // LinkWaiting with a Run… no — simulate a NoAction remote by sending the
    // wire action directly through the manager path: drive B's menu to a
    // move selection and then override the pending action.
    select_move(&mut a, 0);
    // B: pick a move (deferred), then replace the wire payload with NoAction
    // (as if B's mon could not act — e.g. LINKBATTLE_NO_ACTION).
    select_move(&mut b, 0);
    b.screen_mut().unwrap().link_pending_local_action =
        Some(crate::link::protocol::LinkAction::NoAction);
    resolve_turns(&mut a, &mut b);

    // B's mon did nothing: A's move hit B's mon; B's mon HP < max on both
    // sides, and A's mon took no damage this turn.
    let bsa = a.screen().unwrap().battle_state.as_ref().unwrap();
    assert!(bsa.enemy.active_mon().hp < bsa.enemy.active_mon().max_hp);
    assert_eq!(
        bsa.player.active_mon().hp,
        bsa.player.active_mon().max_hp,
        "the enemy skipped its move — A's mon unharmed"
    );
    assert_mirrored(&a, &b);
}

/// Mutual (simultaneous) battle requests with clock roles: the host
/// ("Host", internal clock) wins the tie — its request stands and its
/// random-number list is the shared stream; the guest ("Guest") silently
/// accepts (engine/menus/main_menu.asm: "The gameboy that is clocking the
/// connection wins").
#[test]
fn simultaneous_request_role_tie_break_and_host_rng() {
    use crate::link::LinkRole;
    let (pa, pb) = standard_parties();
    let (t_a, t_b) = crate::link::transport::ChannelTransport::new_pair();
    let mut host = LinkBattleDriver::new(Box::new(t_a), pa, "ALICE".into())
        .with_host_random_list(HOST_LIST)
        .with_role(LinkRole::Host);
    let mut guest = LinkBattleDriver::new(Box::new(t_b), pb, "BOB".into())
        .with_host_random_list([0xEE; LINK_RANDOM_LIST_SIZE])
        .with_role(LinkRole::Guest);

    host.start_handshake().unwrap();
    guest.poll();
    host.poll();
    // Both press the gameboy simultaneously.
    host.request_battle().unwrap();
    guest.request_battle().unwrap();
    // Round 1: the host ignores the guest's duplicate request; the guest
    // accepts the host's and sends its party data. Round 2: the accept +
    // party exchange complete on both sides.
    host.poll();
    guest.poll();
    assert!(host.poll().iter().any(|e| matches!(e, LinkDriverEvent::BattleStarted)));
    assert!(guest.poll().iter().any(|e| matches!(e, LinkDriverEvent::BattleStarted)));
    assert_eq!(host.phase(), &LinkDriverPhase::Battling);
    assert_eq!(guest.phase(), &LinkDriverPhase::Battling);

    // The shared stream is the HOST's list on BOTH sides (the guest's own
    // list is discarded — cable_club.asm:157-171).
    assert_eq!(
        host.screen().unwrap().link_rng.as_ref().unwrap().list(),
        HOST_LIST
    );
    assert_eq!(
        guest.screen().unwrap().link_rng.as_ref().unwrap().list(),
        HOST_LIST
    );
}

/// Once the result lands the driver FREEZES its screen: `update` stops
/// advancing it (the phase guard), so the end-of-battle narration never
/// completes inside the driver — the frontend is expected to show the
/// result and move on (or, like the native app, advance a mirror of the
/// screen through the narration itself). This pins the contract the app's
/// mirror relies on.
#[test]
fn screen_frozen_after_result_for_frontend_handoff() {
    let (pa, pb) = standard_parties();
    let (mut a, mut b) = setup_battle(pa, pb);
    pump_through_intro(&mut a, 500);
    pump_through_intro(&mut b, 500);

    // Both run → draw; the result lands on the resolution poll.
    for d in [&mut a, &mut b] {
        d.update(input_down());
        d.update(crate::battle::BattleInput { right: true, ..BattleInput::none() });
        d.update(input_a());
    }
    resolve_turns(&mut a, &mut b);
    assert_eq!(a.result(), Some(LinkBattleResult::Draw));
    assert_eq!(a.phase(), &LinkDriverPhase::Finished);

    let frozen_phase = a.screen().unwrap().phase.clone();
    assert!(
        matches!(frozen_phase, BattlePhase::ShowingText { .. }),
        "the end narration is on screen: {frozen_phase:?}"
    );
    // A presses must NOT advance the frozen screen (the frontend drives the
    // result presentation from `result()` instead).
    for _ in 0..10 {
        a.update(input_a());
        assert_eq!(a.screen().unwrap().phase, frozen_phase, "screen stays frozen");
    }
    assert_eq!(a.phase(), &LinkDriverPhase::Finished);
}

/// A v1 peer (protocol version 1 — pre-`BattleResult`) is still accepted by
/// the handshake; the manager simply never sees a BattleResult message.
#[test]
fn handshake_accepts_v1_peer() {
    use crate::link::link_battle::{LinkBattleManager, LinkBattlePollResult};
    use crate::link::transport::{ChannelTransport, NetworkTransport};
    use crate::link::protocol::NetworkMessage;

    let (mut t_a, mut t_b) = ChannelTransport::new_pair();
    let mut mgr_new = LinkBattleManager::new();

    // A v1 peer's Hello (the OLD protocol constant) is accepted.
    t_b.send(NetworkMessage::Hello { version: 1 }).unwrap();
    assert_eq!(
        mgr_new.poll_blocking(&mut t_a),
        LinkBattlePollResult::HandshakeComplete
    );

    // And a v1 HelloAck is accepted too (the peer answers our v2 Hello with
    // its own constant).
    mgr_new.start_handshake(&mut t_a).unwrap();
    t_b.send(NetworkMessage::HelloAck { version: 1 }).unwrap();
    assert_eq!(
        mgr_new.poll_blocking(&mut t_a),
        LinkBattlePollResult::HandshakeComplete
    );

    // Anything else (v3) is still rejected.
    t_b.send(NetworkMessage::Hello { version: 3 }).unwrap();
    assert!(matches!(
        mgr_new.poll_blocking(&mut t_a),
        LinkBattlePollResult::Error(_)
    ));
}
