//! Transform identity is published with its result, after the animation.
use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn fixture(mv: MoveId, enemy: bool, first: bool, names: u8, condition: &str) -> BattleScreen {
    let make = |species, id| create_pokemon_with_moves(species, 40, [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut actor = make(Species::Mewtwo, mv);
    let mut target = make(Species::Snorlax, MoveId::Splash);
    if names & 1 != 0 { actor.set_nickname("COPY"); }
    if names & 2 != 0 { target.set_nickname("TANK"); }
    if condition == "frozen" { actor.status = state::StatusCondition::Freeze; }
    let (player, foe) = if enemy { (target, actor) } else { (actor, target) };
    let mut b = BattleScreen::from_parties(true, &[player], &[foe], None);
    b.link_enemy_move_override = Some((foe.moves[0], 0));
    b.link_rng = Some(LinkRng::new([255, 255, 0, 255, 3, 82, 73, 64, 55, 46]));
    let bs = b.battle_state.as_mut().unwrap();
    let (actor, target) = if enemy { (&mut bs.enemy, &mut bs.player) } else { (&mut bs.player, &mut bs.enemy) };
    actor.active_mon_mut().speed = if first { 200 } else { 100 };
    target.active_mon_mut().speed = if first { 100 } else { 200 };
    if condition == "self_invulnerable" { actor.set_status1(state::status1::INVULNERABLE); }
    if condition == "target_invulnerable" { target.set_status1(state::status1::INVULNERABLE); }
    b.set_presentation_enabled(true); b.phase = BattlePhase::PlayerMenu;
    b
}

fn exercise(enemy: bool, first: bool, condition: &str, enabled: bool) -> Option<String> {
    let mut b = fixture(MoveId::Transform, enemy, first, 3, condition);
    b.set_presentation_enabled(enabled);
    let success = condition != "frozen" && (enemy || condition != "self_invulnerable");
    let initial = b.battle_state.as_ref().unwrap();
    let hp = [initial.player.active_mon().hp, initial.enemy.active_mon().hp];
    let a = BattleInput { a: true, ..BattleInput::none() };
    b.update_frame(a); b.update_frame(a);
    let mut result_seen = false; let mut error = None;
    for frame in 0..4000 {
        result_seen |= b.current_message.as_ref().is_some_and(|m| m.contains("transformed"));
        let displayed = if enemy { b.enemy_species } else { b.player_species };
        let expected = if success && (!enabled || result_seen || b.phase == BattlePhase::PlayerMenu) { Species::Snorlax } else { Species::Mewtwo };
        if displayed != expected && error.is_none() {
            error = Some(format!("enemy={enemy}, first={first}, {condition}, enabled={enabled}, frame={frame}: {displayed:?} expected {expected:?}, message={:?}", b.current_message));
        }
        if b.phase == BattlePhase::PlayerMenu {
            let bs = b.battle_state.as_ref().unwrap();
            assert_eq!([bs.player.active_mon().hp, bs.enemy.active_mon().hp], hp);
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(), 0);
            assert_eq!(result_seen, success);
            return error;
        }
        if b.presentation.waiting { b.complete_move_presentation(); }
        b.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
    }
    panic!("turn did not drain: {:?}", b.phase);
}
#[test]
fn transform_identity_waits_for_its_result_on_both_sides_and_orders() {
    let mut errors = Vec::new();
    for enemy in [false, true] { for first in [false, true] {
        for condition in ["normal", "self_invulnerable", "target_invulnerable", "frozen"] {
            if let Some(error) = exercise(enemy, first, condition, true) { errors.push(error); }
        }
    }}
    assert!(errors.is_empty(), "{} early identities:\n{}", errors.len(), errors.join("\n"));
}
#[test]
fn headless_transform_keeps_immediate_final_identity() {
    for enemy in [false, true] { for first in [false, true] {
        assert!(exercise(enemy, first, "normal", false).is_none());
    }}
}
