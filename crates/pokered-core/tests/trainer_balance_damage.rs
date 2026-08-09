mod helpers;

use helpers::*;
use pokered_core::battle::damage::{
    calculate_damage, crit_chance, is_high_crit_move, is_physical, is_special, DamageParams,
};
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn normal_params(
    level: u8,
    power: u8,
    move_type: PokemonType,
    move_id: MoveId,
    attack: u16,
    defense: u16,
    atk_type: PokemonType,
    def_type1: PokemonType,
    def_type2: PokemonType,
) -> DamageParams {
    DamageParams {
        attacker_level: level,
        move_power: power,
        move_type,
        move_id,
        attack_stat: attack,
        defense_stat: defense,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: atk_type,
        attacker_type2: atk_type,
        defender_type1: def_type1,
        defender_type2: def_type2,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    }
}

#[test]
fn stab_adds_50_percent_damage() {
    let no_stab = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Fire,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let with_stab = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );

    let dmg_no_stab = calculate_damage(&no_stab).damage;
    let dmg_stab = calculate_damage(&with_stab).damage;

    assert!(
        dmg_stab > dmg_no_stab,
        "STAB should increase damage: {} vs {}",
        dmg_stab,
        dmg_no_stab
    );
    let ratio = (dmg_stab as f64) / (dmg_no_stab as f64);
    assert!(
        ratio >= 1.3 && ratio <= 1.7,
        "STAB ratio should be ~1.5, got {:.2}",
        ratio
    );
}

#[test]
fn super_effective_doubles_damage() {
    let neutral = normal_params(
        50,
        80,
        PokemonType::Water,
        MoveId::Surf,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let super_eff = normal_params(
        50,
        80,
        PokemonType::Water,
        MoveId::Surf,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Fire,
        PokemonType::Fire,
    );

    let dmg_neutral = calculate_damage(&neutral).damage;
    let dmg_se = calculate_damage(&super_eff).damage;

    assert!(
        dmg_se > dmg_neutral,
        "SE should increase damage: {} vs {}",
        dmg_se,
        dmg_neutral
    );
    let ratio = (dmg_se as f64) / (dmg_neutral as f64);
    assert!(
        ratio >= 1.8 && ratio <= 2.2,
        "SE ratio should be ~2.0, got {:.2}",
        ratio
    );
}

#[test]
fn quad_effective_quadruples_damage() {
    let neutral = normal_params(
        50,
        80,
        PokemonType::Water,
        MoveId::Surf,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let quad = normal_params(
        50,
        80,
        PokemonType::Water,
        MoveId::Surf,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Fire,
        PokemonType::Rock,
    );

    let dmg_neutral = calculate_damage(&neutral).damage;
    let dmg_quad = calculate_damage(&quad).damage;

    let ratio = (dmg_quad as f64) / (dmg_neutral as f64);
    assert!(
        ratio >= 3.5 && ratio <= 4.5,
        "4x SE ratio should be ~4.0, got {:.2}",
        ratio
    );
}

#[test]
fn not_very_effective_halves_damage() {
    let neutral = normal_params(
        50,
        80,
        PokemonType::Fire,
        MoveId::Flamethrower,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let nve = normal_params(
        50,
        80,
        PokemonType::Fire,
        MoveId::Flamethrower,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Water,
        PokemonType::Water,
    );

    let dmg_neutral = calculate_damage(&neutral).damage;
    let dmg_nve = calculate_damage(&nve).damage;

    assert!(
        dmg_nve < dmg_neutral,
        "NVE should decrease damage: {} vs {}",
        dmg_nve,
        dmg_neutral
    );
    let ratio = (dmg_nve as f64) / (dmg_neutral as f64);
    assert!(
        ratio >= 0.4 && ratio <= 0.6,
        "NVE ratio should be ~0.5, got {:.2}",
        ratio
    );
}

#[test]
fn immunity_deals_zero_damage() {
    let params = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Ghost,
        PokemonType::Ghost,
    );
    let result = calculate_damage(&params);
    assert_eq!(result.damage, 0, "Normal vs Ghost should do 0 damage");
}

#[test]
fn ground_immune_to_electric() {
    let params = normal_params(
        50,
        90,
        PokemonType::Electric,
        MoveId::Thunder,
        100,
        100,
        PokemonType::Electric,
        PokemonType::Ground,
        PokemonType::Ground,
    );
    let result = calculate_damage(&params);
    assert_eq!(result.damage, 0, "Electric vs Ground should do 0 damage");
}

#[test]
fn ghost_immune_to_normal() {
    let params = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Ghost,
        PokemonType::Ghost,
    );
    let result = calculate_damage(&params);
    assert_eq!(result.damage, 0);
}

#[test]
fn ghost_immune_to_fighting() {
    let params = normal_params(
        50,
        50,
        PokemonType::Fighting,
        MoveId::KarateChop,
        100,
        100,
        PokemonType::Fighting,
        PokemonType::Ghost,
        PokemonType::Ghost,
    );
    let result = calculate_damage(&params);
    assert_eq!(result.damage, 0);
}

#[test]
fn critical_hit_doubles_damage() {
    let normal = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let mut crit = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    crit.is_critical = true;

    let dmg_normal = calculate_damage(&normal).damage;
    let dmg_crit = calculate_damage(&crit).damage;

    assert!(dmg_crit > dmg_normal, "Crit should increase damage");
    let ratio = (dmg_crit as f64) / (dmg_normal as f64);
    assert!(
        ratio >= 1.8 && ratio <= 2.2,
        "Crit ratio should be ~2.0 for same-level, got {:.2}",
        ratio
    );
}

#[test]
fn higher_level_deals_more_damage() {
    let low = normal_params(
        20,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let high = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );

    let dmg_low = calculate_damage(&low).damage;
    let dmg_high = calculate_damage(&high).damage;
    assert!(
        dmg_high > dmg_low,
        "Higher level should deal more damage: {} vs {}",
        dmg_high,
        dmg_low
    );
}

#[test]
fn higher_attack_deals_more_damage() {
    let low_atk = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        50,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let high_atk = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        150,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );

    let dmg_low = calculate_damage(&low_atk).damage;
    let dmg_high = calculate_damage(&high_atk).damage;
    assert!(
        dmg_high > dmg_low,
        "Higher attack should deal more damage: {} vs {}",
        dmg_high,
        dmg_low
    );
}

#[test]
fn higher_defense_reduces_damage() {
    let low_def = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        50,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let high_def = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        150,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );

    let dmg_low_def = calculate_damage(&low_def).damage;
    let dmg_high_def = calculate_damage(&high_def).damage;
    assert!(
        dmg_low_def > dmg_high_def,
        "Higher defense should reduce damage: {} vs {}",
        dmg_low_def,
        dmg_high_def
    );
}

#[test]
fn random_roll_varies_damage() {
    let mut low_roll = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    low_roll.random_value = 217;
    let mut high_roll = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    high_roll.random_value = 255;

    let dmg_low = calculate_damage(&low_roll).damage;
    let dmg_high = calculate_damage(&high_roll).damage;
    assert!(
        dmg_high >= dmg_low,
        "Higher random roll should deal >= damage"
    );
}

#[test]
fn reflect_halves_physical_damage() {
    let normal = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let mut reflected = normal_params(
        50,
        80,
        PokemonType::Normal,
        MoveId::Strength,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    reflected.has_reflect_or_light_screen = true;

    let dmg_normal = calculate_damage(&normal).damage;
    let dmg_reflected = calculate_damage(&reflected).damage;
    assert!(dmg_reflected < dmg_normal, "Reflect should reduce damage");
}

#[test]
fn explosion_doubles_effective_damage() {
    let normal = normal_params(
        50,
        170,
        PokemonType::Normal,
        MoveId::Explosion,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let mut explode = normal_params(
        50,
        170,
        PokemonType::Normal,
        MoveId::Explosion,
        100,
        100,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    explode.is_explode_effect = true;

    let dmg_normal = calculate_damage(&normal).damage;
    let dmg_explode = calculate_damage(&explode).damage;
    assert!(
        dmg_explode > dmg_normal,
        "Explosion effect should increase damage"
    );
}

#[test]
fn crit_chance_scales_with_speed() {
    let slow = crit_chance(30, false, false);
    let fast = crit_chance(130, false, false);
    assert!(
        fast > slow,
        "Faster Pokémon should have higher crit chance: {} vs {}",
        fast,
        slow
    );
}

#[test]
fn high_crit_move_has_much_higher_crit_chance() {
    let normal_crit = crit_chance(100, false, false);
    let high_crit = crit_chance(100, true, false);
    assert!(
        high_crit > normal_crit * 4,
        "High-crit move should have much higher chance: {} vs {}",
        high_crit,
        normal_crit
    );
}

#[test]
fn focus_energy_bug_reduces_crit() {
    let without = crit_chance(100, false, false);
    let with_focus = crit_chance(100, false, true);
    assert!(
        with_focus < without,
        "Gen 1 Focus Energy bug should REDUCE crit chance"
    );
}

#[test]
fn gen1_physical_special_split_correct() {
    assert!(is_physical(PokemonType::Normal));
    assert!(is_physical(PokemonType::Fighting));
    assert!(is_physical(PokemonType::Flying));
    assert!(is_physical(PokemonType::Ground));
    assert!(is_physical(PokemonType::Rock));
    assert!(is_physical(PokemonType::Bug));
    assert!(is_physical(PokemonType::Ghost));
    assert!(is_physical(PokemonType::Poison));

    assert!(is_special(PokemonType::Fire));
    assert!(is_special(PokemonType::Water));
    assert!(is_special(PokemonType::Grass));
    assert!(is_special(PokemonType::Electric));
    assert!(is_special(PokemonType::Psychic));
    assert!(is_special(PokemonType::Ice));
    assert!(is_special(PokemonType::Dragon));
}

#[test]
fn minimum_damage_is_one() {
    let params = normal_params(
        2,
        10,
        PokemonType::Normal,
        MoveId::Tackle,
        10,
        200,
        PokemonType::Normal,
        PokemonType::Normal,
        PokemonType::Normal,
    );
    let result = calculate_damage(&params);
    assert!(
        result.damage >= 1,
        "Minimum damage should be 1, got {}",
        result.damage
    );
}
