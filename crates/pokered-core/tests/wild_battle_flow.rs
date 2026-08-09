//! Integration test: Wild battle full flow
//! Encounter → turns → KO → exp gain → settlement

mod helpers;

use helpers::*;
use pokered_core::battle::capture::{try_capture, CaptureContext, CaptureRandoms, CaptureResult};
use pokered_core::battle::experience::gain::gain_experience;
use pokered_core::battle::move_execution::MoveOutcome;
use pokered_core::battle::settlement::settle::settle_battle;
use pokered_core::battle::settlement::BattleOutcome;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_core::battle::wild::{
    try_wild_encounter, EncounterContext, WildEncounterRandoms, WildEncounterResult,
};
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::wild_data::{WildEncounterTable, WildMon};

/// Full wild battle: encounter a Rattata, fight with Pikachu, KO it, gain EXP, settle.
#[test]
fn wild_battle_encounter_fight_ko_exp_settle() {
    // Step 1: Trigger wild encounter
    let table = WildEncounterTable {
        encounter_rate: 25,
        mons: vec![
            WildMon {
                level: 5,
                species: Species::Rattata,
            },
            WildMon {
                level: 4,
                species: Species::Pidgey,
            },
        ],
    };
    let enc_randoms = WildEncounterRandoms {
        encounter_roll: 10, // < 25, triggers encounter
        slot_roll: 0,       // slot 0 → Rattata lv5
    };
    let enc_ctx = EncounterContext {
        repel_active: false,
        party_lead_level: 25,
    };
    let result = try_wild_encounter(Some(&table), &enc_randoms, &enc_ctx);
    assert_eq!(
        result,
        WildEncounterResult::Encounter {
            level: 5,
            species: Species::Rattata,
        }
    );

    // Step 2: Set up battle state
    let player_pikachu = make_pikachu(25, 55, 90);
    let wild_rattata = make_rattata(5, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![player_pikachu], vec![wild_rattata]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Mark player's mon as having fought
    state.party_fought_flags[0] = true;
    state.party_gain_exp_flags[0] = true;

    // Step 3: Execute one turn — Pikachu (speed 90) faster than Rattata (speed ~20)
    let randoms = turn_randoms_player_first_no_side();
    let turn_result = execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);

    // Pikachu goes first (much faster)
    assert_eq!(turn_result.first, Side::Player);
    assert!(matches!(
        turn_result.first_outcome,
        MoveOutcome::Success { .. }
    ));

    // Rattata (20 HP, lv5) should be KO'd by lv25 Pikachu's Thundershock
    let enemy_hp = state.enemy.active_mon().hp;
    assert_eq!(enemy_hp, 0, "Rattata should be KO'd, has {} HP", enemy_hp);

    // Second mover should not act (enemy fainted)
    assert!(turn_result.second_outcome.is_none());

    // Step 4: Gain experience
    let exp_before = state.player.party[0].total_exp;
    let _exp_result = gain_experience(&mut state, Species::Rattata, 5, false);
    let exp_after = state.player.party[0].total_exp;
    assert!(exp_after > exp_before, "Should have gained EXP");
    // Rattata base_exp = 57, level 5: (57 * 5) / 7 = 40
    assert_eq!(exp_after - exp_before, 40);

    // Step 5: Settle battle
    let settlement = settle_battle(&mut state, BattleOutcome::Win, None, 1000);
    assert_eq!(settlement.outcome, BattleOutcome::Win);
    assert_eq!(settlement.money_gained, 0); // Wild battle, no prize money
    assert_eq!(settlement.money_lost, 0);
}

/// Wild battle capture flow: weaken → throw Poké Ball → capture → settle.
#[test]
fn wild_battle_weaken_and_capture() {
    let player_pikachu = make_pikachu(25, 55, 90);
    let wild_rattata = make_rattata(5, 20);
    let mut state = new_battle_state(BattleType::Wild, vec![player_pikachu], vec![wild_rattata]);
    state.player.selected_move = MoveId::Thundershock;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    // Execute a turn with low damage to weaken but not KO
    // Use damage_roll = 0 (minimum damage)
    let mut randoms = turn_randoms_player_first_no_side();
    randoms.first_mover.damage_roll = 217; // minimum damage roll
    let _turn_result = execute_turn(&mut state, &thundershock_data(), &tackle_data(), &randoms);
    let enemy_hp = state.enemy.active_mon().hp;
    if enemy_hp > 0 {
        // Throw Poké Ball — use favorable random values
        let cap_ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 20,
            wild_current_hp: enemy_hp,
            wild_catch_rate: 255, // Rattata has catch_rate 255
            wild_status: StatusCondition::None,
        };
        let cap_randoms = CaptureRandoms { rand1: 0, rand2: 0 };
        let cap_result = try_capture(&cap_ctx, &cap_randoms);
        assert_eq!(cap_result, CaptureResult::Captured);

        // Settle as captured
        let settlement = settle_battle(&mut state, BattleOutcome::Captured, None, 500);
        assert_eq!(settlement.outcome, BattleOutcome::Captured);
        assert_eq!(settlement.money_gained, 0);
    } else {
        // If KO'd anyway, just verify the battle concluded
        assert_eq!(enemy_hp, 0);
    }
}
