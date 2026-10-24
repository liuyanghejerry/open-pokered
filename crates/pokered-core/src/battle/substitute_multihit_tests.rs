//! AttackSubstitute clears the move effect before installing a multi-hit loop.
use super::state::status2;
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

const MULTI_MOVES: [MoveId; 10] = [
    MoveId::Doubleslap,
    MoveId::CometPunch,
    MoveId::FuryAttack,
    MoveId::PinMissile,
    MoveId::SpikeCannon,
    MoveId::Barrage,
    MoveId::FurySwipes,
    MoveId::DoubleKick,
    MoveId::Bonemerang,
    MoveId::Twineedle,
];

fn battle(move_: MoveId, seed: u64, target_player: bool, substitute: Option<u8>) -> BattleScreen {
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

fn seed_with_hits(
    move_: MoveId,
    target_player: bool,
    predicate: impl Fn(&[u16; 6]) -> bool,
) -> (u64, [u16; 6]) {
    (0..1024)
        .find_map(|seed| {
            let mut control = battle(move_, seed, target_player, None);
            fight(&mut control);
            let hits = pokered_rules::presented_hits(!target_player);
            predicate(&hits).then_some((seed, hits))
        })
        .expect("positive no-substitute control must satisfy the hit pattern")
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
fn multi_hit_does_not_start_when_the_first_hit_breaks_the_doll() {
    for move_ in MULTI_MOVES {
        for target_player in [false, true] {
            let (seed, _) =
                seed_with_hits(move_, target_player, |hits| hits[5] >= 2 && hits[0] > 1);
            let mut screen = battle(move_, seed, target_player, Some(1));
            fight(&mut screen);
            let target = battler(&screen, target_player);
            assert!(!target.has_status2(status2::HAS_SUBSTITUTE_UP));
            assert_eq!(
                target.active_mon().hp,
                target.active_mon().max_hp,
                "{move_:?}, target_player={target_player}: first-hit break cancels the loop"
            );
            assert_eq!(
                pokered_rules::presented_hits(!target_player)[5],
                1,
                "{move_:?}, target_player={target_player}: no second hit after first-hit break"
            );
        }
    }
}

#[test]
fn intact_doll_keeps_all_hits_for_every_multi_hit_move() {
    for move_ in MULTI_MOVES {
        for target_player in [false, true] {
            let (seed, control) = seed_with_hits(move_, target_player, |hits| {
                hits[5] >= 2 && hits[0] > 0 && hits[..5].iter().sum::<u16>() < 60
            });
            let mut screen = battle(move_, seed, target_player, Some(60));
            fight(&mut screen);
            let target = battler(&screen, target_player);
            assert!(target.has_status2(status2::HAS_SUBSTITUTE_UP));
            assert_eq!(target.active_mon().hp, target.active_mon().max_hp);
            assert_eq!(
                u16::from(target.substitute_hp),
                60 - control[..5].iter().sum::<u16>()
            );
            let recorded = pokered_rules::presented_hits(!target_player);
            assert_eq!(
                recorded[5], control[5],
                "{move_:?}, target_player={target_player}"
            );
            assert_eq!(&recorded[..5], &[0; 5], "all HP loss belongs to the doll");
        }
    }
}

#[test]
fn later_doll_break_keeps_the_already_started_multi_hit_sequence() {
    // The seven 2-5-hit moves can still hit the mon after their second hit
    // breaks the doll. Equality on hit one leaves the doll active at zero HP.
    for &move_ in &MULTI_MOVES[..7] {
        for target_player in [false, true] {
            let (seed, control) = seed_with_hits(move_, target_player, |hits| {
                hits[5] >= 3 && hits[0] > 1 && hits[0] < 60
            });
            let mut screen = battle(move_, seed, target_player, Some(control[0] as u8));
            fight(&mut screen);
            let target = battler(&screen, target_player);
            assert!(!target.has_status2(status2::HAS_SUBSTITUTE_UP));
            assert_eq!(
                target.active_mon().max_hp - target.active_mon().hp,
                control[0] * (control[5] - 2),
                "{move_:?}, target_player={target_player}"
            );
            let recorded = pokered_rules::presented_hits(!target_player);
            assert_eq!(recorded[5], control[5]);
            assert_eq!(&recorded[..2], &[0, 0]);
            assert!(recorded[2..usize::from(recorded[5])]
                .iter()
                .all(|&loss| loss == control[0]));
        }
    }
}

#[test]
fn next_action_restores_multi_hit_after_the_first_hit_broke_a_doll() {
    for target_player in [false, true] {
        let (seed, _) = seed_with_hits(MoveId::DoubleKick, target_player, |hits| {
            hits[5] == 2 && hits[0] > 1
        });
        let mut screen = battle(MoveId::DoubleKick, seed, target_player, Some(1));
        fight(&mut screen);
        assert_eq!(pokered_rules::presented_hits(!target_player)[5], 1);
        finish_presentation(&mut screen);
        let pp = battler(&screen, !target_player).active_mon().pp[0];
        fight(&mut screen);
        let target = battler(&screen, target_player).active_mon();
        assert!(target.hp < target.max_hp);
        assert_eq!(pokered_rules::presented_hits(!target_player)[5], 2);
        if !target_player {
            assert_eq!(battler(&screen, true).active_mon().pp[0], pp - 1);
        }
    }
}
