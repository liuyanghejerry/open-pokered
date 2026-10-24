//! MoveHitTest checks Mist before X Accuracy and the accuracy random byte.
use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;

fn run(move_: MoveId, target_player: bool, mist: bool, x_accuracy: bool, byte: u8) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        20,
        [0x9a, 0x78],
        [move_, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let target = create_pokemon_with_moves(
        Species::Snorlax,
        100,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let (player, enemy) = if target_player {
        (target, attacker)
    } else {
        (attacker, target)
    };
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.link_mode = true;
    screen.link_enemy_move_override = Some((if target_player { move_ } else { MoveId::Splash }, 0));
    screen.link_rng = Some(LinkRng::new([byte, 91, 82, 73, 64, 55, 46, 37, 28, 19]));
    let bs = screen.battle_state.as_mut().unwrap();
    let (attacker, target) = if target_player {
        (&mut bs.enemy, &mut bs.player)
    } else {
        (&mut bs.player, &mut bs.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200;
    if mist {
        target.set_status2(state::status2::PROTECTED_BY_MIST);
    }
    if x_accuracy {
        attacker.set_status2(state::status2::USING_X_ACCURACY);
    }
    screen.execute_turn_with_move(0);
    screen
}

fn primary_stage(target: &state::BattlerState, move_: MoveId) -> i8 {
    match move_ {
        MoveId::Growl => target.stat_stages.attack,
        MoveId::TailWhip | MoveId::Leer | MoveId::Screech => target.stat_stages.defense,
        MoveId::StringShot => target.stat_stages.speed,
        MoveId::SandAttack | MoveId::Smokescreen | MoveId::Kinesis | MoveId::Flash => {
            target.stat_stages.accuracy
        }
        _ => unreachable!(),
    }
}

#[test]
fn primary_stat_drops_reject_mist_before_accuracy_rng_or_x_accuracy() {
    let mut wrong_draws = Vec::new();
    for move_ in [
        MoveId::Growl,
        MoveId::TailWhip,
        MoveId::Leer,
        MoveId::StringShot,
        MoveId::SandAttack,
        MoveId::Smokescreen,
        MoveId::Kinesis,
        MoveId::Flash,
        MoveId::Screech,
    ] {
        for target_player in [false, true] {
            for mist in [false, true] {
                for x_accuracy in [false, true] {
                    for byte in [0, 255] {
                        let screen = run(move_, target_player, mist, x_accuracy, byte);
                        let bs = screen.battle_state.as_ref().unwrap();
                        let target = if target_player { &bs.player } else { &bs.enemy };
                        let expected_stage = if mist || (!x_accuracy && byte == 255) {
                            0
                        } else if move_ == MoveId::Screech {
                            -2
                        } else {
                            -1
                        };
                        assert_eq!(
                            primary_stage(target, move_),
                            expected_stage,
                            "{move_:?}/{target_player}/Mist={mist}/XAccuracy={x_accuracy}/{byte}"
                        );
                        assert_eq!(target.active_mon().hp, target.active_mon().max_hp);
                        assert_eq!(target.has_status2(state::status2::PROTECTED_BY_MIST), mist);
                        let rng = screen.link_rng.as_ref().unwrap();
                        let expected_draws = if mist || x_accuracy { 0 } else { 1 };
                        if rng.consumed() != expected_draws
                            || rng.index() != expected_draws as usize
                        {
                            wrong_draws.push(format!(
                                "{move_:?}/{target_player}/Mist={mist}/XAccuracy={x_accuracy}/{byte}: {}",
                                rng.consumed()
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        wrong_draws.is_empty(),
        "incorrect accuracy draws: {wrong_draws:?}"
    );
}
