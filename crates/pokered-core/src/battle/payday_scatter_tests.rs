//! Production caller regressions for the actual PayDayEffect success receipt.
//! Deterministic core scenarios, not native Game recordings.
use super::*;
use crate::battle::state::status1::INVULNERABLE;
use crate::battle::state::status2::{HAS_SUBSTITUTE_UP, USING_X_ACCURACY};
use crate::pokemon::stats::create_pokemon_with_moves;

const COINS: &str = "Coins scattered everywhere!";

fn mon(species: Species, level: u8, move_: MoveId) -> state::Pokemon {
    create_pokemon_with_moves(
        species,
        level,
        [0x9a, 0x78],
        [move_, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap()
}

fn screen(
    player: state::Pokemon,
    enemy: state::Pokemon,
    class: Option<TrainerClass>,
) -> BattleScreen {
    let mut s = BattleScreen::from_parties(class.is_none(), &[player], &[enemy], class);
    s.phase = BattlePhase::PlayerMenu;
    s.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    let bs = s.battle_state.as_mut().unwrap();
    // A true accuracy bypass isolates damage/effect success, including immunity.
    // It does not bypass the original zero-damage/immune miss.
    bs.player.set_status2(USING_X_ACCURACY);
    bs.enemy.set_status2(USING_X_ACCURACY);
    s
}

fn pages(s: &BattleScreen) -> Vec<String> {
    match &s.phase {
        BattlePhase::ShowingText { messages, .. } => messages
            .iter()
            .map(|line| line.replace('\n', " "))
            .collect(),
        _ => Vec::new(),
    }
}

fn pot(s: &BattleScreen) -> u32 {
    s.battle_state.as_ref().unwrap().total_payday_money
}

fn a() -> BattleInput {
    BattleInput {
        a: true,
        ..BattleInput::none()
    }
}

fn finish_text(s: &mut BattleScreen) {
    for frame in 0..400 {
        if !matches!(s.phase, BattlePhase::ShowingText { .. }) {
            return;
        }
        s.update_frame(if frame % 2 == 0 {
            a()
        } else {
            BattleInput::none()
        });
    }
    panic!("text did not return: {:?}", s.phase);
}

fn coin_count(lines: &[String]) -> usize {
    lines.iter().filter(|line| line.as_str() == COINS).count()
}

#[test]
fn both_turn_callers_reject_ghost_but_accept_real_damage_without_missed_log_guessing() {
    let mut player = screen(
        mon(Species::Meowth, 50, MoveId::PayDay),
        mon(Species::Gastly, 100, MoveId::Splash),
        None,
    );
    let hp = player.battle_state.as_ref().unwrap().enemy.active_mon().hp;
    player.execute_turn_with_move(0);
    assert_eq!(pot(&player), 0);
    assert_eq!(coin_count(&pages(&player)), 0);
    assert_eq!(
        player.battle_state.as_ref().unwrap().enemy.active_mon().hp,
        hp
    );

    for ghost in [false, true] {
        let mut enemy = screen(
            mon(
                if ghost {
                    Species::Gastly
                } else {
                    Species::Snorlax
                },
                100,
                MoveId::Splash,
            ),
            mon(Species::Meowth, 50, MoveId::PayDay),
            None,
        );
        let hp = enemy.battle_state.as_ref().unwrap().player.active_mon().hp;
        enemy.run_enemy_free_turn_stack(Vec::new(), Some((MoveId::PayDay, 0)));
        assert_eq!(pot(&enemy), if ghost { 0 } else { 100 });
        assert_eq!(coin_count(&pages(&enemy)), usize::from(!ghost));
        assert_eq!(
            enemy.battle_state.as_ref().unwrap().player.active_mon().hp == hp,
            ghost
        );
    }
}

#[test]
fn real_substitute_scatters_only_when_the_doll_survives() {
    for breaks in [false, true] {
        for target_player in [false, true] {
            let attacker_level = if breaks { 100 } else { 10 };
            let attacker = mon(Species::Meowth, attacker_level, MoveId::Splash);
            let defender = mon(
                Species::Snorlax,
                if breaks { 20 } else { 100 },
                MoveId::Substitute,
            );
            let (player, enemy) = if target_player {
                (defender, attacker)
            } else {
                (attacker, defender)
            };
            let mut s = screen(player, enemy, None);
            // Both dolls are created through ordinary FIGHT input and the real
            // turn. The attacker's initial Splash lets the defender create it.
            s.update_frame(a());
            assert_eq!(s.phase, BattlePhase::MoveSelect);
            s.update_frame(a());
            finish_text(&mut s);
            assert_eq!(s.phase, BattlePhase::PlayerMenu);
            let bs = s.battle_state.as_mut().unwrap();
            let (attacker, defender) = if target_player {
                (&mut bs.enemy, &mut bs.player)
            } else {
                (&mut bs.player, &mut bs.enemy)
            };
            assert!(defender.has_status2(HAS_SUBSTITUTE_UP));
            let hp = defender.active_mon().hp;
            let doll_hp = defender.substitute_hp;
            attacker.active_mon_mut().moves[0] = MoveId::PayDay;
            attacker.active_mon_mut().pp[0] = 20;
            defender.active_mon_mut().moves[0] = MoveId::Splash;
            defender.active_mon_mut().pp[0] = 40;
            s.update_frame(a());
            assert_eq!(s.phase, BattlePhase::MoveSelect);
            s.update_frame(a());
            let lines = pages(&s);
            assert_eq!(
                pot(&s),
                if breaks {
                    0
                } else {
                    u32::from(attacker_level) * 2
                },
                "target_player={target_player}, breaks={breaks}"
            );
            assert_eq!(coin_count(&lines), usize::from(!breaks), "{lines:?}");
            let bs = s.battle_state.as_ref().unwrap();
            let defender = if target_player { &bs.player } else { &bs.enemy };
            assert_eq!(defender.active_mon().hp, hp, "the doll takes all damage");
            assert_eq!(defender.has_status2(HAS_SUBSTITUTE_UP), !breaks);
            if !breaks {
                assert!(
                    defender.substitute_hp < doll_hp,
                    "the Pay Day hit connected"
                );
            }
            let broken: Vec<_> = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.contains("SUBSTITUTE broke!"))
                .collect();
            assert_eq!(broken.len(), usize::from(breaks), "{lines:?}");
            if breaks {
                let used = lines
                    .iter()
                    .position(|line| line.contains("used PAY DAY!"))
                    .unwrap();
                assert!(used < broken[0].0, "{lines:?}");
                // In the break fixture Meowth moves first. Keep the break text
                // within its action, before the defender's subsequent Splash.
                let splash = lines
                    .iter()
                    .position(|line| line.contains("used SPLASH!"))
                    .unwrap();
                assert!(broken[0].0 < splash, "{lines:?}");
            }
        }
    }
}

#[test]
fn lethal_scatter_precedes_faint_and_both_actors_keep_their_own_action_order() {
    let mut target = mon(Species::Rattata, 1, MoveId::Splash);
    target.hp = 1;
    let mut lethal = screen(mon(Species::Meowth, 100, MoveId::PayDay), target, None);
    lethal.execute_turn_with_move(0);
    let lines = pages(&lethal);
    let coins = lines.iter().position(|line| line == COINS).unwrap();
    let faint = lines
        .iter()
        .position(|line| line.contains("fainted!"))
        .unwrap();
    assert!(coins < faint, "{lines:?}");
    assert_eq!(pot(&lethal), 200);

    let mut both = screen(
        mon(Species::Meowth, 50, MoveId::PayDay),
        mon(Species::Snorlax, 50, MoveId::PayDay),
        None,
    );
    both.execute_turn_with_move(0);
    let lines = pages(&both);
    let coins: Vec<_> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| (line == COINS).then_some(i))
        .collect();
    assert_eq!(coins.len(), 2, "{lines:?}");
    let enemy_used = lines
        .iter()
        .position(|line| line.contains("Enemy SNORLAX") && line.contains("used"))
        .unwrap();
    assert!(coins[0] < enemy_used && enemy_used < coins[1], "{lines:?}");
    assert_eq!(pot(&both), 200, "each actual level50 action scatters once");
}

#[test]
fn mirror_move_resolves_payday_and_a_later_non_payday_turn_cannot_reuse_it() {
    let mut player = mon(Species::Meowth, 50, MoveId::MirrorMove);
    player.moves[1] = MoveId::Splash;
    player.pp[1] = 40;
    let mut s = screen(player, mon(Species::Snorlax, 100, MoveId::Splash), None);
    s.battle_state.as_mut().unwrap().enemy.last_move_used = MoveId::PayDay;
    s.execute_turn_with_move(0);
    let lines = pages(&s);
    assert_eq!(pot(&s), 100);
    assert_eq!(coin_count(&lines), 1, "{lines:?}");
    assert!(
        lines.iter().any(|line| line.contains("MIRROR MOVE")),
        "{lines:?}"
    );
    finish_text(&mut s);
    assert_eq!(s.phase, BattlePhase::PlayerMenu);
    s.execute_turn_with_move(1); // real Splash, not another copy
    assert_eq!(pot(&s), 100);
    assert_eq!(coin_count(&pages(&s)), 0);
}

#[test]
fn missed_sleeping_and_empty_pp_callers_do_not_recollect_a_previous_scatter() {
    for blocked in 0..3 {
        let mut s = screen(
            mon(Species::Meowth, 50, MoveId::PayDay),
            mon(Species::Snorlax, 100, MoveId::Splash),
            None,
        );
        s.execute_turn_with_move(0);
        assert_eq!(pot(&s), 100);
        finish_text(&mut s);
        assert_eq!(s.phase, BattlePhase::PlayerMenu);
        match blocked {
            0 => {
                s.battle_state
                    .as_mut()
                    .unwrap()
                    .enemy
                    .set_status1(INVULNERABLE);
                s.execute_turn_with_move(0);
            }
            1 => {
                s.battle_state
                    .as_mut()
                    .unwrap()
                    .player
                    .active_mon_mut()
                    .status = StatusCondition::Sleep(3);
                s.execute_turn_with_move(0);
            }
            _ => {
                let player = s.battle_state.as_mut().unwrap().player.active_mon_mut();
                player.pp[0] = 0;
                player.moves[1] = MoveId::Splash;
                player.pp[1] = 40;
                s.update_frame(a()); // FIGHT
                assert_eq!(s.phase, BattlePhase::MoveSelect);
                s.update_frame(a()); // actual empty-PP refusal
                assert_eq!(
                    s.current_message.as_deref(),
                    Some("No PP left for\nthis move!")
                );
            }
        }
        assert_eq!(pot(&s), 100);
        assert_eq!(coin_count(&pages(&s)), 0);
    }
}

#[test]
fn earlier_player_scatter_survives_the_later_trainer_ai_arena_rebuild() {
    let mut enemy = mon(Species::Snorlax, 100, MoveId::Splash);
    enemy.status = StatusCondition::Poison;
    let mut s = screen(
        mon(Species::Meowth, 50, MoveId::PayDay),
        enemy,
        Some(TrainerClass::Brock),
    );
    // Brock's Full Heal branch is status-driven and ignores its random byte.
    s.execute_turn_with_move(0);
    let lines = pages(&s);
    assert_eq!(s.enemy_ai_count, 4, "the real AI item callback ran");
    assert!(s
        .battle_state
        .as_ref()
        .unwrap()
        .enemy
        .active_mon()
        .status
        .is_none());
    assert_eq!(
        pot(&s),
        100,
        "the earlier action receipt survived arena replacement"
    );
    assert_eq!(coin_count(&lines), 1, "{lines:?}");
    let coins = lines.iter().position(|line| line == COINS).unwrap();
    let item = lines
        .iter()
        .position(|line| line.contains("used FULL HEAL!"))
        .unwrap();
    assert!(
        coins < item,
        "player action precedes slower trainer AI: {lines:?}"
    );
}
