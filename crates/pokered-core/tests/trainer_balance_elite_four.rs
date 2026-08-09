use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::trainer_data::{get_base_money, trainer_data, TrainerClass};
use pokered_data::types::PokemonType;

fn get_party(class: TrainerClass, party_idx: usize) -> Vec<pokered_data::trainer_data::TrainerMon> {
    let data = trainer_data();
    data.iter().find(|d| d.class == class).unwrap().parties[party_idx]
        .pokemon
        .clone()
}

fn max_level_of(class: TrainerClass, party_idx: usize) -> u8 {
    get_party(class, party_idx)
        .iter()
        .map(|m| m.level)
        .max()
        .unwrap_or(0)
}

fn min_level_of(class: TrainerClass, party_idx: usize) -> u8 {
    get_party(class, party_idx)
        .iter()
        .map(|m| m.level)
        .min()
        .unwrap_or(0)
}

#[test]
fn elite_four_level_progression() {
    let lorelei_max = max_level_of(TrainerClass::Lorelei, 0);
    let bruno_max = max_level_of(TrainerClass::Bruno, 0);
    let agatha_max = max_level_of(TrainerClass::Agatha, 0);
    let lance_max = max_level_of(TrainerClass::Lance, 0);

    assert!(
        lorelei_max <= bruno_max,
        "Lorelei {} <= Bruno {}",
        lorelei_max,
        bruno_max
    );
    assert!(
        bruno_max <= agatha_max,
        "Bruno {} <= Agatha {}",
        bruno_max,
        agatha_max
    );
    assert!(
        agatha_max <= lance_max,
        "Agatha {} <= Lance {}",
        agatha_max,
        lance_max
    );
}

#[test]
fn champion_higher_than_elite_four() {
    let lance_max = max_level_of(TrainerClass::Lance, 0);
    let data = trainer_data();
    let rival3 = data
        .iter()
        .find(|d| d.class == TrainerClass::Rival3)
        .unwrap();

    for (i, party) in rival3.parties.iter().enumerate() {
        let champion_max = party.pokemon.iter().map(|m| m.level).max().unwrap_or(0);
        assert!(
            champion_max > lance_max,
            "Champion variant {} max level {} should exceed Lance's {}",
            i,
            champion_max,
            lance_max
        );
    }
}

#[test]
fn lorelei_is_ice_water_themed() {
    let party = get_party(TrainerClass::Lorelei, 0);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Water
                || stats.type2 == PokemonType::Water
                || stats.type1 == PokemonType::Ice
                || stats.type2 == PokemonType::Ice,
            "Lorelei's {:?} should be Water or Ice type",
            mon.species
        );
    }
}

#[test]
fn bruno_is_fighting_themed() {
    let party = get_party(TrainerClass::Bruno, 0);
    let fighting_or_rock_count = party
        .iter()
        .filter(|mon| {
            let stats = get_base_stats(mon.species).unwrap();
            stats.type1 == PokemonType::Fighting
                || stats.type2 == PokemonType::Fighting
                || stats.type1 == PokemonType::Rock
                || stats.type2 == PokemonType::Rock
        })
        .count();
    assert!(
        fighting_or_rock_count >= 3,
        "Bruno should have mostly Fighting/Rock types, found {}",
        fighting_or_rock_count
    );
}

#[test]
fn agatha_is_ghost_poison_themed() {
    let party = get_party(TrainerClass::Agatha, 0);
    let ghost_poison_count = party
        .iter()
        .filter(|mon| {
            let stats = get_base_stats(mon.species).unwrap();
            stats.type1 == PokemonType::Ghost
                || stats.type2 == PokemonType::Ghost
                || stats.type1 == PokemonType::Poison
                || stats.type2 == PokemonType::Poison
        })
        .count();
    assert!(
        ghost_poison_count >= 3,
        "Agatha should have mostly Ghost/Poison types, found {}",
        ghost_poison_count
    );
}

#[test]
fn lance_is_dragon_flying_themed() {
    let party = get_party(TrainerClass::Lance, 0);
    let dragon_flying_count = party
        .iter()
        .filter(|mon| {
            let stats = get_base_stats(mon.species).unwrap();
            stats.type1 == PokemonType::Dragon
                || stats.type2 == PokemonType::Dragon
                || stats.type1 == PokemonType::Flying
                || stats.type2 == PokemonType::Flying
        })
        .count();
    assert!(
        dragon_flying_count >= 3,
        "Lance should have mostly Dragon/Flying types, found {}",
        dragon_flying_count
    );
}

#[test]
fn lorelei_team_composition() {
    let party = get_party(TrainerClass::Lorelei, 0);
    assert_eq!(party.len(), 5);
    assert_eq!(party[0].species, Species::Dewgong);
    assert_eq!(party[0].level, 54);
    assert_eq!(party[1].species, Species::Cloyster);
    assert_eq!(party[1].level, 53);
    assert_eq!(party[2].species, Species::Slowbro);
    assert_eq!(party[2].level, 54);
    assert_eq!(party[3].species, Species::Jynx);
    assert_eq!(party[3].level, 56);
    assert_eq!(party[4].species, Species::Lapras);
    assert_eq!(party[4].level, 56);
}

#[test]
fn bruno_team_composition() {
    let party = get_party(TrainerClass::Bruno, 0);
    assert_eq!(party.len(), 5);
    assert_eq!(party[0].species, Species::Onix);
    assert_eq!(party[0].level, 53);
    assert_eq!(party[1].species, Species::Hitmonchan);
    assert_eq!(party[1].level, 55);
    assert_eq!(party[2].species, Species::Hitmonlee);
    assert_eq!(party[2].level, 55);
    assert_eq!(party[3].species, Species::Onix);
    assert_eq!(party[3].level, 56);
    assert_eq!(party[4].species, Species::Machamp);
    assert_eq!(party[4].level, 58);
}

#[test]
fn agatha_team_composition() {
    let party = get_party(TrainerClass::Agatha, 0);
    assert_eq!(party.len(), 5);
    assert_eq!(party[0].species, Species::Gengar);
    assert_eq!(party[0].level, 56);
    assert_eq!(party[1].species, Species::Golbat);
    assert_eq!(party[1].level, 56);
    assert_eq!(party[2].species, Species::Haunter);
    assert_eq!(party[2].level, 55);
    assert_eq!(party[3].species, Species::Arbok);
    assert_eq!(party[3].level, 58);
    assert_eq!(party[4].species, Species::Gengar);
    assert_eq!(party[4].level, 60);
}

#[test]
fn lance_team_composition() {
    let party = get_party(TrainerClass::Lance, 0);
    assert_eq!(party.len(), 5);
    assert_eq!(party[0].species, Species::Gyarados);
    assert_eq!(party[0].level, 58);
    assert_eq!(party[1].species, Species::Dragonair);
    assert_eq!(party[1].level, 56);
    assert_eq!(party[2].species, Species::Dragonair);
    assert_eq!(party[2].level, 56);
    assert_eq!(party[3].species, Species::Aerodactyl);
    assert_eq!(party[3].level, 60);
    assert_eq!(party[4].species, Species::Dragonite);
    assert_eq!(party[4].level, 62);
}

#[test]
fn champion_variant_0_team() {
    let data = trainer_data();
    let rival3 = data
        .iter()
        .find(|d| d.class == TrainerClass::Rival3)
        .unwrap();
    let party = &rival3.parties[0].pokemon;
    assert_eq!(party.len(), 6);
    assert_eq!(party[0].species, Species::Pidgeot);
    assert_eq!(party[0].level, 61);
    assert_eq!(party[5].species, Species::Blastoise);
    assert_eq!(party[5].level, 65);
}

#[test]
fn all_elite_four_have_five_pokemon() {
    for class in &[
        TrainerClass::Lorelei,
        TrainerClass::Bruno,
        TrainerClass::Agatha,
        TrainerClass::Lance,
    ] {
        let party = get_party(*class, 0);
        assert_eq!(
            party.len(),
            5,
            "{:?} should have 5 Pokémon, has {}",
            class,
            party.len()
        );
    }
}

#[test]
fn champion_has_six_pokemon() {
    let data = trainer_data();
    let rival3 = data
        .iter()
        .find(|d| d.class == TrainerClass::Rival3)
        .unwrap();
    for (i, party) in rival3.parties.iter().enumerate() {
        assert_eq!(
            party.pokemon.len(),
            6,
            "Champion variant {} should have 6 Pokémon",
            i
        );
    }
}

#[test]
fn elite_four_prize_money() {
    let data = trainer_data();

    for class in &[
        TrainerClass::Lorelei,
        TrainerClass::Bruno,
        TrainerClass::Agatha,
        TrainerClass::Lance,
    ] {
        let base = get_base_money(*class);
        assert_eq!(base, 99, "{:?} base money should be 99", class);

        let last_level = data.iter().find(|d| d.class == *class).unwrap().parties[0]
            .pokemon
            .last()
            .unwrap()
            .level;
        let prize = base as u32 * last_level as u32;
        assert!(prize > 0, "{:?} prize should be non-zero", class);
    }
}

#[test]
fn elite_four_levels_above_50() {
    for class in &[
        TrainerClass::Lorelei,
        TrainerClass::Bruno,
        TrainerClass::Agatha,
        TrainerClass::Lance,
    ] {
        let min = min_level_of(*class, 0);
        assert!(
            min >= 50,
            "{:?} min level should be >= 50, got {}",
            class,
            min
        );
    }
}

#[test]
fn champion_levels_above_55() {
    let data = trainer_data();
    let rival3 = data
        .iter()
        .find(|d| d.class == TrainerClass::Rival3)
        .unwrap();
    for (i, party) in rival3.parties.iter().enumerate() {
        let min = party.pokemon.iter().map(|m| m.level).min().unwrap_or(0);
        assert!(
            min >= 55,
            "Champion variant {} min level should be >= 55, got {}",
            i,
            min
        );
    }
}

#[test]
fn champion_has_three_variants() {
    let data = trainer_data();
    let rival3 = data
        .iter()
        .find(|d| d.class == TrainerClass::Rival3)
        .unwrap();
    assert_eq!(
        rival3.parties.len(),
        3,
        "Champion should have 3 party variants (one per starter)"
    );
}

#[test]
fn champion_ace_is_level_65() {
    let data = trainer_data();
    let rival3 = data
        .iter()
        .find(|d| d.class == TrainerClass::Rival3)
        .unwrap();
    for (i, party) in rival3.parties.iter().enumerate() {
        let max = party.pokemon.iter().map(|m| m.level).max().unwrap_or(0);
        assert_eq!(
            max, 65,
            "Champion variant {} ace should be level 65, got {}",
            i, max
        );
    }
}
