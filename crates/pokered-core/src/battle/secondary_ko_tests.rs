//! A fainted defender returns before ordinary secondary-effect dispatch.
use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;

fn run(move_: MoveId, enemy_attacks: bool, ko: bool, byte: u8, link: bool) -> BattleScreen {
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
    screen.link_enemy_move_override = Some((if enemy_attacks { move_ } else { MoveId::Splash }, 0));
    // Crit, accuracy, damage, secondary chance, and optional confusion duration.
    screen.link_rng = Some(LinkRng::new([255, 0, 255, byte, 0, 82, 73, 64, 55, 46]));
    let bs = screen.battle_state.as_mut().unwrap();
    bs.link_battle = link;
    let (attacker, target) = if enemy_attacks {
        (&mut bs.enemy, &mut bs.player)
    } else {
        (&mut bs.player, &mut bs.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200; // Splash resolves before any status effect.
    if ko {
        target.active_mon_mut().hp = 1;
    }
    screen.execute_turn_with_move(0);
    screen
}

#[test]
fn fainted_targets_skip_secondary_effects_and_their_random_draws() {
    let mut wrong_draws = Vec::new();
    let mut wrong_effects = Vec::new();
    for move_ in [
        MoveId::PoisonSting,
        MoveId::Ember,
        MoveId::IceBeam,
        MoveId::Thundershock,
        MoveId::Bite,
        MoveId::Confusion,
        MoveId::Acid,
    ] {
        for enemy in [false, true] {
            for ko in [false, true] {
                for byte in [0, 255] {
                    let screen = run(move_, enemy, ko, byte, true);
                    let bs = screen.battle_state.as_ref().unwrap();
                    let target = if enemy { &bs.player } else { &bs.enemy };
                    let label = format!("{move_:?}/enemy={enemy}/ko={ko}/{byte}");
                    assert_eq!(target.active_mon().hp == 0, ko, "{label}");
                    assert!(
                        target.active_mon().hp < target.active_mon().max_hp,
                        "{label}: attack connects"
                    );
                    let expected_draws = if ko {
                        3
                    } else if move_ == MoveId::Confusion && byte == 0 {
                        5
                    } else {
                        4
                    };
                    let rng = screen.link_rng.as_ref().unwrap();
                    if rng.consumed() != expected_draws || rng.index() != expected_draws as usize {
                        wrong_draws
                            .push(format!("{label}: {} != {expected_draws}", rng.consumed()));
                    }
                    if ko
                        && (target.active_mon().status != StatusCondition::None
                            || target.has_status1(state::status1::CONFUSED)
                            || target.stat_stages.defense != 0)
                    {
                        wrong_effects.push(label);
                    }
                    if !ko && byte == 0 {
                        match move_ {
                            MoveId::PoisonSting => {
                                assert_eq!(target.active_mon().status, StatusCondition::Poison)
                            }
                            MoveId::Ember => {
                                assert_eq!(target.active_mon().status, StatusCondition::Burn)
                            }
                            MoveId::IceBeam => {
                                assert_eq!(target.active_mon().status, StatusCondition::Freeze)
                            }
                            MoveId::Thundershock => {
                                assert_eq!(target.active_mon().status, StatusCondition::Paralysis)
                            }
                            MoveId::Confusion => {
                                assert!(target.has_status1(state::status1::CONFUSED))
                            }
                            MoveId::Acid => assert_eq!(target.stat_stages.defense, -1),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    assert!(
        wrong_draws.is_empty() && wrong_effects.is_empty(),
        "{} RNG mismatches: {wrong_draws:?}; {} invalid effects: {wrong_effects:?}",
        wrong_draws.len(),
        wrong_effects.len()
    );
}

#[test]
fn recoil_and_self_ko_still_run_when_the_defender_faints() {
    for move_ in [MoveId::TakeDown, MoveId::Explosion] {
        for enemy in [false, true] {
            let screen = run(move_, enemy, true, 255, true);
            let bs = screen.battle_state.as_ref().unwrap();
            let (attacker, target) = if enemy {
                (&bs.enemy, &bs.player)
            } else {
                (&bs.player, &bs.enemy)
            };
            assert_eq!(target.active_mon().hp, 0);
            assert!(attacker.active_mon().hp < attacker.active_mon().max_hp);
            if move_ == MoveId::Explosion {
                assert_eq!(attacker.active_mon().hp, 0);
            }
        }
    }
}

#[test]
fn ordinary_enemy_stat_drop_also_skips_its_extra_miss_roll_after_ko() {
    for enemy in [false, true] {
        for byte in [0, 255] {
            let screen = run(MoveId::Acid, enemy, true, byte, false);
            let bs = screen.battle_state.as_ref().unwrap();
            assert!(!bs.link_battle);
            let target = if enemy { &bs.player } else { &bs.enemy };
            assert_eq!(target.active_mon().hp, 0);
            assert_eq!(target.stat_stages.defense, 0);
            assert_eq!(screen.link_rng.as_ref().unwrap().consumed(), 3);
        }
    }
}
