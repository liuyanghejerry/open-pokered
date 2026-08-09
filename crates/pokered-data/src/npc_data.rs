//! NPC data — JSON-backed NPC definitions for all 248 maps.
//!
//! Type definitions (`NpcEntry`, `NpcFacing`, `NpcMovement`) are preserved
//! for compatibility with existing consumers. Data is loaded at runtime from
//! `map.json` files via `map_data_loader::get_map_json()`.

use std::sync::OnceLock;

use crate::map_data_loader::get_map_json;
use crate::map_json::NpcJson;
use crate::maps::MapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcMovement(pub u8);

impl NpcMovement {
    pub const STATIONARY: Self = Self(0);
    pub const WANDER: Self = Self(1);
    pub const FIXED_PATH: Self = Self(2);
    pub const FACE_PLAYER: Self = Self(3);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcFacing(pub u8);

impl NpcFacing {
    pub const DOWN: Self = Self(0);
    pub const UP: Self = Self(1);
    pub const LEFT: Self = Self(2);
    pub const RIGHT: Self = Self(3);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcEntry {
    pub sprite_id: u8,
    pub x: u8,
    pub y: u8,
    pub movement: NpcMovement,
    pub facing: NpcFacing,
    pub range: u8,
    pub text_id: u8,
    pub is_trainer: bool,
    pub trainer_class: u8,
    pub trainer_set: u8,
    pub item_id: u8,
}

// ── String → type converters ───────────────────────────────────────

fn parse_movement(s: &str) -> NpcMovement {
    match s {
        "Stationary" => NpcMovement::STATIONARY,
        "Wander" => NpcMovement::WANDER,
        "FixedPath" => NpcMovement::FIXED_PATH,
        "FacePlayer" => NpcMovement::FACE_PLAYER,
        _ => NpcMovement::STATIONARY,
    }
}

fn parse_facing(s: &str) -> NpcFacing {
    match s {
        "Down" => NpcFacing::DOWN,
        "Up" => NpcFacing::UP,
        "Left" => NpcFacing::LEFT,
        "Right" => NpcFacing::RIGHT,
        _ => NpcFacing::DOWN,
    }
}

fn parse_trainer_class(name: &str) -> u8 {
    match name {
        "Nobody" => 0,
        "Youngster" => 1,
        "BugCatcher" => 2,
        "Lass" => 3,
        "Sailor" => 4,
        "JrTrainerM" => 5,
        "JrTrainerF" => 6,
        "Pokemaniac" => 7,
        "SuperNerd" => 8,
        "Hiker" => 9,
        "Biker" => 10,
        "Burglar" => 11,
        "Engineer" => 12,
        "UnusedJuggler" => 13,
        "Fisher" => 14,
        "Swimmer" => 15,
        "CueBall" => 16,
        "Gambler" => 17,
        "Beauty" => 18,
        "Psychic" | "PsychicTr" => 19,
        "Rocker" => 20,
        "Juggler" => 21,
        "Tamer" => 22,
        "BirdKeeper" => 23,
        "Blackbelt" => 24,
        "Rival1" => 25,
        "ProfOak" => 26,
        "Chief" => 27,
        "Scientist" => 28,
        "Giovanni" => 29,
        "Rocket" => 30,
        "CooltrainerM" => 31,
        "CooltrainerF" => 32,
        "Bruno" => 33,
        "Brock" => 34,
        "Misty" => 35,
        "LtSurge" => 36,
        "Erika" => 37,
        "Koga" => 38,
        "Blaine" => 39,
        "Sabrina" => 40,
        "Gentleman" => 41,
        "Rival2" => 42,
        "Rival3" => 43,
        "Lorelei" => 44,
        "Channeler" => 45,
        "Agatha" => 46,
        "Lance" => 47,
        _ => 0,
    }
}

fn convert_npc(npc: &NpcJson) -> NpcEntry {
    NpcEntry {
        sprite_id: npc.sprite_id,
        x: npc.x,
        y: npc.y,
        movement: parse_movement(&npc.movement),
        facing: parse_facing(&npc.facing),
        range: npc.range,
        text_id: npc.text_id,
        is_trainer: npc.is_trainer,
        trainer_class: npc
            .trainer_class
            .as_ref()
            .map(|name| parse_trainer_class(name))
            .unwrap_or(0),
        trainer_set: npc.trainer_set.unwrap_or(0),
        item_id: npc.item_id.unwrap_or(0),
    }
}

// ── Cached NPC data per map ────────────────────────────────────────

struct NpcDataCache {
    /// One Vec<NpcEntry> per map, indexed by MapId as usize.
    entries: Vec<Vec<NpcEntry>>,
}

static NPC_CACHE: OnceLock<NpcDataCache> = OnceLock::new();

fn get_cache() -> &'static NpcDataCache {
    NPC_CACHE.get_or_init(|| {
        let mut entries = Vec::with_capacity(248);
        for i in 0..248u8 {
            if let Some(map_id) = MapId::from_u8(i) {
                let npcs = get_map_json(map_id)
                    .map(|json| json.npcs.iter().map(convert_npc).collect::<Vec<_>>())
                    .unwrap_or_default();
                entries.push(npcs);
            } else {
                entries.push(Vec::new());
            }
        }
        NpcDataCache { entries }
    })
}

pub fn get_map_npcs(map: MapId) -> &'static [NpcEntry] {
    let idx = map as usize;
    let cache = get_cache();
    if idx < cache.entries.len() {
        &cache.entries[idx]
    } else {
        &[]
    }
}
