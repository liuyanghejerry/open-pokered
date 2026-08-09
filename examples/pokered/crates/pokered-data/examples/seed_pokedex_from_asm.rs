//! One-shot bootstrap: parses `data/pokemon/dex_entries.asm` and
//! `data/pokemon/dex_text.asm` (at the repo root) and merges the resulting
//! `pokedex` block into the existing `pokemon/{Species}.json` files.
//!
//! Run: `cargo run --example seed_pokedex_from_asm -p pokered-data`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PokemonJson {
    #[serde(rename = "$schema")]
    schema: String,
    species: String,
    base_stats: BaseStatsJson,
    type1: String,
    type2: String,
    catch_rate: u16,
    base_exp: u16,
    growth_rate: String,
    initial_moves: [String; 4],
    tm_hm_flags: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pokedex: Option<PokedexJson>,
    evolutions: Vec<EvolutionJson>,
    learnset: Vec<LearnsetEntryJson>,
}

#[derive(Serialize, Deserialize)]
struct BaseStatsJson {
    hp: u8,
    attack: u8,
    defense: u8,
    speed: u8,
    special: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvolutionJson {
    method: String,
    species: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_level: Option<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LearnsetEntryJson {
    level: u8,
    move_id: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PokedexJson {
    category: String,
    height_feet: u8,
    height_inches: u8,
    weight_decipounds: u16,
    flavor_text_pages: Vec<String>,
}

#[derive(Default, Debug)]
struct DexEntryAsm {
    category: String,
    height_feet: u8,
    height_inches: u8,
    weight_decipounds: u16,
}

fn parse_dex_entries(path: &Path) -> HashMap<String, DexEntryAsm> {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let mut out: HashMap<String, DexEntryAsm> = HashMap::new();
    let mut current: Option<(String, DexEntryAsm)> = None;

    for line in raw.lines() {
        let trimmed = line.trim();

        if let Some(name) = trimmed.strip_suffix("DexEntry:") {
            if !name.is_empty() && name.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
                if let Some((n, e)) = current.take() {
                    out.insert(n, e);
                }
                current = Some((name.to_string(), DexEntryAsm::default()));
            }
            continue;
        }

        let Some((_, ref mut entry)) = current else { continue };

        if let Some(rest) = trimmed.strip_prefix("db \"") {
            if let Some(end) = rest.find("@\"") {
                entry.category = rest[..end].to_string();
            }
        } else if let Some(rest) = trimmed.strip_prefix("db ") {
            let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
            if parts.len() == 2 {
                entry.height_feet = parts[0].parse().unwrap_or(0);
                entry.height_inches = parts[1].parse().unwrap_or(0);
            }
        } else if let Some(rest) = trimmed.strip_prefix("dw ") {
            entry.weight_decipounds = rest.trim().parse().unwrap_or(0);
        }
    }

    if let Some((n, e)) = current {
        out.insert(n, e);
    }

    out
}

fn parse_dex_text(path: &Path) -> HashMap<String, Vec<String>> {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_species: Option<String> = None;
    let mut current_pages: Vec<Vec<String>> = Vec::new();

    fn finalize(
        out: &mut HashMap<String, Vec<String>>,
        species: &mut Option<String>,
        pages: &mut Vec<Vec<String>>,
    ) {
        if let Some(name) = species.take() {
            let joined: Vec<String> =
                pages.drain(..).map(|lines| lines.join("\n")).collect();
            out.insert(name, joined);
        } else {
            pages.clear();
        }
    }

    for line in raw.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix('_') {
            if let Some(name) = rest.strip_suffix("DexEntry::") {
                finalize(&mut out, &mut current_species, &mut current_pages);
                current_species = Some(name.to_string());
                current_pages = vec![Vec::new()];
                continue;
            }
        }

        if trimmed == "dex" {
            finalize(&mut out, &mut current_species, &mut current_pages);
            continue;
        }

        let payload = if let Some(rest) = trimmed.strip_prefix("text ") {
            Some(rest)
        } else if let Some(rest) = trimmed.strip_prefix("next ") {
            Some(rest)
        } else if let Some(rest) = trimmed.strip_prefix("page ") {
            if let Some(pages_ref) = current_pages.last_mut() {
                if pages_ref.is_empty() {
                    pages_ref.push(String::new());
                }
            }
            current_pages.push(Vec::new());
            Some(rest)
        } else {
            None
        };

        let Some(payload) = payload else { continue };
        let text = payload.trim();
        let unquoted = text
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(text);

        if let Some(page) = current_pages.last_mut() {
            page.push(unquoted.to_string());
        }
    }

    out
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("locate repo root from manifest_dir");

    let dex_entries_path = repo_root.join("data/pokemon/dex_entries.asm");
    let dex_text_path = repo_root.join("data/pokemon/dex_text.asm");
    let pokemon_dir = manifest_dir.join("pokemon");

    assert!(
        dex_entries_path.exists(),
        "missing {}",
        dex_entries_path.display()
    );
    assert!(
        dex_text_path.exists(),
        "missing {}",
        dex_text_path.display()
    );
    assert!(pokemon_dir.exists(), "missing {}", pokemon_dir.display());

    let entries = parse_dex_entries(&dex_entries_path);
    let texts = parse_dex_text(&dex_text_path);

    let json_files: Vec<_> = fs::read_dir(&pokemon_dir)
        .expect("read pokemon dir")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().map_or(false, |x| x == "json"))
        .collect();

    let mut merged = 0;
    let mut missing_entry = Vec::new();
    let mut missing_text = Vec::new();

    for dirent in json_files {
        let path = dirent.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("utf-8 filename")
            .to_string();

        let raw = fs::read_to_string(&path).expect("read pokemon json");
        let mut pkm: PokemonJson =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

        let Some(entry) = entries.get(&stem) else {
            missing_entry.push(stem.clone());
            continue;
        };
        let Some(pages) = texts.get(&stem) else {
            missing_text.push(stem.clone());
            continue;
        };

        pkm.pokedex = Some(PokedexJson {
            category: entry.category.clone(),
            height_feet: entry.height_feet,
            height_inches: entry.height_inches,
            weight_decipounds: entry.weight_decipounds,
            flavor_text_pages: pages.clone(),
        });

        let mut out = serde_json::to_string_pretty(&pkm).expect("serialize");
        out.push('\n');
        fs::write(&path, out).expect("write");
        merged += 1;
    }

    println!("Merged pokedex into {} files.", merged);
    if !missing_entry.is_empty() {
        println!("Missing dex_entries: {:?}", missing_entry);
    }
    if !missing_text.is_empty() {
        println!("Missing dex_text: {:?}", missing_text);
    }
}
