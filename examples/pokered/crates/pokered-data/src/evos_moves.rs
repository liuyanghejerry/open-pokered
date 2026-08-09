//! Evolution and level-up learnset data for all 151 Pokemon.
//! Source of truth: `pokemon/{Species}.json`, with the runtime body emitted
//! by `pokered-data/build.rs::generate_pokemon_and_evos_data`.

use crate::items::ItemId;
use crate::moves::MoveId;
use crate::species::Species;
use serde::{Deserialize, Serialize};

/// Method by which a Pokemon can evolve
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvolutionMethod {
    /// Evolves at a specific level
    Level { level: u8, species: Species },
    /// Evolves when a specific item is used
    Item {
        item: ItemId,
        min_level: u8,
        species: Species,
    },
    /// Evolves when traded
    Trade { min_level: u8, species: Species },
}

/// A move learned at a specific level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelUpMove {
    pub level: u8,
    pub move_id: MoveId,
}

/// Evolution and learnset data for a single species
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvosMovesEntry {
    pub species: Species,
    pub evolutions: Vec<EvolutionMethod>,
    pub learnset: Vec<LevelUpMove>,
}


/// Get evolution and learnset data for all 151 Pokemon, ordered by dex number.
pub fn evos_moves_data() -> Vec<EvosMovesEntry> {
    include!(concat!(env!("OUT_DIR"), "/evos_moves_gen.rs"))
}

/// Get evolution and learnset data for a specific species.
pub fn get_evos_moves(species: Species) -> Option<&'static EvosMovesEntry> {
    use std::sync::LazyLock;
    static DATA: LazyLock<Vec<EvosMovesEntry>> = LazyLock::new(evos_moves_data);
    let dex = species as u8;
    if dex >= 1 && dex as usize <= DATA.len() {
        Some(&DATA[(dex - 1) as usize])
    } else {
        None
    }
}
