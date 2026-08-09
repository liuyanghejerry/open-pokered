//! Integration test: Trainer battle full flow
//! Multi-turn combat → exp gain → prize money → settlement

mod helpers;

use helpers::*;
use pokered_core::battle::experience::gain::gain_experience;
use pokered_core::battle::settlement::settle::settle_battle;
use pokered_core::battle::settlement::BattleOutcome;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::trainer_data::TrainerClass;

/// Full trainer battle: Pikachu vs Bug Catcher's Caterpie.
/// Multiple turns until KO, then EXP + prize money.
#[test]
fn trainer_battle_multi_turn_ko_exp_prize() {
    // Bug Catcher's Caterpie
    let caterpie = make_pokemon(
        Species::Caterpie,
        8,
        30, // hp
        30, // attack
        35, // defense
        45, // speed
        20, // special
        pokered_data::types::PokemonType::Bug,
        pokered_data::types::PokemonType::Bug,
        [
            MoveId::Tackle,
            MoveId::StringShot,
            MoveId::None,
            MoveId::None,
        ],
        [35, 40, 0, 0],
    );

    let player_pikachu = make_pikachu(10, 30, 50);
    let mut state = new_battle_state(BattleType::Trainer, vec![player_pikachu], vec![caterpie]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;
    state.party_fought_flags[0] = true;
    state.party_gain_exp_flags[0] = true;

    // Execute turns until enemy faints
    let mut turn_count = 0;
    while state.enemy.active_mon().hp > 0 && turn_count < 10 {
        let randoms = turn_randoms_player_first_no_side();
        execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);
        turn_count += 1;
    }

    assert!(
        turn_count <= 5,
        "Should KO Caterpie within 5 turns, took {}",
        turn_count
    );
    assert_eq!(state.enemy.active_mon().hp, 0, "Caterpie should be KO'd");

    // Gain experience — Caterpie base_exp=53, level=8, is_trainer=true
    // exp = (53 * 8) / 7 = 60; trainer bonus: 60 * 3/2 = 90
    let exp_before = state.player.party[0].total_exp;
    let _exp_result = gain_experience(&mut state, Species::Caterpie, 8, false);
    let exp_after = state.player.party[0].total_exp;
    let gained = exp_after - exp_before;
    assert_eq!(
        gained, 90,
        "Expected 90 EXP (trainer bonus), got {}",
        gained
    );

    // Settle: Bug Catcher base_money=10, last enemy level=8
    // prize = 10 * 8 = 80
    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Win,
        Some(TrainerClass::BugCatcher),
        500,
    );
    assert_eq!(settlement.outcome, BattleOutcome::Win);
    assert_eq!(settlement.money_gained, 80);
    assert_eq!(settlement.money_lost, 0);
}

/// Trainer battle loss: player's Pokémon faints → blackout penalty.
#[test]
fn trainer_battle_loss_blackout_penalty() {
    // Strong enemy Onix
    let onix = make_pokemon(
        Species::Onix,
        14,
        200, // high HP
        45,
        160, // very high defense
        70,
        30,
        pokered_data::types::PokemonType::Rock,
        pokered_data::types::PokemonType::Ground,
        [
            MoveId::Tackle,
            MoveId::RockThrow,
            MoveId::None,
            MoveId::None,
        ],
        [35, 15, 0, 0],
    );

    // Weak Pikachu
    let player_pikachu = make_pikachu(5, 20, 30);
    let mut state = new_battle_state(BattleType::Trainer, vec![player_pikachu], vec![onix]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Execute turns until player faints
    let mut turn_count = 0;
    while state.player.active_mon().hp > 0 && turn_count < 20 {
        let randoms = turn_randoms_player_first_no_side();
        execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);
        turn_count += 1;
    }

    assert_eq!(
        state.player.active_mon().hp,
        0,
        "Pikachu should have fainted"
    );

    // Settle as loss — blackout penalty = money / 2
    let player_money = 1000;
    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Loss,
        Some(TrainerClass::Brock),
        player_money,
    );
    assert_eq!(settlement.outcome, BattleOutcome::Loss);
    assert_eq!(settlement.money_lost, 500); // 1000 / 2
    assert_eq!(settlement.money_gained, 0);
}

/// Trainer battle with PayDay bonus added to prize money.
#[test]
fn trainer_battle_payday_bonus() {
    let caterpie = make_pokemon(
        Species::Caterpie,
        5,
        15,
        30,
        35,
        45,
        20,
        pokered_data::types::PokemonType::Bug,
        pokered_data::types::PokemonType::Bug,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        [35, 0, 0, 0],
    );

    let player_pikachu = make_pikachu(25, 55, 90);
    let mut state = new_battle_state(BattleType::Trainer, vec![player_pikachu], vec![caterpie]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Simulate PayDay money
    state.total_payday_money = 200;

    // KO in one turn
    let randoms = turn_randoms_player_first_no_side();
    execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);
    assert_eq!(state.enemy.active_mon().hp, 0);

    // Settle: Youngster base_money=15, level=5 → prize=75
    // total = 75 + 200 = 275
    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Win,
        Some(TrainerClass::Youngster),
        500,
    );
    assert_eq!(settlement.outcome, BattleOutcome::Win);
    assert_eq!(settlement.money_gained, 275);
}
