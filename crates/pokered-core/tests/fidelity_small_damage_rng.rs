//! Original RandomizeDamage returns without a draw when typed damage is 0/1.
use pokered_core::battle::state::StatusCondition;
use pokered_core::battle::{BattleInput, BattlePhase, BattleScreen};
use pokered_core::link::rng::LinkRng;
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::{moves::MoveId, species::Species, types::PokemonType};

// Observed original ROM results, initial damage 2, each first byte plus fallback 255.
const ORIGINAL_DAMAGE_TWO: [u16; 256] = [
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
    2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
    2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 2,
];
const ORIGINAL_DRAWS_TWO: [u64; 256] = [
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
    2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
    2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1,
];

fn setup(enemy: bool, id: MoveId, types: [PokemonType; 2], raw: u8) -> BattleScreen {
    let attacker = create_pokemon_with_moves(
        Species::Mewtwo,
        1,
        [0x9a, 0x78],
        [id, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut defender = create_pokemon_with_moves(
        Species::Snorlax,
        100,
        [0x9a, 0x78],
        [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    defender.status = StatusCondition::Sleep(7);
    let (player, foe) = if enemy {
        (defender, attacker)
    } else {
        (attacker, defender)
    };
    let mut battle = BattleScreen::from_parties(true, &[player], &[foe], None);
    battle.link_enemy_move_override = Some((if enemy { id } else { MoveId::Splash }, 0));
    let mut bytes = [255; 10];
    bytes[1] = 0;
    bytes[2] = raw;
    battle.link_rng = Some(LinkRng::new(bytes));
    let state = battle.battle_state.as_mut().unwrap();
    let (actor, target) = if enemy {
        (&mut state.enemy, &mut state.player)
    } else {
        (&mut state.player, &mut state.enemy)
    };
    // Prepared stats give base damage 2; converted types reduce it to 0/1.
    actor.active_mon_mut().attack = 1;
    actor.active_mon_mut().special = 1;
    actor.active_mon_mut().speed = 100;
    target.active_mon_mut().defense = 250;
    target.active_mon_mut().special = 250;
    target.conversion_type1 = Some(types[0]);
    target.conversion_type2 = Some(types[1]);
    target.active_mon_mut().speed = 200;
    battle.set_presentation_enabled(true);
    battle.phase = BattlePhase::PlayerMenu;
    battle
}

fn target_hp(battle: &BattleScreen, enemy: bool) -> u16 {
    let state = battle.battle_state.as_ref().unwrap();
    if enemy {
        state.player.active_mon().hp
    } else {
        state.enemy.active_mon().hp
    }
}

fn fight_and_drain(mut battle: BattleScreen, enemy: bool) -> (u16, u64) {
    let hp = target_hp(&battle, enemy);
    let mut input = BattleInput::none();
    input.a = true;
    for _ in 0..4 {
        battle.update_frame(input);
        if matches!(battle.phase, BattlePhase::ShowingText { .. }) {
            break;
        }
    }
    let BattlePhase::ShowingText { messages, .. } = &battle.phase else {
        panic!("FIGHT did not resolve");
    };
    assert!(messages.iter().all(|s| !s.contains("Critical hit!")));
    let outcome = (
        hp - target_hp(&battle, enemy),
        battle.link_rng.as_ref().unwrap().consumed(),
    );
    for frame in 0..4000 {
        if battle.presentation.waiting {
            battle.complete_move_presentation();
        }
        if matches!(battle.phase, BattlePhase::PlayerMenu) {
            assert_eq!(battle.link_rng.as_ref().unwrap().consumed(), outcome.1);
            return outcome;
        }
        let mut input = BattleInput::none();
        input.a = frame % 2 == 0;
        battle.update_frame(input);
    }
    panic!("presentation did not finish: {:?}", battle.phase);
}

#[test]
fn immune_and_resistance_rounded_zero_damage_do_not_draw_variance() {
    for enemy in [false, true] {
        for (id, types) in [
            (MoveId::Pound, [PokemonType::Ghost; 2]),
            (MoveId::VineWhip, [PokemonType::Poison, PokemonType::Bug]),
        ] {
            for raw in 0..=255 {
                assert_eq!(
                    fight_and_drain(setup(enemy, id, types, raw), enemy),
                    (0, 2),
                    "enemy={enemy} id={id:?} raw={raw}"
                );
            }
        }
    }
}

#[test]
fn one_damage_does_not_draw_variance() {
    for enemy in [false, true] {
        for raw in 0..=255 {
            assert_eq!(
                fight_and_drain(
                    setup(enemy, MoveId::Pound, [PokemonType::Rock; 2], raw),
                    enemy
                ),
                (1, 2),
                "enemy={enemy} raw={raw}"
            );
        }
    }
}

#[test]
fn two_damage_keeps_the_original_draws_even_when_variance_reduces_it_to_one() {
    for enemy in [false, true] {
        for raw in 0..=255 {
            assert_eq!(
                fight_and_drain(
                    setup(enemy, MoveId::Pound, [PokemonType::Normal; 2], raw),
                    enemy
                ),
                (
                    ORIGINAL_DAMAGE_TWO[raw as usize],
                    2 + ORIGINAL_DRAWS_TWO[raw as usize]
                ),
                "enemy={enemy} raw={raw}"
            );
        }
    }
}
