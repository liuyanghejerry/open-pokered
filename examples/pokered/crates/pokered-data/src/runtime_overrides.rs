//! Runtime data overrides injected by the browser editor (WYSIWYG).
//!
//! Each data family keeps a `OnceLock<Mutex<HashMap<key, value>>>` override
//! table; the family's query functions check the table first and fall back to
//! the build-time baseline (embedded or filesystem). The release game never
//! injects, so it behaves exactly as before — this is purely an additive
//! editor channel that makes "external data wins over embedded data" possible
//! without a rebuild.
//!
//! Values that must be `&'static` (the query functions return static refs)
//! are leaked once at injection time via `Box::leak`; memory is bounded by the
//! number of injected entries (a handful per editor session), and the editor
//! replaces rather than accumulates entries.

use std::collections::HashMap;
use std::hash::Hash;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use crate::item_data::ItemData;
use crate::items::ItemId;
use crate::map_json::MapJson;
use crate::move_data::MoveData;
use crate::moves::{MoveEffect, MoveId};
use crate::pokemon_data::BaseStats;
use crate::species::{GrowthRate, Species};
use crate::trainer_data::{TrainerClass, TrainerClassData};
use crate::types::PokemonType;

fn overrides<K: Eq + Hash, T>(
    slot: &'static OnceLock<Mutex<HashMap<K, T>>>,
) -> &'static Mutex<HashMap<K, T>> {
    slot.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── Override tables ───────────────────────────────────────────────────────

/// Map directory name → overridden `map.json` (static ref leaked once).
static MAP_OVERRIDES: OnceLock<Mutex<HashMap<String, &'static MapJson>>> = OnceLock::new();
/// Map directory name → overridden `map.blk` bytes (static ref leaked once).
static BLK_OVERRIDES: OnceLock<Mutex<HashMap<String, &'static [u8]>>> = OnceLock::new();
/// Trainer class name (e.g. `"Brock"`) → overridden class data.
static TRAINER_OVERRIDES: OnceLock<Mutex<HashMap<String, &'static TrainerClassData>>> = OnceLock::new();
/// `MoveId` → overridden move (leaked once at injection; `get()` returns the
/// static ref without re-leaking on every query).
static MOVE_OVERRIDES: OnceLock<Mutex<HashMap<MoveId, &'static MoveData>>> = OnceLock::new();
/// `ItemId` → overridden item (name is `&'static str`, leaked once).
static ITEM_OVERRIDES: OnceLock<Mutex<HashMap<ItemId, &'static ItemData>>> = OnceLock::new();
/// `Species` → overridden base stats.
static POKEMON_OVERRIDES: OnceLock<Mutex<HashMap<Species, &'static BaseStats>>> = OnceLock::new();

// ── Query hooks (called by the family query functions, before the baseline) ──

pub(crate) fn map_override(name: &str) -> Option<&'static MapJson> {
    overrides(&MAP_OVERRIDES).lock().unwrap().get(name).copied()
}

pub(crate) fn blk_override(name: &str) -> Option<&'static [u8]> {
    overrides(&BLK_OVERRIDES).lock().unwrap().get(name).copied()
}

pub(crate) fn trainer_override(class: TrainerClass) -> Option<&'static TrainerClassData> {
    let name = crate::trainer_data::trainer_class_name(class);
    overrides(&TRAINER_OVERRIDES)
        .lock()
        .unwrap()
        .get(name)
        .copied()
}

pub(crate) fn move_override(id: MoveId) -> Option<&'static MoveData> {
    overrides(&MOVE_OVERRIDES).lock().unwrap().get(&id).copied()
}

pub(crate) fn item_override(id: ItemId) -> Option<&'static ItemData> {
    overrides(&ITEM_OVERRIDES).lock().unwrap().get(&id).copied()
}

pub(crate) fn base_stats_override(species: Species) -> Option<&'static BaseStats> {
    overrides(&POKEMON_OVERRIDES)
        .lock()
        .unwrap()
        .get(&species)
        .copied()
}

// ── Injection API (wasm bridge / tests) ───────────────────────────────────

fn parse_enum<T: FromStr>(name: &str) -> Option<T> {
    name.parse::<T>().ok()
}

/// `TrainerClass` has no `EnumString` derive — parse by matching its own
/// `trainer_class_name` (uppercase form) or the PascalCase variant name.
/// Accepts both `"BROCK"` and `"Brock"`.
pub fn parse_trainer_class(name: &str) -> Option<TrainerClass> {
    let up = name.to_ascii_uppercase();
    for i in 0..=TrainerClass::Lance as u8 {
        let class = TrainerClass::from_u8(i);
        if crate::trainer_data::trainer_class_name(class) == up
            || format!("{:?}", class).eq_ignore_ascii_case(name)
        {
            return Some(class);
        }
    }
    None
}

fn leak<T>(value: T) -> &'static T {
    Box::leak(Box::new(value))
}

/// Override a map's `map.json` (editor file format — camelCase, same schema
/// the `/api/maps/<name>/map.json` route serves). Returns `false` on bad JSON.
pub fn set_map_data_override(map_name: &str, json: &str) -> bool {
    let parsed: MapJson = match serde_json::from_str(json) {
        Ok(m) => m,
        Err(_) => return false,
    };
    overrides(&MAP_OVERRIDES)
        .lock()
        .unwrap()
        .insert(map_name.to_string(), leak(parsed));
    true
}

/// Override a map's `map.blk` block data. `json` is the editor's number
/// array (`[0, 0, 33, …]`, same as `/api/maps/<name>/map.blk`).
pub fn set_map_blk_override(map_name: &str, json: &str) -> bool {
    let value: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let Some(arr) = value.as_array() else {
        return false;
    };
    let mut bytes = Vec::with_capacity(arr.len());
    for v in arr {
        let Some(n) = v.as_u64() else { return false };
        bytes.push(n.min(255) as u8);
    }
    overrides(&BLK_OVERRIDES)
        .lock()
        .unwrap()
        .insert(map_name.to_string(), leak(bytes));
    true
}

/// Override a trainer class's parties. `class_name` is the uppercase or
/// PascalCase class name (`"BROCK"` / `"Brock"`); `json` is the editor's
/// trainer class data (`{"class":"Brock","parties":[{"pokemon":[{"level":12,
/// "species":"Geodude"}]}]}`).
pub fn set_trainer_override(class_name: &str, json: &str) -> bool {
    let value: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let class = value
        .get("class")
        .and_then(|c| c.as_str())
        .and_then(parse_trainer_class)
        .or_else(|| parse_trainer_class(class_name));
    let Some(class) = class else {
        return false;
    };
    let Some(parties) = parse_parties(value.get("parties")) else {
        return false;
    };
    let name = crate::trainer_data::trainer_class_name(class).to_string();
    overrides(&TRAINER_OVERRIDES)
        .lock()
        .unwrap()
        .insert(name, leak(TrainerClassData { class, parties }));
    true
}

/// Override a move's data. `json` is the editor's move file
/// (`{"id":"Absorb","effect":"DrainHpEffect","power":20,"type":"Grass",
/// "accuracy":100,"pp":20}`).
pub fn set_move_override(move_name: &str, json: &str) -> bool {
    let value: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_enum::<MoveId>(s))
        .or_else(|| parse_enum::<MoveId>(move_name));
    let Some(id) = id else {
        return false;
    };
    let effect = value
        .get("effect")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_enum::<MoveEffect>(s));
    let move_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_enum::<PokemonType>(s));
    let (Some(effect), Some(move_type)) = (effect, move_type) else {
        return false;
    };
    let num = |k: &str| value.get(k).and_then(|v| v.as_u64()).map(|n| n as u8);
    let data = MoveData {
        id,
        effect,
        power: num("power").unwrap_or(0),
        move_type,
        accuracy: num("accuracy").unwrap_or(0),
        pp: num("pp").unwrap_or(0),
    };
    overrides(&MOVE_OVERRIDES).lock().unwrap().insert(id, leak(data));
    true
}

/// Override an item's data. `json` is the editor's item file
/// (`{"id":"Potion","name":"POTION","price":300,"isKeyItem":false}`).
pub fn set_item_override(item_name: &str, json: &str) -> bool {
    let value: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_enum::<ItemId>(s))
        .or_else(|| parse_enum::<ItemId>(item_name));
    let Some(id) = id else {
        return false;
    };
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(item_name)
        .to_string();
    let data = ItemData {
        id,
        name: leak(name),
        price: value.get("price").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
        is_key_item: value.get("isKeyItem").and_then(|v| v.as_bool()).unwrap_or(false),
    };
    overrides(&ITEM_OVERRIDES).lock().unwrap().insert(id, leak(data));
    true
}

/// Override a species' base stats. `json` is the editor's pokemon file
/// (`{"species":"Pikachu","hp":35,"attack":55,"defense":40,"speed":90,
/// "special":50,"type1":"Electric","type2":"None","catchRate":190,
/// "baseExp":82,"initialMoves":["ThunderShock","Growl"],"growthRate":
/// "MediumFast","tmHmFlags":[...]}`). Move/species names accept PascalCase or
/// SCREAMING_SNAKE (see `Species::from_scene_name`).
pub fn set_base_stats_override(species_name: &str, json: &str) -> bool {
    let value: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let species = value
        .get("species")
        .and_then(|v| v.as_str())
        .and_then(Species::from_scene_name)
        .or_else(|| Species::from_scene_name(species_name));
    let Some(species) = species else {
        return false;
    };
    let ty = |k: &str| {
        value
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| parse_enum::<PokemonType>(s))
    };
    let (Some(type1), Some(type2)) = (ty("type1"), ty("type2")) else {
        return false;
    };
    let growth = value
        .get("growthRate")
        .and_then(|v| v.as_str())
        .and_then(|s| parse_enum::<GrowthRate>(s))
        .or_else(|| value.get("growth_rate").and_then(|v| v.as_str()).and_then(|s| parse_enum(s)));
    let Some(growth_rate) = growth else {
        return false;
    };
    // Editor shape nests the six stats under `baseStats`; accept both.
    let stats = value.get("baseStats").unwrap_or(&value);
    let num = |k: &str| stats.get(k).and_then(|v| v.as_u64()).map(|n| n as u8);
    let mut initial_moves = [MoveId::None; 4];
    if let Some(arr) = value.get("initialMoves").and_then(|v| v.as_array()) {
        for (i, m) in arr.iter().enumerate().take(4) {
            if let Some(name) = m.as_str() {
                if let Some(mid) = parse_enum::<MoveId>(name) {
                    initial_moves[i] = mid;
                }
            }
        }
    } else if let Some(arr) = value.get("initial_moves").and_then(|v| v.as_array()) {
        for (i, m) in arr.iter().enumerate().take(4) {
            if let Some(name) = m.as_str() {
                if let Some(mid) = parse_enum::<MoveId>(name) {
                    initial_moves[i] = mid;
                }
            }
        }
    }
    let mut tm_hm_flags = [0u8; 7];
    if let Some(arr) = value.get("tmHmFlags").and_then(|v| v.as_array()) {
        for (i, f) in arr.iter().enumerate().take(7) {
            tm_hm_flags[i] = f.as_u64().unwrap_or(0) as u8;
        }
    } else if let Some(arr) = value.get("tm_hm_flags").and_then(|v| v.as_array()) {
        for (i, f) in arr.iter().enumerate().take(7) {
            tm_hm_flags[i] = f.as_u64().unwrap_or(0) as u8;
        }
    }
    let data = BaseStats {
        species,
        hp: num("hp").unwrap_or(1),
        attack: num("attack").unwrap_or(0),
        defense: num("defense").unwrap_or(0),
        speed: num("speed").unwrap_or(0),
        special: num("special").unwrap_or(0),
        type1,
        type2,
        catch_rate: value.get("catchRate").and_then(|v| v.as_u64()).unwrap_or(3) as u8,
        base_exp: value.get("baseExp").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
        initial_moves,
        growth_rate,
        tm_hm_flags,
    };
    overrides(&POKEMON_OVERRIDES)
        .lock()
        .unwrap()
        .insert(species, leak(data));
    true
}

/// Drop every injected override (used when the editor discards its session).
pub fn clear_data_overrides() {
    overrides(&MAP_OVERRIDES).lock().unwrap().clear();
    overrides(&BLK_OVERRIDES).lock().unwrap().clear();
    overrides(&TRAINER_OVERRIDES).lock().unwrap().clear();
    overrides(&MOVE_OVERRIDES).lock().unwrap().clear();
    overrides(&ITEM_OVERRIDES).lock().unwrap().clear();
    overrides(&POKEMON_OVERRIDES).lock().unwrap().clear();
}

/// Whether any runtime override is currently active.
pub fn has_data_overrides() -> bool {
    !overrides(&MAP_OVERRIDES).lock().unwrap().is_empty()
        || !overrides(&BLK_OVERRIDES).lock().unwrap().is_empty()
        || !overrides(&TRAINER_OVERRIDES).lock().unwrap().is_empty()
        || !overrides(&MOVE_OVERRIDES).lock().unwrap().is_empty()
        || !overrides(&ITEM_OVERRIDES).lock().unwrap().is_empty()
        || !overrides(&POKEMON_OVERRIDES).lock().unwrap().is_empty()
}

// ── Parsing helpers ───────────────────────────────────────────────────────

fn parse_parties(value: Option<&Value>) -> Option<Vec<crate::trainer_data::TrainerParty>> {
    let arr = value?.as_array()?;
    let mut parties = Vec::with_capacity(arr.len());
    for p in arr {
        let pokemon_arr = p.get("pokemon")?.as_array()?;
        let mut pokemon = Vec::with_capacity(pokemon_arr.len());
        for m in pokemon_arr {
            let level = m.get("level")?.as_u64()? as u8;
            let species = Species::from_scene_name(m.get("species")?.as_str()?)?;
            pokemon.push(crate::trainer_data::TrainerMon { level, species });
        }
        parties.push(crate::trainer_data::TrainerParty { pokemon });
    }
    Some(parties)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::MapId;

    /// The override tables are process-global, so tests touching them must run
    /// serialized against each other (Rust's default test threads would
    /// interleave `clear_data_overrides` calls).
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap()
    }

    #[test]
    fn move_override_shadows_baseline() {
        let _g = lock();
        let base = MoveData::get(MoveId::Pound).unwrap().clone();
        assert!(set_move_override(
            "Pound",
            r#"{"id":"Pound","effect":"NoAdditionalEffect","power":99,"type":"Normal","accuracy":100,"pp":35}"#
        ));
        let ov = MoveData::get(MoveId::Pound).unwrap();
        assert_eq!(ov.power, 99);
        // Other moves unaffected.
        assert_ne!(MoveData::get(MoveId::KarateChop).unwrap().power, 99);
        clear_data_overrides();
        assert_eq!(MoveData::get(MoveId::Pound).unwrap().power, base.power);
    }

    #[test]
    fn move_override_rejects_bad_json() {
        let _g = lock();
        assert!(!set_move_override("Pound", "not json"));
        assert!(!set_move_override("Pound", r#"{"id":"NoSuchMove"}"#));
    }

    #[test]
    fn item_override_round_trips() {
        let _g = lock();
        let base = crate::item_data::get_item_data(ItemId::Potion).unwrap();
        let base_price = base.price;
        assert!(set_item_override(
            "Potion",
            r#"{"id":"Potion","name":"POTION","price":999,"isKeyItem":false}"#
        ));
        let ov = crate::item_data::get_item_data(ItemId::Potion).unwrap();
        assert_eq!(ov.price, 999);
        assert_eq!(ov.name, "POTION");
        clear_data_overrides();
        assert_eq!(crate::item_data::get_item_data(ItemId::Potion).unwrap().price, base_price);
    }

    #[test]
    fn base_stats_override_round_trips() {
        let _g = lock();
        let base = crate::pokemon_data::get_base_stats(Species::Pikachu).unwrap();
        let base_hp = base.hp;
        assert!(set_base_stats_override(
            "Pikachu",
            r#"{"species":"Pikachu","baseStats":{"hp":77,"attack":55,"defense":40,"speed":90,"special":50},"type1":"Electric","type2":"Electric","catchRate":190,"baseExp":82,"initialMoves":["Thundershock","Growl","None","None"],"growthRate":"MediumFast","tmHmFlags":[0,0,0,0,0,0,0]}"#
        ));
        let ov = crate::pokemon_data::get_base_stats(Species::Pikachu).unwrap();
        assert_eq!(ov.hp, 77);
        assert_eq!(ov.initial_moves[0], MoveId::Thundershock);
        clear_data_overrides();
        assert_eq!(crate::pokemon_data::get_base_stats(Species::Pikachu).unwrap().hp, base_hp);
    }

    #[test]
    fn trainer_override_round_trips() {
        let _g = lock();
        let base = crate::trainer_data::get_trainer_party(TrainerClass::Brock, 0).unwrap();
        let base_len = base.pokemon.len();
        assert!(set_trainer_override(
            "Brock",
            r#"{"class":"Brock","parties":[{"pokemon":[{"level":12,"species":"Geodude"},{"level":14,"species":"Onix"}]}]}"#
        ));
        let ov = crate::trainer_data::get_trainer_party(TrainerClass::Brock, 0).unwrap();
        assert_eq!(ov.pokemon.len(), 2);
        assert_eq!(ov.pokemon[0].species, Species::Geodude);
        clear_data_overrides();
        assert_eq!(crate::trainer_data::get_trainer_party(TrainerClass::Brock, 0).unwrap().pokemon.len(), base_len);
    }

    #[test]
    fn map_override_round_trips() {
        let _g = lock();
        // Read the real Route1 map.json at compile time — the exact editor
        // file shape the injector must accept.
        let json = include_str!("../maps/Route1/map.json");
        assert!(set_map_data_override("Route1", json));
        let ov = crate::map_data_loader::get_map_json(MapId::Route1).unwrap();
        assert_eq!(ov.header.width, 10);
        assert_eq!(ov.name, "Route1");
        clear_data_overrides();
    }

    #[test]
    fn blk_override_round_trips() {
        let _g = lock();
        assert!(set_map_blk_override("Route1", "[1,2,3,255]"));
        let ov = crate::map_data_loader::get_block_data(MapId::Route1);
        assert_eq!(ov, &[1, 2, 3, 255]);
        clear_data_overrides();
    }
}
