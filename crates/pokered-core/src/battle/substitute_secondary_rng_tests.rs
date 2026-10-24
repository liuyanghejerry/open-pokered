//! Original side-effect chance and early-return contracts through the real turn caller.
use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;

fn run(move_: MoveId, target_player: bool, substitute: Option<u8>, byte: u8) -> BattleScreen {
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
    // Crit, accuracy, damage, secondary chance, then unmistakable continuation bytes.
    screen.link_rng = Some(LinkRng::new([255, 0, 255, byte, 91, 82, 73, 64, 55, 46]));
    let bs = screen.battle_state.as_mut().unwrap();
    bs.link_battle = true;
    let (attacker, target) = if target_player {
        (&mut bs.enemy, &mut bs.player)
    } else {
        (&mut bs.player, &mut bs.enemy)
    };
    attacker.active_mon_mut().speed = 100;
    target.active_mon_mut().speed = 200; // Splash first; a new status cannot gate another action.
    if let Some(hp) = substitute {
        target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
        target.substitute_hp = hp;
    }
    screen.execute_turn_with_move(0);
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

#[test]
fn intact_substitute_skips_secondary_rng_before_the_chance_test() {
    let mut extra_draws = Vec::new();
    for move_ in [
        MoveId::PoisonSting,
        MoveId::Ember,
        MoveId::IceBeam,
        MoveId::Thundershock,
        MoveId::Bite,
        MoveId::Acid,
    ] {
        for player in [false, true] {
            for byte in [0, 255] {
                let screen = run(move_, player, Some(60), byte);
                let t = target(&screen, player);
                assert!(t.has_status2(state::status2::HAS_SUBSTITUTE_UP));
                assert!(
                    t.substitute_hp < 60,
                    "must land on doll: {move_:?}/{player}"
                );
                assert_eq!(t.active_mon().hp, t.active_mon().max_hp);
                let rng = screen.link_rng.as_ref().unwrap();
                if rng.consumed() != 3 || rng.index() != 3 {
                    extra_draws.push(format!("{move_:?}/{player}/{byte}: {}", rng.consumed()));
                }
            }
        }
    }
    assert!(
        extra_draws.is_empty(),
        "extra chance draws: {extra_draws:?}"
    );
}

#[test]
fn broken_doll_and_no_doll_keep_their_respective_chance_draw_counts() {
    for move_ in [
        MoveId::PoisonSting,
        MoveId::Ember,
        MoveId::IceBeam,
        MoveId::Thundershock,
        MoveId::Bite,
        MoveId::Acid,
    ] {
        for player in [false, true] {
            for (hp, expected_draws) in [(Some(1), 3), (None, 4)] {
                let screen = run(move_, player, hp, 255);
                let t = target(&screen, player);
                assert!(!t.has_status2(state::status2::HAS_SUBSTITUTE_UP));
                assert_eq!(t.active_mon().hp == t.active_mon().max_hp, hp.is_some());
                assert_eq!(
                    screen.link_rng.as_ref().unwrap().consumed(),
                    expected_draws,
                    "{move_:?}/{player}/{hp:?}"
                );
            }
        }
    }
}

#[test]
fn damaging_confusion_still_rolls_through_an_intact_doll() {
    for move_ in [MoveId::Confusion, MoveId::Psybeam] {
        for player in [false, true] {
            for (byte, expected_draws, confused) in [(0, 5, true), (25, 4, false)] {
                let screen = run(move_, player, Some(60), byte);
                let t = target(&screen, player);
                assert!(t.has_status2(state::status2::HAS_SUBSTITUTE_UP));
                assert!(t.substitute_hp < 60);
                assert_eq!(t.active_mon().hp, t.active_mon().max_hp);
                assert_eq!(t.has_status1(state::status1::CONFUSED), confused);
                assert_eq!(screen.link_rng.as_ref().unwrap().consumed(), expected_draws);
            }
        }
    }
}
