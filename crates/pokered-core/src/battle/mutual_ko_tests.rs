//! Original HandleEnemyMonFainted checks AnyPartyAlive before wild victory,
//! trainer victory, replacement, and the experience caller (core.asm:699-825).
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn screen(trainer: bool, second_enemy: bool, healthy_bench: bool) -> BattleScreen {
    let electrode = create_pokemon_with_moves(
        Species::Electrode,
        100,
        [0x9a, 0x78],
        [
            MoveId::Explosion,
            MoveId::Swift,
            MoveId::LightScreen,
            MoveId::Screech,
        ],
    )
    .unwrap();
    let zapdos = create_pokemon_with_moves(
        Species::Zapdos,
        50,
        [0x9a, 0x78],
        [
            MoveId::Thundershock,
            MoveId::DrillPeck,
            MoveId::None,
            MoveId::None,
        ],
    )
    .unwrap();
    let mut players = vec![electrode];
    if healthy_bench {
        players.push(
            create_pokemon_with_moves(
                Species::Rattata,
                12,
                [0x9a, 0x78],
                [
                    MoveId::Tackle,
                    MoveId::TailWhip,
                    MoveId::QuickAttack,
                    MoveId::None,
                ],
            )
            .unwrap(),
        );
    }
    let mut enemies = vec![zapdos.clone()];
    if second_enemy {
        enemies.push(zapdos);
    }
    let mut screen = BattleScreen::from_parties(
        !trainer,
        &players,
        &enemies,
        trainer.then_some(TrainerClass::BugCatcher),
    );
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    screen.player_money = 3000;
    screen.phase = BattlePhase::PlayerMenu;
    screen
}

fn queued_phase(phase: &BattlePhase) -> &BattlePhase {
    match phase {
        BattlePhase::ShowingText { next_phase, .. } => queued_phase(next_phase),
        phase => phase,
    }
}

#[test]
fn mutual_ko_canonical_explosion_loses_before_enemy_victory_or_replacement() {
    for (trainer, second_enemy) in [(false, false), (true, false), (true, true)] {
        let mut screen = screen(trainer, second_enemy, false);
        let before = screen.battle_state.as_ref().unwrap().player.party[0].clone();
        screen.execute_turn_with_move(0);
        let bs = screen.battle_state.as_ref().unwrap();
        assert_eq!(
            bs.player.active_mon().hp,
            0,
            "real canonical Explosion self-KO"
        );
        assert_eq!(
            bs.enemy.active_mon().hp,
            0,
            "real canonical Explosion target-KO"
        );
        assert_eq!(bs.player.active_mon().pp[0], 4);
        assert_eq!(bs.player.active_mon().total_exp, before.total_exp);
        assert_eq!(bs.player.active_mon().stat_exp, before.stat_exp);
        assert_eq!(
            bs.enemy.active_pokemon_index, 0,
            "no replacement before blackout"
        );
        let phase = queued_phase(&screen.phase);
        if trainer {
            assert!(
                matches!(
                    phase,
                    BattlePhase::TrainerVictory {
                        player_won: false,
                        ..
                    }
                ),
                "{phase:?}"
            );
        } else {
            assert!(
                matches!(
                    phase,
                    BattlePhase::BattleOver {
                        won: false,
                        escaped: false,
                        ..
                    }
                ),
                "{phase:?}"
            );
        }
    }
}

#[test]
fn mutual_ko_with_healthy_bench_still_wins_the_final_enemy() {
    for trainer in [false, true] {
        let mut screen = screen(trainer, false, true);
        screen.execute_turn_with_move(0);
        let bs = screen.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.party[0].hp, 0);
        assert_eq!(bs.player.party[1].hp, 31);
        assert_eq!(bs.enemy.active_mon().hp, 0);
        let phase = queued_phase(&screen.phase);
        if trainer {
            assert!(
                matches!(
                    phase,
                    BattlePhase::TrainerVictory {
                        player_won: true,
                        ..
                    }
                ),
                "{phase:?}"
            );
        } else {
            assert!(
                matches!(
                    phase,
                    BattlePhase::BattleOver {
                        won: true,
                        escaped: false,
                        ..
                    }
                ),
                "{phase:?}"
            );
        }
    }
}

#[test]
fn mutual_ko_no_party_alive_skips_the_experience_caller() {
    let mut screen = screen(false, false, false);
    let bs = screen.battle_state.as_mut().unwrap();
    bs.player.party[0].hp = 0;
    bs.enemy.party[0].hp = 0;
    assert!(bs.party_fought_flags[0]);
    screen.check_faint_after_turn();
    let mut messages = Vec::new();
    screen.append_exp_messages(&mut messages);
    let bs = screen.battle_state.as_ref().unwrap();
    assert!(messages.is_empty());
    assert!(
        bs.party_fought_flags[0],
        "source AnyPartyAlive returns before GainExperience.done resets fought history"
    );
    assert!(
        !bs.party_gain_exp_flags[0],
        "RemoveFaintedPlayerMon still clears gain eligibility"
    );
    assert!(screen.pending_learn_moves.is_empty());
}

#[test]
fn mutual_ko_priority_preserves_the_existing_player_only_loss_path() {
    let mut screen = screen(false, false, false);
    screen.battle_state.as_mut().unwrap().player.party[0].hp = 0;
    let phase = screen.check_faint_after_turn();
    assert!(matches!(
        phase,
        BattlePhase::BattleOver {
            won: false,
            escaped: false,
            ..
        }
    ));
    assert!(screen.battle_state.as_ref().unwrap().enemy.active_mon().hp > 0);
}
