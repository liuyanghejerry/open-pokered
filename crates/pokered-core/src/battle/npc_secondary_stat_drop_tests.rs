//! StatModifierDownEffect applies the ordinary enemy miss gate to damaging
//! secondaries too, before checking Substitute or drawing the 33% chance.
use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;

#[derive(Clone, Copy, Debug)]
enum Protection {
    None,
    IntactSubstitute,
    BrokenSubstitute,
    Mist,
    MistAndSubstitute,
}

fn run(
    move_: MoveId,
    enemy_attacks: bool,
    link: bool,
    protection: Protection,
    npc_byte: u8,
    chance_byte: u8,
) -> BattleScreen {
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
    // Deterministic RNG and chosen moves are independent of ordinary/link mode.
    // Crit, accuracy and damage are followed by the applicable effect rolls.
    let (fourth, fifth) = if enemy_attacks && !link {
        (npc_byte, chance_byte)
    } else {
        (chance_byte, 91)
    };
    screen.link_rng = Some(LinkRng::new([
        255, 0, 255, fourth, fifth, 82, 73, 64, 55, 46,
    ]));
    let bs = screen.battle_state.as_mut().unwrap();
    bs.link_battle = link;
    let (attacker, target) = if enemy_attacks {
        (&mut bs.enemy, &mut bs.player)
    } else {
        (&mut bs.player, &mut bs.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200; // Splash first, before any speed drop.
    if matches!(protection, Protection::Mist | Protection::MistAndSubstitute) {
        target.set_status2(state::status2::PROTECTED_BY_MIST);
    }
    match protection {
        Protection::IntactSubstitute | Protection::MistAndSubstitute => {
            target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
            target.substitute_hp = 60;
        }
        Protection::BrokenSubstitute => {
            target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
            target.substitute_hp = 1;
        }
        _ => {}
    }
    screen.execute_turn_with_move(0);
    assert_eq!(screen.battle_state.as_ref().unwrap().link_battle, link);
    screen
}

fn stage(target: &state::BattlerState, move_: MoveId) -> i8 {
    match move_ {
        MoveId::Acid => target.stat_stages.defense,
        MoveId::AuroraBeam => target.stat_stages.attack,
        MoveId::PsychicM => target.stat_stages.special,
        _ => target.stat_stages.speed,
    }
}

#[test]
fn secondary_stat_drop_order_is_enemy_miss_then_substitute_then_chance() {
    let mut wrong_stages = Vec::new();
    let mut wrong_draws = Vec::new();
    let mut cases = 0;
    for move_ in [
        MoveId::Acid,
        MoveId::AuroraBeam,
        MoveId::Bubble,
        MoveId::Bubblebeam,
        MoveId::Constrict,
        MoveId::PsychicM,
    ] {
        for enemy in [false, true] {
            for link in [false, true] {
                let npc_bytes: &[u8] = if enemy && !link {
                    &[0, 63, 64, 255]
                } else {
                    &[64]
                };
                for protection in [
                    Protection::None,
                    Protection::IntactSubstitute,
                    Protection::BrokenSubstitute,
                    Protection::Mist,
                    Protection::MistAndSubstitute,
                ] {
                    for &npc_byte in npc_bytes {
                        for chance_byte in [0, 84, 85, 255] {
                            cases += 1;
                            let screen = run(move_, enemy, link, protection, npc_byte, chance_byte);
                            let bs = screen.battle_state.as_ref().unwrap();
                            let target = if enemy { &bs.player } else { &bs.enemy };
                            let broken = matches!(protection, Protection::BrokenSubstitute);
                            let intact = matches!(
                                protection,
                                Protection::IntactSubstitute | Protection::MistAndSubstitute
                            );
                            let npc_roll = enemy && !link && !broken;
                            let npc_missed = npc_roll && npc_byte < 64;
                            let chance_roll = !broken && !intact && !npc_missed;
                            let expected_stage = if chance_roll && chance_byte < 85 {
                                -1
                            } else {
                                0
                            };
                            let expected_draws = 3 + u64::from(npc_roll) + u64::from(chance_roll);
                            let label = format!(
                                "{move_:?}/enemy={enemy}/link={link}/{protection:?}/{npc_byte}/{chance_byte}"
                            );
                            assert_eq!(
                                target.has_status2(state::status2::HAS_SUBSTITUTE_UP),
                                intact,
                                "{label}"
                            );
                            assert_eq!(
                                target.active_mon().hp == target.active_mon().max_hp,
                                intact || broken,
                                "{label}"
                            );
                            if intact {
                                assert!(
                                    (1..60).contains(&target.substitute_hp),
                                    "{label}: damage must connect"
                                );
                            }
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
                                wrong_draws.push(format!(
                                    "{label}: {} != {expected_draws}",
                                    rng.consumed()
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 840);
    assert!(
        wrong_stages.is_empty() && wrong_draws.is_empty(),
        "{} stage mismatches: {wrong_stages:?}; {} RNG mismatches: {wrong_draws:?}",
        wrong_stages.len(),
        wrong_draws.len()
    );
}
