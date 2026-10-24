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
fn poison_side_effect_boundaries_follow_original_percent_plus_one() {
    // macros/data.asm: percent = *255/100; PoisonEffect compares against +1.
    for (move_, threshold) in [(MoveId::PoisonSting, 52u8), (MoveId::Sludge, 103)] {
        for player in [false, true] {
            for byte in [0, threshold - 1, threshold, 255] {
                let screen = run(move_, player, None, byte);
                let t = target(&screen, player);
                assert!(
                    t.active_mon().hp < t.active_mon().max_hp,
                    "must land: {move_:?}/{player}/{byte}"
                );
                assert_eq!(
                    t.active_mon().status == StatusCondition::Poison,
                    byte < threshold,
                    "original PoisonEffect boundary: {move_:?}/{player}/{byte}"
                );
                assert_eq!(screen.link_rng.as_ref().unwrap().consumed(), 4);
            }
        }
    }
}
