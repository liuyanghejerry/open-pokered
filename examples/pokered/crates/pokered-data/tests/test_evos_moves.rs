use pokered_data::evos_moves::{evos_moves_data, get_evos_moves, EvolutionMethod};
use pokered_data::species::Species;
use strum::IntoEnumIterator;

#[test]
fn evos_moves_has_151_entries() {
    let data = evos_moves_data();
    assert_eq!(data.len(), 151);
}

#[test]
fn evos_moves_ordered_by_dex() {
    let data = evos_moves_data();
    for (i, entry) in data.iter().enumerate() {
        let expected_dex = (i + 1) as u8;
        assert_eq!(
            entry.species as u8, expected_dex,
            "evos_moves[{}] has {:?} (dex {}), expected dex {}",
            i, entry.species, entry.species as u8, expected_dex
        );
    }
}

#[test]
fn every_species_has_evos_moves() {
    for species in Species::iter() {
        if species == Species::None {
            assert!(get_evos_moves(species).is_none());
            continue;
        }
        let entry = get_evos_moves(species)
            .unwrap_or_else(|| panic!("Missing evos_moves for {:?}", species));
        assert_eq!(entry.species, species);
    }
}

#[test]
fn evolution_targets_valid() {
    let data = evos_moves_data();
    for entry in &data {
        for evo in &entry.evolutions {
            let target = match evo {
                EvolutionMethod::Level { species, .. } => species,
                EvolutionMethod::Item { species, .. } => species,
                EvolutionMethod::Trade { species, .. } => species,
            };
            assert_ne!(
                *target,
                Species::None,
                "{:?} evolves to None",
                entry.species
            );
            let dex = *target as u8;
            assert!(
                dex >= 1 && dex <= 151,
                "{:?} evolves to invalid dex {}",
                entry.species,
                dex
            );
        }
    }
}

#[test]
fn learnset_levels_ascending() {
    let data = evos_moves_data();
    for entry in &data {
        for window in entry.learnset.windows(2) {
            assert!(
                window[0].level <= window[1].level,
                "{:?} learnset not ascending: level {} before level {}",
                entry.species,
                window[0].level,
                window[1].level
            );
        }
    }
}

#[test]
fn species_with_evolutions_count() {
    let data = evos_moves_data();
    let count = data.iter().filter(|e| !e.evolutions.is_empty()).count();
    assert_eq!(count, 70, "Expected 70 species with evolutions");
}

#[test]
fn species_with_learnsets_count() {
    let data = evos_moves_data();
    let count = data.iter().filter(|e| !e.learnset.is_empty()).count();
    assert_eq!(count, 139, "Expected 139 species with level-up moves");
}

#[test]
fn spot_check_bulbasaur_evo() {
    let entry = get_evos_moves(Species::Bulbasaur).unwrap();
    assert_eq!(entry.evolutions.len(), 1);
    match &entry.evolutions[0] {
        EvolutionMethod::Level { level, species } => {
            assert_eq!(*level, 16);
            assert_eq!(*species, Species::Ivysaur);
        }
        _ => panic!("Bulbasaur should evolve by level"),
    }
}

#[test]
fn spot_check_pikachu_evo() {
    let entry = get_evos_moves(Species::Pikachu).unwrap();
    assert_eq!(entry.evolutions.len(), 1);
    match &entry.evolutions[0] {
        EvolutionMethod::Item { species, .. } => {
            assert_eq!(*species, Species::Raichu);
        }
        _ => panic!("Pikachu should evolve by item (Thunder Stone)"),
    }
}

#[test]
fn spot_check_kadabra_trade_evo() {
    let entry = get_evos_moves(Species::Kadabra).unwrap();
    let has_trade = entry
        .evolutions
        .iter()
        .any(|e| matches!(e, EvolutionMethod::Trade { .. }));
    assert!(has_trade, "Kadabra should have trade evolution");
}

#[test]
fn spot_check_mew_no_evolution() {
    let entry = get_evos_moves(Species::Mew).unwrap();
    assert!(entry.evolutions.is_empty(), "Mew should not evolve");
}
