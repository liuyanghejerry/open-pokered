//! Original damaging confusion bypasses intact substitutes; breaking a doll
//! clears the attacker's move effect before secondary effects are dispatched.
use super::state::{status1, status2};
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn fight(move_: MoveId, seed: u64, target_player: bool, substitute: Option<u8>) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        if move_ == MoveId::PoisonSting {
            Species::Nidoking
        } else {
            Species::Mewtwo
        },
        20,
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
        let target = if target_player {
            &mut bs.player
        } else {
            &mut bs.enemy
        };
        target.set_status2(status2::HAS_SUBSTITUTE_UP);
        target.substitute_hp = hp;
    }
    let confirm = BattleInput {
        a: true,
        ..BattleInput::none()
    };
    screen.update_frame(confirm);
    assert_eq!(screen.phase, BattlePhase::MoveSelect);
    screen.update_frame(confirm);
    assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));
    screen
}

fn target(screen: &BattleScreen, player: bool) -> &state::BattlerState {
    let bs = screen.battle_state.as_ref().unwrap();
    if player {
        &bs.player
    } else {
        &bs.enemy
    }
}

fn status_applied(screen: &BattleScreen, player: bool, move_: MoveId) -> bool {
    let t = target(screen, player);
    if move_ == MoveId::PoisonSting {
        t.active_mon().status == state::StatusCondition::Poison
    } else if move_ == MoveId::Acid {
        t.stat_stages.defense < 0
    } else {
        t.has_status1(status1::CONFUSED)
    }
}

#[test]
fn damaging_confusion_ignores_an_intact_substitute() {
    for move_ in [MoveId::Confusion, MoveId::Psybeam] {
        for target_player in [false, true] {
            let seed = (0..512)
                .find(|&seed| {
                    status_applied(
                        &fight(move_, seed, target_player, None),
                        target_player,
                        move_,
                    )
                })
                .expect("positive no-substitute control must inflict confusion");
            let shielded = fight(move_, seed, target_player, Some(60));
            let t = target(&shielded, target_player);
            assert!(t.has_status2(status2::HAS_SUBSTITUTE_UP));
            assert!(t.substitute_hp < 60, "the hit must connect with the doll");
            assert!(
                status_applied(&shielded, target_player, move_),
                "move={move_:?}, seed={seed}, target_player={target_player}"
            );
        }
    }
}

#[test]
fn breaking_a_substitute_cancels_the_secondary_status() {
    for move_ in [
        MoveId::Confusion,
        MoveId::Psybeam,
        MoveId::PoisonSting,
        MoveId::Acid,
    ] {
        for target_player in [false, true] {
            let seed = (0..512)
                .find(|&seed| {
                    status_applied(
                        &fight(move_, seed, target_player, None),
                        target_player,
                        move_,
                    )
                })
                .expect("positive no-substitute control must inflict the status");
            let broken = fight(move_, seed, target_player, Some(1));
            assert!(!target(&broken, target_player).has_status2(status2::HAS_SUBSTITUTE_UP));
            assert!(
                !status_applied(&broken, target_player, move_),
                "move={move_:?}, seed={seed}, target_player={target_player}"
            );
        }
    }
}

#[test]
fn primary_confuse_ray_still_cannot_pass_a_substitute() {
    for target_player in [false, true] {
        let seed = (0..512)
            .find(|&seed| {
                status_applied(
                    &fight(MoveId::ConfuseRay, seed, target_player, None),
                    target_player,
                    MoveId::ConfuseRay,
                )
            })
            .expect("positive no-substitute Confuse Ray control");
        let shielded = fight(MoveId::ConfuseRay, seed, target_player, Some(60));
        let t = target(&shielded, target_player);
        assert!(t.has_status2(status2::HAS_SUBSTITUTE_UP));
        assert_eq!(t.substitute_hp, 60);
        assert!(!t.has_status1(status1::CONFUSED));
    }
}
