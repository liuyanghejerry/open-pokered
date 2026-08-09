use std::path::Path;

use pokered_core::battle::state::{BattleType, Pokemon, StatusCondition};
use pokered_core::pokemon::stats::create_pokemon_with_moves;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::trainer_data::TrainerClass;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct BattleConfig {
    #[serde(default = "default_battle_type")]
    pub battle_type: BattleType,
    /// Trainer class for enemy AI move selection (e.g. "Rival3").
    /// If omitted, the enemy picks moves randomly.
    #[serde(default)]
    pub trainer_class: Option<TrainerClass>,
    pub player_party: Vec<PokemonConfig>,
    pub enemy_party: Vec<PokemonConfig>,
}

fn default_battle_type() -> BattleType {
    BattleType::Trainer
}

#[derive(Deserialize)]
pub struct PokemonConfig {
    pub species: Species,
    pub level: u8,
    #[serde(default = "default_moves")]
    pub moves: [MoveId; 4],
    #[serde(default = "default_dvs")]
    pub dv_bytes: [u8; 2],
    /// Override current HP (defaults to max HP if omitted)
    pub hp: Option<u16>,
    /// Override status condition (defaults to None if omitted)
    #[serde(default)]
    pub status: StatusCondition,
}

fn default_moves() -> [MoveId; 4] {
    [MoveId::None, MoveId::None, MoveId::None, MoveId::None]
}

fn default_dvs() -> [u8; 2] {
    [0xFF, 0xFF]
}

impl BattleConfig {
    pub fn load(path: &Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
        serde_json::from_slice(&data)
            .map_err(|e| format!("Failed to parse battle config {:?}: {}", path, e))
    }

    pub fn build_parties(&self) -> Result<(Vec<Pokemon>, Vec<Pokemon>), String> {
        let player = build_party(&self.player_party, "player")?;
        let enemy = build_party(&self.enemy_party, "enemy")?;
        Ok((player, enemy))
    }
}

fn build_party(configs: &[PokemonConfig], label: &str) -> Result<Vec<Pokemon>, String> {
    if configs.is_empty() {
        return Err(format!("{} party is empty", label));
    }
    if configs.len() > 6 {
        return Err(format!("{} party exceeds 6 Pokémon", label));
    }
    configs
        .iter()
        .enumerate()
        .map(|(i, cfg)| {
            let moves = if cfg.moves == [MoveId::None; 4] {
                None
            } else {
                Some(cfg.moves)
            };
            let mut pkmn = match moves {
                Some(m) => create_pokemon_with_moves(cfg.species, cfg.level, cfg.dv_bytes, m),
                None => pokered_core::pokemon::stats::create_pokemon(
                    cfg.species,
                    cfg.level,
                    cfg.dv_bytes,
                ),
            }
            .ok_or_else(|| {
                format!(
                    "{} party slot {}: unknown species {:?}",
                    label, i, cfg.species
                )
            })?;
            if let Some(hp) = cfg.hp {
                pkmn.hp = hp.min(pkmn.max_hp);
            }
            pkmn.status = cfg.status;
            Ok(pkmn)
        })
        .collect()
}
