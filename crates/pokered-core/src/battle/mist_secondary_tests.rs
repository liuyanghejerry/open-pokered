//! Original side-effect chance and early-return contracts through the real turn caller.
use super::*;
use crate::link::rng::LinkRng;
use crate::pokemon::stats::create_pokemon_with_moves;

fn run(
    move_: MoveId,
    target_player: bool,
    mist: bool,
    substitute: Option<u8>,
    byte: u8,
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
    if mist {
        target.set_status2(state::status2::PROTECTED_BY_MIST);
    }
    if let Some(hp) = substitute {
        target.set_status2(state::status2::HAS_SUBSTITUTE_UP);
        target.substitute_hp = hp;
    }
    if matches!(move_, MoveId::Growl | MoveId::Leer | MoveId::Screech) {
        screen.link_rng = Some(LinkRng::new([0; 10]));
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

fn stage(target: &state::BattlerState, move_: MoveId) -> i8 {
    match move_ {
        MoveId::Acid | MoveId::Leer | MoveId::Screech => target.stat_stages.defense,
        MoveId::AuroraBeam | MoveId::Growl => target.stat_stages.attack,
        MoveId::PsychicM => target.stat_stages.special,
        _ => target.stat_stages.speed,
    }
}

#[test]
fn mist_allows_damaging_stat_drops_but_substitute_still_blocks_them() {
    let mut mismatches = Vec::new();
    for move_ in [
        MoveId::Acid,
        MoveId::AuroraBeam,
        MoveId::Bubble,
        MoveId::Bubblebeam,
        MoveId::Constrict,
        MoveId::PsychicM,
    ] {
        for player in [false, true] {
            for byte in [0, 84, 85, 255] {
                for (mist, substitute) in [(false, None), (true, None), (true, Some(60))] {
                    let screen = run(move_, player, mist, substitute, byte);
                    let t = target(&screen, player);
                    assert_eq!(
                        t.active_mon().hp == t.active_mon().max_hp,
                        substitute.is_some()
                    );
                    assert_eq!(t.has_status2(state::status2::PROTECTED_BY_MIST), mist);
                    let expected = if byte < 85 && substitute.is_none() {
                        -1
                    } else {
                        0
                    };
                    if stage(t, move_) != expected {
                        mismatches.push(format!(
                            "{move_:?}/{player}/{byte}/Mist={mist}/Sub={substitute:?}: {}",
                            stage(t, move_)
                        ));
                    }
                    assert_eq!(
                        screen.link_rng.as_ref().unwrap().consumed(),
                        if substitute.is_some() { 3 } else { 4 }
                    );
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "Mist incorrectly blocked secondary drops: {mismatches:?}"
    );
}

#[test]
fn mist_still_blocks_primary_stat_down_moves() {
    for move_ in [MoveId::Growl, MoveId::Leer, MoveId::Screech] {
        for player in [false, true] {
            for mist in [false, true] {
                let screen = run(move_, player, mist, None, 0);
                let t = target(&screen, player);
                let expected = if mist {
                    0
                } else if move_ == MoveId::Screech {
                    -2
                } else {
                    -1
                };
                assert_eq!(stage(t, move_), expected, "{move_:?}/{player}/Mist={mist}");
                assert_eq!(t.active_mon().hp, t.active_mon().max_hp);
            }
        }
    }
}
