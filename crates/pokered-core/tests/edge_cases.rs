//! Integration test: Edge cases
//! Multi-pokemon party, evolution triggers, traded mon bonus, exp distribution

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

/// Party of multiple Pokémon: first faints, flags indicate who fought.
/// EXP goes only to participants.
#[test]
fn multi_pokemon_exp_distribution() {
    let pikachu = make_pikachu(15, 40, 70);
    let charmander = make_charmander(12, 35);
    let bulbasaur = make_bulbasaur(10, 32);

    let enemy = make_rattata(5, 15);
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![pikachu, charmander, bulbasaur],
        vec![enemy],
    );
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Only Pikachu (index 0) fought
    state.party_fought_flags[0] = true;
    state.party_gain_exp_flags[0] = true;

    // KO enemy in one turn
    let randoms = turn_randoms_player_first_no_side();
    execute_turn(
        &mut state,
        &thundershock_data(),
        &tackle_data(),
        &randoms,
    );
    assert_eq!(state.enemy.active_mon().hp, 0);

    // Record EXP before
    let pikachu_exp_before = state.player.party[0].total_exp;
    let charmander_exp_before = state.player.party[1].total_exp;
    let bulbasaur_exp_before = state.player.party[2].total_exp;

    // Gain experience
    gain_experience(&mut state, Species::Rattata, 5, false);

    // Only Pikachu should gain EXP
    assert!(state.player.party[0].total_exp > pikachu_exp_before);
    assert_eq!(state.player.party[1].total_exp, charmander_exp_before);
    assert_eq!(state.player.party[2].total_exp, bulbasaur_exp_before);
}

/// EXP ALL distributes to entire party.
#[test]
fn exp_all_distributes_to_entire_party() {
    let pikachu = make_pikachu(15, 40, 70);
    let charmander = make_charmander(12, 35);

    let enemy = make_rattata(10, 25);
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![pikachu, charmander],
        vec![enemy],
    );

    // Only Pikachu fought
    state.party_gain_exp_flags[0] = true;

    let pikachu_exp_before = state.player.party[0].total_exp;
    let charmander_exp_before = state.player.party[1].total_exp;

    // With EXP ALL, both should gain
    gain_experience(&mut state, Species::Rattata, 10, true);

    assert!(state.player.party[0].total_exp > pikachu_exp_before, "Pikachu should gain EXP");
    assert!(state.player.party[1].total_exp > charmander_exp_before, "Charmander should gain EXP with EXP ALL");
}

/// Traded Pokémon gets 1.5x EXP bonus.
#[test]
fn traded_mon_bonus_exp() {
    let mut pikachu = make_pikachu(15, 40, 70);
    pikachu.is_traded = true;

    let mut pikachu_not_traded = make_pikachu(15, 40, 70);
    pikachu_not_traded.is_traded = false;

    let enemy = make_rattata(10, 25);

    // Test traded mon
    let mut state1 = new_battle_state(
        BattleType::Wild,
        vec![pikachu],
        vec![enemy.clone()],
    );
    state1.party_gain_exp_flags[0] = true;
    gain_experience(&mut state1, Species::Rattata, 10, false);
    let traded_exp = state1.player.party[0].total_exp;

    // Test non-traded mon
    let enemy2 = make_rattata(10, 25);
    let mut state2 = new_battle_state(
        BattleType::Wild,
        vec![pikachu_not_traded],
        vec![enemy2],
    );
    state2.party_gain_exp_flags[0] = true;
    gain_experience(&mut state2, Species::Rattata, 10, false);
    let normal_exp = state2.player.party[0].total_exp;

    // Traded should get 1.5x
    // Rattata base_exp=57, level=10: (57*10)/7 = 81
    // Traded: 81 * 3/2 = 121
    // Normal: 81
    assert_eq!(normal_exp, 81);
    assert_eq!(traded_exp, 121);
}

/// Level-up evolution triggered after battle settlement.
#[test]
fn level_up_triggers_evolution_in_settlement() {
    // Ivysaur evolves to Venusaur at level 32
    // Create an Ivysaur at level 32 (just hit the evolution level)
    let ivysaur = make_pokemon(
        Species::Ivysaur,
        32,
        80,
        62, 63, 60, 80,
        pokered_data::types::PokemonType::Grass,
        pokered_data::types::PokemonType::Poison,
        [MoveId::VineWhip, MoveId::Tackle, MoveId::None, MoveId::None],
        [10, 35, 0, 0],
    );

    let enemy = make_rattata(5, 15);
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![ivysaur],
        vec![enemy],
    );
    // wCanEvolveFlags: the mon leveled up during this battle.
    state.party_leveled_up_flags[0] = true;

    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Win,
        None,
        500,
    );

    // The evolution is detected (queued for the post-battle cutscene)…
    assert!(
        !settlement.evolutions.is_empty(),
        "Ivysaur at level 32 should evolve to Venusaur"
    );
    assert_eq!(settlement.evolutions[0].old_species, Species::Ivysaur);
    assert_eq!(settlement.evolutions[0].new_species, Species::Venusaur);
    assert_eq!(settlement.evolutions[0].party_index, 0);

    // …but NOT applied here — the evolution cutscene (and its B-cancel)
    // runs first; only a confirmed evolution swaps the species.
    assert_eq!(state.player.party[0].species, Species::Ivysaur);
}

/// Fainted Pokémon don't trigger evolution.
#[test]
fn fainted_mon_no_evolution() {
    let mut ivysaur = make_pokemon(
        Species::Ivysaur,
        32,
        80,
        62, 63, 60, 80,
        pokered_data::types::PokemonType::Grass,
        pokered_data::types::PokemonType::Poison,
        [MoveId::VineWhip, MoveId::None, MoveId::None, MoveId::None],
        [10, 0, 0, 0],
    );
    ivysaur.hp = 0; // fainted

    let enemy = make_rattata(5, 15);
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![ivysaur],
        vec![enemy],
    );

    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Win,
        None,
        500,
    );

    // Fainted Ivysaur should NOT evolve
    assert!(
        settlement.evolutions.is_empty(),
        "Fainted Pokémon should not evolve"
    );
    assert_eq!(state.player.party[0].species, Species::Ivysaur);
}

/// Multiple turns with PP tracking — PP decrements each use.
#[test]
fn pp_decrements_across_turns() {
    let player = make_pikachu(25, 160, 90);
    let enemy = make_rattata(25, 160);
    let mut state = new_battle_state(
        BattleType::Wild,
        vec![player],
        vec![enemy],
    );
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    let initial_pp = state.player.active_mon().pp[0];
    assert_eq!(initial_pp, 30);

    // Execute 3 turns
    for _ in 0..3 {
        if state.player.active_mon().hp == 0 || state.enemy.active_mon().hp == 0 {
            break;
        }
        let randoms = turn_randoms_player_first_no_side();
        execute_turn(
            &mut state,
            &thundershock_data(),
            &tackle_data(),
            &randoms,
        );
    }

    // PP should have decreased by number of turns executed (player used move each turn)
    let final_pp = state.player.active_mon().pp[0];
    let turns_used = (initial_pp - final_pp) as usize;
    assert!(turns_used >= 1 && turns_used <= 3, "PP should have decreased by 1-3, decreased by {}", turns_used);
}

/// Prize money cap at 999,999.
#[test]
fn prize_money_cap() {
    let enemy = make_rattata(5, 15);
    let mut state = new_battle_state(
        BattleType::Trainer,
        vec![make_pikachu(50, 100, 120)],
        vec![enemy],
    );

    // Set huge PayDay amount
    state.total_payday_money = 999_990;

    // Giovanni base_money=99, level=5 → prize=495
    // total = 495 + 999990 = 1000485 → capped at 999999
    let settlement = settle_battle(
        &mut state,
        BattleOutcome::Win,
        Some(TrainerClass::Giovanni),
        0,
    );
    assert_eq!(settlement.money_gained, 999_999);
}
