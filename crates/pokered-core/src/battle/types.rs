use pokered_data::type_chart::TYPE_CHART;
use pokered_data::types::{Effectiveness, PokemonType};

/// Combined type multiplier result for display messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeMultiplier {
    Zero,
    Quarter,
    Half,
    Normal,
    Double,
    Quadruple,
}

impl TypeMultiplier {
    pub fn is_super_effective(self) -> bool {
        matches!(self, TypeMultiplier::Double | TypeMultiplier::Quadruple)
    }

    pub fn is_not_very_effective(self) -> bool {
        matches!(self, TypeMultiplier::Half | TypeMultiplier::Quarter)
    }

    pub fn is_no_effect(self) -> bool {
        matches!(self, TypeMultiplier::Zero)
    }
}

/// Result of applying type effectiveness to damage, matching ASM AdjustDamageForMoveType.
pub struct TypeEffectivenessResult {
    /// The final damage after applying type effectiveness sequentially.
    pub damage: u16,
    /// Combined multiplier category for display ("super effective", etc.)
    pub multiplier: TypeMultiplier,
    /// Whether damage became 0 after type effectiveness (ASM sets wMoveMissed=1).
    pub caused_miss: bool,
}

/// Returns the raw multiplier value (out of 10) for a single attacker-type vs defender-type pair.
/// 0 = immune, 5 = not very effective, 10 = neutral, 20 = super effective.
fn single_type_multiplier(attack_type: PokemonType, defend_type: PokemonType) -> u32 {
    for matchup in TYPE_CHART.iter() {
        if matchup.attacker == attack_type && matchup.defender == defend_type {
            return match matchup.effectiveness {
                Effectiveness::SuperEffective => 20,
                Effectiveness::NotVeryEffective => 5,
                Effectiveness::NoEffect => 0,
                Effectiveness::Normal => 10,
            };
        }
    }
    10
}

/// Compute the combined TypeMultiplier category from two raw multiplier values.
/// Used for determining which message to display ("super effective", etc.)
fn combined_multiplier_category(mult1: u32, mult2: u32) -> TypeMultiplier {
    let combined = mult1 * mult2;
    match combined {
        0 => TypeMultiplier::Zero,
        25 => TypeMultiplier::Quarter,
        50 => TypeMultiplier::Half,
        100 => TypeMultiplier::Normal,
        200 => TypeMultiplier::Double,
        400 => TypeMultiplier::Quadruple,
        _ => TypeMultiplier::Normal,
    }
}

/// Get the combined type effectiveness category for display purposes.
/// Does NOT apply damage — use `apply_type_effectiveness` for that.
pub fn get_type_effectiveness(
    attack_type: PokemonType,
    defend_type1: PokemonType,
    defend_type2: PokemonType,
) -> TypeMultiplier {
    let eff1 = single_type_multiplier(attack_type, defend_type1);
    let eff2 = if defend_type1 == defend_type2 {
        10
    } else {
        single_type_multiplier(attack_type, defend_type2)
    };
    combined_multiplier_category(eff1, eff2)
}

/// Apply type effectiveness to damage sequentially, matching ASM AdjustDamageForMoveType.
///
/// For each defender type, looks up the type chart. For each matching entry:
///   damage = floor(damage * multiplier / 10)
///
/// This is applied once per defender type (skipping type2 if same as type1).
/// If damage becomes 0 after application, `caused_miss` is set (ASM: wMoveMissed = 1).
///
/// Also applies STAB: if move_type matches either attacker type, damage = damage + floor(damage / 2).
pub fn apply_type_effectiveness(
    damage: u16,
    move_type: PokemonType,
    attacker_type1: PokemonType,
    attacker_type2: PokemonType,
    defender_type1: PokemonType,
    defender_type2: PokemonType,
) -> TypeEffectivenessResult {
    let mut d = damage as u32;
    let mut stab = false;

    if move_type == attacker_type1 || move_type == attacker_type2 {
        let half = d >> 1;
        d += half;
        stab = true;
    }

    let mut caused_miss = false;

    let mult1 = single_type_multiplier(move_type, defender_type1);
    d = d * mult1 / 10;
    if d == 0 && mult1 != 10 && damage > 0 {
        caused_miss = true;
    }

    let mult2 = if defender_type1 == defender_type2 {
        10
    } else {
        let m = single_type_multiplier(move_type, defender_type2);
        d = d * m / 10;
        if d == 0 && m != 10 && damage > 0 {
            caused_miss = true;
        }
        m
    };

    let _ = stab;
    let multiplier = combined_multiplier_category(mult1, mult2);

    TypeEffectivenessResult {
        damage: d as u16,
        multiplier,
        caused_miss,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poison_vs_grass_poison() {
        // Poison→Grass = 2x, Poison→Poison = 0.5x, combined = 1x
        let eff =
            get_type_effectiveness(PokemonType::Poison, PokemonType::Grass, PokemonType::Poison);
        assert_eq!(eff, TypeMultiplier::Normal);
    }

    #[test]
    fn water_vs_fire_rock() {
        let eff = get_type_effectiveness(PokemonType::Water, PokemonType::Fire, PokemonType::Rock);
        assert_eq!(eff, TypeMultiplier::Quadruple);
    }

    #[test]
    fn normal_vs_ghost() {
        let eff =
            get_type_effectiveness(PokemonType::Normal, PokemonType::Ghost, PokemonType::Ghost);
        assert_eq!(eff, TypeMultiplier::Zero);
    }

    #[test]
    fn electric_vs_ground_flying() {
        let eff = get_type_effectiveness(
            PokemonType::Electric,
            PokemonType::Ground,
            PokemonType::Flying,
        );
        assert_eq!(eff, TypeMultiplier::Zero);
    }

    #[test]
    fn fire_vs_grass_poison() {
        let eff =
            get_type_effectiveness(PokemonType::Fire, PokemonType::Grass, PokemonType::Poison);
        assert_eq!(eff, TypeMultiplier::Double);
    }

    #[test]
    fn normal_vs_normal() {
        let eff = get_type_effectiveness(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        assert_eq!(eff, TypeMultiplier::Normal);
    }

    #[test]
    fn grass_vs_water_ground() {
        let eff =
            get_type_effectiveness(PokemonType::Grass, PokemonType::Water, PokemonType::Ground);
        assert_eq!(eff, TypeMultiplier::Quadruple);
    }

    // Tests for sequential application via apply_type_effectiveness

    #[test]
    fn sequential_type_application_neutral() {
        // Normal move vs Normal/Normal, no STAB attacker
        let result = apply_type_effectiveness(
            100,
            PokemonType::Normal,
            PokemonType::Fire,
            PokemonType::Fire,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        // No STAB (attacker is Fire, move is Normal), neutral vs Normal
        // damage = 100 * 10 / 10 = 100
        assert_eq!(result.damage, 100);
        assert_eq!(result.multiplier, TypeMultiplier::Normal);
        assert!(!result.caused_miss);
    }

    #[test]
    fn sequential_type_application_super_effective() {
        // Water vs Fire/Rock, no STAB
        let result = apply_type_effectiveness(
            100,
            PokemonType::Water,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Fire,
            PokemonType::Rock,
        );
        // No STAB, Water vs Fire = 20/10, then vs Rock = 20/10
        // 100 * 20 / 10 = 200, 200 * 20 / 10 = 400
        assert_eq!(result.damage, 400);
        assert_eq!(result.multiplier, TypeMultiplier::Quadruple);
    }

    #[test]
    fn sequential_type_application_with_stab() {
        // Water move, Water attacker, vs Fire/Fire
        let result = apply_type_effectiveness(
            100,
            PokemonType::Water,
            PokemonType::Water,
            PokemonType::Water,
            PokemonType::Fire,
            PokemonType::Fire,
        );
        // STAB: 100 + 50 = 150, then Water vs Fire = 150 * 20 / 10 = 300
        // type2 same as type1, skipped
        assert_eq!(result.damage, 300);
        assert_eq!(result.multiplier, TypeMultiplier::Double);
    }

    #[test]
    fn sequential_type_application_causes_miss() {
        // Very low damage with 0.25x effectiveness should cause miss
        let result = apply_type_effectiveness(
            2,
            PokemonType::Fire,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Water,
            PokemonType::Rock,
        );
        // No STAB, Fire vs Water = 5/10 → 2*5/10 = 1, Fire vs Rock = 5/10 → 1*5/10 = 0
        assert_eq!(result.damage, 0);
        assert!(result.caused_miss);
        assert_eq!(result.multiplier, TypeMultiplier::Quarter);
    }

    #[test]
    fn sequential_type_immunity_causes_miss() {
        // Normal vs Ghost — immunity
        let result = apply_type_effectiveness(
            100,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Ghost,
            PokemonType::Ghost,
        );
        // Normal vs Ghost = 0/10 → 100*0/10 = 0
        assert_eq!(result.damage, 0);
        assert!(result.caused_miss);
        assert_eq!(result.multiplier, TypeMultiplier::Zero);
    }

    #[test]
    fn sequential_stab_floor_division() {
        // Test that STAB uses floor division: 99 → 99 + floor(99/2) = 99 + 49 = 148
        let result = apply_type_effectiveness(
            99,
            PokemonType::Fire,
            PokemonType::Fire,
            PokemonType::Fire,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        // STAB: 99 + 49 = 148, neutral = 148
        assert_eq!(result.damage, 148);
    }
}
