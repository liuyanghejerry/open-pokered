//! Copying moves announce the original success/failure result while retaining
//! the target's nickname and the original Transform invulnerability bugs.
use pokered_core::battle::{state, BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

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

fn turn(b: &mut BattleScreen, enemy: bool) -> String {
    let initial = b.battle_state.as_ref().unwrap();
    let hp = [initial.player.active_mon().hp, initial.enemy.active_mon().hp];
    let actor = if enemy { &initial.enemy } else { &initial.player };
    let pp = actor.active_mon().pp[0]; let mv = actor.active_mon().moves[0];
    let frozen = actor.active_mon().status == state::StatusCondition::Freeze;
    let draws = b.link_rng.as_ref().unwrap().consumed();
    let a = BattleInput { a: true, ..BattleInput::none() };
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
            let actor = if enemy { &bs.enemy } else { &bs.player };
            assert_eq!([bs.player.active_mon().hp, bs.enemy.active_mon().hp], hp);
            assert_eq!(b.link_rng.as_ref().unwrap().consumed(), draws);
            // Successful Transform replaces moves/PP. EnemyCanExecuteMove
            // omits the player's DecrementPP call for unchanged move slots.
            if actor.active_mon().moves[0] == mv {
                assert_eq!(actor.active_mon().pp[0], pp - u8::from(!frozen && !enemy));
            }
            return messages.join(" ");
        }
        if b.presentation.waiting { b.complete_move_presentation(); }
        b.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
    }
    panic!("turn did not finish: {:?}", b.phase);
}

fn actor_name(enemy: bool, names: u8) -> String {
    format!("{}{}", if enemy { "Enemy " } else { "" }, if names & 1 != 0 { "COPY" } else { "MEWTWO" })
}
fn check(errors: &mut Vec<String>, b: &BattleScreen, mv: MoveId, enemy: bool, names: u8, success: bool, text: &str) {
    let actor = actor_name(enemy, names);
    let result = if mv == MoveId::Conversion {
        format!("Converted type to {}{}'s!", if enemy { "" } else { "Enemy " }, if names & 2 != 0 { "TANK" } else { "SNORLAX" })
    } else { format!("{actor} transformed into SNORLAX!") };
    let used = text.find(&format!("{actor} used {}!", if mv == MoveId::Conversion { "CONVERSION" } else { "TRANSFORM" }));
    let outcome = text.find(if success { &result } else { "But, it failed!" });
    if text.matches(&result).count() != usize::from(success)
        || text.matches("But, it failed!").count() != usize::from(!success)
        || text.contains("attack missed")
        || !matches!((used, outcome), (Some(a), Some(b)) if a < b) {
        errors.push(format!("{mv:?} enemy={enemy} names={names} success={success}: {text}"));
    }
    let bs = b.battle_state.as_ref().unwrap();
    let side = if enemy { &bs.enemy } else { &bs.player };
    assert_eq!(side.active_mon().species, if mv == MoveId::Transform && success { Species::Snorlax } else { Species::Mewtwo });
    if mv == MoveId::Conversion {
        let expected = if success { Some(PokemonType::Normal) } else { None };
        assert_eq!((side.conversion_type1, side.conversion_type2), (expected, expected));
    }
}
fn finish(errors: Vec<String>) {
    assert!(errors.is_empty(), "{} wrong results (first six):\n{}", errors.len(), errors.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn conversion_names_its_target_in_both_orders_and_nickname_combinations() {
    let mut errors = Vec::new();
    for enemy in [false, true] { for first in [false, true] { for names in 0..=3 {
        let mut b = fixture(MoveId::Conversion, enemy, first, names, "normal");
        let text = turn(&mut b, enemy);
        check(&mut errors, &b, MoveId::Conversion, enemy, names, true, &text);
    }}}
    finish(errors);
}

#[test]
fn copying_failure_text_preserves_original_invulnerability_guards() {
    let mut errors = Vec::new();
    for mv in [MoveId::Conversion, MoveId::Transform] { for enemy in [false, true] {
        for first in [false, true] { for condition in ["self_invulnerable", "target_invulnerable"] {
            let mut b = fixture(mv, enemy, first, 3, condition);
            // Conversion checks its target. Original player Transform checks
            // itself; enemy Transform's hWhoseTurn bug never rejects this flag.
            let success = if mv == MoveId::Conversion { condition != "target_invulnerable" }
                else { enemy || condition != "self_invulnerable" };
            let text = turn(&mut b, enemy);
            check(&mut errors, &b, mv, enemy, 3, success, &text);
        }}
    }}
    finish(errors);
}

#[test]
fn repeated_conversion_publishes_one_result_per_use() {
    let mut errors = Vec::new();
    for enemy in [false, true] {
        let mut b = fixture(MoveId::Conversion, enemy, true, 3, "normal");
        for _ in 0..2 {
            let text = turn(&mut b, enemy);
            check(&mut errors, &b, MoveId::Conversion, enemy, 3, true, &text);
        }
    }
    finish(errors);
}

#[test]
fn frozen_copying_users_have_no_move_or_result_caption() {
    for mv in [MoveId::Conversion, MoveId::Transform] { for enemy in [false, true] { for first in [false, true] {
        let mut b = fixture(mv, enemy, first, 3, "frozen");
        let text = turn(&mut b, enemy);
        assert!(text.contains(&format!("{} is frozen solid!", actor_name(enemy, 3))), "{text}");
        assert!(!text.contains("used CONVERSION") && !text.contains("used TRANSFORM"));
        assert!(!text.contains("Converted type") && !text.contains("transformed into") && !text.contains("But, it failed!"));
    }}}
}
