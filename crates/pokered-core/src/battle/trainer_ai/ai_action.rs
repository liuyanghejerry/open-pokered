//! Per-class AI action routines — item usage, switching, X items.
//!
//! Faithfully implements `engine/battle/trainer_ai.asm` logic.
//! Each routine checks random chance and/or HP thresholds, then
//! returns an `AiAction` the battle engine should execute.

use super::AiRoutine;
use crate::battle::state::BattlerState;

/// Action the AI wants to perform this turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiAction {
    /// Do nothing special — pick a move normally.
    DoNothing,
    /// Use a healing item. `heal_amount` is 0 for Full Restore (heals to max + cures status).
    UsePotion { heal_amount: u16 },
    /// Use Full Heal to cure status condition.
    UseFullHeal,
    /// Use X Attack (+1 attack stage).
    UseXAttack,
    /// Use X Defend (+1 defense stage).
    UseXDefend,
    /// Use X Speed (+1 speed stage).
    UseXSpeed,
    /// Use X Special (+1 special stage).
    UseXSpecial,
    /// Use Guard Spec. (protect against stat-downs).
    UseGuardSpec,
    /// Switch to a different Pokémon.
    SwitchPokemon,
}

impl AiAction {
    /// Whether this action consumes an AI count charge.
    pub fn consumes_charge(&self) -> bool {
        !matches!(self, AiAction::DoNothing)
    }
}

/// Check if HP is below `max_hp / denominator`.
fn hp_below_fraction(mon: &crate::battle::state::Pokemon, denominator: u16) -> bool {
    if denominator == 0 {
        return false;
    }
    let threshold = mon.max_hp / denominator;
    mon.hp < threshold
}

/// Main entry point: execute the AI action routine for the given class.
///
/// `rand_val` is a 0..255 random byte provided by the caller.
/// `ai_count` is decremented when an action is taken.
///
/// Returns the action the battle engine should execute.
pub fn execute_ai_action(
    routine: AiRoutine,
    ai_count: &mut u8,
    enemy: &BattlerState,
    rand_val: u8,
) -> AiAction {
    if *ai_count == 0 {
        return AiAction::DoNothing;
    }

    let action = match routine {
        AiRoutine::Generic => ai_generic(),
        AiRoutine::Juggler => ai_juggler(rand_val),
        AiRoutine::Blackbelt => ai_blackbelt(rand_val),
        AiRoutine::Giovanni => ai_giovanni(rand_val),
        AiRoutine::CooltrainerM => ai_cooltrainer_m(rand_val),
        AiRoutine::CooltrainerF => ai_cooltrainer_f(enemy, rand_val),
        AiRoutine::Bruno => ai_bruno(rand_val),
        AiRoutine::Brock => ai_brock(enemy),
        AiRoutine::Misty => ai_misty(rand_val),
        AiRoutine::LtSurge => ai_lt_surge(rand_val),
        AiRoutine::Erika => ai_erika(enemy, rand_val),
        AiRoutine::Koga => ai_koga(rand_val),
        AiRoutine::Blaine => ai_blaine(rand_val),
        AiRoutine::Sabrina => ai_sabrina(enemy, rand_val),
        AiRoutine::Rival2 => ai_rival2(enemy, rand_val),
        AiRoutine::Rival3 => ai_rival3(enemy, rand_val),
        AiRoutine::Lorelei => ai_lorelei(enemy, rand_val),
        AiRoutine::Agatha => ai_agatha(enemy, rand_val),
        AiRoutine::Lance => ai_lance(enemy, rand_val),
    };

    if action.consumes_charge() {
        *ai_count = ai_count.saturating_sub(1);
    }
    action
}

// ---------------------------------------------------------------------------
// Per-class routines
// ---------------------------------------------------------------------------

fn ai_generic() -> AiAction {
    AiAction::DoNothing
}

/// 25% chance to switch.
fn ai_juggler(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::SwitchPokemon
    } else {
        AiAction::DoNothing
    }
}

/// ~13% chance to use X Attack.
fn ai_blackbelt(rand_val: u8) -> AiAction {
    if rand_val < 33 {
        AiAction::UseXAttack
    } else {
        AiAction::DoNothing
    }
}

/// 25% chance to use Guard Spec.
fn ai_giovanni(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::UseGuardSpec
    } else {
        AiAction::DoNothing
    }
}

/// 25% chance to use X Attack.
fn ai_cooltrainer_m(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::UseXAttack
    } else {
        AiAction::DoNothing
    }
}

/// BUG: The 25% random-check-for-switch never early-returns (ret nc is missing).
/// Execution always falls through:
///   - If HP < 20% → Hyper Potion (200 HP).
///   - Else if rand < 51 (~20%) → switch.
///   - Else → do nothing.
fn ai_cooltrainer_f(enemy: &BattlerState, rand_val: u8) -> AiAction {
    // NOTE: The initial `call Random; cp 25% + 1; ret nc` is commented out in ASM,
    // so we skip it entirely — matching the bug.
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 5) {
        AiAction::UsePotion { heal_amount: 200 }
    } else if rand_val < 51 {
        AiAction::SwitchPokemon
    } else {
        AiAction::DoNothing
    }
}

/// 25% chance to use X Defend.
fn ai_bruno(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::UseXDefend
    } else {
        AiAction::DoNothing
    }
}

/// Use Full Heal if active Pokémon has a non-volatile status condition.
fn ai_brock(enemy: &BattlerState) -> AiAction {
    if !enemy.active_mon().status.is_none() {
        AiAction::UseFullHeal
    } else {
        AiAction::DoNothing
    }
}

/// 25% chance to use X Defend.
fn ai_misty(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::UseXDefend
    } else {
        AiAction::DoNothing
    }
}

/// 25% chance to use X Speed.
fn ai_lt_surge(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::UseXSpeed
    } else {
        AiAction::DoNothing
    }
}

/// 50% random gate, then Super Potion (50 HP) if HP < 10%.
fn ai_erika(enemy: &BattlerState, rand_val: u8) -> AiAction {
    if rand_val >= 128 {
        return AiAction::DoNothing;
    }
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 10) {
        AiAction::UsePotion { heal_amount: 50 }
    } else {
        AiAction::DoNothing
    }
}

/// 25% chance to use X Attack.
fn ai_koga(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::UseXAttack
    } else {
        AiAction::DoNothing
    }
}

/// 25% chance to use Super Potion (50 HP) — NO HP check! (ASM quirk)
fn ai_blaine(rand_val: u8) -> AiAction {
    if rand_val < 64 {
        AiAction::UsePotion { heal_amount: 50 }
    } else {
        AiAction::DoNothing
    }
}

/// 50% random gate, then Hyper Potion (200 HP) if HP < 10%.
fn ai_sabrina(enemy: &BattlerState, rand_val: u8) -> AiAction {
    if rand_val >= 128 {
        return AiAction::DoNothing;
    }
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 10) {
        AiAction::UsePotion { heal_amount: 200 }
    } else {
        AiAction::DoNothing
    }
}

/// ~13% random gate, then Potion (20 HP) if HP < 20%.
fn ai_rival2(enemy: &BattlerState, rand_val: u8) -> AiAction {
    if rand_val >= 33 {
        return AiAction::DoNothing;
    }
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 5) {
        AiAction::UsePotion { heal_amount: 20 }
    } else {
        AiAction::DoNothing
    }
}

/// ~13% random gate, then Full Restore (heal_amount=0) if HP < 20%.
fn ai_rival3(enemy: &BattlerState, rand_val: u8) -> AiAction {
    if rand_val >= 33 {
        return AiAction::DoNothing;
    }
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 5) {
        // heal_amount=0 signals "Full Restore" — heals fully + cures status
        AiAction::UsePotion { heal_amount: 0 }
    } else {
        AiAction::DoNothing
    }
}

/// 50% random gate, then Super Potion (50 HP) if HP < 20%.
fn ai_lorelei(enemy: &BattlerState, rand_val: u8) -> AiAction {
    if rand_val >= 128 {
        return AiAction::DoNothing;
    }
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 5) {
        AiAction::UsePotion { heal_amount: 50 }
    } else {
        AiAction::DoNothing
    }
}

/// Two-phase AI:
///   1. ~8% chance (rand < 20) → switch.
///   2. Otherwise, 50% random gate, then Super Potion (50 HP) if HP < 25%.
fn ai_agatha(enemy: &BattlerState, rand_val: u8) -> AiAction {
    if rand_val < 20 {
        return AiAction::SwitchPokemon;
    }
    if rand_val >= 128 {
        return AiAction::DoNothing;
    }
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 4) {
        AiAction::UsePotion { heal_amount: 50 }
    } else {
        AiAction::DoNothing
    }
}

/// 50% random gate, then Hyper Potion (200 HP) if HP < 20%.
fn ai_lance(enemy: &BattlerState, rand_val: u8) -> AiAction {
    if rand_val >= 128 {
        return AiAction::DoNothing;
    }
    let mon = enemy.active_mon();
    if hp_below_fraction(mon, 5) {
        AiAction::UsePotion { heal_amount: 200 }
    } else {
        AiAction::DoNothing
    }
}

#[path = "ai_action_tests.rs"]
#[cfg(test)]
mod ai_action_tests;
