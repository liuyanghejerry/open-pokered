use pokered_data::moves::MoveId;
use pokered_data::types::PokemonType;

use super::stat_stages::apply_stage;
use super::types::{apply_type_effectiveness, TypeMultiplier};

pub fn is_physical(move_type: PokemonType) -> bool {
    matches!(
        move_type,
        PokemonType::Normal
            | PokemonType::Fighting
            | PokemonType::Flying
            | PokemonType::Ground
            | PokemonType::Rock
            | PokemonType::Bug
            | PokemonType::Ghost
            | PokemonType::Poison
    )
}

pub fn is_special(move_type: PokemonType) -> bool {
    !is_physical(move_type)
}

pub fn is_high_crit_move(move_id: MoveId) -> bool {
    matches!(
        move_id,
        MoveId::KarateChop | MoveId::RazorLeaf | MoveId::Crabhammer | MoveId::Slash
    )
}

/// Gen 1 crit threshold. Focus Energy BUG preserved: divides by 4 instead of multiplying.
pub fn crit_chance(base_speed: u8, is_high_crit: bool, is_focus_energy: bool) -> u8 {
    let threshold = if is_high_crit {
        // Gen-1: high-crit ratio is (base_speed / 2) * 8, not base_speed * 8
        // (both saturate at 255 for base_speed >= 64, so only slow high-crit
        // users like Slash/Razor Leaf/Crabhammer were affected).
        let val = (base_speed as u16 / 2) * 8;
        if is_focus_energy {
            val / 4
        } else {
            val
        }
    } else {
        let val = base_speed as u16 / 2;
        if is_focus_energy {
            val / 4
        } else {
            val
        }
    };
    threshold.min(255) as u8
}

pub struct DamageParams {
    pub attacker_level: u8,
    pub move_power: u8,
    pub move_type: PokemonType,
    pub move_id: MoveId,
    pub attack_stat: u16,
    pub defense_stat: u16,
    pub attack_stage: i8,
    pub defense_stage: i8,
    pub attacker_type1: PokemonType,
    pub attacker_type2: PokemonType,
    pub defender_type1: PokemonType,
    pub defender_type2: PokemonType,
    pub is_critical: bool,
    pub random_value: u8,
    pub has_reflect_or_light_screen: bool,
    pub is_explode_effect: bool,
    /// Attacker is burned — halves a physical move's Attack (non-crit only),
    /// matching the Gen-1 burn penalty.
    pub attacker_burned: bool,
}

pub struct DamageResult {
    pub damage: u16,
    pub type_effectiveness: TypeMultiplier,
    pub is_miss: bool,
}

/// Scale stats when either exceeds 255: both divided by 4, min 1.
/// ASM bug: defense could become 0 causing a freeze — we prevent that here.
fn scale_stats(attack: u16, defense: u16) -> (u16, u16) {
    if attack > 255 || defense > 255 {
        let a = (attack >> 2).max(1);
        let d = (defense >> 2).max(1);
        (a, d)
    } else {
        (attack, defense)
    }
}

pub fn calculate_damage(params: &DamageParams) -> DamageResult {
    if params.move_power == 0 {
        let eff = super::types::get_type_effectiveness(
            params.move_type,
            params.defender_type1,
            params.defender_type2,
        );
        return DamageResult {
            damage: 0,
            type_effectiveness: eff,
            is_miss: false,
        };
    }

    let level = if params.is_critical {
        (params.attacker_level as u32) * 2
    } else {
        params.attacker_level as u32
    };

    let (mut attack, mut defense) = if params.is_critical {
        (params.attack_stat as u32, params.defense_stat as u32)
    } else {
        (
            apply_stage(params.attack_stat, params.attack_stage) as u32,
            apply_stage(params.defense_stat, params.defense_stage) as u32,
        )
    };

    // Burn halves a physical attacker's Attack (crits ignore the penalty, just
    // as they ignore stat stages above).
    if !params.is_critical && params.attacker_burned && is_physical(params.move_type) {
        attack = (attack / 2).max(1);
    }

    if params.has_reflect_or_light_screen {
        defense = (defense as u16).wrapping_mul(2) as u32;
    }

    if params.is_explode_effect {
        defense = (defense / 2).max(1);
    }

    let (attack_scaled, defense_scaled) = scale_stats(attack as u16, defense as u16);
    attack = attack_scaled as u32;
    defense = defense_scaled as u32;

    let defense = defense.max(1);

    let base = (2u32 * level / 5 + 2)
        .wrapping_mul(params.move_power as u32)
        .wrapping_mul(attack)
        / defense
        / 50;

    let base = base.min(997) + 2;

    let type_result = apply_type_effectiveness(
        base as u16,
        params.move_type,
        params.attacker_type1,
        params.attacker_type2,
        params.defender_type1,
        params.defender_type2,
    );

    if type_result.damage == 0 {
        return DamageResult {
            damage: 0,
            type_effectiveness: type_result.multiplier,
            is_miss: type_result.caused_miss,
        };
    }

    // Gen-1 random spread: only applied when base damage > 1, using a roll in
    // [217, 255] (the caller draws that constrained byte). A base of exactly 1
    // is dealt as-is.
    let final_damage = if type_result.damage > 1 {
        (type_result.damage as u32) * (params.random_value as u32) / 255
    } else {
        type_result.damage as u32
    };

    DamageResult {
        damage: final_damage.max(1) as u16,
        type_effectiveness: type_result.multiplier,
        is_miss: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params() -> DamageParams {
        DamageParams {
            attacker_level: 50,
            move_power: 80,
            move_type: PokemonType::Normal,
            move_id: MoveId::Strength,
            attack_stat: 100,
            defense_stat: 100,
            attack_stage: 0,
            defense_stage: 0,
            attacker_type1: PokemonType::Normal,
            attacker_type2: PokemonType::Normal,
            defender_type1: PokemonType::Normal,
            defender_type2: PokemonType::Normal,
            is_critical: false,
            random_value: 255,
            has_reflect_or_light_screen: false,
            is_explode_effect: false,
            attacker_burned: false,
        }
    }

    #[test]
    fn basic_damage_nonzero() {
        let result = calculate_damage(&make_params());
        assert!(result.damage > 0);
        assert!(!result.is_miss);
    }

    #[test]
    fn burn_halves_physical_attack_noncrit() {
        // make_params uses Strength (Normal = physical), non-crit, roll 255.
        let mut p = make_params();
        p.is_critical = false;
        p.random_value = 255;
        let normal = calculate_damage(&p).damage;
        p.attacker_burned = true;
        let burned = calculate_damage(&p).damage;
        assert!(burned < normal, "burn must reduce physical damage");
        assert!(
            (burned as f32) <= (normal as f32) * 0.65,
            "burned {burned} should be roughly half of {normal}"
        );
    }

    #[test]
    fn burn_ignored_on_crit_and_special() {
        let mut p = make_params();
        p.random_value = 255;
        // Crits ignore the burn penalty.
        p.is_critical = true;
        let crit = calculate_damage(&p).damage;
        p.attacker_burned = true;
        assert_eq!(crit, calculate_damage(&p).damage);
        // Special moves are unaffected by burn.
        p.is_critical = false;
        p.attacker_burned = false;
        p.move_type = PokemonType::Fire;
        let special = calculate_damage(&p).damage;
        p.attacker_burned = true;
        assert_eq!(special, calculate_damage(&p).damage);
    }

    #[test]
    fn damage_roll_floor_keeps_variance_tight() {
        // With base damage > 1, a max roll (255) beats a min Gen-1 roll (217)
        // but only by ~15% — the 217/255 floor, not a full 0..255 spread.
        let mut p = make_params();
        p.random_value = 255;
        let hi = calculate_damage(&p).damage as f32;
        p.random_value = 217;
        let lo = calculate_damage(&p).damage as f32;
        assert!(lo > hi * 0.8, "min roll {lo} should be within ~85% of max {hi}");
    }

    #[test]
    fn zero_power_zero_damage() {
        let mut p = make_params();
        p.move_power = 0;
        let result = calculate_damage(&p);
        assert_eq!(result.damage, 0);
    }

    #[test]
    fn immunity_zero_damage() {
        let mut p = make_params();
        p.move_type = PokemonType::Normal;
        p.attacker_type1 = PokemonType::Fire;
        p.attacker_type2 = PokemonType::Fire;
        p.defender_type1 = PokemonType::Ghost;
        p.defender_type2 = PokemonType::Ghost;
        let result = calculate_damage(&p);
        assert_eq!(result.damage, 0);
        assert!(result.is_miss);
    }

    #[test]
    fn stab_increases_damage() {
        let mut no_stab = make_params();
        no_stab.attacker_type1 = PokemonType::Fire;
        no_stab.attacker_type2 = PokemonType::Fire;
        let r1 = calculate_damage(&no_stab);

        let r2 = calculate_damage(&make_params());

        assert!(r2.damage > r1.damage, "STAB should increase damage");
    }

    #[test]
    fn super_effective_increases_damage() {
        let mut p = make_params();
        p.move_type = PokemonType::Water;
        p.attacker_type1 = PokemonType::Water;
        p.defender_type1 = PokemonType::Fire;
        p.defender_type2 = PokemonType::Fire;
        let se = calculate_damage(&p);

        let mut p2 = make_params();
        p2.move_type = PokemonType::Water;
        p2.attacker_type1 = PokemonType::Water;
        p2.defender_type1 = PokemonType::Normal;
        p2.defender_type2 = PokemonType::Normal;
        let neutral = calculate_damage(&p2);

        assert!(se.damage > neutral.damage);
    }

    #[test]
    fn critical_hit_increases_damage() {
        let normal = calculate_damage(&make_params());
        let mut p = make_params();
        p.is_critical = true;
        let crit = calculate_damage(&p);
        assert!(crit.damage > normal.damage);
    }

    #[test]
    fn random_value_affects_damage() {
        let mut low = make_params();
        low.random_value = 217;
        let mut high = make_params();
        high.random_value = 255;
        let r_low = calculate_damage(&low);
        let r_high = calculate_damage(&high);
        assert!(r_high.damage >= r_low.damage);
    }

    #[test]
    fn physical_special_split() {
        assert!(is_physical(PokemonType::Normal));
        assert!(is_physical(PokemonType::Fighting));
        assert!(is_physical(PokemonType::Rock));
        assert!(is_physical(PokemonType::Ground));
        assert!(is_physical(PokemonType::Ghost));
        assert!(is_physical(PokemonType::Poison));
        assert!(is_physical(PokemonType::Bug));
        assert!(is_physical(PokemonType::Flying));

        assert!(is_special(PokemonType::Fire));
        assert!(is_special(PokemonType::Water));
        assert!(is_special(PokemonType::Grass));
        assert!(is_special(PokemonType::Electric));
        assert!(is_special(PokemonType::Psychic));
        assert!(is_special(PokemonType::Ice));
        assert!(is_special(PokemonType::Dragon));
    }

    #[test]
    fn high_crit_moves() {
        assert!(is_high_crit_move(MoveId::KarateChop));
        assert!(is_high_crit_move(MoveId::RazorLeaf));
        assert!(is_high_crit_move(MoveId::Crabhammer));
        assert!(is_high_crit_move(MoveId::Slash));
        assert!(!is_high_crit_move(MoveId::Tackle));
        assert!(!is_high_crit_move(MoveId::Thunder));
    }

    #[test]
    fn crit_chance_normal_move() {
        let chance = crit_chance(100, false, false);
        assert_eq!(chance, 50);
    }

    #[test]
    fn crit_chance_high_crit_move() {
        let chance = crit_chance(100, true, false);
        assert_eq!(chance, 255);
    }

    #[test]
    fn crit_chance_with_focus_energy_bug() {
        let normal = crit_chance(100, false, false);
        let focus = crit_chance(100, false, true);
        assert!(
            focus < normal,
            "Gen 1 Focus Energy bug: should reduce crit rate"
        );
    }

    #[test]
    fn stat_scaling_both_high() {
        let (a, d) = scale_stats(512, 256);
        assert_eq!(a, 128);
        assert_eq!(d, 64);
    }

    #[test]
    fn stat_scaling_not_needed() {
        let (a, d) = scale_stats(200, 150);
        assert_eq!(a, 200);
        assert_eq!(d, 150);
    }

    #[test]
    fn stat_scaling_prevents_zero_attack() {
        let (a, _d) = scale_stats(3, 300);
        assert!(a >= 1, "scaled attack should be at least 1");
    }

    #[test]
    fn reflect_doubles_defense() {
        let normal = calculate_damage(&make_params());
        let mut p = make_params();
        p.has_reflect_or_light_screen = true;
        let reflected = calculate_damage(&p);
        assert!(reflected.damage < normal.damage);
    }

    #[test]
    fn explode_halves_defense() {
        let normal = calculate_damage(&make_params());
        let mut p = make_params();
        p.is_explode_effect = true;
        let exploded = calculate_damage(&p);
        assert!(exploded.damage > normal.damage);
    }

    #[test]
    fn miss_on_zero_damage_after_type_effectiveness() {
        let mut p = make_params();
        p.attacker_level = 2;
        p.move_power = 10;
        p.attack_stat = 10;
        p.defense_stat = 200;
        p.move_type = PokemonType::Fire;
        p.attacker_type1 = PokemonType::Normal;
        p.attacker_type2 = PokemonType::Normal;
        p.defender_type1 = PokemonType::Water;
        p.defender_type2 = PokemonType::Rock;
        let result = calculate_damage(&p);
        if result.damage == 0 {
            assert!(result.is_miss);
        }
    }
}
