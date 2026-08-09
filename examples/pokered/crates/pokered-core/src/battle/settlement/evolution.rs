use pokered_data::evos_moves::{evos_moves_data, EvolutionMethod};
use pokered_data::species::Species;

use crate::battle::experience::stats::calc_all_stats;
use crate::battle::state::Pokemon;

pub fn check_level_evolution(species: Species, level: u8) -> Option<Species> {
    let all = evos_moves_data();
    let entry = all.iter().find(|e| e.species == species)?;
    for evo in &entry.evolutions {
        if let EvolutionMethod::Level {
            level: req_level,
            species: target,
        } = evo
        {
            if level >= *req_level {
                return Some(*target);
            }
        }
    }
    None
}

pub fn check_trade_evolution(species: Species, level: u8) -> Option<Species> {
    let all = evos_moves_data();
    let entry = all.iter().find(|e| e.species == species)?;
    for evo in &entry.evolutions {
        if let EvolutionMethod::Trade {
            min_level,
            species: target,
        } = evo
        {
            if level >= *min_level {
                return Some(*target);
            }
        }
    }
    None
}

pub fn check_item_evolution(
    species: Species,
    level: u8,
    item: pokered_data::items::ItemId,
) -> Option<Species> {
    let all = evos_moves_data();
    let entry = all.iter().find(|e| e.species == species)?;
    for evo in &entry.evolutions {
        if let EvolutionMethod::Item {
            item: evo_item,
            min_level,
            species: target,
        } = evo
        {
            if *evo_item == item && level >= *min_level {
                return Some(*target);
            }
        }
    }
    None
}

/// Apply evolution to a Pokemon: change species, recalculate stats, adjust HP by delta.
pub fn apply_evolution(pokemon: &mut Pokemon, new_species: Species) {
    let base = pokered_data::pokemon_data::get_base_stats(new_species);
    let base = match base {
        Some(b) => b,
        None => return,
    };

    let old_max_hp = pokemon.max_hp;
    pokemon.species = new_species;
    pokemon.type1 = base.type1;
    pokemon.type2 = base.type2;

    let stats = calc_all_stats(base, pokemon.dv_bytes, &pokemon.stat_exp, pokemon.level);
    pokemon.max_hp = stats.0;
    pokemon.attack = stats.1;
    pokemon.defense = stats.2;
    pokemon.speed = stats.3;
    pokemon.special = stats.4;

    // Adjust current HP by the delta in max HP (same as original game)
    let hp_delta = pokemon.max_hp as i32 - old_max_hp as i32;
    pokemon.hp = ((pokemon.hp as i32) + hp_delta).max(0) as u16;
    if pokemon.hp > pokemon.max_hp {
        pokemon.hp = pokemon.max_hp;
    }
}
