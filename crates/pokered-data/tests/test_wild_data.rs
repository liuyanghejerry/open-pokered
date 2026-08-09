use pokered_data::species::Species;
use pokered_data::wild_data::*;

#[test]
fn encounter_slot_chances_sum() {
    let sum: u16 = ENCOUNTER_SLOT_CHANCES.iter().map(|&c| c as u16).sum();
    assert_eq!(sum, 256, "Encounter slot chances should sum to 256");
}

#[test]
fn encounter_slot_chances_has_10_slots() {
    assert_eq!(ENCOUNTER_SLOT_CHANCES.len(), 10);
}

#[test]
fn red_wild_data_nonempty() {
    let data = wild_data(GameVersion::Red);
    assert!(!data.is_empty());
    assert_eq!(
        data.len(),
        57,
        "Expected 57 maps with wild encounters for Red"
    );
}

#[test]
fn blue_wild_data_nonempty() {
    let data = wild_data(GameVersion::Blue);
    assert!(!data.is_empty());
    assert_eq!(
        data.len(),
        57,
        "Expected 57 maps with wild encounters for Blue"
    );
}

#[test]
fn grass_encounters_have_10_mons() {
    for version in [GameVersion::Red, GameVersion::Blue] {
        let data = wild_data(version);
        for map in &data {
            if map.grass.encounter_rate > 0 {
                assert_eq!(
                    map.grass.mons.len(),
                    10,
                    "{:?} {} grass should have 10 encounter slots",
                    version,
                    map.name
                );
            }
        }
    }
}

#[test]
fn water_encounters_have_10_mons_or_empty() {
    for version in [GameVersion::Red, GameVersion::Blue] {
        let data = wild_data(version);
        for map in &data {
            if map.water.encounter_rate > 0 {
                assert_eq!(
                    map.water.mons.len(),
                    10,
                    "{:?} {} water should have 10 encounter slots",
                    version,
                    map.name
                );
            } else {
                assert!(
                    map.water.mons.is_empty(),
                    "{:?} {} water rate 0 but has mons",
                    version,
                    map.name
                );
            }
        }
    }
}

#[test]
fn all_wild_species_valid() {
    for version in [GameVersion::Red, GameVersion::Blue] {
        let data = wild_data(version);
        for map in &data {
            for mon in map.grass.mons.iter().chain(map.water.mons.iter()) {
                assert_ne!(mon.species, Species::None, "{} has Species::None", map.name);
                let dex = mon.species as u8;
                assert!(
                    dex >= 1 && dex <= 151,
                    "{} has invalid species dex {}",
                    map.name,
                    dex
                );
                assert!(
                    mon.level >= 1 && mon.level <= 100,
                    "{} has level {}",
                    map.name,
                    mon.level
                );
            }
        }
    }
}

#[test]
fn red_blue_have_differences() {
    let red = wild_data(GameVersion::Red);
    let blue = wild_data(GameVersion::Blue);
    assert_eq!(red.len(), blue.len());
    let mut diff_count = 0;
    for (r, b) in red.iter().zip(blue.iter()) {
        if r != b {
            diff_count += 1;
        }
    }
    assert!(
        diff_count >= 30,
        "Expected 30+ maps with Red/Blue differences, got {}",
        diff_count
    );
}

#[test]
fn good_rod_data_valid() {
    let data = good_rod_data();
    assert_eq!(data.len(), 2, "Good rod should have 2 pokemon");
    for mon in &data {
        assert_ne!(mon.species, Species::None);
        assert!(mon.level >= 1 && mon.level <= 100);
    }
}

#[test]
fn super_rod_groups_valid() {
    let groups = super_rod_groups();
    assert_eq!(groups.len(), 10, "Expected 10 super rod fishing groups");
    for (i, group) in groups.iter().enumerate() {
        assert!(!group.mons.is_empty(), "Super rod group {} is empty", i);
        for mon in &group.mons {
            assert_ne!(mon.species, Species::None);
            assert!(mon.level >= 1 && mon.level <= 100);
        }
    }
}

#[test]
fn super_rod_map_entries_valid() {
    let entries = super_rod_map_entries();
    let groups = super_rod_groups();
    assert_eq!(entries.len(), 33, "Expected 33 super rod map entries");
    for entry in &entries {
        assert!(!entry.map_name.is_empty());
        assert!(
            entry.group_index < groups.len(),
            "Map {} references invalid group {}",
            entry.map_name,
            entry.group_index
        );
    }
}
