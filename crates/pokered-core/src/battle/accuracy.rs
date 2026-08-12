use pokered_data::moves::MoveEffect;

use super::state::{status2, BattlerState};

/// ASM StatModifierRatios — numerator/denominator pairs indexed by stage+6 (0..12).
/// Stage -6 = index 0 = 25/100, stage 0 = index 6 = 100/100, stage +6 = index 12 = 400/100.
const STAGE_RATIOS: [(u16, u16); 13] = [
    (25, 100),
    (28, 100),
    (33, 100),
    (40, 100),
    (50, 100),
    (66, 100),
    (100, 100),
    (150, 100),
    (200, 100),
    (250, 100),
    (300, 100),
    (350, 100),
    (400, 100),
];

/// Gen 1 CalcHitChance algorithm.
///
/// 1. Convert move base accuracy from percentage (0-100) to 0-255 scale.
///    Original ASM: `percent EQUS "* $ff / 100"` → 100% = 255, 85% = 217, etc.
/// 2. Apply attacker's accuracy stage ratio.
/// 3. Apply defender's evasion stage ratio (inverted: uses `14 - evasion_mod` as index).
/// 4. Clamp to 255.
/// 5. If random_byte >= scaled accuracy → miss.
///
/// Bypassed entirely by Swift (SwiftEffect) and X Accuracy (status2::USING_X_ACCURACY).
pub fn accuracy_check(
    attacker: &BattlerState,
    defender: &BattlerState,
    move_accuracy: u8,
    move_effect: MoveEffect,
    random_byte: u8,
) -> bool {
    if move_effect == MoveEffect::SwiftEffect {
        return true;
    }
    if attacker.has_status2(status2::USING_X_ACCURACY) {
        return true;
    }

    // Convert percentage (0-100) to 0-255 range matching original game's percent macro.
    // Original ASM: percent EQUS "* $ff / 100"
    // 100% → 255, 85% → 217, 55% → 140, etc.
    let base_accuracy = (move_accuracy as u32 * 255 / 100) as u8;
    let mut accuracy = base_accuracy as u32;

    // Apply attacker's accuracy stage
    let acc_index = (attacker.stat_stages.accuracy + 6) as usize;
    let (num, denom) = STAGE_RATIOS[acc_index];
    accuracy = accuracy * num as u32 / denom as u32;

    // Apply defender's evasion stage (inverted index: higher evasion = harder to hit)
    // ASM uses `14 - evasion_mod` where evasion_mod is 1-based (7=neutral)
    // Our stages are -6..+6 (0=neutral), so invert: index = (-defender_evasion + 6)
    let eva_index = ((-defender.stat_stages.evasion) + 6) as usize;
    let (num, denom) = STAGE_RATIOS[eva_index];
    accuracy = accuracy * num as u32 / denom as u32;

    let accuracy = accuracy.min(255) as u8;

    random_byte < accuracy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_battler() -> BattlerState {
        let mon = Pokemon {
            species: Species::Pikachu,
            nickname: [0x50; 11],
            level: 50,
            hp: 100,
            max_hp: 100,
            attack: 80,
            defense: 60,
            speed: 90,
            special: 70,
            type1: PokemonType::Electric,
            type2: PokemonType::Electric,
            moves: [
                MoveId::Thundershock,
                MoveId::None,
                MoveId::None,
                MoveId::None,
            ],
            pp: [30, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        };
        new_battler_state(vec![mon])
    }

    #[test]
    fn perfect_accuracy_always_hits() {
        let attacker = make_battler();
        let defender = make_battler();
        let hit = accuracy_check(
            &attacker,
            &defender,
            100,
            MoveEffect::NoAdditionalEffect,
            254,
        );
        assert!(hit);
    }

    #[test]
    fn zero_accuracy_always_misses() {
        let attacker = make_battler();
        let defender = make_battler();
        let hit = accuracy_check(&attacker, &defender, 0, MoveEffect::NoAdditionalEffect, 0);
        assert!(!hit);
    }

    #[test]
    fn swift_always_hits() {
        let attacker = make_battler();
        let defender = make_battler();
        let hit = accuracy_check(&attacker, &defender, 0, MoveEffect::SwiftEffect, 255);
        assert!(hit);
    }

    #[test]
    fn x_accuracy_always_hits() {
        let mut attacker = make_battler();
        attacker.set_status2(status2::USING_X_ACCURACY);
        let defender = make_battler();
        let hit = accuracy_check(
            &attacker,
            &defender,
            50,
            MoveEffect::NoAdditionalEffect,
            200,
        );
        assert!(hit);
    }

    #[test]
    fn accuracy_stage_boost_helps() {
        let mut attacker = make_battler();
        attacker.stat_stages.accuracy = 2;
        let defender = make_battler();
        let hit = accuracy_check(
            &attacker,
            &defender,
            100,
            MoveEffect::NoAdditionalEffect,
            254,
        );
        assert!(hit);
    }

    #[test]
    fn evasion_stage_makes_miss() {
        let attacker = make_battler();
        let mut defender = make_battler();
        defender.stat_stages.evasion = 2;
        let hit = accuracy_check(
            &attacker,
            &defender,
            100,
            MoveEffect::NoAdditionalEffect,
            130,
        );
        assert!(!hit);
    }

    #[test]
    fn accuracy_clamped_to_255() {
        let mut attacker = make_battler();
        attacker.stat_stages.accuracy = 6;
        let defender = make_battler();
        // base 255, accuracy stage +6 → 255 * 400/100 = 1020, clamped to 255
        let hit = accuracy_check(
            &attacker,
            &defender,
            255,
            MoveEffect::NoAdditionalEffect,
            254,
        );
        assert!(hit);
    }

    #[test]
    fn neutral_stages_no_modification() {
        let attacker = make_battler();
        let defender = make_battler();
        // 100% accuracy → 255. Only random=255 causes miss (255/256 hit rate)
        assert!(accuracy_check(
            &attacker,
            &defender,
            100,
            MoveEffect::NoAdditionalEffect,
            254
        ));
        assert!(!accuracy_check(
            &attacker,
            &defender,
            100,
            MoveEffect::NoAdditionalEffect,
            255
        ));
    }
}
