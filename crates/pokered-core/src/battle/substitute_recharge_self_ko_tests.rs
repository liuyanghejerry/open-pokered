//! AttackSubstitute clears the attacker's move effect before recharge/self-KO.
use super::state::status2;
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn battle(move_: MoveId, seed: u64, target_player: bool, substitute: Option<u8>) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        5,
        [0x9a, 0x78],
        [move_, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let target = create_pokemon_with_moves(
        Species::Snorlax,
        60,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    assert!(target.max_hp / 4 >= 60);
    let (player, enemy) = if target_player {
        (target, attacker)
    } else {
        (attacker, target)
    };
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.phase = BattlePhase::PlayerMenu;
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(seed);
    if let Some(hp) = substitute {
        let bs = screen.battle_state.as_mut().unwrap();
        let t = if target_player {
            &mut bs.player
        } else {
            &mut bs.enemy
        };
        t.set_status2(status2::HAS_SUBSTITUTE_UP);
        t.substitute_hp = hp;
    }
    screen
}

fn battler(screen: &BattleScreen, player: bool) -> &state::BattlerState {
    let bs = screen.battle_state.as_ref().unwrap();
    if player {
        &bs.player
    } else {
        &bs.enemy
    }
}

fn fight(screen: &mut BattleScreen) {
    let confirm = BattleInput {
        a: true,
        ..BattleInput::none()
    };
    assert_eq!(screen.phase, BattlePhase::PlayerMenu);
    screen.update_frame(confirm);
    assert_eq!(screen.phase, BattlePhase::MoveSelect);
    screen.update_frame(confirm);
    assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));
}

fn connected_seed(move_: MoveId, target_player: bool) -> u64 {
    (0..512)
        .find(|&seed| {
            let mut control = battle(move_, seed, target_player, None);
            fight(&mut control);
            let target = battler(&control, target_player).active_mon();
            target.hp < target.max_hp
        })
        .expect("positive no-substitute control must connect")
}

fn finish_presentation(screen: &mut BattleScreen) {
    for frame in 0..2400 {
        if screen.phase == BattlePhase::PlayerMenu {
            return;
        }
        screen.update_frame(BattleInput {
            a: frame % 2 == 1,
            ..BattleInput::none()
        });
    }
    panic!("turn did not return to the menu: {:?}", screen.phase);
}

#[test]
fn hyper_beam_recharges_unless_the_hit_breaks_a_substitute() {
    for target_player in [false, true] {
        let seed = connected_seed(MoveId::HyperBeam, target_player);
        for substitute in [None, Some(60), Some(1)] {
            let mut screen = battle(MoveId::HyperBeam, seed, target_player, substitute);
            fight(&mut screen);
            let target = battler(&screen, target_player);
            if let Some(hp) = substitute {
                assert_eq!(target.active_mon().hp, target.active_mon().max_hp);
                assert_eq!(target.has_status2(status2::HAS_SUBSTITUTE_UP), hp == 60);
                if hp == 60 {
                    assert!(target.substitute_hp < hp);
                }
            }
            assert_eq!(
                battler(&screen, !target_player).has_status2(status2::NEEDS_TO_RECHARGE),
                substitute != Some(1),
                "target_player={target_player}, doll={substitute:?}"
            );
        }
    }
}

#[test]
fn breaking_a_substitute_cancels_explosion_self_ko() {
    for move_ in [MoveId::Explosion, MoveId::Selfdestruct] {
        for target_player in [false, true] {
            let seed = connected_seed(move_, target_player);
            for substitute in [None, Some(60), Some(1)] {
                let mut screen = battle(move_, seed, target_player, substitute);
                fight(&mut screen);
                let target = battler(&screen, target_player);
                if let Some(hp) = substitute {
                    assert_eq!(target.active_mon().hp, target.active_mon().max_hp);
                    assert_eq!(target.has_status2(status2::HAS_SUBSTITUTE_UP), hp == 60);
                    if hp == 60 {
                        assert!(target.substitute_hp < hp);
                    }
                }
                let attacker = battler(&screen, !target_player).active_mon();
                assert_eq!(
                    attacker.hp,
                    if substitute == Some(1) {
                        attacker.max_hp
                    } else {
                        0
                    },
                    "{move_:?}, target_player={target_player}, doll={substitute:?}"
                );
            }
        }
    }
}

#[test]
fn next_hyper_beam_can_recharge_after_the_previous_doll_broke() {
    for target_player in [false, true] {
        let seed = connected_seed(MoveId::HyperBeam, target_player);
        let mut screen = battle(MoveId::HyperBeam, seed, target_player, Some(1));
        fight(&mut screen);
        assert!(!battler(&screen, !target_player).has_status2(status2::NEEDS_TO_RECHARGE));
        finish_presentation(&mut screen);
        let pp = battler(&screen, !target_player).active_mon().pp[0];
        fight(&mut screen);
        let attacker = battler(&screen, !target_player);
        if !target_player {
            assert_eq!(attacker.active_mon().pp[0], pp - 1);
        }
        assert!(attacker.has_status2(status2::NEEDS_TO_RECHARGE));
        let target = battler(&screen, target_player).active_mon();
        assert!(target.hp < target.max_hp);
    }
}
