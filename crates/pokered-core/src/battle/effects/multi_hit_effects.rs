use crate::battle::state::{status1, BattleState};
use pokered_data::types::PokemonType;

use super::{EffectRandoms, EffectResult};

/// Gen 1 two-to-five hit distribution (3/8 each for 2/3, 1/8 each for 4/5):
/// roll 0..96  => 2 hits
/// roll 96..192 => 3 hits
/// roll 192..224 => 4 hits
/// roll 224..256 => 5 hits
/// (Single-byte equivalent of the asm's two-draw `BattleRandom & 3, re-roll if
/// ≥2` in TwoToFiveAttacksEffect/TrappingEffect; also used for trapping-move
/// durations, which roll the SAME distribution — effects.asm:1094-1103.)
pub(crate) fn determine_hit_count(roll: u8) -> u8 {
    if roll < 96 {
        2
    } else if roll < 192 {
        3
    } else if roll < 224 {
        4
    } else {
        5
    }
}

pub fn apply_two_to_five(state: &mut BattleState, randoms: &EffectRandoms) -> EffectResult {
    let attacker = state.attacker_mut();
    if attacker.num_attacks_left == 0 {
        let count = determine_hit_count(randoms.multi_hit_roll);
        attacker.num_attacks_left = count;
        attacker.num_hits = 0;
        attacker.set_status1(status1::MULTI_HIT);
    }
    let attacker = state.attacker_mut();
    attacker.num_attacks_left -= 1;
    attacker.num_hits += 1;
    if attacker.num_attacks_left == 0 {
        attacker.clear_status1(status1::MULTI_HIT);
    }
    EffectResult::NoEffect
}

pub fn apply_attack_twice(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker_mut();
    if attacker.num_attacks_left == 0 {
        attacker.num_attacks_left = 2;
        attacker.num_hits = 0;
        attacker.set_status1(status1::MULTI_HIT);
    }
    attacker.num_attacks_left -= 1;
    attacker.num_hits += 1;
    if attacker.num_attacks_left == 0 {
        attacker.clear_status1(status1::MULTI_HIT);
    }
    EffectResult::NoEffect
}

pub fn apply_twineedle(state: &mut BattleState, randoms: &EffectRandoms) -> EffectResult {
    // ASM: Twineedle sets effect to POISON_SIDE_EFFECT1 for second hit
    // This means poison is attempted on the final hit with 20% chance
    let attacker = state.attacker_mut();
    if attacker.num_attacks_left == 0 {
        attacker.num_attacks_left = 2;
        attacker.num_hits = 0;
        attacker.set_status1(status1::MULTI_HIT);
    }
    attacker.num_attacks_left -= 1;
    attacker.num_hits += 1;

    // On second hit (num_hits == 2), attempt poison with 20% chance
    // POISON_SIDE_EFFECT1 chance = 20% + 1 = 52/256
    if attacker.num_hits == 2 {
        attacker.clear_status1(status1::MULTI_HIT);
        // Check poison chance: 20% = 51/255 (ASM uses "20 percent + 1")
        if randoms.side_effect_roll < 52 {
            // Check poison immunity (Poison-type can't be poisoned)
            let defender = state.defender();
            let d_mon = defender.active_mon();
            if d_mon.type1 == PokemonType::Poison || d_mon.type2 == PokemonType::Poison {
                return EffectResult::StatusFailed;
            }
            return super::status_effects::apply_poison_primary(
                state,
                &pokered_data::move_data::MoveData::get(pokered_data::moves::MoveId::Twineedle)
                    .unwrap(),
            );
        }
    }

    if attacker.num_attacks_left == 0 {
        attacker.clear_status1(status1::MULTI_HIT);
    }
    EffectResult::NoEffect
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_pokemon() -> Pokemon {
        Pokemon {
            species: Species::Pikachu,
            nickname: [0x50; 11],
            level: 50,
            hp: 200,
            max_hp: 200,
            attack: 100,
            defense: 80,
            speed: 110,
            special: 80,
            type1: PokemonType::Electric,
            type2: PokemonType::Electric,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [35, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        }
    }

    fn make_state() -> BattleState {
        new_battle_state(BattleType::Wild, vec![make_pokemon()], vec![make_pokemon()])
    }

    #[test]
    fn two_to_five_low_roll_gives_2_hits() {
        let mut state = make_state();
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 50, stat_down_miss_roll: 255,
        };
        apply_two_to_five(&mut state, &randoms);
        assert_eq!(state.player.num_hits, 1);
        assert_eq!(state.player.num_attacks_left, 1);
        assert!(state.player.has_status1(status1::MULTI_HIT));

        apply_two_to_five(&mut state, &randoms);
        assert_eq!(state.player.num_hits, 2);
        assert_eq!(state.player.num_attacks_left, 0);
        assert!(!state.player.has_status1(status1::MULTI_HIT));
    }

    #[test]
    fn two_to_five_high_roll_gives_5_hits() {
        let mut state = make_state();
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 240, stat_down_miss_roll: 255,
        };
        for i in 1..=5 {
            apply_two_to_five(&mut state, &randoms);
            assert_eq!(state.player.num_hits, i);
        }
        assert_eq!(state.player.num_attacks_left, 0);
        assert!(!state.player.has_status1(status1::MULTI_HIT));
    }

    #[test]
    fn attack_twice_always_2_hits() {
        let mut state = make_state();
        apply_attack_twice(&mut state);
        assert_eq!(state.player.num_hits, 1);
        assert_eq!(state.player.num_attacks_left, 1);

        apply_attack_twice(&mut state);
        assert_eq!(state.player.num_hits, 2);
        assert_eq!(state.player.num_attacks_left, 0);
    }

    #[test]
    fn twineedle_is_2_hits() {
        let mut state = make_state();
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 0, stat_down_miss_roll: 255,
        };
        apply_twineedle(&mut state, &randoms);
        assert_eq!(state.player.num_hits, 1);
        apply_twineedle(&mut state, &randoms);
        assert_eq!(state.player.num_hits, 2);
        assert_eq!(state.player.num_attacks_left, 0);
    }

    #[test]
    fn hit_count_distribution() {
        assert_eq!(determine_hit_count(0), 2);
        assert_eq!(determine_hit_count(95), 2);
        assert_eq!(determine_hit_count(96), 3);
        assert_eq!(determine_hit_count(191), 3);
        assert_eq!(determine_hit_count(192), 4);
        assert_eq!(determine_hit_count(223), 4);
        assert_eq!(determine_hit_count(224), 5);
        assert_eq!(determine_hit_count(255), 5);
    }
}
