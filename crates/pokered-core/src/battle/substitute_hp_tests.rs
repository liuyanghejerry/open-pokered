//! AttackSubstitute returns on no borrow, including exact depletion to zero.
//! Keep the up flag until a later positive hit, using production FIGHT inputs.
use super::state::status2;
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

fn battle(target_player: bool, substitute_hp: u8) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        20,
        [0x9a, 0x78],
        [
            MoveId::SeismicToss,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
    )
    .unwrap();
    let target = create_pokemon_with_moves(
        Species::Snorlax,
        20,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    assert!(attacker.speed > target.speed);
    let (player, enemy) = if target_player {
        (target, attacker)
    } else {
        (attacker, target)
    };
    let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
    screen.phase = BattlePhase::PlayerMenu;
    screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
    let target = target_mut(&mut screen, target_player);
    target.set_status2(status2::HAS_SUBSTITUTE_UP);
    target.substitute_hp = substitute_hp;
    screen
}

fn target_mut(screen: &mut BattleScreen, player: bool) -> &mut state::BattlerState {
    let bs = screen.battle_state.as_mut().unwrap();
    if player {
        &mut bs.player
    } else {
        &mut bs.enemy
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
    panic!(
        "turn presentation did not return to the menu: {:?}",
        screen.phase
    );
}

#[test]
fn substitute_fight_damage_preserves_the_exact_zero_hp_boundary() {
    for target_player in [false, true] {
        for hp in [19, 20, 21] {
            let mut screen = battle(target_player, hp);
            let real_hp = target_mut(&mut screen, target_player).active_mon().hp;
            let pp = screen.battle_state.as_ref().unwrap().player.active_mon().pp[0];
            fight(&mut screen);
            let target = target_mut(&mut screen, target_player);
            assert_eq!(
                target.active_mon().hp,
                real_hp,
                "damage never spills into real HP"
            );
            if hp >= 20 {
                assert_eq!(target.substitute_hp, hp - 20);
            }
            assert_eq!(
                target.has_status2(status2::HAS_SUBSTITUTE_UP),
                hp >= 20,
                "target_player={target_player}, substitute_hp={hp}"
            );
            assert_eq!(
                screen.battle_state.as_ref().unwrap().player.active_mon().pp[0],
                pp - 1
            );
        }
    }
}

#[test]
fn zero_hp_substitute_absorbs_the_next_hit_before_real_hp_can_fall() {
    for target_player in [false, true] {
        let mut screen = battle(target_player, 20);
        let real_hp = target_mut(&mut screen, target_player).active_mon().hp;
        fight(&mut screen);
        finish_presentation(&mut screen);
        let target = target_mut(&mut screen, target_player);
        assert!(target.has_status2(status2::HAS_SUBSTITUTE_UP));
        assert_eq!(target.substitute_hp, 0);
        fight(&mut screen);
        let target = target_mut(&mut screen, target_player);
        assert!(!target.has_status2(status2::HAS_SUBSTITUTE_UP));
        assert_eq!(target.active_mon().hp, real_hp);
        finish_presentation(&mut screen);
        fight(&mut screen);
        assert_eq!(
            target_mut(&mut screen, target_player).active_mon().hp,
            real_hp - 20
        );
    }
}
