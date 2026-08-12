//! Editor save-snapshot → in-game `SaveData` conversion.
//!
//! The Save Editor's "▶ 用此存档试玩" quick entry hands the floating playtest
//! a snapshot in the editor's simplified JSON shape (`SaveDataSnapshot` in
//! `src/types/save-data.ts`); this module converts it into a real
//! [`SaveData`] the game can boot with. Fields the editor doesn't model stay
//! at their defaults. Unknown species/moves/items are skipped with a warning
//! instead of failing the whole snapshot.

use std::collections::HashMap;
use std::str::FromStr;

use pokered_core::items::inventory::Inventory;
use pokered_core::pokemon::stats::{create_pokemon, create_pokemon_with_moves};
use pokered_core::save::game_data::{MapPosition, PlayTime};
use pokered_core::save::SaveData;
use pokered_data::charmap;
use pokered_data::items::ItemId;
use pokered_data::maps::NUM_MAPS;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use serde::Deserialize;

/// The Save Editor snapshot (`SaveDataSnapshot` in save-data.ts).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSaveSnapshot {
    pub player: EditorPlayerInfo,
    #[serde(default)]
    pub badges: Vec<bool>,
    #[serde(default)]
    pub party: Vec<EditorPartyMon>,
    #[serde(default)]
    pub items: Vec<EditorItem>,
    #[serde(default)]
    pub flags: HashMap<String, bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPlayerInfo {
    pub player_name: String,
    pub rival_name: String,
    pub map_name: String,
    pub position_x: u8,
    pub position_y: u8,
    pub play_time_hours: u8,
    pub play_time_minutes: u8,
    pub money: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPartyMon {
    pub species: String,
    pub level: u8,
    #[serde(default)]
    pub current_hp: u16,
    #[serde(default)]
    pub max_hp: u16,
    #[serde(default)]
    pub moves: Vec<String>,
    #[serde(default)]
    pub nickname: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorItem {
    pub name: String,
    pub quantity: u8,
}

/// Convert an editor save snapshot into a bootable [`SaveData`]. Unknown
/// species/moves/items are skipped (logged); an unresolvable map name is the
/// only hard error.
pub fn apply_editor_save(snapshot: &EditorSaveSnapshot) -> Result<SaveData, String> {
    let mut save = SaveData::new();

    save.player_name = charmap::encode_string(&snapshot.player.player_name)
        .unwrap_or_default();
    save.game_data.rival_name =
        charmap::encode_string(&snapshot.player.rival_name).unwrap_or_default();
    save.game_data.player_money = snapshot.player.money;

    let mut badges = 0u8;
    for (i, on) in snapshot.badges.iter().take(8).enumerate() {
        if *on {
            badges |= 1 << i;
        }
    }
    save.game_data.obtained_badges = badges;

    let map_id = resolve_map_id(&snapshot.player.map_name)
        .ok_or_else(|| format!("unknown map '{}'", snapshot.player.map_name))?;
    save.game_data.position = MapPosition {
        map_id: map_id as u8,
        x: snapshot.player.position_x,
        y: snapshot.player.position_y,
        ..MapPosition::new()
    };
    save.game_data.play_time = PlayTime {
        hours: snapshot.player.play_time_hours,
        minutes: snapshot.player.play_time_minutes,
        maxed: false,
        seconds: 0,
        frames: 0,
    };

    for mon in &snapshot.party {
        let Some(species) = Species::from_scene_name(&mon.species) else {
            log::warn!("[save_editor] skipping unknown species '{}'", mon.species);
            continue;
        };
        let dv = [0xFF, 0xFF];
        let mut pkmn = if mon.moves.is_empty() {
            create_pokemon(species, mon.level, dv)
        } else {
            create_pokemon_with_moves(species, mon.level, dv, parse_moves(&mon.moves))
        };
        if let Some(p) = pkmn.as_mut() {
            p.hp = mon.current_hp.min(p.max_hp);
            if mon.nickname.is_empty() {
                p.clear_nickname();
            } else {
                p.set_nickname(&mon.nickname);
            }
        }
        if let Some(p) = pkmn {
            let _ = save.party.add(p);
        }
    }

    for item in &snapshot.items {
        match parse_item(&item.name) {
            Some(id) => {
                let _ = save.game_data.bag.add_item(id, item.quantity);
            }
            None => {
                log::warn!("[save_editor] skipping unknown item '{}'", item.name);
            }
        }
    }

    // Editor flags are event flags (named or __RAW_BIT_n): route them into
    // the fixed event-flag bitset that serializes into the SRAM region.
    // Runtime-only keys with no bit representation cannot be stored in the
    // save file and are dropped.
    if !snapshot.flags.is_empty() {
        let mut ef = pokered_core::overworld::event_flags::EventFlags::new();
        ef.merge_from(&snapshot.flags);
        save.game_data.event_flags = ef.as_bytes().to_vec();
    }
    Ok(save)
}

/// Parse the editor's move-name list into a 4-slot moveset; unknown or empty
/// names become `MoveId::None` (the original's empty slot).
fn parse_moves(names: &[String]) -> [MoveId; 4] {
    let mut out = [MoveId::None; 4];
    for (slot, name) in names.iter().take(4).enumerate() {
        out[slot] = parse_move(name).unwrap_or(MoveId::None);
    }
    out
}

/// Tolerant move-name parse: exact PascalCase variant ("Tackle"), the
/// all-caps display form ("TACKLE"), or any case variant of the variant name.
fn parse_move(name: &str) -> Option<MoveId> {
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        return Some(MoveId::None);
    }
    MoveId::from_str(name)
        .ok()
        .or_else(|| {
            (0..=pokered_data::moves::NUM_MOVES)
                .map(MoveId::from_id)
                .find(|m| format!("{:?}", m).eq_ignore_ascii_case(name))
        })
}

/// Tolerant item-name parse: PascalCase variant ("PokeBall"), SCREAMING_SNAKE
/// const name ("POKE_BALL"), or the spaced display form ("POKE BALL").
fn parse_item(name: &str) -> Option<ItemId> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    ItemId::from_str(name).ok().or_else(|| {
        (0..=pokered_data::items::NUM_ITEMS)
            .map(ItemId::from_id)
            .find(|i| {
                format!("{:?}", i).eq_ignore_ascii_case(name)
                    || i.const_name().eq_ignore_ascii_case(&name.replace(' ', "_"))
            })
    })
}

/// Resolve a map directory/debug name (`"PalletTown"`) to a [`MapId`] —
/// mirrors `pokered_app::parse_warp_arg`'s lookup.
fn resolve_map_id(name: &str) -> Option<pokered_core::data::maps::MapId> {
    for i in 0..NUM_MAPS {
        if let Some(m) = pokered_core::data::maps::MapId::from_u8(i as u8) {
            if format!("{:?}", m) == name {
                return Some(m);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EditorSaveSnapshot {
        serde_json::from_str(
            r#"{
                "player": {
                    "playerName": "RED",
                    "rivalName": "BLUE",
                    "mapName": "PalletTown",
                    "positionX": 10,
                    "positionY": 8,
                    "playTimeHours": 3,
                    "playTimeMinutes": 12,
                    "money": 5000
                },
                "badges": [true, false, true],
                "party": [
                    {
                        "species": "Pikachu",
                        "level": 12,
                        "currentHp": 30,
                        "maxHp": 40,
                        "moves": ["ThunderShock", "QuickAttack"],
                        "nickname": "Sparky"
                    }
                ],
                "items": [{ "name": "Potion", "quantity": 5 }],
                "flags": { "EVENT_GOT_POKEDEX": true }
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn applies_snapshot_fields() {
        let save = apply_editor_save(&sample()).unwrap();
        assert_eq!(save.game_data.player_money, 5000);
        assert_eq!(save.game_data.obtained_badges, 0b101);
        assert_eq!(
            save.game_data.position.map_id,
            pokered_core::data::maps::MapId::PalletTown as u8
        );
        assert_eq!(save.game_data.position.x, 10);
        assert_eq!(save.game_data.play_time.hours, 3);
        assert_eq!(save.party.count(), 1);
        let mon = save.party.leader().unwrap();
        assert_eq!(mon.species, Species::Pikachu);
        assert_eq!(mon.level, 12);
        assert_eq!(mon.hp, 30);
        let mut name_buf = [0u8; pokered_core::battle::state::NAME_TEXT_BUF];
        assert_eq!(pokered_core::battle::state::decode_name(&mon.nickname, &mut name_buf), "Sparky");
        assert_eq!(mon.moves[0], MoveId::Thundershock);
        assert_eq!(mon.moves[1], MoveId::QuickAttack);
        assert!(save.game_data.bag.has_item(ItemId::Potion, 5));
        assert!(
            pokered_core::overworld::event_flags::EventFlags::from_event_bytes(&save.game_data.event_flags)
                .get_flag("EVENT_GOT_POKEDEX")
        );
        assert!(!save.player_name.is_empty());
    }

    #[test]
    fn skips_unknown_records() {
        let json = r#"{
            "player": { "playerName": "RED", "rivalName": "BLUE", "mapName": "PalletTown",
                        "positionX": 0, "positionY": 0, "playTimeHours": 0,
                        "playTimeMinutes": 0, "money": 0 },
            "party": [
                { "species": "NopeMon", "level": 5, "moves": [] },
                { "species": "Bulbasaur", "level": 5, "moves": ["NopeMove"] }
            ],
            "items": [{ "name": "NopeItem", "quantity": 1 }]
        }"#;
        let snap: EditorSaveSnapshot = serde_json::from_str(json).unwrap();
        let save = apply_editor_save(&snap).unwrap();
        assert_eq!(save.party.count(), 1);
        assert_eq!(save.party.leader().unwrap().species, Species::Bulbasaur);
        assert!(save.game_data.bag.is_empty());
    }

    #[test]
    fn rejects_unknown_map() {
        let mut snap = sample();
        snap.player.map_name = "NopeTown".to_string();
        assert!(apply_editor_save(&snap).is_err());
    }
}
