//! StatModifierDownEffect checks the ordinary enemy's 25% miss and Substitute
//! before MoveHitTest. Link battles and the player's attacks skip the extra miss.
use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;

fn run(move_: MoveId, enemy_attacks: bool, link: bool, protection: u8, byte: u8) -> BattleScreen {
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
    let (player, enemy) = if enemy_attacks {
        (target, attacker)
    } else {
        (attacker, target)
    };
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.link_mode = link;
    // This slot supplies deterministic bytes only. The actual battle mode is
    // separately set above and asserted below; move selection/AI is controlled.
    screen.link_rng = Some(LinkRng::new([byte, 0, 91, 82, 73, 64, 55, 46, 37, 28]));
    screen.link_enemy_move_override = Some((if enemy_attacks { move_ } else { MoveId::Splash }, 0));
    let bs = screen.battle_state.as_mut().unwrap();
    bs.link_battle = link;
    let (attacker, target) = if enemy_attacks {
        (&mut bs.enemy, &mut bs.player)
    } else {
        (&mut bs.player, &mut bs.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200;
    match protection {
        1 => {
            target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
            target.substitute_hp = 60;
        }
        2 => target.set_status2(state::status2::PROTECTED_BY_MIST),
        _ => {}
    }
    screen.execute_turn_with_move(0);
    assert_eq!(screen.battle_state.as_ref().unwrap().link_battle, link);
    screen
}

fn stage(target: &state::BattlerState, move_: MoveId) -> i8 {
    match move_ {
        MoveId::Growl => target.stat_stages.attack,
        MoveId::TailWhip | MoveId::Leer | MoveId::Screech => target.stat_stages.defense,
        MoveId::StringShot => target.stat_stages.speed,
        _ => target.stat_stages.accuracy,
    }
}

#[test]
fn primary_stat_drop_order_is_enemy_miss_then_substitute_then_accuracy() {
    let mut wrong_stages = Vec::new();
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
        for enemy in [false, true] {
            for link in [false, true] {
                for protection in [0, 1, 2] {
                    for byte in [0, 63, 64, 255] {
                        let screen = run(move_, enemy, link, protection, byte);
                        let bs = screen.battle_state.as_ref().unwrap();
                        let target = if enemy { &bs.player } else { &bs.enemy };
                        let npc_roll = enemy && !link;
                        let npc_missed = npc_roll && byte < 64;
                        let accuracy_missed = !npc_roll && byte == 255;
                        let expected_stage = if protection != 0 || npc_missed || accuracy_missed {
                            0
                        } else if move_ == MoveId::Screech {
                            -2
                        } else {
                            -1
                        };
                        let expected_draws =
                            u64::from(npc_roll) + u64::from(!npc_missed && protection == 0);
                        let label = format!(
                            "{move_:?}/enemy={enemy}/link={link}/guard={protection}/{byte}"
                        );
                        if stage(target, move_) != expected_stage {
                            wrong_stages.push(format!(
                                "{label}: {} != {expected_stage}",
                                stage(target, move_)
                            ));
                        }
                        let rng = screen.link_rng.as_ref().unwrap();
                        if rng.consumed() != expected_draws
                            || rng.index() != expected_draws as usize
                        {
                            wrong_draws
                                .push(format!("{label}: {} != {expected_draws}", rng.consumed()));
                        }
                        assert_eq!(target.active_mon().hp, target.active_mon().max_hp);
                    }
                }
            }
        }
    }
    assert!(
        wrong_stages.is_empty() && wrong_draws.is_empty(),
        "{} stage mismatches: {wrong_stages:?}; {} RNG mismatches: {wrong_draws:?}",
        wrong_stages.len(),
        wrong_draws.len()
    );
}
