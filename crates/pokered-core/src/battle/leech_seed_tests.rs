//! Original LeechSeedEffect_ checks accuracy, both Grass types and SEEDED.
//! A Substitute is not an immunity. Exercise the production FIGHT input path,
//! with a prepared battle fixture at the player menu (not a legacy oracle).
use super::state::status2;
use super::*;
use crate::pokemon::stats::create_pokemon_with_moves;

#[test]
fn leech_seed_fight_input_bypasses_substitute_on_either_side() {
    for target_player in [false, true] {
        for substitute in [false, true] {
            // Paras tests the second type slot; Bulbasaur tests the first.
            for (species, immune) in [
                (Species::Rattata, false),
                (Species::Bulbasaur, true),
                (Species::Paras, true),
            ] {
                let mut seeder = create_pokemon_with_moves(
                    Species::Venusaur,
                    50,
                    [0x9a, 0x78],
                    [MoveId::LeechSeed, MoveId::None, MoveId::None, MoveId::None],
                )
                .unwrap();
                seeder.hp -= 20;
                let target = create_pokemon_with_moves(
                    species,
                    20,
                    [0x9a, 0x78],
                    [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
                )
                .unwrap();
                assert!(seeder.speed > target.speed);
                let seeder_hp = seeder.hp;
                let target_hp = target.hp;
                let drain = if immune {
                    0
                } else {
                    (target.max_hp / 16).max(1)
                };
                let (player, enemy) = if target_player {
                    (target, seeder)
                } else {
                    (seeder, target)
                };
                let player_pp = player.pp[0];
                let mut screen = BattleScreen::from_parties(true, &[player], &[enemy], None);
                screen.phase = BattlePhase::PlayerMenu;
                screen.rng = pokered_rules::runtime::StdBattleRng::from_seed(42);
                if substitute {
                    let bs = screen.battle_state.as_mut().unwrap();
                    let target = if target_player {
                        &mut bs.player
                    } else {
                        &mut bs.enemy
                    };
                    target.set_status2(status2::HAS_SUBSTITUTE_UP);
                    target.substitute_hp = 10;
                }

                let confirm = BattleInput {
                    a: true,
                    ..BattleInput::none()
                };
                screen.update_frame(confirm);
                assert_eq!(screen.phase, BattlePhase::MoveSelect);
                screen.update_frame(confirm);
                assert!(matches!(screen.phase, BattlePhase::ShowingText { .. }));

                let bs = screen.battle_state.as_ref().unwrap();
                let (target, seeder) = if target_player {
                    (&bs.player, &bs.enemy)
                } else {
                    (&bs.enemy, &bs.player)
                };
                assert_eq!(seeder.last_move_used, MoveId::LeechSeed);
                assert_eq!(target.last_move_used, MoveId::Splash);
                assert_eq!(
                    target.has_status2(status2::SEEDED),
                    !immune,
                    "target_player={target_player}, substitute={substitute}, species={species:?}"
                );
                assert_eq!(target.active_mon().hp, target_hp - drain);
                assert_eq!(seeder.active_mon().hp, seeder_hp + drain);
                assert_eq!(target.has_status2(status2::HAS_SUBSTITUTE_UP), substitute);
                assert_eq!(target.substitute_hp, if substitute { 10 } else { 0 });
                assert_eq!(bs.player.active_mon().pp[0], player_pp - 1);
            }
        }
    }
}
