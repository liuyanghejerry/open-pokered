use super::state::{status1, status2, BattlerState, StatusCondition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCheckResult {
    CanMove,
    CannotMove(CannotMoveReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CannotMoveReason {
    Asleep,
    WokeUpButLostTurn,
    Frozen,
    TrappedByEnemy,
    Flinched,
    MustRecharge,
    ConfusedSelfHit,
    MoveDisabled,
    FullyParalyzed,
}

/// Check all status conditions in Gen 1 order before a move executes.
/// ASM order: Sleep → Freeze → Trapped → Flinch → Recharge → (Disabled counter) →
///            Confusion → (Disabled move check) → Paralysis
///
/// `opponent` needed for trapped check (enemy using trapping move on us).
/// `random_confusion` used for confusion 50% self-hit check (< 128 = hit self).
/// `random_paralysis` used for paralysis 25% full-para check (< 63 = fully paralyzed).
pub fn check_status_conditions(
    battler: &mut BattlerState,
    opponent: &BattlerState,
    random_confusion: u8,
    random_paralysis: u8,
) -> StatusCheckResult {
    // 1. Sleep
    if let StatusCondition::Sleep(counter) = battler.active_mon().status {
        if counter > 0 {
            let new_counter = counter - 1;
            battler.active_mon_mut().status = if new_counter == 0 {
                StatusCondition::None
            } else {
                StatusCondition::Sleep(new_counter)
            };
            // ASM: even if counter just hit 0 (woke up), still cannot move this turn
            battler.player_used_move = false;
            return if new_counter == 0 {
                StatusCheckResult::CannotMove(CannotMoveReason::WokeUpButLostTurn)
            } else {
                StatusCheckResult::CannotMove(CannotMoveReason::Asleep)
            };
        }
    }

    // 2. Freeze — no per-turn thaw in Gen 1
    if battler.active_mon().status == StatusCondition::Freeze {
        battler.player_used_move = false;
        return StatusCheckResult::CannotMove(CannotMoveReason::Frozen);
    }

    // 3. Trapped (opponent using Wrap/Bind/etc on us)
    if opponent.has_status1(status1::USING_TRAPPING_MOVE) {
        battler.player_used_move = false;
        return StatusCheckResult::CannotMove(CannotMoveReason::TrappedByEnemy);
    }

    // 4. Flinch
    if battler.has_status1(status1::FLINCHED) {
        battler.clear_status1(status1::FLINCHED);
        battler.player_used_move = false;
        return StatusCheckResult::CannotMove(CannotMoveReason::Flinched);
    }

    // 5. Recharge (Hyper Beam)
    if battler.has_status2(status2::NEEDS_TO_RECHARGE) {
        battler.clear_status2(status2::NEEDS_TO_RECHARGE);
        battler.player_used_move = false;
        return StatusCheckResult::CannotMove(CannotMoveReason::MustRecharge);
    }

    // 6. Decrement disabled counter
    if battler.disabled_turns_left > 0 {
        battler.disabled_turns_left -= 1;
        if battler.disabled_turns_left == 0 {
            battler.disabled_move = 0;
        }
    }

    // 7. Confusion
    if battler.has_status1(status1::CONFUSED) {
        if battler.confused_turns_left > 0 {
            battler.confused_turns_left -= 1;
        }
        if battler.confused_turns_left == 0 {
            battler.clear_status1(status1::CONFUSED);
        } else {
            // 50% chance to hit self: random < 128
            if random_confusion < 128 {
                return StatusCheckResult::CannotMove(CannotMoveReason::ConfusedSelfHit);
            }
        }
    }

    // 8. Disabled move check
    if battler.disabled_move != 0 && battler.selected_move_index == battler.disabled_move {
        battler.player_used_move = false;
        return StatusCheckResult::CannotMove(CannotMoveReason::MoveDisabled);
    }

    // 9. Paralysis — 25% full paralysis: random < 63
    if battler.active_mon().status == StatusCondition::Paralysis {
        if random_paralysis < 63 {
            battler.player_used_move = false;
            return StatusCheckResult::CannotMove(CannotMoveReason::FullyParalyzed);
        }
    }

    StatusCheckResult::CanMove
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

    fn make_battler() -> BattlerState {
        new_battler_state(vec![make_pokemon()])
    }

    #[test]
    fn no_status_can_move() {
        let mut battler = make_battler();
        let opponent = make_battler();
        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(result, StatusCheckResult::CanMove);
    }

    #[test]
    fn sleep_prevents_move_and_decrements() {
        let mut battler = make_battler();
        battler.active_mon_mut().status = StatusCondition::Sleep(3);
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::Asleep)
        );
        assert_eq!(battler.active_mon().status, StatusCondition::Sleep(2));

        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::Asleep)
        );
        assert_eq!(battler.active_mon().status, StatusCondition::Sleep(1));

        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::WokeUpButLostTurn)
        );
        assert_eq!(battler.active_mon().status, StatusCondition::None);

        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(result, StatusCheckResult::CanMove);
    }

    #[test]
    fn freeze_always_prevents() {
        let mut battler = make_battler();
        battler.active_mon_mut().status = StatusCondition::Freeze;
        let opponent = make_battler();
        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::Frozen)
        );
        assert_eq!(battler.active_mon().status, StatusCondition::Freeze);
    }

    #[test]
    fn trapped_by_opponent() {
        let mut battler = make_battler();
        let mut opponent = make_battler();
        opponent.set_status1(status1::USING_TRAPPING_MOVE);
        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::TrappedByEnemy)
        );
    }

    #[test]
    fn flinch_clears_after_check() {
        let mut battler = make_battler();
        battler.set_status1(status1::FLINCHED);
        let opponent = make_battler();
        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::Flinched)
        );
        assert!(!battler.has_status1(status1::FLINCHED));
    }

    #[test]
    fn recharge_clears_after_check() {
        let mut battler = make_battler();
        battler.set_status2(status2::NEEDS_TO_RECHARGE);
        let opponent = make_battler();
        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::MustRecharge)
        );
        assert!(!battler.has_status2(status2::NEEDS_TO_RECHARGE));
    }

    #[test]
    fn confusion_self_hit() {
        let mut battler = make_battler();
        battler.set_status1(status1::CONFUSED);
        battler.confused_turns_left = 3;
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 0, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::ConfusedSelfHit)
        );
    }

    #[test]
    fn confusion_snaps_out() {
        let mut battler = make_battler();
        battler.set_status1(status1::CONFUSED);
        battler.confused_turns_left = 1;
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(result, StatusCheckResult::CanMove);
        assert!(!battler.has_status1(status1::CONFUSED));
    }

    #[test]
    fn confusion_can_move_through() {
        let mut battler = make_battler();
        battler.set_status1(status1::CONFUSED);
        battler.confused_turns_left = 3;
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 200, 255);
        assert_eq!(result, StatusCheckResult::CanMove);
    }

    #[test]
    fn paralysis_full_para() {
        let mut battler = make_battler();
        battler.active_mon_mut().status = StatusCondition::Paralysis;
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 255, 0);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::FullyParalyzed)
        );
    }

    #[test]
    fn paralysis_can_move() {
        let mut battler = make_battler();
        battler.active_mon_mut().status = StatusCondition::Paralysis;
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 255, 200);
        assert_eq!(result, StatusCheckResult::CanMove);
    }

    #[test]
    fn disabled_move_check() {
        let mut battler = make_battler();
        battler.disabled_move = 1;
        battler.disabled_turns_left = 3;
        battler.selected_move_index = 1;
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(
            result,
            StatusCheckResult::CannotMove(CannotMoveReason::MoveDisabled)
        );
    }

    #[test]
    fn disabled_counter_decrements() {
        let mut battler = make_battler();
        battler.disabled_move = 2;
        battler.disabled_turns_left = 1;
        battler.selected_move_index = 0;
        let opponent = make_battler();

        let result = check_status_conditions(&mut battler, &opponent, 255, 255);
        assert_eq!(result, StatusCheckResult::CanMove);
        assert_eq!(battler.disabled_turns_left, 0);
        assert_eq!(battler.disabled_move, 0);
    }
}
