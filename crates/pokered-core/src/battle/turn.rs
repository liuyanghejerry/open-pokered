use pokered_data::move_data::MoveData;

use super::move_execution::{execute_move, MoveOutcome, MoveRandoms};
use super::residual::apply_all_residual;
use super::state::{BattleState, Side};
use super::turn_order::{determine_order, TurnOrder};

pub struct TurnRandoms {
    pub order_random: u8,
    pub first_mover: MoveRandoms,
    pub second_mover: MoveRandoms,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnResult {
    pub first: Side,
    pub first_outcome: MoveOutcome,
    pub first_fainted: Option<Side>,
    pub second_outcome: Option<MoveOutcome>,
    pub second_fainted: Option<Side>,
}

/// Execute one full battle turn: determine order → first mover acts → residual →
/// faint check → second mover acts → residual → faint check.
///
/// ASM: MainInBattleLoop flow. Each side's move is followed by residual damage.
/// If first mover's target faints, second mover does not act.
pub fn execute_turn(
    state: &mut BattleState,
    player_move: &MoveData,
    enemy_move: &MoveData,
    randoms: &TurnRandoms,
) -> TurnResult {
    let order = determine_order(state, randoms.order_random);
    let (first_side, second_side) = match order {
        TurnOrder::PlayerFirst => (Side::Player, Side::Enemy),
        TurnOrder::EnemyFirst => (Side::Enemy, Side::Player),
    };

    let first_move_data = match first_side {
        Side::Player => player_move,
        Side::Enemy => enemy_move,
    };

    state.whose_turn = first_side;
    let first_outcome = execute_move(state, first_move_data, &randoms.first_mover);

    let first_fainted = apply_residual_and_check_faint(state, first_side);

    let defender_fainted = is_side_fainted(state, second_side);
    if defender_fainted || first_fainted.is_some() {
        return TurnResult {
            first: first_side,
            first_outcome,
            first_fainted,
            second_outcome: None,
            second_fainted: None,
        };
    }

    let second_move_data = match second_side {
        Side::Player => player_move,
        Side::Enemy => enemy_move,
    };

    state.whose_turn = second_side;
    let second_outcome = execute_move(state, second_move_data, &randoms.second_mover);

    let second_fainted = apply_residual_and_check_faint(state, second_side);

    TurnResult {
        first: first_side,
        first_outcome,
        first_fainted,
        second_outcome: Some(second_outcome),
        second_fainted,
    }
}

fn apply_residual_and_check_faint(state: &mut BattleState, acting_side: Side) -> Option<Side> {
    let (attacker, opponent) = match acting_side {
        Side::Player => (&mut state.player, &mut state.enemy),
        Side::Enemy => (&mut state.enemy, &mut state.player),
    };
    apply_all_residual(attacker, opponent);

    if attacker.active_mon().hp == 0 {
        Some(acting_side)
    } else if opponent.active_mon().hp == 0 {
        Some(acting_side.opposite())
    } else {
        None
    }
}

fn is_side_fainted(state: &BattleState, side: Side) -> bool {
    state.side(side).active_mon().hp == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::*;
    use pokered_data::move_data::MoveData;
    use pokered_data::moves::{MoveEffect, MoveId};
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    fn make_pokemon_with(hp: u16, speed: u16) -> Pokemon {
        Pokemon {
            species: Species::Pikachu,
            nickname: [0x50; 11],
            level: 50,
            hp,
            max_hp: hp,
            attack: 100,
            defense: 80,
            speed,
            special: 80,
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
        }
    }

    fn make_state_with_speeds(player_speed: u16, enemy_speed: u16) -> BattleState {
        let p = vec![make_pokemon_with(200, player_speed)];
        let e = vec![make_pokemon_with(200, enemy_speed)];
        let mut state = new_battle_state(BattleType::Wild, p, e);
        state.player.selected_move = MoveId::Thundershock;
        state.player.selected_move_index = 0;
        state.enemy.selected_move = MoveId::Thundershock;
        state.enemy.selected_move_index = 0;
        state
    }

    fn thundershock() -> MoveData {
        MoveData {
            id: MoveId::Thundershock,
            effect: MoveEffect::NoAdditionalEffect,
            power: 40,
            move_type: PokemonType::Electric,
            accuracy: 255,
            pp: 30,
        }
    }

    fn hit_randoms() -> MoveRandoms {
        MoveRandoms {
            confusion_roll: 255,
            paralysis_roll: 255,
            crit_roll: 255,
            accuracy_roll: 0,
            damage_roll: 255,
            effect_randoms: crate::battle::effects::EffectRandoms {
                side_effect_roll: 255,
                duration_roll: 0,
                multi_hit_roll: 0,
            },
        }
    }

    fn turn_randoms() -> TurnRandoms {
        TurnRandoms {
            order_random: 0,
            first_mover: hit_randoms(),
            second_mover: hit_randoms(),
        }
    }

    #[test]
    fn faster_player_goes_first() {
        let mut state = make_state_with_speeds(100, 50);
        let move_data = thundershock();
        let randoms = turn_randoms();

        let result = execute_turn(&mut state, &move_data, &move_data, &randoms);
        assert_eq!(result.first, Side::Player);
        assert!(matches!(result.first_outcome, MoveOutcome::Success { .. }));
        assert!(matches!(
            result.second_outcome,
            Some(MoveOutcome::Success { .. })
        ));
    }

    #[test]
    fn both_sides_take_damage() {
        let mut state = make_state_with_speeds(100, 50);
        let move_data = thundershock();
        let randoms = turn_randoms();

        execute_turn(&mut state, &move_data, &move_data, &randoms);
        assert!(state.player.active_mon().hp < 200);
        assert!(state.enemy.active_mon().hp < 200);
    }

    #[test]
    fn ko_prevents_second_move() {
        let p = vec![make_pokemon_with(200, 100)];
        let e_mon = Pokemon {
            species: Species::Pikachu,
            nickname: [0x50; 11],
            level: 50,
            hp: 1,
            max_hp: 200,
            attack: 100,
            defense: 80,
            speed: 50,
            special: 80,
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
        let e = vec![e_mon];
        let mut state = new_battle_state(BattleType::Wild, p, e);
        state.player.selected_move = MoveId::Thundershock;
        state.player.selected_move_index = 0;
        state.enemy.selected_move = MoveId::Thundershock;
        state.enemy.selected_move_index = 0;

        let move_data = thundershock();
        let randoms = turn_randoms();

        let result = execute_turn(&mut state, &move_data, &move_data, &randoms);
        assert_eq!(result.first, Side::Player);
        assert!(state.enemy.active_mon().hp == 0);
        assert_eq!(result.second_outcome, None);
        assert_eq!(state.player.active_mon().hp, 200);
    }

    #[test]
    fn slower_enemy_goes_first_when_faster() {
        let mut state = make_state_with_speeds(50, 100);
        let move_data = thundershock();
        let randoms = turn_randoms();

        let result = execute_turn(&mut state, &move_data, &move_data, &randoms);
        assert_eq!(result.first, Side::Enemy);
    }

    #[test]
    fn burn_residual_applies_after_move() {
        let mut state = make_state_with_speeds(100, 50);
        state.player.active_mon_mut().status = StatusCondition::Burn;
        state.player.active_mon_mut().max_hp = 160;
        state.player.active_mon_mut().hp = 160;

        let move_data = thundershock();
        let randoms = turn_randoms();

        execute_turn(&mut state, &move_data, &move_data, &randoms);

        let player_hp = state.player.active_mon().hp;
        assert!(
            player_hp < 150,
            "Player should have burn + attack damage, got {}",
            player_hp
        );
    }
}
