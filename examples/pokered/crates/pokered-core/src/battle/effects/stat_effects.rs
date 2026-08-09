use crate::battle::stat_stages::StatIndex;
use crate::battle::state::{status2, BattleState};

use super::{EffectRandoms, EffectResult};

fn stat_index_from_u8(idx: u8) -> StatIndex {
    match idx {
        0 => StatIndex::Attack,
        1 => StatIndex::Defense,
        2 => StatIndex::Speed,
        3 => StatIndex::Special,
        4 => StatIndex::Accuracy,
        _ => StatIndex::Evasion,
    }
}

pub fn apply_stat_up(state: &mut BattleState, stat_idx: u8, stages: i8) -> EffectResult {
    let stat = stat_index_from_u8(stat_idx);
    let attacker = state.attacker_mut();
    let changed = attacker.stat_stages.modify(stat, stages);
    if changed {
        EffectResult::StatModified {
            stat: stat_idx,
            stages,
        }
    } else {
        EffectResult::StatBlocked
    }
}

pub fn apply_stat_down(state: &mut BattleState, stat_idx: u8, stages: i8) -> EffectResult {
    let stat = stat_index_from_u8(stat_idx);
    let defender = state.defender();
    if defender.has_status2(status2::PROTECTED_BY_MIST) {
        return EffectResult::StatBlocked;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatBlocked;
    }
    let defender = state.defender_mut();
    let changed = defender.stat_stages.modify(stat, -stages);
    if changed {
        EffectResult::StatModified {
            stat: stat_idx,
            stages: -stages,
        }
    } else {
        EffectResult::StatBlocked
    }
}

const STAT_DOWN_SIDE_THRESHOLD: u8 = 85; // ~33% of 256

pub fn apply_stat_down_side(
    state: &mut BattleState,
    stat_idx: u8,
    randoms: &EffectRandoms,
) -> EffectResult {
    if randoms.side_effect_roll >= STAT_DOWN_SIDE_THRESHOLD {
        return EffectResult::NoEffect;
    }
    apply_stat_down(state, stat_idx, 1)
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
            nickname: None,
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
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    fn make_state() -> BattleState {
        let p = vec![make_pokemon()];
        let e = vec![make_pokemon()];
        new_battle_state(BattleType::Wild, p, e)
    }

    fn default_randoms() -> EffectRandoms {
        EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 0,
        }
    }

    #[test]
    fn stat_up_increases_stage() {
        let mut state = make_state();
        let result = apply_stat_up(&mut state, 0, 1);
        assert_eq!(result, EffectResult::StatModified { stat: 0, stages: 1 });
        assert_eq!(state.player.stat_stages.attack, 1);
    }

    #[test]
    fn stat_up_2_increases_by_two() {
        let mut state = make_state();
        let result = apply_stat_up(&mut state, 0, 2);
        assert_eq!(result, EffectResult::StatModified { stat: 0, stages: 2 });
        assert_eq!(state.player.stat_stages.attack, 2);
    }

    #[test]
    fn stat_up_blocked_at_max() {
        let mut state = make_state();
        state.player.stat_stages.attack = 6;
        let result = apply_stat_up(&mut state, 0, 1);
        assert_eq!(result, EffectResult::StatBlocked);
    }

    #[test]
    fn stat_down_decreases_opponent() {
        let mut state = make_state();
        let result = apply_stat_down(&mut state, 1, 1);
        assert_eq!(
            result,
            EffectResult::StatModified {
                stat: 1,
                stages: -1
            }
        );
        assert_eq!(state.enemy.stat_stages.defense, -1);
    }

    #[test]
    fn stat_down_blocked_by_mist() {
        let mut state = make_state();
        state.enemy.set_status2(status2::PROTECTED_BY_MIST);
        let result = apply_stat_down(&mut state, 0, 1);
        assert_eq!(result, EffectResult::StatBlocked);
    }

    #[test]
    fn stat_down_blocked_by_substitute() {
        let mut state = make_state();
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = 50;
        let result = apply_stat_down(&mut state, 0, 1);
        assert_eq!(result, EffectResult::StatBlocked);
    }

    #[test]
    fn stat_down_side_triggers_at_low_roll() {
        let mut state = make_state();
        let randoms = default_randoms();
        let result = apply_stat_down_side(&mut state, 0, &randoms);
        assert_eq!(
            result,
            EffectResult::StatModified {
                stat: 0,
                stages: -1
            }
        );
    }

    #[test]
    fn stat_down_side_no_trigger_at_high_roll() {
        let mut state = make_state();
        let randoms = EffectRandoms {
            side_effect_roll: 200,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_stat_down_side(&mut state, 0, &randoms);
        assert_eq!(result, EffectResult::NoEffect);
    }

    #[test]
    fn stat_down_at_min_is_blocked() {
        let mut state = make_state();
        state.enemy.stat_stages.attack = -6;
        let result = apply_stat_down(&mut state, 0, 1);
        assert_eq!(result, EffectResult::StatBlocked);
    }
}
