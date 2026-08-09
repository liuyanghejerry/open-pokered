//! Integration test: Escape mechanics
//! Failed escape → retry → successful escape → settlement

mod helpers;

use helpers::*;
use pokered_core::battle::escape::{
    try_escape_move, try_run_from_battle, RunResult, TeleportResult,
};
use pokered_core::battle::settlement::settle::settle_battle;
use pokered_core::battle::settlement::BattleOutcome;
use pokered_core::battle::state::*;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;

/// Wild battle: fail to escape, then succeed on retry due to attempt bonus.
#[test]
fn escape_fail_then_succeed_on_retry() {
    let player = make_pikachu(10, 30, 40); // speed 40
    let enemy = make_rattata(12, 25); // speed 72 (faster)
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);

    // First attempt: likely to fail with high random
    // escape_odds = (40 * 32) / ((72 / 4) % 256) = 1280 / 18 = 71
    // With random_byte = 200 (> 71), should fail
    let result1 = try_run_from_battle(&mut state, 200);
    assert_eq!(result1, RunResult::FailedToEscape);
    assert_eq!(state.num_run_attempts, 1);
    assert!(!state.escaped);

    // Second attempt: escape_odds = 71 + 30 = 101
    // With random_byte = 80 (< 101), should succeed
    let result2 = try_run_from_battle(&mut state, 80);
    assert_eq!(result2, RunResult::Escaped);
    assert_eq!(state.num_run_attempts, 2);
    assert!(state.escaped);

    // Settle as escaped
    let settlement = settle_battle(&mut state, BattleOutcome::Escaped, None, 500);
    assert_eq!(settlement.outcome, BattleOutcome::Escaped);
    assert_eq!(settlement.money_gained, 0);
    assert_eq!(settlement.money_lost, 0);
}

/// Trainer battle: cannot escape.
#[test]
fn trainer_battle_cannot_escape() {
    let player = make_pikachu(10, 30, 40);
    let enemy = make_rattata(12, 25);
    let mut state = new_battle_state(BattleType::Trainer, vec![player], vec![enemy]);

    let result = try_run_from_battle(&mut state, 0);
    assert_eq!(result, RunResult::CannotRun);
    assert!(!state.escaped);
}

/// Faster player always escapes on first try.
#[test]
fn faster_player_always_escapes() {
    let player = make_pikachu(10, 30, 100); // speed 100
    let enemy = make_rattata(5, 20); // speed 72
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);

    let result = try_run_from_battle(&mut state, 255); // worst random
    assert_eq!(result, RunResult::Escaped);
    assert!(state.escaped);
}

/// Teleport in wild battle: higher level user always succeeds.
#[test]
fn teleport_higher_level_always_succeeds() {
    let player = make_pikachu(25, 55, 90);
    let enemy = make_rattata(5, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);

    let result = try_escape_move(&mut state, Side::Player, 255);
    assert_eq!(result, TeleportResult::Success);
    assert!(state.escaped);
}

/// Teleport in trainer battle always fails.
#[test]
fn teleport_trainer_battle_always_fails() {
    let player = make_pikachu(50, 100, 120);
    let enemy = make_rattata(5, 20);
    let mut state = new_battle_state(BattleType::Trainer, vec![player], vec![enemy]);

    let result = try_escape_move(&mut state, Side::Player, 0);
    assert_eq!(result, TeleportResult::Failed);
    assert!(!state.escaped);
}

/// Multiple failed escape attempts: attempt bonus accumulates → eventual guaranteed escape.
#[test]
fn escape_attempt_bonus_accumulates() {
    let player = make_pikachu(5, 20, 10); // very slow (speed 10)
    let enemy = make_pokemon(
        Species::Onix,
        20,
        50,
        45,
        160,
        70,
        30,
        pokered_data::types::PokemonType::Rock,
        pokered_data::types::PokemonType::Ground,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        [35, 0, 0, 0],
    );
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);

    // escape_odds = (10 * 32) / ((70 / 4) % 256) = 320 / 17 = 18
    // Each attempt adds 30: 18, 48, 78, 108, 138, 168, 198, 228, 258 (overflow → guaranteed)
    // With random_byte = 255, need at least 9 attempts for overflow guarantee.
    // But with enough attempts and high enough odds, we'll escape.

    let mut escaped = false;
    for _ in 0..10 {
        let result = try_run_from_battle(&mut state, 250);
        if result == RunResult::Escaped {
            escaped = true;
            break;
        }
    }
    assert!(
        escaped,
        "Should eventually escape due to attempt bonus accumulation"
    );
}
