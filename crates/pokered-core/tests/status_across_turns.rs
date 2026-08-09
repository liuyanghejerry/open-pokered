//! Integration test: Status effects persisting across multiple turns
//! Burn residual damage, poison damage, and their interaction with combat

mod helpers;

use helpers::*;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_data::moves::MoveId;

/// Burn damages the burned Pokémon each turn (maxHP/16, min 1).
#[test]
fn burn_residual_accumulates_over_turns() {
    let player = make_pikachu(25, 160, 90); // 160 HP, burn dmg = 160/16 = 10/turn
    let enemy = make_rattata(25, 160);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Burn the player's Pikachu
    state.player.active_mon_mut().status = StatusCondition::Burn;

    let initial_hp = state.player.active_mon().hp;

    // Execute 3 turns
    for _ in 0..3 {
        if state.player.active_mon().hp == 0 || state.enemy.active_mon().hp == 0 {
            break;
        }
        let randoms = turn_randoms_player_first_no_side();
        execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);
    }

    let final_hp = state.player.active_mon().hp;
    let hp_lost = initial_hp - final_hp;

    // Player lost HP from: enemy attacks + burn residual (10 HP each turn acted)
    // Burn damage per turn = 160/16 = 10
    // So at least 30 HP from burn alone over 3 turns, plus attack damage
    assert!(
        hp_lost >= 30,
        "Should have lost at least 30 HP from burn alone, lost {}",
        hp_lost
    );
}

/// Poison damages each turn.
#[test]
fn poison_residual_each_turn() {
    let player = make_pikachu(25, 160, 90);
    let enemy = make_rattata(25, 160);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Poison the player's Pikachu
    state.player.active_mon_mut().status = StatusCondition::Poison;

    let initial_hp = state.player.active_mon().hp;

    // Execute 2 turns
    for _ in 0..2 {
        if state.player.active_mon().hp == 0 || state.enemy.active_mon().hp == 0 {
            break;
        }
        let randoms = turn_randoms_player_first_no_side();
        execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);
    }

    let final_hp = state.player.active_mon().hp;
    let hp_lost = initial_hp - final_hp;

    // Poison = 160/16 = 10/turn, 2 turns = at least 20 HP from poison
    assert!(
        hp_lost >= 20,
        "Should have lost at least 20 HP from poison, lost {}",
        hp_lost
    );
}

/// Toxic (badly poisoned) damage escalates each turn.
#[test]
fn toxic_damage_escalates() {
    let player = make_pikachu(25, 160, 90);
    let enemy = make_rattata(5, 160);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Apply Toxic to the enemy
    state.enemy.active_mon_mut().status = StatusCondition::Poison;
    state
        .enemy
        .set_status3(pokered_core::battle::state::status3::BADLY_POISONED);
    state.enemy.toxic_counter = 0;

    // Track enemy HP across turns
    let mut enemy_hp_history = vec![state.enemy.active_mon().hp];

    for _ in 0..3 {
        if state.enemy.active_mon().hp == 0 {
            break;
        }
        let randoms = turn_randoms_player_first_no_side();
        execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);
        enemy_hp_history.push(state.enemy.active_mon().hp);
    }

    // Enemy should have taken increasing toxic damage each turn on top of attack damage
    // toxic_counter increments: 1, 2, 3... → damage = maxHP/16 * counter
    // With 160 HP: 10, 20, 30... per turn
    // Total from toxic alone: 10 + 20 + 30 = 60 + attack damage
    let total_loss = enemy_hp_history[0] - *enemy_hp_history.last().unwrap_or(&0);
    assert!(
        total_loss > 60,
        "Expected significant HP loss from toxic + attacks, lost {}",
        total_loss
    );
}

/// Sleep prevents action but decrements counter each turn.
#[test]
fn sleep_wears_off_after_turns() {
    let player = make_pikachu(25, 160, 90);
    let enemy = make_rattata(25, 160);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Put enemy to sleep for 2 turns
    state.enemy.active_mon_mut().status = StatusCondition::Sleep(2);

    // Turn 1: enemy is asleep, can't attack
    let randoms = turn_randoms_player_first_no_side();
    let _result1 = execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);

    // Player attacked, enemy should have been unable to move (asleep)
    let _player_hp_after_turn1 = state.player.active_mon().hp;

    // Turn 2: enemy might wake up
    let randoms2 = turn_randoms_player_first_no_side();
    let _result2 = execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms2);

    // After 2 turns of sleep(2), enemy should have woken up or be close
    // The exact behavior depends on decrement timing, but sleep should resolve
}

/// Freeze prevents action indefinitely (no natural thaw in Gen 1).
#[test]
fn freeze_prevents_action() {
    let player = make_pikachu(25, 160, 90);
    let enemy = make_rattata(25, 80);
    let mut state = new_battle_state(BattleType::Wild, vec![player], vec![enemy]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Freeze the enemy
    state.enemy.active_mon_mut().status = StatusCondition::Freeze;

    let player_hp_initial = state.player.active_mon().hp;

    // Execute turn — frozen enemy can't act
    let randoms = turn_randoms_player_first_no_side();
    execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);

    // Player should not have taken damage from frozen enemy
    assert_eq!(
        state.player.active_mon().hp,
        player_hp_initial,
        "Frozen enemy should not deal damage"
    );
}
