pub mod evolution;
pub mod money;
pub mod settle;
pub mod writeback;

pub use writeback::{settle_battle_into_save, SettleWriteback};

#[cfg(test)]
mod writeback_tests;
#[cfg(test)]
mod evolution_tests;
#[cfg(test)]
mod money_tests;
#[cfg(test)]
mod settle_tests;

use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleOutcome {
    Win,
    Loss,
    Draw,
    Escaped,
    Captured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionEvent {
    pub party_index: usize,
    pub old_species: Species,
    pub new_species: Species,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelUpEvent {
    pub party_index: usize,
    pub old_level: u8,
    pub new_level: u8,
    pub learned_moves: Vec<MoveId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpGainEntry {
    pub party_index: usize,
    pub species: Species,
    pub exp_gained: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleSettlement {
    pub outcome: BattleOutcome,
    pub money_gained: u32,
    pub money_lost: u32,
    pub payday_bonus: u32,
    pub exp_entries: Vec<ExpGainEntry>,
    pub level_ups: Vec<LevelUpEvent>,
    pub evolutions: Vec<EvolutionEvent>,
}
