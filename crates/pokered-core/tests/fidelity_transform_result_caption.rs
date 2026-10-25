//! Transform prints its result after the move, retaining the user's nickname
//! and naming the copied species rather than the target's nickname.
use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species};

fn fixture(enemy: bool, first: bool, names: u8, condition: &str) -> BattleScreen {
    let mon = |species, id| create_pokemon_with_moves(species, 40, [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None]).unwrap();
    let mut actor = mon(Species::Ditto, MoveId::Transform);
    let mut target = mon(Species::Tangela, MoveId::Splash);
    if names & 1 != 0 { actor.set_nickname("COPY"); }
    if names & 2 != 0 { target.set_nickname("VINE"); }
    if condition == "frozen" { actor.status = state::StatusCondition::Freeze; }
    let (player, foe) = if enemy { (target, actor) } else { (actor, target) };
    let mut b = BattleScreen::from_parties(true, &[player], &[foe], None);
    b.link_enemy_move_override = Some((if enemy { MoveId::Transform } else { MoveId::Splash }, 0));
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

fn turn(b: &mut BattleScreen, expected_draws: u64) -> String {
    let initial = b.battle_state.as_ref().unwrap();
    let hp = [initial.player.active_mon().hp, initial.enemy.active_mon().hp];
    let draws = b.link_rng.as_ref().unwrap().consumed();
    let mut a = BattleInput::none(); a.a = true;
    b.update_frame(a); b.update_frame(a);
    let mut messages = Vec::new(); let mut previous = None;
    for frame in 0..4000 {
        if b.current_message != previous {
            if let Some(message) = &b.current_message {
                messages.push(message.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            previous = b.current_message.clone();
        }
        if b.phase == BattlePhase::PlayerMenu {
            let bs = b.battle_state.as_ref().unwrap();
            assert_eq!([bs.player.active_mon().hp, bs.enemy.active_mon().hp], hp);
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(), draws + expected_draws);
            return messages.join(" ");
        }
        if b.presentation.waiting { b.complete_move_presentation(); }
        b.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
    }
    panic!("turn did not finish: {:?}", b.phase);
}

fn actor_name(enemy: bool, names: u8) -> String {
    format!("{}{}", if enemy { "Enemy " } else { "" }, if names & 1 != 0 { "COPY" } else { "DITTO" })
}

#[test]
fn successful_transform_announces_original_user_and_copied_species_in_both_orders() {
    let mut errors = Vec::new();
    for enemy in [false, true] {
        for first in [false, true] {
            for names in 0..=3 {
                let mut b = fixture(enemy, first, names, "normal");
                let text = turn(&mut b, 0); let actor = actor_name(enemy, names);
                let expected = format!("{actor} transformed into TANGELA!");
                let used = text.find(&format!("{actor} used TRANSFORM!")).unwrap();
                let result = text.find(&expected);
                let bs = b.battle_state.as_ref().unwrap();
                let mon = if enemy { bs.enemy.active_mon() } else { bs.player.active_mon() };
                assert_eq!(mon.species, Species::Tangela);
                if text.matches(&expected).count() != 1 || result.is_none_or(|index| index <= used) {
                    errors.push(format!("enemy={enemy}, first={first}, names={names}: {text}"));
                }
            }
        }
    }
    assert!(errors.is_empty(), "{} missing/wrong results:\n{}", errors.len(), errors.join("\n"));
}

#[test]
fn blocked_actions_have_no_result_and_original_transform_invulnerability_bugs_remain() {
    let mut errors = Vec::new();
    for enemy in [false, true] {
        for condition in ["frozen", "self_invulnerable", "target_invulnerable"] {
            let mut b = fixture(enemy, true, 3, condition);
            let text = turn(&mut b, 0);
            // Original player checks its own invulnerability; enemy checks
            // hWhoseTurn instead. Neither checks the intended target correctly.
            let success = condition != "frozen" && (enemy || condition != "self_invulnerable");
            let expected = format!("{} transformed into TANGELA!", actor_name(enemy, 3));
            if text.matches(&expected).count() != usize::from(success) {
                errors.push(format!("enemy={enemy}, {condition}, success={success}: {text}"));
            }
            let bs = b.battle_state.as_ref().unwrap();
            let mon = if enemy { bs.enemy.active_mon() } else { bs.player.active_mon() };
            assert_eq!(mon.species, if success { Species::Tangela } else { Species::Ditto });
        }
    }
    assert!(errors.is_empty(), "{} wrong results:\n{}", errors.len(), errors.join("\n"));
}

#[test]
fn copied_move_next_turn_does_not_repeat_the_transform_result() {
    for enemy in [false, true] {
        let mut b = fixture(enemy, true, 3, "normal");
        turn(&mut b, 0);
        // Copying the foe's Speed creates a tie: one turn-order byte, then
        // both copied/ordinary Splash actions consume no further random bytes.
        b.link_enemy_move_override = Some((MoveId::Splash, 0));
        let text = turn(&mut b, 1);
        assert!(!text.contains("transformed into"), "{text}");
        assert!(text.contains(&format!("{} used SPLASH!", actor_name(enemy, 3))), "{text}");
    }
}
