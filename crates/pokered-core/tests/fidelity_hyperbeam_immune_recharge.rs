//! A type-immune Hyper Beam must not spend the following turn recharging.
use pokered_core::battle::state::status2;
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

fn setup(enemy: bool, mode: &str) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        20,
        [0x9a, 0x78],
        [MoveId::HyperBeam, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let species = match mode {
        "gastly" => Species::Gastly,
        "haunter" => Species::Haunter,
        "gengar" => Species::Gengar,
        _ => Species::Snorlax,
    };
    let defender = create_pokemon_with_moves(
        species,
        100,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let (player, foe) = if enemy {
        (defender, attacker)
    } else {
        (attacker, defender)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_rng = Some(LinkRng::new(if mode == "miss" {
        [255; 10]
    } else {
        [255, 0, 255, 255, 0, 255, 255, 0, 255, 255]
    }));
    let state = battle.battle_state.as_mut().unwrap();
    let (actor, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    actor.active_mon_mut().speed = 200;
    target.active_mon_mut().speed = 100;
    if mode == "converted" {
        target.conversion_type1 = Some(PokemonType::Ghost);
        target.conversion_type2 = Some(PokemonType::Ghost);
    }
    if mode.starts_with("doll") {
        target.battle_status2 |= status2::HAS_SUBSTITUTE_UP;
        target.substitute_hp = if mode == "doll_break" { 1 } else { 100 };
    }
    if mode == "ko" {
        target.active_mon_mut().hp = 1;
    }
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn fight(battle: &mut BattleScreen, enemy: bool) -> Vec<String> {
    battle.link_enemy_move_override = Some((
        if enemy {
            MoveId::HyperBeam
        } else {
            MoveId::Splash
        },
        0,
    ));
    let mut input = BattleInput::none();
    input.a = true;
    for _ in 0..4 {
        battle.update_frame(input);
        if let BattlePhase::ShowingText { messages, .. } = &battle.phase {
            return messages
                .iter()
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect();
        }
    }
    panic!("FIGHT did not resolve: {:?}", battle.phase);
}

fn drain(battle: &mut BattleScreen) {
    for frame in 0..4000 {
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            return;
        }
        let mut input = BattleInput::none();
        input.a = frame % 2 == 0;
        battle.update_frame(input);
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

fn recharging(battle: &BattleScreen, enemy: bool) -> bool {
    let state = battle.battle_state.as_ref().unwrap();
    let actor = if enemy { &state.enemy } else { &state.player };
    actor.battle_status2 & status2::NEEDS_TO_RECHARGE != 0
}

fn target_hp(battle: &BattleScreen, enemy: bool) -> u16 {
    let state = battle.battle_state.as_ref().unwrap();
    if enemy {
        state.player.active_mon().hp
    } else {
        state.enemy.active_mon().hp
    }
}

#[test]
fn ghost_immunity_allows_another_attack_next_turn_on_both_sides() {
    for enemy in [false, true] {
        for mode in ["gastly", "haunter", "gengar", "converted"] {
            let mut battle = setup(enemy, mode);
            let hp = target_hp(&battle, enemy);
            let first = fight(&mut battle, enemy);
            assert!(
                first.iter().any(|s| s.contains("doesn't affect")),
                "{first:?}"
            );
            assert_eq!(target_hp(&battle, enemy), hp);
            assert!(
                !recharging(&battle, enemy),
                "enemy={enemy} mode={mode} {first:?}"
            );
            drain(&mut battle);
            let before = battle.link_rng.as_ref().unwrap().consumed();
            let second = fight(&mut battle, enemy);
            assert!(
                second.iter().any(|s| s.contains("used HYPER BEAM!")),
                "{second:?}"
            );
            assert!(
                second.iter().all(|s| !s.contains("must recharge!")),
                "{second:?}"
            );
            assert_eq!(target_hp(&battle, enemy), hp);
            assert!(!recharging(&battle, enemy));
            // Critical and accuracy only: immune damage skips RandomizeDamage.
            assert_eq!(battle.link_rng.as_ref().unwrap().consumed() - before, 2);
            drain(&mut battle);
        }
    }
}

#[test]
fn hit_miss_and_substitute_controls_keep_their_next_turn_behavior() {
    for enemy in [false, true] {
        for mode in ["ordinary", "miss", "doll_intact", "doll_break"] {
            let mut battle = setup(enemy, mode);
            let hp = target_hp(&battle, enemy);
            let expected = matches!(mode, "ordinary" | "doll_intact");
            let first = fight(&mut battle, enemy);
            assert_eq!(
                recharging(&battle, enemy),
                expected,
                "mode={mode} {first:?}"
            );
            assert_eq!(target_hp(&battle, enemy) < hp, mode == "ordinary");
            drain(&mut battle);
            let before = battle.link_rng.as_ref().unwrap().consumed();
            let second = fight(&mut battle, enemy);
            assert_eq!(
                second.iter().any(|s| s.contains("must recharge!")),
                expected,
                "mode={mode} {second:?}"
            );
            assert_eq!(
                second.iter().any(|s| s.contains("used HYPER BEAM!")),
                !expected
            );
            assert_eq!(
                battle.link_rng.as_ref().unwrap().consumed() - before,
                if expected {
                    0
                } else if mode == "miss" {
                    2
                } else {
                    3
                }
            );
            drain(&mut battle);
        }
    }
}

#[test]
fn a_knockout_still_skips_recharge() {
    for enemy in [false, true] {
        let mut battle = setup(enemy, "ko");
        let messages = fight(&mut battle, enemy);
        assert_eq!(target_hp(&battle, enemy), 0, "{messages:?}");
        assert!(!recharging(&battle, enemy), "{messages:?}");
    }
}
