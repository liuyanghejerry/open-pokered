use pokered_data::moves::MoveId;

use crate::battle::state::{status1, status2, status3, BattleState};

use super::{EffectRandoms, EffectResult};

pub fn apply_flinch_side(
    state: &mut BattleState,
    randoms: &EffectRandoms,
    threshold: u8,
) -> EffectResult {
    if randoms.side_effect_roll >= threshold {
        return EffectResult::NoEffect;
    }
    let defender = state.defender();
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let defender = state.defender_mut();
    defender.set_status1(status1::FLINCHED);
    EffectResult::FlinchApplied
}

pub fn apply_confusion_primary(state: &mut BattleState, randoms: &EffectRandoms) -> EffectResult {
    let defender = state.defender();
    if defender.has_status1(status1::CONFUSED) {
        return EffectResult::StatusFailed;
    }
    if defender.has_status2(status2::HAS_SUBSTITUTE_UP) {
        return EffectResult::StatusFailed;
    }
    let turns = (randoms.duration_roll & 0x03) + 2;
    let defender = state.defender_mut();
    defender.set_status1(status1::CONFUSED);
    defender.confused_turns_left = turns;
    EffectResult::ConfusionApplied
}

pub fn apply_confusion_side(
    state: &mut BattleState,
    randoms: &EffectRandoms,
    threshold: u8,
) -> EffectResult {
    if randoms.side_effect_roll >= threshold {
        return EffectResult::NoEffect;
    }
    apply_confusion_primary(state, randoms)
}

pub fn apply_transform(state: &mut BattleState) -> EffectResult {
    let defender = state.defender();
    let d_mon = defender.active_mon().clone();
    let d_stages = defender.stat_stages;
    let attacker = state.attacker_mut();
    let a_mon = attacker.active_mon_mut();
    a_mon.species = d_mon.species;
    a_mon.type1 = d_mon.type1;
    a_mon.type2 = d_mon.type2;
    a_mon.attack = d_mon.attack;
    a_mon.defense = d_mon.defense;
    a_mon.speed = d_mon.speed;
    a_mon.special = d_mon.special;
    a_mon.moves = d_mon.moves;
    for i in 0..4 {
        a_mon.pp[i] = 5;
    }
    attacker.stat_stages = d_stages;
    attacker.set_status3(status3::TRANSFORMED);
    EffectResult::Transformed
}

pub fn apply_mimic(state: &mut BattleState) -> EffectResult {
    let last_move = state.defender().last_move_used;
    if last_move == MoveId::None {
        return EffectResult::StatusFailed;
    }
    let attacker = state.attacker_mut();
    let mimic_slot = attacker.selected_move_index as usize;
    if mimic_slot < 4 {
        attacker.active_mon_mut().moves[mimic_slot] = last_move;
        attacker.active_mon_mut().pp[mimic_slot] = 5;
    }
    EffectResult::MoveCopied
}

pub fn apply_metronome(randoms: &EffectRandoms) -> EffectResult {
    // Metronome picks a random move (1-165, excluding Metronome and Struggle)
    let raw = (randoms.duration_roll as u16 % 163) + 1;
    let move_val = if raw >= 0x76 { raw + 1 } else { raw }; // skip Metronome (0x76)
    let picked = pokered_data::moves::move_id_from_u8(move_val as u8);
    EffectResult::MetronomeMove {
        picked_move: picked,
    }
}

pub fn apply_mirror_move(state: &mut BattleState) -> EffectResult {
    let defender = state.defender();
    let last_move = defender.last_move_used;
    if last_move == MoveId::None {
        return EffectResult::StatusFailed;
    }
    EffectResult::MirrorMove {
        mirrored_move: last_move,
    }
}

pub fn apply_disable(state: &mut BattleState, randoms: &EffectRandoms) -> EffectResult {
    let defender = state.defender();
    if defender.disabled_move != 0 {
        return EffectResult::StatusFailed;
    }
    let d_mon = defender.active_mon();
    let last_move = defender.last_move_used;
    if last_move == MoveId::None {
        return EffectResult::StatusFailed;
    }
    let mut target_slot = None;
    for i in 0..4 {
        if d_mon.moves[i] == last_move && d_mon.pp[i] > 0 {
            target_slot = Some(i);
            break;
        }
    }
    match target_slot {
        Some(slot) => {
            let turns = (randoms.duration_roll & 0x07) + 1;
            let turns = turns.max(1);
            let defender = state.defender_mut();
            defender.disabled_move = (slot + 1) as u8;
            defender.disabled_turns_left = turns;
            EffectResult::Disabled
        }
        None => EffectResult::StatusFailed,
    }
}

pub fn apply_pay_day(state: &mut BattleState, damage_dealt: u16) -> EffectResult {
    let coins = (state.attacker().active_mon().level as u16) * 2;
    state.total_payday_money += coins as u32;
    let _ = damage_dealt;
    EffectResult::PayDay { coins }
}

pub fn apply_switch_teleport(state: &mut BattleState) -> EffectResult {
    if state.battle_type == crate::battle::state::BattleType::Trainer {
        return EffectResult::StatusFailed;
    }
    state.escaped = true;
    EffectResult::SwitchedOut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
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
        new_battle_state(BattleType::Wild, vec![make_pokemon()], vec![make_pokemon()])
    }

    fn default_randoms() -> EffectRandoms {
        EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 3,
            multi_hit_roll: 0,
        }
    }

    #[test]
    fn flinch_side_triggers_on_low_roll() {
        let mut state = make_state();
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_flinch_side(&mut state, &randoms, 26);
        assert_eq!(result, EffectResult::FlinchApplied);
        assert!(state.enemy.has_status1(status1::FLINCHED));
    }

    #[test]
    fn flinch_side_no_trigger_on_high_roll() {
        let mut state = make_state();
        let randoms = EffectRandoms {
            side_effect_roll: 200,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_flinch_side(&mut state, &randoms, 26);
        assert_eq!(result, EffectResult::NoEffect);
    }

    #[test]
    fn flinch_blocked_by_substitute() {
        let mut state = make_state();
        state.enemy.set_status2(status2::HAS_SUBSTITUTE_UP);
        state.enemy.substitute_hp = 50;
        let randoms = EffectRandoms {
            side_effect_roll: 0,
            duration_roll: 0,
            multi_hit_roll: 0,
        };
        let result = apply_flinch_side(&mut state, &randoms, 26);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn confusion_primary_applies() {
        let mut state = make_state();
        let randoms = default_randoms();
        let result = apply_confusion_primary(&mut state, &randoms);
        assert_eq!(result, EffectResult::ConfusionApplied);
        assert!(state.enemy.has_status1(status1::CONFUSED));
        assert!(state.enemy.confused_turns_left >= 2);
    }

    #[test]
    fn confusion_fails_if_already_confused() {
        let mut state = make_state();
        state.enemy.set_status1(status1::CONFUSED);
        state.enemy.confused_turns_left = 3;
        let randoms = default_randoms();
        let result = apply_confusion_primary(&mut state, &randoms);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn confusion_side_chance() {
        let mut state = make_state();
        let randoms = EffectRandoms {
            side_effect_roll: 200,
            duration_roll: 3,
            multi_hit_roll: 0,
        };
        let result = apply_confusion_side(&mut state, &randoms, 26);
        assert_eq!(result, EffectResult::NoEffect);
    }

    #[test]
    fn transform_copies_opponent() {
        let mut state = make_state();
        state.enemy.active_mon_mut().species = Species::Charizard;
        state.enemy.active_mon_mut().type1 = PokemonType::Fire;
        state.enemy.active_mon_mut().type2 = PokemonType::Flying;
        state.enemy.active_mon_mut().attack = 150;
        state.enemy.stat_stages.attack = 2;

        let result = apply_transform(&mut state);
        assert_eq!(result, EffectResult::Transformed);
        assert_eq!(state.player.active_mon().species, Species::Charizard);
        assert_eq!(state.player.active_mon().type1, PokemonType::Fire);
        assert_eq!(state.player.active_mon().attack, 150);
        assert_eq!(state.player.stat_stages.attack, 2);
        assert!(state.player.has_status3(status3::TRANSFORMED));
        assert_eq!(state.player.active_mon().pp[0], 5);
    }

    #[test]
    fn mimic_copies_last_move() {
        let mut state = make_state();
        state.player.selected_move_index = 2;
        state.enemy.last_move_used = MoveId::Flamethrower;
        let result = apply_mimic(&mut state);
        assert_eq!(result, EffectResult::MoveCopied);
        assert_eq!(state.player.active_mon().moves[2], MoveId::Flamethrower);
    }

    #[test]
    fn mimic_fails_if_no_last_move() {
        let mut state = make_state();
        state.enemy.last_move_used = MoveId::None;
        let result = apply_mimic(&mut state);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn mirror_move_returns_last_move() {
        let mut state = make_state();
        state.enemy.last_move_used = MoveId::Surf;
        let result = apply_mirror_move(&mut state);
        assert_eq!(
            result,
            EffectResult::MirrorMove {
                mirrored_move: MoveId::Surf
            }
        );
    }

    #[test]
    fn mirror_move_fails_if_no_last() {
        let mut state = make_state();
        state.enemy.last_move_used = MoveId::None;
        let result = apply_mirror_move(&mut state);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn disable_disables_last_move() {
        let mut state = make_state();
        state.enemy.last_move_used = MoveId::Thundershock;
        let randoms = default_randoms();
        let result = apply_disable(&mut state, &randoms);
        assert_eq!(result, EffectResult::Disabled);
        assert_eq!(state.enemy.disabled_move, 1);
        assert!(state.enemy.disabled_turns_left > 0);
    }

    #[test]
    fn disable_fails_if_already_disabled() {
        let mut state = make_state();
        state.enemy.disabled_move = 1;
        state.enemy.disabled_turns_left = 3;
        state.enemy.last_move_used = MoveId::Thundershock;
        let randoms = default_randoms();
        let result = apply_disable(&mut state, &randoms);
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn pay_day_adds_coins() {
        let mut state = make_state();
        let result = apply_pay_day(&mut state, 40);
        assert_eq!(result, EffectResult::PayDay { coins: 100 });
        assert_eq!(state.total_payday_money, 100);
    }

    #[test]
    fn switch_teleport_in_wild_escapes() {
        let mut state = make_state();
        let result = apply_switch_teleport(&mut state);
        assert_eq!(result, EffectResult::SwitchedOut);
        assert!(state.escaped);
    }

    #[test]
    fn switch_teleport_in_trainer_fails() {
        let mut state = new_battle_state(
            BattleType::Trainer,
            vec![make_pokemon()],
            vec![make_pokemon()],
        );
        let result = apply_switch_teleport(&mut state);
        assert_eq!(result, EffectResult::StatusFailed);
        assert!(!state.escaped);
    }

    #[test]
    fn splash_does_nothing() {
        assert_eq!(EffectResult::NothingHappened, EffectResult::NothingHappened);
    }
}
