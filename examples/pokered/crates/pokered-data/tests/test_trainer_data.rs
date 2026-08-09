use pokered_data::species::Species;
use pokered_data::trainer_data::{trainer_data, TrainerClass, NUM_TRAINER_CLASSES};

#[test]
fn trainer_class_count() {
    assert_eq!(NUM_TRAINER_CLASSES, 47);
}

#[test]
fn trainer_data_covers_all_classes() {
    let data = trainer_data();
    assert_eq!(data.len(), 47, "Expected 47 trainer classes");
}

#[test]
fn total_party_count() {
    let data = trainer_data();
    let total: usize = data.iter().map(|c| c.parties.len()).sum();
    assert_eq!(total, 391, "Expected 391 total trainer parties");
}

#[test]
fn all_parties_nonempty() {
    let data = trainer_data();
    for class_data in &data {
        for (i, party) in class_data.parties.iter().enumerate() {
            assert!(
                !party.pokemon.is_empty(),
                "{:?} party {} is empty",
                class_data.class,
                i
            );
        }
    }
}

#[test]
fn all_species_valid() {
    let data = trainer_data();
    for class_data in &data {
        for party in &class_data.parties {
            for mon in &party.pokemon {
                assert_ne!(
                    mon.species,
                    Species::None,
                    "{:?} party has Species::None",
                    class_data.class
                );
                let dex = mon.species as u8;
                assert!(
                    dex >= 1 && dex <= 151,
                    "{:?} party has invalid species {:?} (dex {})",
                    class_data.class,
                    mon.species,
                    dex
                );
            }
        }
    }
}

#[test]
fn all_levels_valid() {
    let data = trainer_data();
    for class_data in &data {
        for party in &class_data.parties {
            for mon in &party.pokemon {
                assert!(
                    mon.level >= 1 && mon.level <= 100,
                    "{:?} has {:?} at invalid level {}",
                    class_data.class,
                    mon.species,
                    mon.level
                );
            }
        }
    }
}

#[test]
fn party_sizes_max_six() {
    let data = trainer_data();
    for class_data in &data {
        for (i, party) in class_data.parties.iter().enumerate() {
            assert!(
                party.pokemon.len() <= 6,
                "{:?} party {} has {} pokemon (max 6)",
                class_data.class,
                i,
                party.pokemon.len()
            );
        }
    }
}

#[test]
fn spot_check_brock() {
    let data = trainer_data();
    let brock = data
        .iter()
        .find(|c| c.class == TrainerClass::Brock)
        .unwrap();
    assert_eq!(brock.parties.len(), 1);
    assert_eq!(brock.parties[0].pokemon.len(), 2);
    assert_eq!(brock.parties[0].pokemon[0].species, Species::Geodude);
    assert_eq!(brock.parties[0].pokemon[0].level, 12);
    assert_eq!(brock.parties[0].pokemon[1].species, Species::Onix);
    assert_eq!(brock.parties[0].pokemon[1].level, 14);
}

#[test]
fn spot_check_lance() {
    let data = trainer_data();
    let lance = data
        .iter()
        .find(|c| c.class == TrainerClass::Lance)
        .unwrap();
    assert_eq!(lance.parties.len(), 1);
    assert_eq!(lance.parties[0].pokemon.len(), 5);
}
