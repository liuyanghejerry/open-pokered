use pokered_data::move_data::MoveData;
use pokered_data::moves::MoveEffect;
use pokered_data::pokemon_data::get_base_stats;

use super::accuracy::accuracy_check;
use super::damage::{calculate_damage, crit_chance, is_high_crit_move, is_physical, DamageParams};
use super::effects::{apply_move_effect, EffectRandoms, EffectResult};
use super::state::{status2, status3, BattleState, Side};
use super::status_checks::{check_status_conditions, CannotMoveReason, StatusCheckResult};
use super::types::TypeMultiplier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveOutcome {
    Success {
        damage: u16,
        effect: EffectResult,
        is_critical: bool,
        type_effectiveness: TypeMultiplier,
    },
    /// Move missed the accuracy check.
    Missed,
    /// Attacker could not move due to status (sleep, freeze, paralysis, etc.)
    CannotMove(CannotMoveReason),
    /// Non-damaging move was used (power=0). Effect result inside.
    NoDamageMove { effect: EffectResult },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveRandoms {
    pub confusion_roll: u8,
    pub paralysis_roll: u8,
    pub crit_roll: u8,
    pub accuracy_roll: u8,
    pub damage_roll: u8,
    pub effect_randoms: EffectRandoms,
}

/// Execute a single move for the current attacker (state.whose_turn).
///
/// ASM pipeline: CheckPlayerStatusConditions → DecrementPP → CriticalHitTest →
///               CalcDamage → CalcHitChance → ApplyDamage → JumpMoveEffect
///
/// Confusion self-hit uses attacker's own Attack vs own Defense, power=40, typeless.
pub fn execute_move(
    state: &mut BattleState,
    move_data: &MoveData,
    randoms: &MoveRandoms,
) -> MoveOutcome {
    let whose_turn = state.whose_turn;

    let status_result = {
        let (attacker, opponent) = split_battlers(state, whose_turn);
        check_status_conditions(
            attacker,
            opponent,
            randoms.confusion_roll,
            randoms.paralysis_roll,
        )
    };

    match status_result {
        StatusCheckResult::CannotMove(reason) => {
            if reason == CannotMoveReason::ConfusedSelfHit {
                let self_damage = calc_confusion_self_hit(state);
                let attacker = state.attacker_mut();
                let mon = attacker.active_mon_mut();
                mon.hp = mon.hp.saturating_sub(self_damage);
                return MoveOutcome::CannotMove(reason);
            }
            return MoveOutcome::CannotMove(reason);
        }
        StatusCheckResult::CanMove => {}
    }

    decrement_pp(state);

    // power=0 moves: accuracy check → effect dispatch (no damage calc)
    if move_data.power == 0 {
        let hit = {
            let attacker = state.attacker();
            let defender = state.defender();
            accuracy_check(
                attacker,
                defender,
                move_data.accuracy,
                move_data.effect,
                randoms.accuracy_roll,
            )
        };

        state.attacker_mut().player_used_move = true;
        state.attacker_mut().last_move_used = move_data.id;

        if !hit {
            state.move_missed = true;
            return MoveOutcome::Missed;
        }

        let effect = apply_move_effect(state, move_data, &randoms.effect_randoms, 0);
        return MoveOutcome::NoDamageMove { effect };
    }

    let is_crit = test_critical_hit(state, move_data, randoms.crit_roll);

    let hit = {
        let attacker = state.attacker();
        let defender = state.defender();
        accuracy_check(
            attacker,
            defender,
            move_data.accuracy,
            move_data.effect,
            randoms.accuracy_roll,
        )
    };

    if !hit {
        state.move_missed = true;
        state.attacker_mut().player_used_move = true;
        state.attacker_mut().last_move_used = move_data.id;
        return MoveOutcome::Missed;
    }

    // Dream Eater fails ENTIRELY against an awake target (core.asm:5240-5245 —
    // MoveHitTest's dreamEaterCheck jumps to .moveMissed before any damage).
    if move_data.effect == MoveEffect::DreamEaterEffect
        && !state.defender().active_mon().status.is_sleep()
    {
        state.move_missed = true;
        state.attacker_mut().player_used_move = true;
        state.attacker_mut().last_move_used = move_data.id;
        return MoveOutcome::Missed;
    }

    let (damage, type_effectiveness) =
        calc_and_apply_damage(state, move_data, is_crit, randoms.damage_roll);
    state.move_missed = false;
    state.attacker_mut().player_used_move = true;
    state.attacker_mut().last_move_used = move_data.id;

    let effect = apply_move_effect(state, move_data, &randoms.effect_randoms, damage);

    MoveOutcome::Success {
        damage,
        effect,
        is_critical: is_crit,
        type_effectiveness,
    }
}

fn split_battlers(
    state: &mut BattleState,
    whose_turn: Side,
) -> (&mut super::state::BattlerState, &super::state::BattlerState) {
    match whose_turn {
        Side::Player => (&mut state.player, &state.enemy),
        Side::Enemy => (&mut state.enemy, &state.player),
    }
}

fn decrement_pp(state: &mut BattleState) {
    let attacker = state.attacker_mut();
    let idx = attacker.selected_move_index as usize;
    if idx < 4 {
        let mon = attacker.active_mon_mut();
        if mon.pp[idx] > 0 {
            mon.pp[idx] -= 1;
        }
    }
}

fn test_critical_hit(state: &mut BattleState, move_data: &MoveData, crit_roll: u8) -> bool {
    let attacker = state.attacker();
    let species = attacker.active_mon().species;
    let base_speed = get_base_stats(species).map_or(0, |s| s.speed);
    let is_high_crit = is_high_crit_move(move_data.id);
    let is_focus_energy = attacker.has_status2(status2::GETTING_PUMPED);
    let threshold = crit_chance(base_speed, is_high_crit, is_focus_energy);
    let is_crit = crit_roll < threshold;
    if is_crit {
        state.critical_or_ohko = 1;
    } else {
        state.critical_or_ohko = 0;
    }
    is_crit
}

/// Confusion self-hit: power=40, typeless, uses attacker's own Attack vs own Defense, no crit.
fn calc_confusion_self_hit(state: &BattleState) -> u16 {
    let attacker = state.attacker();
    let mon = attacker.active_mon();
    let params = DamageParams {
        attacker_level: mon.level,
        move_power: 40,
        move_type: pokered_data::types::PokemonType::Normal,
        move_id: pokered_data::moves::MoveId::None,
        attack_stat: mon.attack,
        defense_stat: mon.defense,
        attack_stage: attacker.stat_stages.attack,
        defense_stage: attacker.stat_stages.defense,
        attacker_type1: pokered_data::types::PokemonType::Normal,
        attacker_type2: pokered_data::types::PokemonType::Normal,
        defender_type1: pokered_data::types::PokemonType::Normal,
        defender_type2: pokered_data::types::PokemonType::Normal,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        // Confusion self-hit is typeless and unaffected by burn.
        attacker_burned: false,
    };
    let result = calculate_damage(&params);
    result.damage
}

fn calc_and_apply_damage(
    state: &mut BattleState,
    move_data: &MoveData,
    is_crit: bool,
    damage_roll: u8,
) -> (u16, TypeMultiplier) {
    let (
        attack_stat,
        defense_stat,
        attack_stage,
        defense_stage,
        has_screen,
        attacker_type1,
        attacker_type2,
        defender_type1,
        defender_type2,
        attacker_level,
    ) = {
        let attacker = state.attacker();
        let defender = state.defender();
        let a_mon = attacker.active_mon();
        let d_mon = defender.active_mon();

        let (atk, def) = if is_physical(move_data.move_type) {
            (a_mon.attack, d_mon.defense)
        } else {
            (a_mon.special, d_mon.special)
        };

        let (atk_stage, def_stage) = if is_physical(move_data.move_type) {
            (attacker.stat_stages.attack, defender.stat_stages.defense)
        } else {
            (attacker.stat_stages.special, defender.stat_stages.special)
        };

        let has_screen = if is_physical(move_data.move_type) {
            defender.has_status3(status3::HAS_REFLECT_UP)
        } else {
            defender.has_status3(status3::HAS_LIGHT_SCREEN_UP)
        };

        (
            atk,
            def,
            atk_stage,
            def_stage,
            has_screen,
            a_mon.type1,
            a_mon.type2,
            d_mon.type1,
            d_mon.type2,
            a_mon.level,
        )
    };

    let is_explode = move_data.effect == MoveEffect::ExplodeEffect;

    let params = DamageParams {
        attacker_level,
        move_power: move_data.power,
        move_type: move_data.move_type,
        move_id: move_data.id,
        attack_stat,
        defense_stat,
        attack_stage,
        defense_stage,
        attacker_type1,
        attacker_type2,
        defender_type1,
        defender_type2,
        is_critical: is_crit,
        random_value: damage_roll,
        has_reflect_or_light_screen: has_screen,
        is_explode_effect: is_explode,
        attacker_burned: state.attacker().active_mon().status
            == super::state::StatusCondition::Burn,
    };

    let result = calculate_damage(&params);
    state.damage = result.damage;

    if result.is_miss {
        state.move_missed = true;
        return (0, result.type_effectiveness);
    }

    let defender = state.defender_mut();
    let has_sub = defender.has_status2(status2::HAS_SUBSTITUTE_UP);
    if has_sub {
        let sub_hp = defender.substitute_hp as u16;
        if result.damage >= sub_hp {
            defender.substitute_hp = 0;
            defender.clear_status2(status2::HAS_SUBSTITUTE_UP);
        } else {
            defender.substitute_hp = (sub_hp - result.damage) as u8;
        }
    } else {
        let d_mon = defender.active_mon_mut();
        d_mon.hp = d_mon.hp.saturating_sub(result.damage);
    }

    (result.damage, result.type_effectiveness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::move_data::MoveData;
    use pokered_data::moves::{MoveEffect, MoveId};
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
            moves: [
                MoveId::Thundershock,
                MoveId::QuickAttack,
                MoveId::None,
                MoveId::None,
            ],
            pp: [30, 30, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: [0x50; 11],
        }
    }

    fn make_state() -> BattleState {
        let p = vec![make_pokemon()];
        let e = vec![make_pokemon()];
        let mut state = new_battle_state(BattleType::Wild, p, e);
        state.player.selected_move = MoveId::Thundershock;
        state.player.selected_move_index = 0;
        state.whose_turn = Side::Player;
        state
    }

    fn thundershock_data() -> MoveData {
        MoveData {
            id: MoveId::Thundershock,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40,
            move_type: PokemonType::Electric,
            accuracy: 255,
            pp: 30,
        }
    }

    fn always_hit_randoms() -> MoveRandoms {
        MoveRandoms {
            confusion_roll: 255,
            paralysis_roll: 255,
            crit_roll: 255,
            accuracy_roll: 0,
            damage_roll: 255,
            effect_randoms: EffectRandoms {
                side_effect_roll: 255,
                duration_roll: 0,
                multi_hit_roll: 0,
            },
        }
    }

    #[test]
    fn successful_attack_deals_damage() {
        let mut state = make_state();
        let move_data = thundershock_data();
        let randoms = always_hit_randoms();

        let result = execute_move(&mut state, &move_data, &randoms);
        match result {
            MoveOutcome::Success { damage, .. } => {
                assert!(damage > 0);
                assert!(state.enemy.active_mon().hp < 200);
            }
            _ => panic!("Expected Success, got {:?}", result),
        }
    }

    #[test]
    fn pp_decremented_after_move() {
        let mut state = make_state();
        let move_data = thundershock_data();
        let randoms = always_hit_randoms();
        assert_eq!(state.player.active_mon().pp[0], 30);

        execute_move(&mut state, &move_data, &randoms);
        assert_eq!(state.player.active_mon().pp[0], 29);
    }

    #[test]
    fn miss_does_no_damage() {
        let mut state = make_state();
        let move_data = MoveData {
            id: MoveId::Thundershock,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40,
            move_type: PokemonType::Electric,
            accuracy: 50,
            pp: 30,
        };
        let randoms = MoveRandoms {
            confusion_roll: 255,
            paralysis_roll: 255,
            crit_roll: 255,
            accuracy_roll: 200,
            damage_roll: 255,
            effect_randoms: EffectRandoms {
                side_effect_roll: 255,
                duration_roll: 0,
                multi_hit_roll: 0,
            },
        };

        let result = execute_move(&mut state, &move_data, &randoms);
        assert_eq!(result, MoveOutcome::Missed);
        assert_eq!(state.enemy.active_mon().hp, 200);
    }

    #[test]
    fn sleep_prevents_move() {
        let mut state = make_state();
        state.player.active_mon_mut().status = StatusCondition::Sleep(3);
        let move_data = thundershock_data();
        let randoms = always_hit_randoms();

        let result = execute_move(&mut state, &move_data, &randoms);
        assert_eq!(result, MoveOutcome::CannotMove(CannotMoveReason::Asleep));
        assert_eq!(state.enemy.active_mon().hp, 200);
        assert_eq!(state.player.active_mon().pp[0], 30);
    }

    #[test]
    fn paralysis_full_para_prevents_move() {
        let mut state = make_state();
        state.player.active_mon_mut().status = StatusCondition::Paralysis;
        let move_data = thundershock_data();
        let randoms = MoveRandoms {
            confusion_roll: 255,
            paralysis_roll: 0,
            crit_roll: 255,
            accuracy_roll: 0,
            damage_roll: 255,
            effect_randoms: EffectRandoms {
                side_effect_roll: 255,
                duration_roll: 0,
                multi_hit_roll: 0,
            },
        };

        let result = execute_move(&mut state, &move_data, &randoms);
        assert_eq!(
            result,
            MoveOutcome::CannotMove(CannotMoveReason::FullyParalyzed)
        );
    }

    #[test]
    fn zero_power_move_returns_no_damage() {
        let mut state = make_state();
        state.player.selected_move = MoveId::ThunderWave;
        let move_data = MoveData {
            id: MoveId::ThunderWave,
            effect: MoveEffect::ParalyzeEffect,
            power: 0,
            move_type: PokemonType::Electric,
            accuracy: 255,
            pp: 20,
        };
        let randoms = always_hit_randoms();

        let result = execute_move(&mut state, &move_data, &randoms);
        assert!(matches!(result, MoveOutcome::NoDamageMove { .. }));
    }

    #[test]
    fn substitute_absorbs_damage() {
        let mut state = make_state();
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = 100;
        let move_data = thundershock_data();
        let randoms = always_hit_randoms();

        let result = execute_move(&mut state, &move_data, &randoms);
        match result {
            MoveOutcome::Success { damage, .. } => {
                assert!(damage > 0);
                assert_eq!(state.enemy.active_mon().hp, 200);
                assert!(state.enemy.substitute_hp < 100);
            }
            _ => panic!("Expected Success"),
        }
    }

    #[test]
    fn substitute_breaks_on_overkill() {
        let mut state = make_state();
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = 1;
        let move_data = thundershock_data();
        let randoms = always_hit_randoms();

        execute_move(&mut state, &move_data, &randoms);
        assert_eq!(state.enemy.substitute_hp, 0);
        assert!(!state.enemy.has_status2(status2::HAS_SUBSTITUTE_UP));
        assert_eq!(state.enemy.active_mon().hp, 200);
    }

    #[test]
    fn last_move_updated_on_success() {
        let mut state = make_state();
        let move_data = thundershock_data();
        let randoms = always_hit_randoms();

        execute_move(&mut state, &move_data, &randoms);
        assert_eq!(state.player.last_move_used, MoveId::Thundershock);
        assert!(state.player.player_used_move);
    }
}
