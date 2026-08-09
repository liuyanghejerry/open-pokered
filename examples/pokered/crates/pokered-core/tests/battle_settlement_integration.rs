//! Integration test: Battle settlement with exp gain, level-up, evolution, money
//! Verifies the full post-battle settlement pipeline.

mod helpers;

use helpers::*;
use pokered_core::battle::experience::gain::{calc_exp_gain, gain_experience};
use pokered_core::battle::experience::growth::exp_for_level;
use pokered_core::battle::settlement::settle::settle_battle;
use pokered_core::battle::settlement::{BattleOutcome, ExpGainEntry};
use pokered_core::battle::state::*;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::{GrowthRate, Species};
use pokered_data::trainer_data::TrainerClass;

#[test]
fn exp_gain_formula_wild_battle() {
    // Rattata base_exp=57, level=5: (57*5)/7 = 40
    assert_eq!(calc_exp_gain(57, 5, false, false), 40);
}

#[test]
fn exp_gain_formula_trainer_boost() {
    // base_exp=57, level=5: (57*5)/7=40, trainer 1.5x = 60
    assert_eq!(calc_exp_gain(57, 5, false, true), 60);
}

#[test]
fn exp_gain_formula_traded_boost() {
    // base_exp=57, level=5: (57*5)/7=40, traded 1.5x = 60
    assert_eq!(calc_exp_gain(57, 5, true, false), 60);
}

#[test]
fn exp_gain_formula_both_boosts() {
    // base_exp=57, level=5: (57*5)/7=40, traded 1.5x=60, then trainer 1.5x=90
    assert_eq!(calc_exp_gain(57, 5, true, true), 90);
}

#[test]
fn wild_battle_exp_gain_triggers_level_up() {
    let bulbasaur_base = get_base_stats(Species::Bulbasaur).unwrap();
    let start_exp = exp_for_level(bulbasaur_base.growth_rate, 5);

    let mut player = make_bulbasaur(5, 30);
    player.total_exp = start_exp;

    let enemy = make_rattata(10, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.party_gain_exp_flags[0] = true;

    let result = gain_experience(&mut state, Species::Rattata, 10, false);

    assert!(!result.leveled_up.is_empty(), "Bulbasaur should level up");
    assert!(state.player.party[0].level > 5);
}

#[test]
fn settlement_captures_exp_entries() {
    let mut player = make_pikachu(25, 55, 90);
    player.total_exp = 5000;

    let enemy = make_rattata(5, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.party_gain_exp_flags[0] = true;

    gain_experience(&mut state, Species::Rattata, 5, false);

    let settlement = settle_battle(&mut state, BattleOutcome::Win, None, 0);
    assert_eq!(settlement.outcome, BattleOutcome::Win);
    assert!(!settlement.exp_entries.is_empty());
    assert_eq!(settlement.exp_entries[0].species, Species::Pikachu);
    assert!(settlement.exp_entries[0].exp_gained > 5000);
}

#[test]
fn trainer_battle_settlement_money_and_exp() {
    let mut player = make_pikachu(10, 30, 50);
    player.total_exp = 1000;

    let caterpie = make_pokemon(
        Species::Caterpie,
        8,
        30,
        30,
        35,
        45,
        20,
        pokered_data::types::PokemonType::Bug,
        pokered_data::types::PokemonType::Bug,
        [
            pokered_data::moves::MoveId::Tackle,
            pokered_data::moves::MoveId::StringShot,
            pokered_data::moves::MoveId::None,
            pokered_data::moves::MoveId::None,
        ],
        [35, 40, 0, 0],
    );

    let mut state = new_battle_state(BattleType::Trainer, vec![player], vec![caterpie]);
    state.party_gain_exp_flags[0] = true;

    gain_experience(&mut state, Species::Caterpie, 8, false);

    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Win,
        Some(TrainerClass::BugCatcher),
        500,
    );

    assert_eq!(settlement.outcome, BattleOutcome::Win);
    // Bug Catcher base_money=10, last level=8 → 80
    assert_eq!(settlement.money_gained, 80);
    assert!(!settlement.exp_entries.is_empty());
}

#[test]
fn loss_settlement_no_exp_entries() {
    let player = make_pikachu(5, 20, 30);
    let onix = make_geodude(14, 200);
    let mut state = new_battle_state(BattleType::Trainer, vec![player], vec![onix]);
    state.party_gain_exp_flags[0] = true;

    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Loss,
        Some(TrainerClass::Brock),
        1000,
    );

    assert_eq!(settlement.outcome, BattleOutcome::Loss);
    assert_eq!(settlement.money_lost, 500);
    assert!(settlement.exp_entries.is_empty());
}

#[test]
fn escaped_settlement_no_money_skips_evolution() {
    // EndOfBattle skips EvolutionAfterBattle unless wBattleResult == 0 (win):
    // running away sets wBattleResult = $2 (engine/battle/core.asm:2293;
    // engine/battle/end_of_battle.asm:29-33).
    let mut player = make_pokemon(
        Species::Bulbasaur,
        16,
        40,
        49,
        49,
        45,
        65,
        pokered_data::types::PokemonType::Grass,
        pokered_data::types::PokemonType::Poison,
        [
            pokered_data::moves::MoveId::VineWhip,
            pokered_data::moves::MoveId::Tackle,
            pokered_data::moves::MoveId::None,
            pokered_data::moves::MoveId::None,
        ],
        [10, 35, 0, 0],
    );
    player.total_exp = exp_for_level(GrowthRate::MediumSlow, 16);

    let enemy = make_rattata(5, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.party_gain_exp_flags[0] = true;
    state.party_leveled_up_flags[0] = true;

    let settlement = settle_battle(&mut state, BattleOutcome::Escaped, None, 0);
    assert_eq!(settlement.outcome, BattleOutcome::Escaped);
    assert_eq!(settlement.money_gained, 0);
    assert!(settlement.evolutions.is_empty(), "no evolution after running away");
}

#[test]
fn payday_money_included_in_settlement() {
    let mut player = make_pikachu(25, 55, 90);
    player.total_exp = 5000;

    let enemy = make_rattata(5, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.party_gain_exp_flags[0] = true;
    state.total_payday_money = 500;

    gain_experience(&mut state, Species::Rattata, 5, false);

    let settlement = settle_battle(&mut state, BattleOutcome::Win, None, 0);
    assert_eq!(settlement.payday_bonus, 500);
    assert_eq!(settlement.money_gained, 500);
}

#[test]
fn multiple_party_members_gain_exp() {
    let mut p1 = make_pikachu(25, 55, 90);
    p1.total_exp = 5000;
    let mut p2 = make_charmander(10, 30);
    p2.total_exp = 500;

    let enemy = make_rattata(10, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![p1, p2], vec![enemy]);
    state.party_gain_exp_flags[0] = true;
    state.party_gain_exp_flags[1] = true;

    let result = gain_experience(&mut state, Species::Rattata, 10, false);

    let exp0 = state.player.party[0].total_exp;
    let exp1 = state.player.party[1].total_exp;
    assert!(exp0 > 5000, "Pikachu should gain exp");
    assert!(exp1 > 500, "Charmander should gain exp");
}
