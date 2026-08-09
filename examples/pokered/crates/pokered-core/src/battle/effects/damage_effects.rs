use crate::battle::state::{status2, BattleState};
use pokered_data::moves::MoveId;

use super::EffectResult;

pub fn apply_drain(state: &mut BattleState, damage_dealt: u16) -> EffectResult {
    let drain_amount = (damage_dealt / 2).max(1);
    let attacker = state.attacker_mut();
    let a_mon = attacker.active_mon_mut();
    let healed = drain_amount.min(a_mon.max_hp - a_mon.hp);
    a_mon.hp = a_mon.hp.saturating_add(drain_amount).min(a_mon.max_hp);
    EffectResult::HpDrained { drained: healed }
}

pub fn apply_dream_eater(state: &mut BattleState, damage_dealt: u16) -> EffectResult {
    let defender = state.defender();
    if !defender.active_mon().status.is_sleep() {
        return EffectResult::DreamEaterFailed;
    }
    let drain_amount = (damage_dealt / 2).max(1);
    let attacker = state.attacker_mut();
    let a_mon = attacker.active_mon_mut();
    let healed = drain_amount.min(a_mon.max_hp - a_mon.hp);
    a_mon.hp = a_mon.hp.saturating_add(drain_amount).min(a_mon.max_hp);
    EffectResult::DreamEaterHealed { drained: healed }
}

pub fn apply_recoil(state: &mut BattleState, damage_dealt: u16, move_id: MoveId) -> EffectResult {
    // ASM: Struggle deals 50% recoil, other recoil moves deal 25%
    // recoil.asm: srl b / rr c (divide by 2), then check STRUGGLE, if not, another srl b / rr c (divide by 4)
    let recoil = if move_id == MoveId::Struggle {
        (damage_dealt / 2).max(1)
    } else {
        (damage_dealt / 4).max(1)
    };
    let attacker = state.attacker_mut();
    let a_mon = attacker.active_mon_mut();
    a_mon.hp = a_mon.hp.saturating_sub(recoil);
    EffectResult::RecoilDamage { recoil }
}

pub fn apply_explode(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker_mut();
    attacker.active_mon_mut().hp = 0;
    attacker.clear_status2(status2::SEEDED);
    EffectResult::Exploded
}

pub fn apply_jump_kick_crash(state: &mut BattleState) -> EffectResult {
    let attacker = state.attacker_mut();
    let a_mon = attacker.active_mon_mut();
    let crash = 1u16;
    a_mon.hp = a_mon.hp.saturating_sub(crash);
    EffectResult::CrashDamage { damage: crash }
}

pub fn apply_super_fang(state: &mut BattleState) -> EffectResult {
    let defender = state.defender();
    let current_hp = defender.active_mon().hp;
    let damage = (current_hp / 2).max(1);
    let has_sub = defender.has_status2(status2::HAS_SUBSTITUTE_UP);
    if has_sub {
        let defender = state.defender_mut();
        let sub_hp = defender.substitute_hp as u16;
        if damage >= sub_hp {
            defender.substitute_hp = 0;
            defender.clear_status2(status2::HAS_SUBSTITUTE_UP);
        } else {
            defender.substitute_hp = (sub_hp - damage) as u8;
        }
    } else {
        let defender = state.defender_mut();
        let d_mon = defender.active_mon_mut();
        d_mon.hp = d_mon.hp.saturating_sub(damage);
    }
    state.damage = damage;
    EffectResult::SuperFangDamage { damage }
}

/// Seismic Toss / Night Shade: damage = attacker's level
/// Dragon Rage: fixed 40 damage
/// Sonic Boom: fixed 20 damage
/// Psywave: random 0 to 1.5x level (handled by caller providing damage)
pub fn apply_special_damage(
    state: &mut BattleState,
    move_id: pokered_data::moves::MoveId,
) -> EffectResult {
    use pokered_data::moves::MoveId;
    let attacker_level = state.attacker().active_mon().level as u16;
    let damage = match move_id {
        MoveId::SeismicToss | MoveId::NightShade => attacker_level,
        MoveId::DragonRage => 40,
        MoveId::Sonicboom => 20,
        MoveId::Psywave => {
            // Psywave: random * 1.5 * level / 256, but minimum 1
            // This is pre-computed and passed as state.damage by the caller
            state.damage.max(1)
        }
        _ => attacker_level,
    };
    let defender = state.defender();
    let has_sub = defender.has_status2(status2::HAS_SUBSTITUTE_UP);
    if has_sub {
        let defender = state.defender_mut();
        let sub_hp = defender.substitute_hp as u16;
        if damage >= sub_hp {
            defender.substitute_hp = 0;
            defender.clear_status2(status2::HAS_SUBSTITUTE_UP);
        } else {
            defender.substitute_hp = (sub_hp - damage) as u8;
        }
    } else {
        let defender = state.defender_mut();
        let d_mon = defender.active_mon_mut();
        d_mon.hp = d_mon.hp.saturating_sub(damage);
    }
    state.damage = damage;
    EffectResult::SpecialDamageDealt { damage }
}

pub fn apply_ohko(state: &mut BattleState) -> EffectResult {
    let attacker_level = state.attacker().active_mon().level;
    let defender_level = state.defender().active_mon().level;
    if attacker_level < defender_level {
        state.critical_or_ohko = 0xFF;
        return EffectResult::OhkoFailed;
    }
    let defender = state.defender();
    let has_sub = defender.has_status2(status2::HAS_SUBSTITUTE_UP);
    if has_sub {
        let defender = state.defender_mut();
        defender.substitute_hp = 0;
        defender.clear_status2(status2::HAS_SUBSTITUTE_UP);
    } else {
        let hp = state.defender().active_mon().hp;
        state.damage = hp;
        state.defender_mut().active_mon_mut().hp = 0;
    }
    state.critical_or_ohko = 2;
    EffectResult::OhkoSuccess
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
        new_battle_state(BattleType::Wild, vec![make_pokemon()], vec![make_pokemon()])
    }

    #[test]
    fn drain_heals_half_damage() {
        let mut state = make_state();
        state.player.active_mon_mut().hp = 100;
        let result = apply_drain(&mut state, 60);
        assert_eq!(result, EffectResult::HpDrained { drained: 30 });
        assert_eq!(state.player.active_mon().hp, 130);
    }

    #[test]
    fn drain_caps_at_max_hp() {
        let mut state = make_state();
        state.player.active_mon_mut().hp = 190;
        let result = apply_drain(&mut state, 60);
        assert_eq!(result, EffectResult::HpDrained { drained: 10 });
        assert_eq!(state.player.active_mon().hp, 200);
    }

    #[test]
    fn dream_eater_fails_if_not_asleep() {
        let mut state = make_state();
        let result = apply_dream_eater(&mut state, 60);
        assert_eq!(result, EffectResult::DreamEaterFailed);
    }

    #[test]
    fn dream_eater_heals_if_asleep() {
        let mut state = make_state();
        state.enemy.active_mon_mut().status = StatusCondition::Sleep(3);
        state.player.active_mon_mut().hp = 100;
        let result = apply_dream_eater(&mut state, 80);
        assert_eq!(result, EffectResult::DreamEaterHealed { drained: 40 });
        assert_eq!(state.player.active_mon().hp, 140);
    }

    #[test]
    fn recoil_deals_quarter_damage() {
        let mut state = make_state();
        let result = apply_recoil(&mut state, 100, MoveId::TakeDown);
        assert_eq!(result, EffectResult::RecoilDamage { recoil: 25 });
        assert_eq!(state.player.active_mon().hp, 175);
    }

    #[test]
    fn struggle_recoil_deals_half_damage() {
        let mut state = make_state();
        let result = apply_recoil(&mut state, 100, MoveId::Struggle);
        assert_eq!(result, EffectResult::RecoilDamage { recoil: 50 });
        assert_eq!(state.player.active_mon().hp, 150);
    }

    #[test]
    fn explode_sets_hp_to_zero() {
        let mut state = make_state();
        let result = apply_explode(&mut state);
        assert_eq!(result, EffectResult::Exploded);
        assert_eq!(state.player.active_mon().hp, 0);
    }

    #[test]
    fn super_fang_halves_hp() {
        let mut state = make_state();
        let result = apply_super_fang(&mut state);
        assert_eq!(result, EffectResult::SuperFangDamage { damage: 100 });
        assert_eq!(state.enemy.active_mon().hp, 100);
    }

    #[test]
    fn super_fang_minimum_1() {
        let mut state = make_state();
        state.enemy.active_mon_mut().hp = 1;
        let result = apply_super_fang(&mut state);
        assert_eq!(result, EffectResult::SuperFangDamage { damage: 1 });
        assert_eq!(state.enemy.active_mon().hp, 0);
    }

    #[test]
    fn ohko_fails_if_lower_level() {
        let mut state = make_state();
        state.player.active_mon_mut().level = 30;
        state.enemy.active_mon_mut().level = 50;
        let result = apply_ohko(&mut state);
        assert_eq!(result, EffectResult::OhkoFailed);
        assert_eq!(state.critical_or_ohko, 0xFF);
    }

    #[test]
    fn ohko_succeeds_if_higher_level() {
        let mut state = make_state();
        state.player.active_mon_mut().level = 50;
        state.enemy.active_mon_mut().level = 30;
        let result = apply_ohko(&mut state);
        assert_eq!(result, EffectResult::OhkoSuccess);
        assert_eq!(state.enemy.active_mon().hp, 0);
        assert_eq!(state.critical_or_ohko, 2);
    }

    #[test]
    fn special_damage_seismic_toss() {
        let mut state = make_state();
        let result = apply_special_damage(&mut state, MoveId::SeismicToss);
        assert_eq!(result, EffectResult::SpecialDamageDealt { damage: 50 });
        assert_eq!(state.enemy.active_mon().hp, 150);
    }

    #[test]
    fn special_damage_dragon_rage() {
        let mut state = make_state();
        let result = apply_special_damage(&mut state, MoveId::DragonRage);
        assert_eq!(result, EffectResult::SpecialDamageDealt { damage: 40 });
        assert_eq!(state.enemy.active_mon().hp, 160);
    }

    #[test]
    fn special_damage_sonic_boom() {
        let mut state = make_state();
        let result = apply_special_damage(&mut state, MoveId::Sonicboom);
        assert_eq!(result, EffectResult::SpecialDamageDealt { damage: 20 });
        assert_eq!(state.enemy.active_mon().hp, 180);
    }
}
