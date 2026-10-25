//! Rest ends its sleep caption with DONE; regained-health and failure use PROMPT.
use pokered_core::battle::{state::StatusCondition, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn fixture(mv: MoveId, enemy: bool, first: bool, status: StatusCondition, deficit: u16, haze: bool) -> BattleScreen {
    let make = |species, mv| create_pokemon_with_moves(species, 100, [0x9a, 0x78],
        [mv, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut actor = make(Species::Chansey, mv); actor.set_nickname("WARD");
    let mut other = make(Species::Snorlax, if haze { MoveId::Haze } else { MoveId::Splash }); other.set_nickname("TARGET");
    let (player, foe) = if enemy { (other, actor) } else { (actor, other) };
    let mut b = BattleScreen::from_parties(true, &[player], &[foe], None);
    b.link_rng = Some(LinkRng::new([255, 255, 0, 255, 3, 82, 73, 64, 55, 46]));
    b.link_enemy_move_override = Some((foe.moves[0], 0));
    let bs = b.battle_state.as_mut().unwrap();
    let (actor, other) = if enemy { (&mut bs.enemy, &mut bs.player) } else { (&mut bs.player, &mut bs.enemy) };
    // Even after paralysis quarters speed, the intended first actor is faster.
    actor.active_mon_mut().speed = if first { 800 } else { 100 };
    other.active_mon_mut().speed = if first { 100 } else { 200 };
    let mon = actor.active_mon_mut(); mon.hp = mon.max_hp - deficit; mon.status = status;
    b.set_presentation_enabled(true); b.phase = BattlePhase::PlayerMenu; b
}

fn actor(b: &BattleScreen, enemy: bool) -> &pokered_core::battle::state::BattlerState {
    let bs = b.battle_state.as_ref().unwrap(); if enemy { &bs.enemy } else { &bs.player }
}

fn reach(b: &mut BattleScreen, caption: &str) {
    let a = BattleInput { a: true, ..BattleInput::none() };
    b.update_frame(a); b.update_frame(a);
    for frame in 0..5000 {
        if b.current_message.as_ref().is_some_and(|m| m.contains(caption)) { return; }
        if b.presentation.waiting { b.complete_move_presentation(); }
        b.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
    }
    panic!("missing {caption}: {:?}", b.phase);
}

fn reaches_health_without_input(b: &mut BattleScreen) -> bool {
    for _ in 0..600 {
        b.update_frame(BattleInput::none());
        if b.current_message.as_ref().is_some_and(|m| m.contains("regained health!")) { return true; }
    }
    false
}

fn assert_prompt_waits(b: &mut BattleScreen) {
    let message = b.current_message.clone();
    for _ in 0..100 { b.update_frame(BattleInput::none()); }
    assert_eq!(b.current_message, message);
    assert!(matches!(b.phase, BattlePhase::ShowingText { .. }));
}

#[test]
fn rest_done_waits_for_animation_and_hp_then_advances_without_an_ack() {
    let mut errors = Vec::new();
    for enemy in [false, true] { for first in [false, true] { for status in [StatusCondition::None, StatusCondition::Poison] {
        let mut b = fixture(MoveId::Rest, enemy, first, status, 30, false);
        let hp = actor(&b, enemy).active_mon().hp;
        reach(&mut b, if status == StatusCondition::None { "started sleeping!" } else { "became healthy!" });
        assert!(b.presentation.waiting);
        let message = b.current_message.clone();
        for _ in 0..50 { b.update_frame(BattleInput::none()); }
        assert!(b.presentation.waiting); assert_eq!(b.current_message, message);
        assert_eq!(if enemy { b.enemy_hp } else { b.player_hp }, hp);
        b.complete_move_presentation();
        if !reaches_health_without_input(&mut b) {
            errors.push(format!("enemy={enemy} first={first} status={status:?}: {:?}", b.current_message));
        } else {
            assert!(!b.hp_bar_anim.is_active());
            assert_eq!(if enemy { b.enemy_hp } else { b.player_hp }, actor(&b, enemy).active_mon().max_hp);
            assert_prompt_waits(&mut b);
        }
    }}}
    assert!(errors.is_empty(), "{} Rest sleep pages still require an ACK:\n{}", errors.len(), errors.join("\n"));
}

#[test]
fn failed_and_frozen_rest_keep_their_existing_prompt() {
    for enemy in [false, true] { for (deficit, status) in [
        (0, StatusCondition::None), (255, StatusCondition::None), (511, StatusCondition::None), (30, StatusCondition::Freeze),
    ] {
        let mut b = fixture(MoveId::Rest, enemy, true, status, deficit, false);
        let hp = actor(&b, enemy).active_mon().hp;
        reach(&mut b, if status == StatusCondition::Freeze { "frozen solid" } else { "But, it failed!" });
        assert_prompt_waits(&mut b);
        assert_eq!(if enemy { b.enemy_hp } else { b.player_hp }, hp);
        assert_eq!(actor(&b, enemy).active_mon().hp, hp);
        assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
    }}
}

#[test]
fn headless_rest_done_continues_without_requiring_a_visual_ack() {
    let mut errors = Vec::new();
    for enemy in [false, true] { for status in [StatusCondition::None, StatusCondition::Poison] {
        let mut b = fixture(MoveId::Rest, enemy, true, status, 30, false);
        b.set_presentation_enabled(false);
        reach(&mut b, if status == StatusCondition::None { "started sleeping!" } else { "became healthy!" });
        assert!(!b.presentation.waiting);
        if !reaches_health_without_input(&mut b) { errors.push(format!("enemy={enemy} status={status:?}")); }
        else { assert_prompt_waits(&mut b); }
    }}
    assert!(errors.is_empty(), "{} headless Rest pages still require an ACK: {errors:?}", errors.len());
}
