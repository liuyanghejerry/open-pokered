use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

use crate::battle::state::StatusCondition;
use crate::pokemon::stats::*;

const MAX_DVS: [u8; 2] = [0xFF, 0xFF];
const ZERO_DVS: [u8; 2] = [0x00, 0x00];

#[test]
fn create_pokemon_bulbasaur_level_5() {
    let mon = create_pokemon(Species::Bulbasaur, 5, MAX_DVS).unwrap();
    assert_eq!(mon.species, Species::Bulbasaur);
    assert_eq!(mon.level, 5);
    assert_eq!(mon.type1, PokemonType::Grass);
    assert_eq!(mon.type2, PokemonType::Poison);
    assert_eq!(mon.status, StatusCondition::None);
    assert!(!mon.is_traded);
    assert_eq!(mon.hp, mon.max_hp);
    assert!(mon.max_hp > 0);
    assert!(mon.attack > 0);
    assert!(mon.defense > 0);
    assert!(mon.speed > 0);
    assert!(mon.special > 0);
}

#[test]
fn create_pokemon_has_initial_moves() {
    let mon = create_pokemon(Species::Bulbasaur, 5, MAX_DVS).unwrap();
    assert_eq!(mon.moves[0], MoveId::Tackle);
    assert_eq!(mon.moves[1], MoveId::Growl);
    assert_eq!(mon.moves[2], MoveId::None);
    assert_eq!(mon.moves[3], MoveId::None);
    assert!(mon.pp[0] > 0);
    assert!(mon.pp[1] > 0);
    assert_eq!(mon.pp[2], 0);
    assert_eq!(mon.pp[3], 0);
}

#[test]
fn create_pokemon_invalid_species() {
    let result = create_pokemon(Species::None, 5, MAX_DVS);
    assert!(result.is_none());
}

#[test]
fn create_pokemon_has_correct_exp() {
    let mon = create_pokemon(Species::Bulbasaur, 5, MAX_DVS).unwrap();
    assert!(mon.total_exp > 0);
}

#[test]
fn create_pokemon_with_custom_moves() {
    let moves = [MoveId::VineWhip, MoveId::Toxic, MoveId::None, MoveId::None];
    let mon = create_pokemon_with_moves(Species::Bulbasaur, 20, MAX_DVS, moves).unwrap();
    assert_eq!(mon.moves, moves);
    assert!(mon.pp[0] > 0);
    assert!(mon.pp[1] > 0);
    assert_eq!(mon.pp[2], 0);
    assert_eq!(mon.pp[3], 0);
}

#[test]
fn recalculate_stats_preserves_hp_ratio() {
    let mut mon = create_pokemon(Species::Pikachu, 10, MAX_DVS).unwrap();
    let old_max = mon.max_hp;
    mon.hp = old_max / 2;

    mon.level = 20;
    recalculate_stats(&mut mon);

    assert!(mon.max_hp > old_max);
    assert!(mon.hp > old_max / 2);
}

#[test]
fn recalculate_stats_updates_all_stats() {
    let mut mon = create_pokemon(Species::Pikachu, 5, MAX_DVS).unwrap();
    let old_atk = mon.attack;
    let old_def = mon.defense;
    let old_spd = mon.speed;
    let old_spc = mon.special;

    mon.level = 50;
    recalculate_stats(&mut mon);

    assert!(mon.attack > old_atk);
    assert!(mon.defense > old_def);
    assert!(mon.speed > old_spd);
    assert!(mon.special > old_spc);
}

#[test]
fn zero_dvs_give_lower_stats() {
    let max_mon = create_pokemon(Species::Pikachu, 50, MAX_DVS).unwrap();
    let zero_mon = create_pokemon(Species::Pikachu, 50, ZERO_DVS).unwrap();

    assert!(max_mon.max_hp > zero_mon.max_hp);
    assert!(max_mon.attack > zero_mon.attack);
    assert!(max_mon.defense > zero_mon.defense);
    assert!(max_mon.speed > zero_mon.speed);
    assert!(max_mon.special > zero_mon.special);
}

#[test]
fn stat_exp_increases_stats() {
    let mut mon = create_pokemon(Species::Pikachu, 50, MAX_DVS).unwrap();
    let base_atk = mon.attack;

    mon.stat_exp = [65535, 65535, 65535, 65535, 65535];
    recalculate_stats(&mut mon);

    assert!(mon.attack > base_atk);
}

#[test]
fn stat_summary_max_dvs() {
    let mon = create_pokemon(Species::Pikachu, 50, MAX_DVS).unwrap();
    let summary = stat_summary(&mon);

    assert_eq!(summary.hp, mon.max_hp);
    assert_eq!(summary.attack, mon.attack);
    assert_eq!(summary.defense, mon.defense);
    assert_eq!(summary.speed, mon.speed);
    assert_eq!(summary.special, mon.special);
    assert_eq!(summary.hp_iv, 15);
    assert_eq!(summary.atk_iv, 15);
    assert_eq!(summary.def_iv, 15);
    assert_eq!(summary.spd_iv, 15);
    assert_eq!(summary.spc_iv, 15);
}

#[test]
fn stat_summary_zero_dvs() {
    let mon = create_pokemon(Species::Pikachu, 50, ZERO_DVS).unwrap();
    let summary = stat_summary(&mon);

    assert_eq!(summary.hp_iv, 0);
    assert_eq!(summary.atk_iv, 0);
    assert_eq!(summary.def_iv, 0);
    assert_eq!(summary.spd_iv, 0);
    assert_eq!(summary.spc_iv, 0);
}

#[test]
fn stat_summary_mixed_dvs() {
    let dv = [0xA5, 0x3C];
    let mon = create_pokemon(Species::Pikachu, 50, dv).unwrap();
    let summary = stat_summary(&mon);

    assert_eq!(summary.atk_iv, 10);
    assert_eq!(summary.def_iv, 5);
    assert_eq!(summary.spd_iv, 3);
    assert_eq!(summary.spc_iv, 12);
    assert_eq!(summary.hp_iv, 6);
}

#[test]
fn pikachu_level_25_max_dvs_known_values() {
    let mon = create_pokemon(Species::Pikachu, 25, MAX_DVS).unwrap();
    assert_eq!(mon.max_hp, 60);
    assert_eq!(mon.attack, 40);
    assert_eq!(mon.defense, 27);
    assert_eq!(mon.speed, 57);
    assert_eq!(mon.special, 37);
}

#[test]
fn recalculate_stats_on_fresh_pokemon_is_idempotent() {
    let mut mon = create_pokemon(Species::Bulbasaur, 30, MAX_DVS).unwrap();
    let original = mon.clone();

    recalculate_stats(&mut mon);

    assert_eq!(mon.max_hp, original.max_hp);
    assert_eq!(mon.attack, original.attack);
    assert_eq!(mon.defense, original.defense);
    assert_eq!(mon.speed, original.speed);
    assert_eq!(mon.special, original.special);
    assert_eq!(mon.hp, original.hp);
}
