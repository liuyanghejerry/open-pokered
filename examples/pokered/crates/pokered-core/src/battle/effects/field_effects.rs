use pokered_data::move_data::MoveData;
use pokered_data::types::PokemonType;

use crate::battle::state::{status1, status2, status3, BattleState, StatusCondition};

use super::EffectResult;

pub fn apply_mist(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker();
    if attacker.has_status2(status2::PROTECTED_BY_MIST) {
        return EffectResult::FieldEffectAlreadyActive;
    }
    let attacker = state.attacker_mut();
    attacker.set_status2(status2::PROTECTED_BY_MIST);
    EffectResult::FieldEffectSet
}

pub fn apply_focus_energy(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker();
    if attacker.has_status2(status2::GETTING_PUMPED) {
        return EffectResult::FieldEffectAlreadyActive;
    }
    let attacker = state.attacker_mut();
    attacker.set_status2(status2::GETTING_PUMPED);
    EffectResult::FieldEffectSet
}

pub fn apply_light_screen(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker();
    if attacker.has_status3(status3::HAS_LIGHT_SCREEN_UP) {
        return EffectResult::FieldEffectAlreadyActive;
    }
    let attacker = state.attacker_mut();
    attacker.set_status3(status3::HAS_LIGHT_SCREEN_UP);
    EffectResult::FieldEffectSet
}

pub fn apply_reflect(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker();
    if attacker.has_status3(status3::HAS_REFLECT_UP) {
        return EffectResult::FieldEffectAlreadyActive;
    }
    let attacker = state.attacker_mut();
    attacker.set_status3(status3::HAS_REFLECT_UP);
    EffectResult::FieldEffectSet
}

pub fn apply_leech_seed(state: &mut BattleState, move_data: &MoveData) -> EffectResult {
    let defender = state.defender();
    if defender.has_status2(status2::SEEDED) {
        return EffectResult::FieldEffectAlreadyActive;
    }
    let d_mon = defender.active_mon();
    if d_mon.type1 == PokemonType::Grass || d_mon.type2 == PokemonType::Grass {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let _ = move_data;
    let defender = state.defender_mut();
    defender.set_status2(status2::SEEDED);
    EffectResult::Seeded
}

pub fn apply_haze(state: &mut BattleState) -> EffectResult {
    state.player.stat_stages.reset();
    state.enemy.stat_stages.reset();
    state.player.battle_status1 &= !(status1::CONFUSED);
    state.enemy.battle_status1 &= !(status1::CONFUSED);
    state.player.confused_turns_left = 0;
    state.enemy.confused_turns_left = 0;
    state.player.clear_status2(status2::SEEDED);
    state.enemy.clear_status2(status2::SEEDED);
    state.player.clear_status3(status3::BADLY_POISONED);
    state.enemy.clear_status3(status3::BADLY_POISONED);
    state.player.toxic_counter = 0;
    state.enemy.toxic_counter = 0;
    state.player.clear_status2(status2::GETTING_PUMPED);
    state.enemy.clear_status2(status2::GETTING_PUMPED);
    state.player.disabled_move = 0;
    state.player.disabled_turns_left = 0;
    state.enemy.disabled_move = 0;
    state.enemy.disabled_turns_left = 0;
    // Haze also cures all non-volatile status in Gen 1
    state.player.active_mon_mut().status = StatusCondition::None;
    state.enemy.active_mon_mut().status = StatusCondition::None;
    EffectResult::HazeReset
}

pub fn apply_substitute(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker();
    if attacker.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::SubstituteFailed;
    }
    let a_mon = attacker.active_mon();
    let cost = a_mon.max_hp / 4;
    // ASM BUG: only fails if HP - cost would underflow (HP < cost)
    // This means HP == cost leaves user with 0 HP (Gen 1 bug preserved)
    if cost == 0 || a_mon.hp < cost {
        return EffectResult::SubstituteFailed;
    }
    let attacker = state.attacker_mut();
    attacker.active_mon_mut().hp -= cost;
    attacker.substitute_hp = cost as u8;
    attacker.set_status2(status2::HAS_SUBSTITUTE_UP);
    EffectResult::SubstituteCreated { hp_cost: cost }
}

pub fn apply_conversion(state: &mut BattleState, move_data: &MoveData) -> EffectResult {
    let defender = state.defender();
    let d_type1 = defender.active_mon().type1;
    let d_type2 = defender.active_mon().type2;
    let _ = move_data;
    let attacker = state.attacker_mut();
    attacker.active_mon_mut().type1 = d_type1;
    attacker.active_mon_mut().type2 = d_type2;
    EffectResult::TypesChanged
}

pub fn apply_heal(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker();
    let a_mon = attacker.active_mon();
    if a_mon.hp == a_mon.max_hp {
        return EffectResult::StatBlocked;
    }
    let heal_amount = a_mon.max_hp / 2;
    let actual_heal = heal_amount.min(a_mon.max_hp - a_mon.hp);
    let is_rest = attacker.selected_move == pokered_data::moves::MoveId::Rest;
    let max_hp = a_mon.max_hp;
    let attacker = state.attacker_mut();
    let a_mon = attacker.active_mon_mut();
    a_mon.hp = (a_mon.hp + heal_amount).min(a_mon.max_hp);
    if is_rest {
        a_mon.hp = max_hp;
        a_mon.status = StatusCondition::Sleep(2);
        attacker.clear_status3(status3::BADLY_POISONED);
        attacker.toxic_counter = 0;
    }
    EffectResult::Healed {
        amount: actual_heal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;

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
        new_battle_state(BattleType::Wild, vec![make_pokemon()], vec![make_pokemon()])
    }

    fn recover_data() -> MoveData {
        MoveData {
            id: MoveId::Recover,
            effect: pokered_data::moves::MoveEffect::HealEffect,
            power: 0,
            move_type: PokemonType::Normal,
            accuracy: 255,
            pp: 20,
        }
    }

    #[test]
    fn mist_sets_protection() {
        let mut state = make_state();
        let result = apply_mist(&mut state);
        assert_eq!(result, EffectResult::FieldEffectSet);
        assert!(state.player.has_status2(status2::PROTECTED_BY_MIST));
    }

    #[test]
    fn mist_already_active() {
        let mut state = make_state();
        state.player.set_status2(status2::PROTECTED_BY_MIST);
        let result = apply_mist(&mut state);
        assert_eq!(result, EffectResult::FieldEffectAlreadyActive);
    }

    #[test]
    fn focus_energy_sets_flag() {
        let mut state = make_state();
        let result = apply_focus_energy(&mut state);
        assert_eq!(result, EffectResult::FieldEffectSet);
        assert!(state.player.has_status2(status2::GETTING_PUMPED));
    }

    #[test]
    fn light_screen_sets_flag() {
        let mut state = make_state();
        let result = apply_light_screen(&mut state);
        assert_eq!(result, EffectResult::FieldEffectSet);
        assert!(state.player.has_status3(status3::HAS_LIGHT_SCREEN_UP));
    }

    #[test]
    fn reflect_sets_flag() {
        let mut state = make_state();
        let result = apply_reflect(&mut state);
        assert_eq!(result, EffectResult::FieldEffectSet);
        assert!(state.player.has_status3(status3::HAS_REFLECT_UP));
    }

    #[test]
    fn leech_seed_on_normal_type() {
        let mut state = make_state();
        let move_data = MoveData {
            id: MoveId::LeechSeed,
            effect: pokered_data::moves::MoveEffect::LeechSeedEffect,
            power: 0,
            move_type: PokemonType::Grass,
            accuracy: 90,
            pp: 10,
        };
        let result = apply_leech_seed(&mut state, &move_data);
        assert_eq!(result, EffectResult::Seeded);
        assert!(state.enemy.has_status2(status2::SEEDED));
    }

    #[test]
    fn leech_seed_fails_on_grass_type() {
        let mut state = make_state();
        state.enemy.active_mon_mut().type1 = PokemonType::Grass;
        let move_data = MoveData {
            id: MoveId::LeechSeed,
            effect: pokered_data::moves::MoveEffect::LeechSeedEffect,
            power: 0,
            move_type: PokemonType::Grass,
            accuracy: 90,
            pp: 10,
        };
        let result = apply_leech_seed(&mut state, &move_data);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn haze_resets_everything() {
        let mut state = make_state();
        state.player.stat_stages.attack = 3;
        state.enemy.stat_stages.defense = -2;
        state.player.set_status1(status1::CONFUSED);
        state.player.confused_turns_left = 3;
        state.enemy.set_status2(status2::SEEDED);
        state.player.active_mon_mut().status = StatusCondition::Burn;
        state.enemy.active_mon_mut().status = StatusCondition::Poison;

        let result = apply_haze(&mut state);
        assert_eq!(result, EffectResult::HazeReset);
        assert_eq!(state.player.stat_stages.attack, 0);
        assert_eq!(state.enemy.stat_stages.defense, 0);
        assert!(!state.player.has_status1(status1::CONFUSED));
        assert!(!state.enemy.has_status2(status2::SEEDED));
        assert!(state.player.active_mon().status.is_none());
        assert!(state.enemy.active_mon().status.is_none());
    }

    #[test]
    fn substitute_costs_quarter_hp() {
        let mut state = make_state();
        let result = apply_substitute(&mut state);
        assert_eq!(result, EffectResult::SubstituteCreated { hp_cost: 50 });
        assert_eq!(state.player.active_mon().hp, 150);
        assert_eq!(state.player.substitute_hp, 50);
        assert!(state.player.has_status2(status2::HAS_SUBSTITUTE_UP));
    }

    #[test]
    fn substitute_fails_if_not_enough_hp() {
        let mut state = make_state();
        // Gen 1 BUG: HP == cost is allowed (leaves user with 0 HP)
        // Only fails if HP < cost
        state.player.active_mon_mut().hp = 49; // Less than cost (50)
        let result = apply_substitute(&mut state);
        assert_eq!(result, EffectResult::SubstituteFailed);
    }

    #[test]
    fn substitute_bug_allows_zero_hp() {
        // Gen 1 BUG: HP == cost leaves user with 0 HP but succeeds
        let mut state = make_state();
        state.player.active_mon_mut().hp = 50; // Equal to cost
        let result = apply_substitute(&mut state);
        assert_eq!(result, EffectResult::SubstituteCreated { hp_cost: 50 });
        assert_eq!(state.player.active_mon().hp, 0); // BUG: user has 0 HP
    }

    #[test]
    fn substitute_fails_if_already_up() {
        let mut state = make_state();
        state.player.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.player.substitute_hp = 50;
        let result = apply_substitute(&mut state);
        assert_eq!(result, EffectResult::SubstituteFailed);
    }

    #[test]
    fn conversion_copies_opponent_types() {
        let mut state = make_state();
        state.enemy.active_mon_mut().type1 = PokemonType::Water;
        state.enemy.active_mon_mut().type2 = PokemonType::Ice;
        let move_data = MoveData {
            id: MoveId::Conversion,
            effect: pokered_data::moves::MoveEffect::ConversionEffect,
            power: 0,
            move_type: PokemonType::Normal,
            accuracy: 255,
            pp: 30,
        };
        let result = apply_conversion(&mut state, &move_data);
        assert_eq!(result, EffectResult::TypesChanged);
        assert_eq!(state.player.active_mon().type1, PokemonType::Water);
        assert_eq!(state.player.active_mon().type2, PokemonType::Ice);
    }

    #[test]
    fn heal_restores_half_max_hp() {
        let mut state = make_state();
        state.player.active_mon_mut().hp = 50;
        state.player.selected_move = MoveId::Recover;
        let result = apply_heal(&mut state);
        match result {
            EffectResult::Healed { amount } => assert!(amount > 0),
            _ => panic!("Expected Healed"),
        }
        assert!(state.player.active_mon().hp > 50);
    }

    #[test]
    fn heal_fails_at_full_hp() {
        let mut state = make_state();
        let result = apply_heal(&mut state);
        assert_eq!(result, EffectResult::StatBlocked);
    }

    #[test]
    fn rest_fully_heals_and_sleeps() {
        let mut state = make_state();
        state.player.active_mon_mut().hp = 10;
        state.player.selected_move = MoveId::Rest;
        let result = apply_heal(&mut state);
        match result {
            EffectResult::Healed { .. } => {}
            _ => panic!("Expected Healed"),
        }
        assert_eq!(state.player.active_mon().hp, 200);
        assert_eq!(state.player.active_mon().status, StatusCondition::Sleep(2));
    }
}
