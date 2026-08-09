//! Data integrity tests for pokemon_data module.

use pokered_data::pokemon_data::{get_base_stats, BASE_STATS};
use pokered_data::species::Species;
use strum::IntoEnumIterator;

#[test]
fn base_stats_has_151_entries() {
    assert_eq!(BASE_STATS.len(), 151);
}

#[test]
fn every_species_has_base_stats() {
    for species in Species::iter() {
        if species == Species::None {
            assert!(get_base_stats(species).is_none());
            continue;
        }
        let stats = get_base_stats(species)
            .unwrap_or_else(|| panic!("Missing base stats for {:?}", species));
        assert_eq!(
            stats.species, species,
            "Species mismatch in BASE_STATS entry"
        );
    }
}

#[test]
fn base_stats_ordered_by_dex_number() {
    for (i, stats) in BASE_STATS.iter().enumerate() {
        let expected_dex = (i + 1) as u8;
        assert_eq!(
            stats.species as u8, expected_dex,
            "BASE_STATS[{}] has species {:?} (dex {}), expected dex {}",
            i, stats.species, stats.species as u8, expected_dex
        );
    }
}

#[test]
fn base_stats_hp_nonzero() {
    for stats in BASE_STATS.iter() {
        assert!(stats.hp > 0, "{:?} has 0 HP", stats.species);
    }
}

#[test]
fn base_stats_catch_rate_valid() {
    for stats in BASE_STATS.iter() {
        assert!(stats.catch_rate > 0, "{:?} has 0 catch rate", stats.species);
    }
}

#[test]
fn spot_check_bulbasaur() {
    let s = get_base_stats(Species::Bulbasaur).unwrap();
    assert_eq!(s.hp, 45);
    assert_eq!(s.attack, 49);
    assert_eq!(s.defense, 49);
    assert_eq!(s.speed, 45);
    assert_eq!(s.special, 65);
}

#[test]
fn spot_check_pikachu() {
    let s = get_base_stats(Species::Pikachu).unwrap();
    assert_eq!(s.hp, 35);
    assert_eq!(s.attack, 55);
    assert_eq!(s.defense, 30);
    assert_eq!(s.speed, 90);
    assert_eq!(s.special, 50);
}

#[test]
fn spot_check_mewtwo() {
    let s = get_base_stats(Species::Mewtwo).unwrap();
    assert_eq!(s.hp, 106);
    assert_eq!(s.attack, 110);
    assert_eq!(s.defense, 90);
    assert_eq!(s.speed, 130);
    assert_eq!(s.special, 154);
}

#[test]
fn spot_check_mew() {
    let s = get_base_stats(Species::Mew).unwrap();
    assert_eq!(s.hp, 100);
    assert_eq!(s.attack, 100);
    assert_eq!(s.defense, 100);
    assert_eq!(s.speed, 100);
    assert_eq!(s.special, 100);
}
