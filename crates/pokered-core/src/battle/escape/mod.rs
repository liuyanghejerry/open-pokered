// M2.9: Escape mechanism — run from battle, Teleport/Whirlwind/Roar
//
// Reference: engine/battle/core.asm TryRunningFromBattle (lines 1496-1583)
//            engine/battle/effects.asm SwitchAndTeleportEffect (lines 810-909)

use super::state::{BattleState, BattleType, Side};

/// Result of attempting to run from battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResult {
    /// Successfully escaped.
    Escaped,
    /// Cannot escape (trainer battle).
    CannotRun,
    /// Failed to escape this turn (wild battle odds check failed).
    FailedToEscape,
}

/// Attempt to run from battle (player presses RUN).
///
/// Faithfully implements TryRunningFromBattle from core.asm:
/// - Ghost battle / Safari / Link → always escape
/// - Trainer battle → always fail ("No running from a trainer battle!")
/// - Wild battle → speed-based probability with attempt bonus
///
/// `random_byte`: a random u8 value [0, 255] passed in for testability.
pub fn try_run_from_battle(state: &mut BattleState, random_byte: u8) -> RunResult {
    // Trainer battle: can't run
    if state.battle_type == BattleType::Trainer {
        return RunResult::CannotRun;
    }

    // Wild battle
    state.num_run_attempts += 1;

    let player_speed = state.player.active_mon().speed;
    let enemy_speed = state.enemy.active_mon().speed;

    // If player speed >= enemy speed → always escape
    // ASM: StringCmp on 2-byte big-endian speeds, jump if player >= enemy
    if player_speed >= enemy_speed {
        state.escaped = true;
        return RunResult::Escaped;
    }

    // escape_odds = (player_speed * 32) / ((enemy_speed / 4) % 256)
    // ASM multiplies player speed (16-bit) by 32, then divides by low byte of (enemy_speed / 4)

    let divisor = ((enemy_speed / 4) & 0xFF) as u8;

    // If divisor is 0 → always escape
    // ASM: "jr z, .canEscape ; jump if enemy speed divided by 4, mod 256 is 0"
    if divisor == 0 {
        state.escaped = true;
        return RunResult::Escaped;
    }

    let numerator = (player_speed as u32) * 32;
    let quotient = (numerator / divisor as u32) as u16;

    // If quotient >= 256 → always escape
    // ASM: "ldh a, [hQuotient + 2] / and a / jr nz, .canEscape"
    if quotient >= 256 {
        state.escaped = true;
        return RunResult::Escaped;
    }

    // Add 30 for each previous run attempt (attempts - 1, since we already incremented)
    // ASM loops (wNumRunAttempts - 1) times adding 30 to quotient byte
    let mut escape_odds = quotient as u8;
    for _ in 1..state.num_run_attempts {
        let (new_odds, overflow) = escape_odds.overflowing_add(30);
        if overflow {
            // Overflow means > 255, guaranteed escape
            state.escaped = true;
            return RunResult::Escaped;
        }
        escape_odds = new_odds;
    }

    // Compare with random byte
    // ASM: "cp b / jr nc, .canEscape" — escape if escape_odds >= random_byte
    if escape_odds >= random_byte {
        state.escaped = true;
        return RunResult::Escaped;
    }

    // Failed to escape — player loses their turn
    RunResult::FailedToEscape
}

/// Result of using Teleport, Whirlwind, or Roar as a move effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeleportResult {
    /// Move succeeded — battle ends.
    Success,
    /// Move failed — "But it failed!" / "X is unaffected!"
    Failed,
}

/// The move being used for the SwitchAndTeleport effect.
/// This affects the failure message (Teleport → "But it failed!", Roar/Whirlwind → "unaffected").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeMove {
    Teleport,
    Whirlwind,
    Roar,
}

/// Try to use Teleport, Roar, or Whirlwind to escape from battle.
///
/// Faithfully implements SwitchAndTeleportEffect from effects.asm:
/// - In trainer battles → always fails
/// - In wild battles:
///   - If user's level >= target's level → always succeeds
///   - Otherwise: rejection sampling: rand(0, user_lvl + target_lvl + 1) >= target_lvl / 4
///
/// `whose_turn`: which side is using the move.
/// `random_value`: for testability, a random u8 in [0, 255].
///
/// NOTE: The ASM uses rejection sampling to get a uniform random in [0, c)
/// where c = user_level + target_level + 1. We simplify this to a modular check
/// since the distribution is the same.
pub fn try_escape_move(
    state: &mut BattleState,
    whose_turn: Side,
    random_value: u8,
) -> TeleportResult {
    // Trainer battles: always fails
    if state.battle_type == BattleType::Trainer {
        return TeleportResult::Failed;
    }

    // Wild battle
    let (user_level, target_level) = match whose_turn {
        Side::Player => (
            state.player.active_mon().level,
            state.enemy.active_mon().level,
        ),
        Side::Enemy => (
            state.enemy.active_mon().level,
            state.player.active_mon().level,
        ),
    };

    // If user's level >= target's level → always succeeds
    if user_level >= target_level {
        state.escaped = true;
        return TeleportResult::Success;
    }

    // Otherwise: probability check
    // c = user_level + target_level + 1
    // threshold = target_level / 4
    // rand[0, c) >= threshold → success
    //
    // The ASM uses rejection sampling: keep calling BattleRandom until result < c.
    // We use modulo for the same uniform distribution.
    let range = user_level as u16 + target_level as u16 + 1;
    let sampled = (random_value as u16) % range;
    let threshold = target_level / 4;

    if sampled >= threshold as u16 {
        state.escaped = true;
        TeleportResult::Success
    } else {
        TeleportResult::Failed
    }
}

#[cfg(test)]
mod tests;
