//! HealEffect captions and the Rest sleep-text/animation/healing sequence.
//! Full HP and the original 255/511-deficit failures remain failures.
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

fn turn(b: &mut BattleScreen, mv: MoveId, enemy: bool) -> (String, Vec<bool>) {
    let a = BattleInput { a: true, ..BattleInput::none() };
    b.update_frame(a); b.update_frame(a);
    let mut out = Vec::new(); let mut prev = None; let mut serial = None; let mut animations = Vec::new();
    for frame in 0..5000 {
        if b.current_message != prev {
            if let Some(m) = &b.current_message { out.push(m.split_whitespace().collect::<Vec<_>>().join(" ")); }
            prev = b.current_message.clone();
        }
        if let Some((id, move_id, player)) = b.presentation.current_move() {
            if serial != Some(id) && move_id == mv && player != enemy {
                animations.push(b.presentation.current_move_missed()); serial = Some(id);
            }
        }
        if b.phase == BattlePhase::PlayerMenu { return (out.join(" "), animations); }
        if b.presentation.waiting { b.complete_move_presentation(); }
        b.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
    }
    panic!("turn did not drain: {:?}", b.phase);
}

fn finish(errors: Vec<String>) {
    assert!(errors.is_empty(), "{} wrong results (first six):\n{}", errors.len(), errors.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn recovery_success_failure_and_frozen_results_preserve_hp_pp_and_rng() {
    let mut errors = Vec::new();
    for mv in [MoveId::Recover, MoveId::Softboiled, MoveId::Rest] { for enemy in [false, true] { for first in [false, true] {
        let mut cases = vec![(30, StatusCondition::None), (0, StatusCondition::None),
            (255, StatusCondition::None), (511, StatusCondition::None), (30, StatusCondition::Freeze)];
        if mv == MoveId::Rest { cases.extend([(30, StatusCondition::Poison), (30, StatusCondition::Burn), (30, StatusCondition::Paralysis)]); }
        for (deficit, status) in cases {
            let mut b = fixture(mv, enemy, first, status, deficit, false);
            let old = *actor(&b, enemy).active_mon();
            let (text, animation) = turn(&mut b, mv, enemy);
            let frozen = status == StatusCondition::Freeze;
            let success = !frozen && deficit == 30;
            let mon = actor(&b, enemy).active_mon();
            assert_eq!(mon.hp, if success { mon.max_hp } else { old.hp });
            assert_eq!(mon.pp[0], old.pp[0] - u8::from(!frozen && !enemy));
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(), u64::from(status == StatusCondition::Paralysis));
            assert_eq!(mon.status, if mv == MoveId::Rest && success { StatusCondition::Sleep(2) } else { status });
            let name = if enemy { "Enemy WARD" } else { "WARD" };
            let result = format!("{name} regained health!");
            let sleep = format!("{name} {}", if status == StatusCondition::None { "started sleeping!" } else { "fell asleep and became healthy!" });
            if text.matches(&result).count() != usize::from(success)
                || text.matches("But, it failed!").count() != usize::from(!frozen && !success)
                || text.matches(&sleep).count() != usize::from(success && mv == MoveId::Rest)
                || animation != if frozen { vec![] } else { vec![!success] }
                || text.contains("attack missed") || text.contains("WARD fell asleep!") {
                errors.push(format!("{mv:?} enemy={enemy} first={first} deficit={deficit} status={status:?}: {text}"));
            }
            if let Some(index) = text.find(&result) { assert_eq!(index < text.find("used SPLASH!").unwrap(), first); }
        }
    }}}
    finish(errors);
}

#[test]
fn haze_does_not_rewrite_the_status_rest_saw_during_its_action() {
    let mut errors = Vec::new();
    for enemy in [false, true] { for first in [false, true] { for status in [StatusCondition::None, StatusCondition::Poison, StatusCondition::Burn] {
        let mut b = fixture(MoveId::Rest, enemy, first, status, 30, true);
        let (text, animation) = turn(&mut b, MoveId::Rest, enemy);
        let name = if enemy { "Enemy WARD" } else { "WARD" };
        let had_status = first && status != StatusCondition::None;
        let sleep = format!("{name} {}", if had_status { "fell asleep and became healthy!" } else { "started sleeping!" });
        if !text.contains(&sleep) || text.matches("regained health!").count() != 1 || animation != [false] {
            errors.push(format!("enemy={enemy} first={first} status={status:?}: {text}"));
        }
        if let Some(index) = text.find(&sleep) { assert_eq!(index < text.find("used HAZE!").unwrap(), first); }
        let mon = actor(&b, enemy).active_mon(); assert_eq!(mon.hp, mon.max_hp);
        assert_eq!(mon.status, if first { StatusCondition::None } else { StatusCondition::Sleep(2) });
        assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
    }}}
    finish(errors);
}

#[test]
fn rest_keeps_burn_and_paralysis_working_stats_on_the_next_sleep_turn() {
    for enemy in [false, true] { for status in [StatusCondition::Burn, StatusCondition::Paralysis] {
        let mut b = fixture(MoveId::Rest, enemy, true, status, 30, false);
        let mon = *actor(&b, enemy).active_mon();
        let expected = [if status == StatusCondition::Burn { mon.attack / 2 } else { mon.attack }, mon.defense,
            if status == StatusCondition::Paralysis { mon.speed / 4 } else { mon.speed }, mon.special];
        turn(&mut b, MoveId::Rest, enemy);
        assert_eq!(actor(&b, enemy).staged_badge_stats, Some(expected));
        let draws = b.link_rng.as_ref().unwrap().consumed();
        let (text, animation) = turn(&mut b, MoveId::Rest, enemy);
        assert_eq!(actor(&b, enemy).staged_badge_stats, Some(expected));
        assert_eq!(actor(&b, enemy).active_mon().status, StatusCondition::Sleep(1));
        assert!(text.contains("fast asleep") && !text.contains("regained health") && !text.contains("became healthy"));
        assert!(animation.is_empty()); assert_eq!(b.link_rng.as_ref().unwrap().consumed(), draws);
    }}
}

#[test]
fn rest_animation_follows_sleep_text_and_health_text_follows_hp_refill() {
    for enemy in [false, true] { for status in [StatusCondition::None, StatusCondition::Poison] {
        let mut b = fixture(MoveId::Rest, enemy, true, status, 30, false);
        let hp = actor(&b, enemy).active_mon().hp; let max = actor(&b, enemy).active_mon().max_hp;
        let a = BattleInput { a: true, ..BattleInput::none() }; b.update_frame(a); b.update_frame(a);
        let mut animated = false; let mut result = false;
        for frame in 0..5000 {
            let message = b.current_message.as_deref().unwrap_or("");
            if message.contains("used REST!") {
                assert!(b.presentation.current_move().is_none());
                assert_eq!(if enemy { b.enemy_hp } else { b.player_hp }, hp);
            }
            if b.presentation.waiting && b.presentation.current_move().is_some_and(|(_, mv, _)| mv == MoveId::Rest) {
                assert!(message.contains("started sleeping!") || message.contains("became healthy!"), "{message}");
                assert!(!animated); animated = true;
                assert_eq!(if enemy { b.enemy_hp } else { b.player_hp }, hp);
            }
            if message.contains("regained health!") {
                assert!(animated); assert_eq!(if enemy { b.enemy_hp } else { b.player_hp }, max);
                assert!(!b.hp_bar_anim.is_active()); result = true;
            }
            if b.phase == BattlePhase::PlayerMenu { break; }
            if b.presentation.waiting { b.complete_move_presentation(); }
            b.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
        }
        assert!(animated && result); assert_eq!(b.phase, BattlePhase::PlayerMenu);
    }}
}
