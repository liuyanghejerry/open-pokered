mod helpers;

use helpers::*;
use pokered_core::battle::damage::{calculate_damage, DamageParams};
use pokered_core::battle::settlement::settle::settle_battle;
use pokered_core::battle::settlement::BattleOutcome;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::trainer_data::{get_base_money, trainer_data, TrainerClass};
use pokered_data::types::PokemonType;

fn get_gym_party(
    class: TrainerClass,
    party_idx: usize,
) -> Vec<pokered_data::trainer_data::TrainerMon> {
    let data = trainer_data();
    data.iter().find(|d| d.class == class).unwrap().parties[party_idx]
        .pokemon
        .clone()
}

#[test]
fn brock_is_rock_themed() {
    let party = get_gym_party(TrainerClass::Brock, 0);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Rock
                || stats.type2 == PokemonType::Rock
                || stats.type1 == PokemonType::Ground
                || stats.type2 == PokemonType::Ground,
            "Brock's {:?} should be Rock or Ground type",
            mon.species
        );
    }
}

#[test]
fn misty_is_water_themed() {
    let party = get_gym_party(TrainerClass::Misty, 0);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Water || stats.type2 == PokemonType::Water,
            "Misty's {:?} should be Water type",
            mon.species
        );
    }
}

#[test]
fn lt_surge_is_electric_themed() {
    let party = get_gym_party(TrainerClass::LtSurge, 0);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Electric || stats.type2 == PokemonType::Electric,
            "Lt. Surge's {:?} should be Electric type",
            mon.species
        );
    }
}

#[test]
fn erika_is_grass_themed() {
    let party = get_gym_party(TrainerClass::Erika, 0);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Grass || stats.type2 == PokemonType::Grass,
            "Erika's {:?} should be Grass type",
            mon.species
        );
    }
}

#[test]
fn koga_is_poison_themed() {
    let party = get_gym_party(TrainerClass::Koga, 0);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Poison || stats.type2 == PokemonType::Poison,
            "Koga's {:?} should be Poison type",
            mon.species
        );
    }
}

#[test]
fn sabrina_is_psychic_themed() {
    let party = get_gym_party(TrainerClass::Sabrina, 0);
    let psychic_count = party
        .iter()
        .filter(|mon| {
            let stats = get_base_stats(mon.species).unwrap();
            stats.type1 == PokemonType::Psychic || stats.type2 == PokemonType::Psychic
        })
        .count();
    // Sabrina has Kadabra, Mr. Mime, Alakazam (Psychic) + Venomoth (Bug/Poison)
    assert!(
        psychic_count >= 3,
        "Sabrina should have at least 3 Psychic types, found {}",
        psychic_count
    );
}

#[test]
fn blaine_is_fire_themed() {
    let party = get_gym_party(TrainerClass::Blaine, 0);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Fire || stats.type2 == PokemonType::Fire,
            "Blaine's {:?} should be Fire type",
            mon.species
        );
    }
}

#[test]
fn giovanni_gym_is_ground_themed() {
    let party = get_gym_party(TrainerClass::Giovanni, 2);
    for mon in &party {
        let stats = get_base_stats(mon.species).unwrap();
        assert!(
            stats.type1 == PokemonType::Ground
                || stats.type2 == PokemonType::Ground
                || stats.type1 == PokemonType::Rock
                || stats.type2 == PokemonType::Rock
                || stats.type1 == PokemonType::Poison
                || stats.type2 == PokemonType::Poison,
            "Giovanni's {:?} should be Ground/Rock/Poison type",
            mon.species
        );
    }
}

#[test]
fn gym_leader_prize_money_calculations() {
    let data = trainer_data();

    let brock_last_level = data
        .iter()
        .find(|d| d.class == TrainerClass::Brock)
        .unwrap()
        .parties[0]
        .pokemon
        .last()
        .unwrap()
        .level;
    let brock_prize = get_base_money(TrainerClass::Brock) as u32 * brock_last_level as u32;
    assert_eq!(brock_prize, 99 * 14); // Onix L14

    let misty_last_level = data
        .iter()
        .find(|d| d.class == TrainerClass::Misty)
        .unwrap()
        .parties[0]
        .pokemon
        .last()
        .unwrap()
        .level;
    let misty_prize = get_base_money(TrainerClass::Misty) as u32 * misty_last_level as u32;
    assert_eq!(misty_prize, 99 * 21); // Starmie L21
}

#[test]
fn gym_leaders_each_have_exactly_one_party() {
    let data = trainer_data();
    let single_party_gyms = [
        TrainerClass::Brock,
        TrainerClass::Misty,
        TrainerClass::LtSurge,
        TrainerClass::Erika,
        TrainerClass::Koga,
        TrainerClass::Blaine,
        TrainerClass::Sabrina,
        TrainerClass::Lorelei,
        TrainerClass::Bruno,
        TrainerClass::Agatha,
        TrainerClass::Lance,
    ];

    for class in &single_party_gyms {
        let class_data = data.iter().find(|d| d.class == *class).unwrap();
        assert_eq!(
            class_data.parties.len(),
            1,
            "{:?} should have exactly 1 party, has {}",
            class,
            class_data.parties.len()
        );
    }
}

#[test]
fn giovanni_has_three_encounters() {
    let data = trainer_data();
    let giovanni = data
        .iter()
        .find(|d| d.class == TrainerClass::Giovanni)
        .unwrap();
    assert_eq!(
        giovanni.parties.len(),
        3,
        "Giovanni should have 3 parties (Hideout, Silph, Gym)"
    );
}

#[test]
fn lt_surge_team_composition() {
    let party = get_gym_party(TrainerClass::LtSurge, 0);
    assert_eq!(party.len(), 3);
    assert_eq!(party[0].species, Species::Voltorb);
    assert_eq!(party[0].level, 21);
    assert_eq!(party[1].species, Species::Pikachu);
    assert_eq!(party[1].level, 18);
    assert_eq!(party[2].species, Species::Raichu);
    assert_eq!(party[2].level, 24);
}

#[test]
fn erika_team_composition() {
    let party = get_gym_party(TrainerClass::Erika, 0);
    assert_eq!(party.len(), 3);
    assert_eq!(party[0].species, Species::Victreebel);
    assert_eq!(party[0].level, 29);
    assert_eq!(party[1].species, Species::Tangela);
    assert_eq!(party[1].level, 24);
    assert_eq!(party[2].species, Species::Vileplume);
    assert_eq!(party[2].level, 29);
}

#[test]
fn koga_team_composition() {
    let party = get_gym_party(TrainerClass::Koga, 0);
    assert_eq!(party.len(), 4);
    assert_eq!(party[0].species, Species::Koffing);
    assert_eq!(party[0].level, 37);
    assert_eq!(party[1].species, Species::Muk);
    assert_eq!(party[1].level, 39);
    assert_eq!(party[2].species, Species::Koffing);
    assert_eq!(party[2].level, 37);
    assert_eq!(party[3].species, Species::Weezing);
    assert_eq!(party[3].level, 43);
}

#[test]
fn player_pikachu_can_damage_brocks_geodude() {
    let geodude = make_geodude(12, 33);
    let pikachu = make_pikachu(12, 35, 60);

    let params = DamageParams {
        attacker_level: pikachu.level,
        move_power: 35, // Tackle
        move_type: PokemonType::Normal,
        move_id: MoveId::Tackle,
        attack_stat: pikachu.attack,
        defense_stat: geodude.defense,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: pikachu.type1,
        attacker_type2: pikachu.type2,
        defender_type1: geodude.type1,
        defender_type2: geodude.type2,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);
    assert!(
        result.damage > 0,
        "Pikachu Tackle should damage Geodude (Normal isn't immune to Rock)"
    );
}

#[test]
fn electric_immune_to_ground_pokemon() {
    let params = DamageParams {
        attacker_level: 12,
        move_power: 40, // Thundershock
        move_type: PokemonType::Electric,
        move_id: MoveId::Thundershock,
        attack_stat: 55,
        defense_stat: 30,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Electric,
        attacker_type2: PokemonType::Electric,
        defender_type1: PokemonType::Rock,
        defender_type2: PokemonType::Ground,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);
    assert_eq!(
        result.damage, 0,
        "Electric moves should do 0 damage to Ground types"
    );
}

#[test]
fn water_super_effective_vs_brock() {
    let params = DamageParams {
        attacker_level: 12,
        move_power: 40, // Water Gun
        move_type: PokemonType::Water,
        move_id: MoveId::WaterGun,
        attack_stat: 44,
        defense_stat: 100, // Geodude high defense
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Water,
        attacker_type2: PokemonType::Water,
        defender_type1: PokemonType::Rock,
        defender_type2: PokemonType::Ground,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);
    // 4x super effective (Rock + Ground)
    assert!(
        result.damage > 10,
        "Water should deal significant damage to Rock/Ground (4x SE)"
    );
}
