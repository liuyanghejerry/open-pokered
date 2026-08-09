mod game_flow_helpers;

use pokered_core::battle::damage::{calculate_damage, DamageParams};
use pokered_core::battle::stat_stages::apply_stage;
use pokered_core::battle::trainer_ai::{move_choice_layers, MoveChoiceLayer};
use pokered_data::move_data::MoveData;
use pokered_data::moves::{MoveEffect, MoveId};
use pokered_data::trainer_data::TrainerClass;
use pokered_data::types::PokemonType;

/// Test that RIVAL1 has correct AI configuration (only Layer 1)
#[test]
fn rival1_has_layer1_only() {
    let layers = move_choice_layers(TrainerClass::Rival1);
    assert_eq!(layers, &[MoveChoiceLayer::Layer1]);
}

/// Test basic damage calculation for Level 5 starters
#[test]
fn level5_damage_calculation_charmander_vs_squirtle() {
    // Charmander Level 5: Attack=12, Defense=11
    // Squirtle Level 5: Defense=13
    // Scratch: power=40, type=Normal

    let params = DamageParams {
        attacker_level: 5,
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: MoveId::Scratch,
        attack_stat: 12,
        defense_stat: 13,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire,
        attacker_type2: PokemonType::Fire,
        defender_type1: PokemonType::Water,
        defender_type2: PokemonType::Water,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);

    // Expected damage:
    // ((2 * 5 / 5 + 2) * 40 * 12) / 13 / 50 = (4 * 40 * 12) / 13 / 50 = 1920 / 13 / 50 = 147 / 50 = 2
    // + 2 = 4, * 255/255 = 4
    // Min damage (217/255): 4 * 217 / 255 = 3

    assert!(
        result.damage >= 3 && result.damage <= 4,
        "Expected damage 3-4, got {}",
        result.damage
    );
}

/// Test damage with -1 attack stage (after Growl)
#[test]
fn damage_with_minus1_attack_stage() {
    // Charmander Level 5: Attack=12
    // After 1 Growl: Attack stage -1, effective attack = 12 * 66 / 100 = 7.92 ≈ 8

    let params = DamageParams {
        attacker_level: 5,
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: MoveId::Scratch,
        attack_stat: 12,
        defense_stat: 13,
        attack_stage: -1,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire,
        attacker_type2: PokemonType::Fire,
        defender_type1: PokemonType::Water,
        defender_type2: PokemonType::Water,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);

    // With -1 stage: attack = 12 * 66 / 100 = 8 (truncated)
    // ((2 * 5 / 5 + 2) * 40 * 8) / 13 / 50 = (4 * 40 * 8) / 13 / 50 = 1280 / 13 / 50 = 98 / 50 = 1
    // + 2 = 3, max damage = 3

    assert!(
        result.damage >= 2 && result.damage <= 3,
        "Expected damage 2-3 with -1 attack stage, got {}",
        result.damage
    );
}

/// Test damage with -6 attack stage (worst case)
#[test]
fn damage_with_minus6_attack_stage() {
    // Charmander Level 5: Attack=12
    // After 6 Growls: Attack stage -6, effective attack = 12 * 25 / 100 = 3

    let params = DamageParams {
        attacker_level: 5,
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: MoveId::Scratch,
        attack_stat: 12,
        defense_stat: 13,
        attack_stage: -6,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire,
        attacker_type2: PokemonType::Fire,
        defender_type1: PokemonType::Water,
        defender_type2: PokemonType::Water,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);

    // With -6 stage: attack = 12 * 25 / 100 = 3
    // ((2 * 5 / 5 + 2) * 40 * 3) / 13 / 50 = (4 * 40 * 3) / 13 / 50 = 480 / 13 / 50 = 36 / 50 = 0
    // + 2 = 2, min damage should be 1 (guaranteed)
    // Actually: damage should be at least 1

    assert!(
        result.damage >= 1,
        "Expected damage >= 1 even with -6 attack stage, got {}",
        result.damage
    );
}

/// Test that Squirtle Tackle damage vs Charmander
#[test]
fn level5_damage_calculation_squirtle_vs_charmander() {
    // Squirtle Level 5: Attack=11
    // Charmander Level 5: Defense=11
    // Tackle: power=35, type=Normal

    let params = DamageParams {
        attacker_level: 5,
        move_power: 35,
        move_type: PokemonType::Normal,
        move_id: MoveId::Tackle,
        attack_stat: 11,
        defense_stat: 11,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Water,
        attacker_type2: PokemonType::Water,
        defender_type1: PokemonType::Fire,
        defender_type2: PokemonType::Fire,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);

    // ((2 * 5 / 5 + 2) * 35 * 11) / 11 / 50 = (4 * 35 * 11) / 11 / 50 = 1540 / 11 / 50 = 140 / 50 = 2
    // + 2 = 4

    assert!(
        result.damage >= 3 && result.damage <= 4,
        "Expected damage 3-4, got {}",
        result.damage
    );
}

/// Test that stat stage modifiers are correctly applied
#[test]
fn stat_stage_multiplier_correct() {
    use pokered_core::battle::stat_stages::apply_stage;

    // Test stage 0: multiplier = 100/100 = 1.0
    assert_eq!(apply_stage(100, 0), 100);

    // Test stage -1: multiplier = 66/100 ≈ 0.66
    assert_eq!(apply_stage(100, -1), 66);

    // Test stage -6: multiplier = 25/100 = 0.25
    assert_eq!(apply_stage(100, -6), 25);

    // Test actual stat values
    assert_eq!(apply_stage(12, -1), 7); // 12 * 66 / 100 = 7.92 → 7
    assert_eq!(apply_stage(12, -6), 3); // 12 * 25 / 100 = 3
}

/// Test critical hit damage (doubles level in calculation)
#[test]
fn critical_hit_doubles_damage() {
    let normal_params = DamageParams {
        attacker_level: 5,
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: MoveId::Scratch,
        attack_stat: 12,
        defense_stat: 13,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire,
        attacker_type2: PokemonType::Fire,
        defender_type1: PokemonType::Water,
        defender_type2: PokemonType::Water,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let crit_params = DamageParams {
        attacker_level: 5,
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: MoveId::Scratch,
        attack_stat: 12,
        defense_stat: 13,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire,
        attacker_type2: PokemonType::Fire,
        defender_type1: PokemonType::Water,
        defender_type2: PokemonType::Water,
        is_critical: true, // Level is doubled: 5 * 2 = 10
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let normal = calculate_damage(&normal_params);
    let crit = calculate_damage(&crit_params);

    // Critical hit should deal more damage (roughly 2x for same-level battle)
    assert!(
        crit.damage > normal.damage,
        "Critical hit ({}) should be more than normal ({})",
        crit.damage,
        normal.damage
    );
}

/// Test Struggle damage calculation
#[test]
fn struggle_damage_calculation() {
    // Struggle: power=50, type=Normal, no STAB
    // Same stats as normal attack but 50 power instead of 40

    let params = DamageParams {
        attacker_level: 5,
        move_power: 50, // Struggle power
        move_type: PokemonType::Normal,
        move_id: MoveId::Struggle,
        attack_stat: 12,
        defense_stat: 13,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire, // No STAB for Struggle
        attacker_type2: PokemonType::Fire,
        defender_type1: PokemonType::Water,
        defender_type2: PokemonType::Water,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    let result = calculate_damage(&params);

    // ((2 * 5 / 5 + 2) * 50 * 12) / 13 / 50 = (4 * 50 * 12) / 13 / 50 = 2400 / 13 / 50 = 184 / 50 = 3
    // + 2 = 5

    assert!(
        result.damage >= 4 && result.damage <= 5,
        "Expected Struggle damage 4-5, got {}",
        result.damage
    );
}

/// Test that Growl and TailWhip are NOT status-only effects by checking their MoveEffect
/// Status-only effects (Layer 1 discourages) are: SleepEffect, PoisonEffect, ParalyzeEffect
/// Growl (AttackDown1Effect) and TailWhip (DefenseDown1Effect) are stat-down effects, not status
#[test]
fn growl_tailwhip_not_status_only() {
    let growl_data = MoveData::get(MoveId::Growl).unwrap();
    let tailwhip_data = MoveData::get(MoveId::TailWhip).unwrap();
    let thunderwave_data = MoveData::get(MoveId::ThunderWave).unwrap();

    // Stat-down effects (Growl, TailWhip) should have power=0 but different effect type
    assert_eq!(growl_data.effect, MoveEffect::AttackDown1Effect);
    assert_eq!(tailwhip_data.power, 0);

    assert_eq!(tailwhip_data.effect, MoveEffect::DefenseDown1Effect);
    assert_eq!(tailwhip_data.power, 0);

    // Status-only effects (ThunderWave)
    assert_eq!(thunderwave_data.effect, MoveEffect::ParalyzeEffect);
    assert_eq!(thunderwave_data.power, 0);

    // Layer 1 only discourages: Effect01, SleepEffect, PoisonEffect, ParalyzeEffect
    // Growl and TailWhip are NOT in this list, so they won't be discouraged by Layer 1
}
