use pokered_data::move_data::MoveData;
use pokered_data::types::PokemonType;

use crate::battle::state::{status2, BattleState, StatusCondition};

use super::{EffectRandoms, EffectResult, StatusEffectType};

pub fn apply_sleep(state: &mut BattleState, randoms: &EffectRandoms) -> EffectResult {
    let defender = state.defender();
    if !defender.active_mon().status.is_none() {
        return EffectResult::StatusFailed;
    }
    // ASM: SleepEffect does NOT check Substitute - sleep can go through substitute
    // ASM: BattleRandom, and SLP_MASK (0x07), re-roll if 0, result is 1-7
    // SLP_MASK = 0x07, and results in 0-7, but 0 is invalid so minimum is 1
    let turns = (randoms.duration_roll & 0x07).max(1);
    let defender = state.defender_mut();
    defender.active_mon_mut().status = StatusCondition::Sleep(turns);
    EffectResult::StatusInflicted(StatusEffectType::Sleep)
}

pub fn apply_poison_primary(state: &mut BattleState, move_data: &MoveData) -> EffectResult {
    let defender = state.defender();
    let d_mon = defender.active_mon();
    if !d_mon.status.is_none() {
        return EffectResult::StatusFailed;
    }
    if d_mon.type1 == PokemonType::Poison || d_mon.type2 == PokemonType::Poison {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let is_toxic = move_data.id == pokered_data::moves::MoveId::Toxic;
    let defender = state.defender_mut();
    defender.active_mon_mut().status = StatusCondition::Poison;
    if is_toxic {
        defender.set_status3(crate::battle::state::status3::BADLY_POISONED);
        defender.toxic_counter = 0;
        return EffectResult::StatusInflicted(StatusEffectType::BadlyPoisoned);
    }
    EffectResult::StatusInflicted(StatusEffectType::Poison)
}

pub fn apply_poison_side(
    state: &mut BattleState,
    move_data: &MoveData,
    randoms: &EffectRandoms,
    threshold: u8,
) -> EffectResult {
    if randoms.side_effect_roll >= threshold {
        return EffectResult::NoEffect;
    }
    let defender = state.defender();
    let d_mon = defender.active_mon();
    if !d_mon.status.is_none() {
        return EffectResult::StatusFailed;
    }
    if d_mon.type1 == PokemonType::Poison || d_mon.type2 == PokemonType::Poison {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let _ = move_data;
    let defender = state.defender_mut();
    defender.active_mon_mut().status = StatusCondition::Poison;
    EffectResult::StatusInflicted(StatusEffectType::Poison)
}

pub fn apply_burn_side(
    state: &mut BattleState,
    move_data: &MoveData,
    randoms: &EffectRandoms,
    threshold: u8,
) -> EffectResult {
    if randoms.side_effect_roll >= threshold {
        return EffectResult::NoEffect;
    }
    let defender = state.defender();
    let d_mon = defender.active_mon();
    if !d_mon.status.is_none() {
        return EffectResult::StatusFailed;
    }
    if d_mon.type1 == move_data.move_type || d_mon.type2 == move_data.move_type {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let defender = state.defender_mut();
    defender.active_mon_mut().status = StatusCondition::Burn;
    EffectResult::StatusInflicted(StatusEffectType::Burn)
}

pub fn apply_freeze_side(
    state: &mut BattleState,
    move_data: &MoveData,
    randoms: &EffectRandoms,
    threshold: u8,
) -> EffectResult {
    if randoms.side_effect_roll >= threshold {
        return EffectResult::NoEffect;
    }
    let defender = state.defender();
    let d_mon = defender.active_mon();
    if !d_mon.status.is_none() {
        return EffectResult::StatusFailed;
    }
    if d_mon.type1 == move_data.move_type || d_mon.type2 == move_data.move_type {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let defender = state.defender_mut();
    defender.active_mon_mut().status = StatusCondition::Freeze;
    EffectResult::StatusInflicted(StatusEffectType::Freeze)
}

pub fn apply_paralyze_side(
    state: &mut BattleState,
    move_data: &MoveData,
    randoms: &EffectRandoms,
    threshold: u8,
) -> EffectResult {
    if randoms.side_effect_roll >= threshold {
        return EffectResult::NoEffect;
    }
    let defender = state.defender();
    let d_mon = defender.active_mon();
    if !d_mon.status.is_none() {
        return EffectResult::StatusFailed;
    }
    if d_mon.type1 == move_data.move_type || d_mon.type2 == move_data.move_type {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let defender = state.defender_mut();
    defender.active_mon_mut().status = StatusCondition::Paralysis;
    EffectResult::StatusInflicted(StatusEffectType::Paralysis)
}

pub fn apply_paralyze_primary(state: &mut BattleState, move_data: &MoveData) -> EffectResult {
    let defender = state.defender();
    let d_mon = defender.active_mon();
    if !d_mon.status.is_none() {
        return EffectResult::StatusFailed;
    }
    if d_mon.type1 == move_data.move_type || d_mon.type2 == move_data.move_type {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let defender = state.defender_mut();
    defender.active_mon_mut().status = StatusCondition::Paralysis;
    EffectResult::StatusInflicted(StatusEffectType::Paralysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_pokemon_with_type(t1: PokemonType, t2: PokemonType) -> Pokemon {
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
            type1: t1,
            type2: t2,
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

    fn make_state_with_types(
        atk_t1: PokemonType,
        atk_t2: PokemonType,
        def_t1: PokemonType,
        def_t2: PokemonType,
    ) -> BattleState {
        let p = vec![make_pokemon_with_type(atk_t1, atk_t2)];
        let e = vec![make_pokemon_with_type(def_t1, def_t2)];
        new_battle_state(BattleType::Wild, p, e)
    }

    fn default_randoms() -> EffectRandoms {
        EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 3,
            multi_hit_roll: 0,
        }
    }

    fn thunderwave_data() -> MoveData {
        MoveData {
            id: MoveId::ThunderWave,
            effect: MoveEffect::ParalyzeEffect,
            power: 0,
            move_type: PokemonType::Electric,
            accuracy: 255,
            pp: 20,
        }
    }

    #[test]
    fn sleep_inflicts_on_healthy_target() {
        let mut state = make_state_with_types(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        let randoms = default_randoms();
        let result = apply_sleep(&mut state, &randoms);
        assert_eq!(
            result,
            EffectResult::StatusInflicted(StatusEffectType::Sleep)
        );
        assert!(state.enemy.active_mon().status.is_sleep());
    }

    #[test]
    fn sleep_fails_on_already_statused() {
        let mut state = make_state_with_types(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        state.enemy.active_mon_mut().status = StatusCondition::Burn;
        let randoms = default_randoms();
        let result = apply_sleep(&mut state, &randoms);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn poison_primary_inflicts() {
        let mut state = make_state_with_types(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        let move_data = MoveData {
            id: MoveId::Poisonpowder,
            effect: MoveEffect::PoisonEffect,
            power: 0,
            move_type: PokemonType::Poison,
            accuracy: 75,
            pp: 35,
        };
        let result = apply_poison_primary(&mut state, &move_data);
        assert_eq!(
            result,
            EffectResult::StatusInflicted(StatusEffectType::Poison)
        );
    }

    #[test]
    fn poison_fails_on_poison_type() {
        let mut state = make_state_with_types(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Poison,
            PokemonType::Poison,
        );
        let move_data = MoveData {
            id: MoveId::Poisonpowder,
            effect: MoveEffect::PoisonEffect,
            power: 0,
            move_type: PokemonType::Poison,
            accuracy: 75,
            pp: 35,
        };
        let result = apply_poison_primary(&mut state, &move_data);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn toxic_sets_badly_poisoned() {
        let mut state = make_state_with_types(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        let move_data = MoveData {
            id: MoveId::Toxic,
            effect: MoveEffect::PoisonEffect,
            power: 0,
            move_type: PokemonType::Poison,
            accuracy: 85,
            pp: 10,
        };
        let result = apply_poison_primary(&mut state, &move_data);
        assert_eq!(
            result,
            EffectResult::StatusInflicted(StatusEffectType::BadlyPoisoned)
        );
        assert!(state
            .enemy
            .has_status3(crate::battle::state::status3::BADLY_POISONED));
        assert_eq!(state.enemy.toxic_counter, 0);
    }

    #[test]
    fn burn_side_chance_triggers() {
        let mut state = make_state_with_types(
            PokemonType::Fire,
            PokemonType::Fire,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        let move_data = MoveData {
            id: MoveId::Ember,
            effect: MoveEffect::BurnSideEffect1,
            power: 40,
            move_type: PokemonType::Fire,
            accuracy: 100,
            pp: 25,
        };
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_burn_side(&mut state, &move_data, &randoms, 26);
        assert_eq!(
            result,
            EffectResult::StatusInflicted(StatusEffectType::Burn)
        );
    }

    #[test]
    fn burn_side_blocked_by_fire_type() {
        let mut state = make_state_with_types(
            PokemonType::Fire,
            PokemonType::Fire,
            PokemonType::Fire,
            PokemonType::Fire,
        );
        let move_data = MoveData {
            id: MoveId::Ember,
            effect: MoveEffect::BurnSideEffect1,
            power: 40,
            move_type: PokemonType::Fire,
            accuracy: 100,
            pp: 25,
        };
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_burn_side(&mut state, &move_data, &randoms, 26);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn paralyze_primary_blocked_by_electric_type() {
        let mut state = make_state_with_types(
            PokemonType::Electric,
            PokemonType::Electric,
            PokemonType::Electric,
            PokemonType::Electric,
        );
        let result = apply_paralyze_primary(&mut state, &thunderwave_data());
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn side_effect_no_trigger_when_roll_too_high() {
        let mut state = make_state_with_types(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        let move_data = MoveData {
            id: MoveId::Sludge,
            effect: MoveEffect::PoisonSideEffect2,
            power: 65,
            move_type: PokemonType::Poison,
            accuracy: 100,
            pp: 20,
        };
        let randoms = EffectRandoms {
            side_effect_roll: 200,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_poison_side(&mut state, &move_data, &randoms, 102);
        assert_eq!(result, EffectResult::NoEffect);
    }

    #[test]
    fn freeze_side_blocked_by_ice_type() {
        let mut state = make_state_with_types(
            PokemonType::Ice,
            PokemonType::Ice,
            PokemonType::Ice,
            PokemonType::Ice,
        );
        let move_data = MoveData {
            id: MoveId::IceBeam,
            effect: MoveEffect::FreezeSideEffect1,
            power: 95,
            move_type: PokemonType::Ice,
            accuracy: 100,
            pp: 10,
        };
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_freeze_side(&mut state, &move_data, &randoms, 26);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn substitute_blocks_status() {
        let mut state = make_state_with_types(
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
            PokemonType::Normal,
        );
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = 50;
        let randoms = default_randoms();
        let result = apply_sleep(&mut state, &randoms);
        assert_eq!(
            result,
            EffectResult::StatusInflicted(StatusEffectType::Sleep)
        );

        let move_data = MoveData {
            id: MoveId::Poisonpowder,
            effect: MoveEffect::PoisonEffect,
            power: 0,
            move_type: PokemonType::Poison,
            accuracy: 75,
            pp: 35,
        };
        state.enemy.active_mon_mut().status = StatusCondition::None;
        let result = apply_poison_primary(&mut state, &move_data);
        assert_eq!(result, EffectResult::StatusFailed);
    }
}
