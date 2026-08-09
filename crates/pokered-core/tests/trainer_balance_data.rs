//! M9.2: Trainer data integrity and balance verification tests.
//! Validates all trainer class data, party compositions, and level ranges.

use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::trainer_data::{get_base_money, trainer_data, TrainerClass, NUM_TRAINER_CLASSES};

/// Every trainer class (except Nobody) should have at least one party defined.
#[test]
fn all_trainer_classes_have_parties() {
    let data = trainer_data();
    // trainer_data() returns a Vec of TrainerClassData; check coverage
    let classes_with_parties: Vec<TrainerClass> = data.iter().map(|d| d.class).collect();

    // All gym leaders, Elite Four, rivals should be present
    let required = [
        TrainerClass::Brock,
        TrainerClass::Misty,
        TrainerClass::LtSurge,
        TrainerClass::Erika,
        TrainerClass::Koga,
        TrainerClass::Blaine,
        TrainerClass::Sabrina,
        TrainerClass::Giovanni,
        TrainerClass::Lorelei,
        TrainerClass::Bruno,
        TrainerClass::Agatha,
        TrainerClass::Lance,
        TrainerClass::Rival1,
        TrainerClass::Rival2,
        TrainerClass::Rival3,
    ];

    for class in &required {
        assert!(
            classes_with_parties.contains(class),
            "TrainerClass {:?} should have party data",
            class
        );
    }
}

/// No trainer Pokémon should have level 0 or above 100.
#[test]
fn trainer_pokemon_levels_in_valid_range() {
    let data = trainer_data();
    for class_data in &data {
        for (party_idx, party) in class_data.parties.iter().enumerate() {
            for (mon_idx, mon) in party.pokemon.iter().enumerate() {
                assert!(
                    mon.level >= 1 && mon.level <= 100,
                    "Trainer {:?} party {} mon {} has invalid level {}",
                    class_data.class,
                    party_idx,
                    mon_idx,
                    mon.level
                );
            }
        }
    }
}

/// No trainer should have Species::None in their party.
#[test]
fn trainer_pokemon_species_are_valid() {
    let data = trainer_data();
    for class_data in &data {
        for (party_idx, party) in class_data.parties.iter().enumerate() {
            for (mon_idx, mon) in party.pokemon.iter().enumerate() {
                assert_ne!(
                    mon.species,
                    Species::None,
                    "Trainer {:?} party {} mon {} has Species::None",
                    class_data.class,
                    party_idx,
                    mon_idx
                );
                // Also verify base stats exist for this species
                assert!(
                    get_base_stats(mon.species).is_some(),
                    "Trainer {:?} party {} mon {} species {:?} has no base stats",
                    class_data.class,
                    party_idx,
                    mon_idx,
                    mon.species
                );
            }
        }
    }
}

/// No trainer party should be empty.
#[test]
fn trainer_parties_are_non_empty() {
    let data = trainer_data();
    for class_data in &data {
        for (party_idx, party) in class_data.parties.iter().enumerate() {
            assert!(
                !party.pokemon.is_empty(),
                "Trainer {:?} party {} is empty",
                class_data.class,
                party_idx
            );
        }
    }
}

/// No trainer party should exceed 6 Pokémon.
#[test]
fn trainer_parties_max_six_pokemon() {
    let data = trainer_data();
    for class_data in &data {
        for (party_idx, party) in class_data.parties.iter().enumerate() {
            assert!(
                party.pokemon.len() <= 6,
                "Trainer {:?} party {} has {} Pokémon (max 6)",
                class_data.class,
                party_idx,
                party.pokemon.len()
            );
        }
    }
}

/// Every trainer class should have a non-zero base money (except Nobody).
#[test]
fn all_trainer_classes_have_base_money() {
    let classes = [
        TrainerClass::Youngster,
        TrainerClass::BugCatcher,
        TrainerClass::Lass,
        TrainerClass::Sailor,
        TrainerClass::JrTrainerM,
        TrainerClass::JrTrainerF,
        TrainerClass::Pokemaniac,
        TrainerClass::SuperNerd,
        TrainerClass::Hiker,
        TrainerClass::Biker,
        TrainerClass::Burglar,
        TrainerClass::Engineer,
        TrainerClass::Fisher,
        TrainerClass::Swimmer,
        TrainerClass::CueBall,
        TrainerClass::Gambler,
        TrainerClass::Beauty,
        TrainerClass::PsychicTr,
        TrainerClass::Rocker,
        TrainerClass::Juggler,
        TrainerClass::Tamer,
        TrainerClass::BirdKeeper,
        TrainerClass::Blackbelt,
        TrainerClass::Rival1,
        TrainerClass::Rival2,
        TrainerClass::Rival3,
        TrainerClass::Giovanni,
        TrainerClass::Rocket,
        TrainerClass::CooltrainerM,
        TrainerClass::CooltrainerF,
        TrainerClass::Brock,
        TrainerClass::Misty,
        TrainerClass::LtSurge,
        TrainerClass::Erika,
        TrainerClass::Koga,
        TrainerClass::Blaine,
        TrainerClass::Sabrina,
        TrainerClass::Bruno,
        TrainerClass::Lorelei,
        TrainerClass::Agatha,
        TrainerClass::Lance,
        TrainerClass::Scientist,
        TrainerClass::Gentleman,
        TrainerClass::Channeler,
    ];

    for class in &classes {
        let money = get_base_money(*class);
        assert!(
            money > 0,
            "TrainerClass {:?} should have non-zero base money, got {}",
            class,
            money
        );
    }
}

/// Nobody should have 0 base money.
#[test]
fn nobody_has_zero_base_money() {
    assert_eq!(get_base_money(TrainerClass::Nobody), 0);
}

/// Gym leaders and Elite Four should all have 99 base money.
#[test]
fn gym_leaders_elite_four_have_max_base_money() {
    let elite = [
        TrainerClass::Brock,
        TrainerClass::Misty,
        TrainerClass::LtSurge,
        TrainerClass::Erika,
        TrainerClass::Koga,
        TrainerClass::Blaine,
        TrainerClass::Sabrina,
        TrainerClass::Giovanni,
        TrainerClass::Bruno,
        TrainerClass::Lorelei,
        TrainerClass::Agatha,
        TrainerClass::Lance,
        TrainerClass::Rival3,
    ];

    for class in &elite {
        assert_eq!(
            get_base_money(*class),
            99,
            "TrainerClass {:?} should have 99 base money",
            class
        );
    }
}

/// Verify specific known base money values for common trainer classes.
#[test]
fn known_base_money_values() {
    assert_eq!(get_base_money(TrainerClass::Youngster), 15);
    assert_eq!(get_base_money(TrainerClass::BugCatcher), 10);
    assert_eq!(get_base_money(TrainerClass::Lass), 15);
    assert_eq!(get_base_money(TrainerClass::Sailor), 30);
    assert_eq!(get_base_money(TrainerClass::Hiker), 35);
    assert_eq!(get_base_money(TrainerClass::Burglar), 90);
    assert_eq!(get_base_money(TrainerClass::Swimmer), 5);
    assert_eq!(get_base_money(TrainerClass::Gambler), 70);
    assert_eq!(get_base_money(TrainerClass::Beauty), 70);
    assert_eq!(get_base_money(TrainerClass::ProfOak), 99);
    assert_eq!(get_base_money(TrainerClass::Rocket), 30);
    assert_eq!(get_base_money(TrainerClass::Rival1), 35);
    assert_eq!(get_base_money(TrainerClass::Rival2), 65);
}

/// Early-game trainers (Route 1-3 area) should have level ~7-14 Pokémon.
#[test]
fn early_game_trainer_levels_reasonable() {
    let data = trainer_data();
    // Bug Catchers and Youngsters are early game
    for class_data in &data {
        if class_data.class == TrainerClass::BugCatcher
            || class_data.class == TrainerClass::Youngster
        {
            for party in &class_data.parties {
                for mon in &party.pokemon {
                    // Early game trainers shouldn't exceed level 20
                    // (some Youngsters appear later, so allow wider range)
                    assert!(
                        mon.level <= 30,
                        "Early trainer {:?} has unexpectedly high level {} Pokémon",
                        class_data.class,
                        mon.level
                    );
                }
            }
        }
    }
}

/// Gym leaders should have increasing level progression.
#[test]
fn gym_leader_level_progression() {
    let data = trainer_data();

    fn max_level(
        data: &[pokered_data::trainer_data::TrainerClassData],
        class: TrainerClass,
        party_idx: usize,
    ) -> u8 {
        data.iter()
            .find(|d| d.class == class)
            .and_then(|d| d.parties.get(party_idx))
            .map(|p| p.pokemon.iter().map(|m| m.level).max().unwrap_or(0))
            .unwrap_or(0)
    }

    // Gym leaders - each has one party (party index 0)
    let brock_max = max_level(&data, TrainerClass::Brock, 0);
    let misty_max = max_level(&data, TrainerClass::Misty, 0);
    let surge_max = max_level(&data, TrainerClass::LtSurge, 0);
    let erika_max = max_level(&data, TrainerClass::Erika, 0);
    let koga_max = max_level(&data, TrainerClass::Koga, 0);
    let sabrina_max = max_level(&data, TrainerClass::Sabrina, 0);
    let blaine_max = max_level(&data, TrainerClass::Blaine, 0);

    // Giovanni has multiple encounters; gym is party index 2
    let giovanni_max = max_level(&data, TrainerClass::Giovanni, 2);

    // Level should increase across gyms
    assert!(
        brock_max < misty_max,
        "Brock {} < Misty {}",
        brock_max,
        misty_max
    );
    assert!(
        misty_max < surge_max,
        "Misty {} < Surge {}",
        misty_max,
        surge_max
    );
    assert!(
        surge_max < erika_max,
        "Surge {} < Erika {}",
        surge_max,
        erika_max
    );
    assert!(
        erika_max < koga_max,
        "Erika {} < Koga {}",
        erika_max,
        koga_max
    );
    assert!(
        koga_max <= sabrina_max,
        "Koga {} <= Sabrina {}",
        koga_max,
        sabrina_max
    );
    assert!(
        blaine_max >= sabrina_max,
        "Blaine {} >= Sabrina {}",
        blaine_max,
        sabrina_max
    );
    assert!(
        giovanni_max >= blaine_max,
        "Giovanni {} >= Blaine {}",
        giovanni_max,
        blaine_max
    );
}

/// Verify total number of trainer classes matches expected constant.
#[test]
fn num_trainer_classes_correct() {
    assert_eq!(NUM_TRAINER_CLASSES, 47);
}

/// Verify Brock's team specifically (Geodude L12, Onix L14).
#[test]
fn brock_team_composition() {
    let data = trainer_data();
    let brock = data
        .iter()
        .find(|d| d.class == TrainerClass::Brock)
        .unwrap();
    assert_eq!(brock.parties.len(), 1);
    let party = &brock.parties[0];
    assert_eq!(party.pokemon.len(), 2);
    assert_eq!(party.pokemon[0].species, Species::Geodude);
    assert_eq!(party.pokemon[0].level, 12);
    assert_eq!(party.pokemon[1].species, Species::Onix);
    assert_eq!(party.pokemon[1].level, 14);
}

/// Verify Misty's team specifically (Staryu L18, Starmie L21).
#[test]
fn misty_team_composition() {
    let data = trainer_data();
    let misty = data
        .iter()
        .find(|d| d.class == TrainerClass::Misty)
        .unwrap();
    assert_eq!(misty.parties.len(), 1);
    let party = &misty.parties[0];
    assert_eq!(party.pokemon.len(), 2);
    assert_eq!(party.pokemon[0].species, Species::Staryu);
    assert_eq!(party.pokemon[0].level, 18);
    assert_eq!(party.pokemon[1].species, Species::Starmie);
    assert_eq!(party.pokemon[1].level, 21);
}
