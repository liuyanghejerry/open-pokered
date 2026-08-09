use crate::moves::MoveId;
use crate::species::{GrowthRate, Species};
use crate::types::PokemonType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseStats {
    pub species: Species,
    pub hp: u8,
    pub attack: u8,
    pub defense: u8,
    pub speed: u8,
    pub special: u8,
    pub type1: PokemonType,
    pub type2: PokemonType,
    pub catch_rate: u8,
    pub base_exp: u8,
    pub initial_moves: [MoveId; 4],
    pub growth_rate: GrowthRate,
    pub tm_hm_flags: [u8; 7],
}

/// Get base stats for a species. Returns None for Species::None.
pub fn get_base_stats(species: Species) -> Option<&'static BaseStats> {
    // Editor-injected runtime override shadows the baseline.
    if let Some(ov) = crate::runtime_overrides::base_stats_override(species) {
        return Some(ov);
    }
    let idx = species as usize;
    if idx == 0 || idx > BASE_STATS.len() {
        None
    } else {
        Some(&BASE_STATS[idx - 1])
    }
}

pub const BASE_STATS: &[BaseStats] =
    &include!(concat!(env!("OUT_DIR"), "/pokemon_data_gen.rs"));

