//! One-shot seeding tool: writes the in-source `BASE_STATS`, `MOVES`, and
//! `evos_moves_data()` tables to `pokemon/*.json` and `moves/*.json` so the
//! editor + `build.rs` can use them as the source of truth from then on.
//! Output shape matches the editor JSON files exactly (including the
//! `pokedex` block, sourced from `POKEDEX_ENTRIES`).
//!
//! Run: `cargo run --example dump_pokemon_and_moves -p pokered-data`

use pokered_data::evos_moves::{evos_moves_data, EvolutionMethod};
use pokered_data::items::ItemId;
use pokered_data::move_data::MOVES;
use pokered_data::moves::MoveId;
use pokered_data::pokedex::get_pokedex_entry;
use pokered_data::pokemon_data::BASE_STATS;
use pokered_data::species::{GrowthRate, Species};
use pokered_data::types::PokemonType;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PokemonJson {
    #[serde(rename = "$schema")]
    schema: &'static str,
    species: String,
    base_stats: BaseStatsJson,
    type1: String,
    type2: String,
    catch_rate: u8,
    base_exp: u8,
    growth_rate: String,
    initial_moves: [String; 4],
    tm_hm_flags: [u8; 7],
    pokedex: PokedexJson,
    evolutions: Vec<EvolutionJson>,
    learnset: Vec<LearnsetEntryJson>,
}

#[derive(Serialize)]
struct BaseStatsJson {
    hp: u8,
    attack: u8,
    defense: u8,
    speed: u8,
    special: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PokedexJson {
    category: String,
    height_feet: u8,
    height_inches: u8,
    weight_decipounds: u16,
    flavor_text_pages: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EvolutionJson {
    method: &'static str,
    species: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_level: Option<u8>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LearnsetEntryJson {
    level: u8,
    move_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveJson {
    #[serde(rename = "$schema")]
    schema: &'static str,
    id: String,
    effect: String,
    power: u8,
    #[serde(rename = "type")]
    move_type: String,
    accuracy: u8,
    pp: u8,
}

fn species_name(s: Species) -> String {
    format!("{:?}", s)
}

fn move_name(m: MoveId) -> String {
    format!("{:?}", m)
}

fn type_name(t: PokemonType) -> String {
    format!("{:?}", t)
}

fn growth_rate_name(g: GrowthRate) -> String {
    format!("{:?}", g)
}

fn item_name(i: ItemId) -> String {
    format!("{:?}", i)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut s = serde_json::to_string_pretty(value).expect("serialize");
    s.push('\n');
    fs::write(path, s)
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pokemon_dir = manifest_dir.join("pokemon");
    let moves_dir = manifest_dir.join("moves");
    fs::create_dir_all(&pokemon_dir).expect("create pokemon dir");
    fs::create_dir_all(&moves_dir).expect("create moves dir");

    let evos = evos_moves_data();
    let evos_lookup: std::collections::HashMap<String, (Vec<EvolutionJson>, Vec<LearnsetEntryJson>)> =
        evos
            .into_iter()
            .map(|e| {
                let evolutions = e
                    .evolutions
                    .into_iter()
                    .map(|ev| match ev {
                        EvolutionMethod::Level { level, species } => EvolutionJson {
                            method: "level",
                            species: species_name(species),
                            level: Some(level),
                            item: None,
                            min_level: None,
                        },
                        EvolutionMethod::Item {
                            item,
                            min_level,
                            species,
                        } => EvolutionJson {
                            method: "item",
                            species: species_name(species),
                            level: None,
                            item: Some(item_name(item)),
                            min_level: Some(min_level),
                        },
                        EvolutionMethod::Trade { min_level, species } => EvolutionJson {
                            method: "trade",
                            species: species_name(species),
                            level: None,
                            item: None,
                            min_level: Some(min_level),
                        },
                    })
                    .collect();

                let learnset = e
                    .learnset
                    .into_iter()
                    .map(|m| LearnsetEntryJson {
                        level: m.level,
                        move_id: move_name(m.move_id),
                    })
                    .collect();

                (species_name(e.species), (evolutions, learnset))
            })
            .collect();

    let mut pokemon_count = 0;
    for mon in BASE_STATS.iter() {
        let name = species_name(mon.species);
        let (evolutions, learnset) = evos_lookup
            .get(&name)
            .cloned()
            .unwrap_or_else(|| (Vec::new(), Vec::new()));

        let json = PokemonJson {
            schema: "../schemas/pokemon.schema.json",
            species: name.clone(),
            base_stats: BaseStatsJson {
                hp: mon.hp,
                attack: mon.attack,
                defense: mon.defense,
                speed: mon.speed,
                special: mon.special,
            },
            type1: type_name(mon.type1),
            type2: type_name(mon.type2),
            catch_rate: mon.catch_rate,
            base_exp: mon.base_exp,
            growth_rate: growth_rate_name(mon.growth_rate),
            initial_moves: [
                move_name(mon.initial_moves[0]),
                move_name(mon.initial_moves[1]),
                move_name(mon.initial_moves[2]),
                move_name(mon.initial_moves[3]),
            ],
            tm_hm_flags: mon.tm_hm_flags,
            pokedex: {
                let dex = get_pokedex_entry(mon.species)
                    .unwrap_or_else(|| panic!("no pokedex entry for {}", name));
                PokedexJson {
                    category: dex.category.to_string(),
                    height_feet: dex.height_feet,
                    height_inches: dex.height_inches,
                    weight_decipounds: dex.weight_decipounds,
                    flavor_text_pages: dex.flavor_text_pages.iter().map(|s| s.to_string()).collect(),
                }
            },
            evolutions,
            learnset,
        };

        let path = pokemon_dir.join(format!("{}.json", name));
        write_json(&path, &json).expect("write pokemon json");
        pokemon_count += 1;
    }

    let mut move_count = 0;
    for m in MOVES.iter() {
        let name = move_name(m.id);
        let json = MoveJson {
            schema: "../schemas/move.schema.json",
            id: name.clone(),
            effect: format!("{:?}", m.effect),
            power: m.power,
            move_type: type_name(m.move_type),
            accuracy: m.accuracy,
            pp: m.pp,
        };

        let path = moves_dir.join(format!("{}.json", name));
        write_json(&path, &json).expect("write move json");
        move_count += 1;
    }

    println!(
        "Wrote {} pokemon JSON files to {}",
        pokemon_count,
        pokemon_dir.display()
    );
    println!(
        "Wrote {} move JSON files to {}",
        move_count,
        moves_dir.display()
    );
}
