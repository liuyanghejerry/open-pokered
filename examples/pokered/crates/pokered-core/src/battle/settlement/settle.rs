use pokered_data::trainer_data::TrainerClass;

use super::evolution::check_level_evolution;
use super::money::{calc_blackout_penalty, calc_prize_money, calc_total_winnings};
use super::{BattleOutcome, BattleSettlement, EvolutionEvent, ExpGainEntry};
use crate::battle::state::{BattleState, BattleType};

pub fn settle_battle(
    state: &mut BattleState,
    outcome: BattleOutcome,
    trainer_class: Option<TrainerClass>,
    player_money: u32,
) -> BattleSettlement {
    let mut settlement = BattleSettlement {
        outcome,
        money_gained: 0,
        money_lost: 0,
        payday_bonus: 0,
        exp_entries: Vec::new(),
        level_ups: Vec::new(),
        evolutions: Vec::new(),
    };

    match outcome {
        BattleOutcome::Win | BattleOutcome::Captured => {
            let prize = if state.battle_type == BattleType::Trainer {
                if let Some(class) = trainer_class {
                    let last_level = last_enemy_level(state);
                    calc_prize_money(class, last_level)
                } else {
                    0
                }
            } else {
                0
            };

            let payday = state.total_payday_money;
            settlement.payday_bonus = payday;
            settlement.money_gained = calc_total_winnings(prize, payday);

            collect_exp_entries(state, &mut settlement.exp_entries);
        }
        BattleOutcome::Loss => {
            settlement.money_lost = calc_blackout_penalty(player_money);
        }
        BattleOutcome::Escaped | BattleOutcome::Draw => {
            collect_exp_entries(state, &mut settlement.exp_entries);
        }
    }

    // EvolutionAfterBattle (engine/pokemon/evos_moves.asm:13) runs from
    // EndOfBattle only when the player WON: `ld a, [wBattleResult] / and a /
    // jr nz, .resetVariables` (engine/battle/end_of_battle.asm:29-33) skips it
    // for wBattleResult != 0 — loss ($1), and running away / catching ($2,
    // core.asm:2293 GotAwayText / core.asm:1603 capture). The evolutions are
    // only DETECTED here; the frontend plays the evolution cutscene
    // (`crate::evolution_screen`) and applies each confirmed one with
    // `pokemon::evolution::finalize_evolution`.
    if outcome == BattleOutcome::Win {
        detect_evolutions(state, &mut settlement.evolutions);
    }

    settlement
}

fn last_enemy_level(state: &BattleState) -> u8 {
    state.enemy.party.last().map(|m| m.level).unwrap_or(1)
}

fn collect_exp_entries(state: &BattleState, entries: &mut Vec<ExpGainEntry>) {
    for (i, mon) in state.player.party.iter().enumerate() {
        if state.party_gain_exp_flags[i] && mon.total_exp > 0 {
            entries.push(ExpGainEntry {
                party_index: i,
                species: mon.species,
                exp_gained: mon.total_exp,
            });
        }
    }
}

/// `Evolution_PartyMonLoop` (evos_moves.asm:26-68): walk the party, skip
/// mons whose `wCanEvolveFlags` bit is clear (i.e. they did not level up this
/// battle), and record the first level-evolution whose requirement the mon's
/// current level meets. The mon itself is NOT mutated — the cutscene decides
/// (B-cancel) before `finalize_evolution` applies anything.
fn detect_evolutions(state: &BattleState, events: &mut Vec<EvolutionEvent>) {
    for (i, mon) in state.player.party.iter().enumerate() {
        if mon.hp == 0 {
            continue;
        }
        if !state.party_leveled_up_flags.get(i).copied().unwrap_or(false) {
            continue;
        }
        if let Some(new_species) = check_level_evolution(mon.species, mon.level) {
            events.push(EvolutionEvent {
                party_index: i,
                old_species: mon.species,
                new_species,
            });
        }
    }
}
