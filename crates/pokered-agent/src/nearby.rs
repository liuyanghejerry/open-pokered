//! Nearby-entity aggregation: NPCs, trainers, item balls, signs, warps,
//! and hidden items around the player, sorted by Manhattan distance.
//!
//! All coordinates are step units (1 step = 2 GB tiles = half a block);
//! player, NPC, warp, sign, and hidden-item sources share that space, so
//! no conversion is applied. No raycast/reachability in v1 — the
//! `interactable` flag is a distance-and-state heuristic only.

use pokered_core::overworld::npc_movement::NpcRuntimeState;
use pokered_data::hidden_items::HIDDEN_ITEMS;
use pokered_data::items::ItemId;
use pokered_data::map_json::MapJson;
use pokered_data::maps::MapId;
use serde::{Deserialize, Serialize};

/// Default radius (step units) for nearby queries when none is supplied.
pub const DEFAULT_NEARBY_RADIUS: i32 = 10;
/// Interaction reach (step units): the faced tile is 2 steps from the
/// player, so `distance <= INTERACT_REACH` reads as "adjacent".
pub const INTERACT_REACH: i32 = 2;

/// What a nearby entity is, semantically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Npc,
    Trainer,
    /// An item ball on the ground (a static NPC entry whose sprite is
    /// `PokeBall` and which carries an `item_id`).
    Item,
    Sign,
    Warp,
    HiddenItem,
    /// Reserved for kinds v1 does not classify yet.
    Other,
}

/// A position in step units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    /// Manhattan distance between two positions, in step units.
    pub fn manhattan(self, other: Position) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

/// The slice of runtime NPC state nearby aggregation needs. Extracted
/// from [`NpcRuntimeState`] so the observation layer depends on a
/// minimal, stable shape.
#[derive(Debug, Clone, Copy)]
pub struct NpcObs {
    pub npc_index: u8,
    pub x: u16,
    pub y: u16,
    pub visible: bool,
    pub defeated: bool,
}

impl From<&NpcRuntimeState> for NpcObs {
    fn from(npc: &NpcRuntimeState) -> Self {
        Self {
            npc_index: npc.npc_index,
            x: npc.x,
            y: npc.y,
            visible: npc.visible,
            defeated: npc.defeated,
        }
    }
}

/// A hidden item spawn on the current map plus its obtained state.
#[derive(Debug, Clone)]
pub struct HiddenItemSpot {
    /// Index into the original `HiddenItemCoords` table (the flag bit index).
    pub table_index: usize,
    pub x: u8,
    pub y: u8,
    /// Item variant name (Debug form, e.g. "Potion").
    pub item: String,
    pub obtained: bool,
}

/// Hidden items on `map`, decoded against the obtained-flags bitset
/// (save `obtained_hidden_items` layout: bit `i % 8` of byte `i / 8`).
pub fn hidden_item_spots(map: MapId, obtained_flags: &[u8]) -> Vec<HiddenItemSpot> {
    HIDDEN_ITEMS
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.map == map)
        .map(|(index, entry)| HiddenItemSpot {
            table_index: index,
            x: entry.x,
            y: entry.y,
            item: format!("{:?}", entry.item),
            obtained: obtained_flags
                .get(index / 8)
                .is_some_and(|byte| byte & (1 << (index % 8)) != 0),
        })
        .collect()
}

/// One entity near the player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyEntity {
    /// Stable id: `npc:{index}`, `sign:{index}`, `warp:{index}`,
    /// `hidden:{table_index}`.
    pub id: String,
    pub kind: EntityKind,
    pub position: Position,
    /// Manhattan distance from the player, in step units.
    pub distance: i32,
    /// Heuristic only (no reachability): NPCs/trainers/items need to be
    /// visible, undefeated, and adjacent (`distance <= 2`); signs need to
    /// be adjacent; warps are usable "on tile" (`distance == 0`); hidden
    /// items must be unobtained and adjacent.
    pub interactable: bool,
    /// Human/agent-readable label when known: NPC sprite name, trainer
    /// class, item name, warp destination map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Aggregate every entity within `radius` steps of `player`, nearest
/// first (ties keep source order: NPCs, signs, warps, hidden items).
pub fn nearby_entities(
    player: Position,
    npcs: &[NpcObs],
    map: Option<&MapJson>,
    hidden: &[HiddenItemSpot],
    radius: i32,
) -> Vec<NearbyEntity> {
    let mut entities = Vec::new();

    // NPCs live at runtime positions; the static map entry (same index)
    // refines the kind: trainer flag, or PokeBall-with-item → ground item.
    for npc in npcs {
        let position = Position {
            x: npc.x as i32,
            y: npc.y as i32,
        };
        let distance = player.manhattan(position);
        let static_npc = map.and_then(|m| m.npcs.get(npc.npc_index as usize));
        let (kind, name) = match static_npc {
            Some(def) if def.is_trainer => (
                EntityKind::Trainer,
                def.trainer_class.clone().or_else(|| def.sprite_name.clone()),
            ),
            Some(def)
                if def.sprite_name.as_deref() == Some("PokeBall") && def.item_id.is_some() =>
            {
                (
                    EntityKind::Item,
                    def.item_id.map(|id| format!("{:?}", ItemId::from_id(id))),
                )
            }
            // `trainer_class` when the map data carries one, even if
            // `is_trainer` is false. Brock's entry is `spriteName:
            // "SuperNerd"` with `trainerClass: "Brock"` — Gen 1 reuses
            // sprites — so labelling by sprite alone leaves a gym leader
            // with no name an agent could match a goal against.
            // `is_trainer` marks trainers who challenge on sight, which
            // is a different question from what to call them.
            Some(def) => (
                EntityKind::Npc,
                def.trainer_class.clone().or_else(|| def.sprite_name.clone()),
            ),
            None => (EntityKind::Npc, None),
        };
        entities.push(NearbyEntity {
            id: format!("npc:{}", npc.npc_index),
            kind,
            position,
            distance,
            interactable: npc.visible && !npc.defeated && distance <= INTERACT_REACH,
            name,
        });
    }

    if let Some(map) = map {
        for (index, sign) in map.signs.iter().enumerate() {
            let position = Position {
                x: sign.x as i32,
                y: sign.y as i32,
            };
            let distance = player.manhattan(position);
            entities.push(NearbyEntity {
                id: format!("sign:{index}"),
                kind: EntityKind::Sign,
                position,
                distance,
                interactable: distance <= INTERACT_REACH,
                name: None,
            });
        }
        for (index, warp) in map.warps.iter().enumerate() {
            let position = Position {
                x: warp.x as i32,
                y: warp.y as i32,
            };
            let distance = player.manhattan(position);
            entities.push(NearbyEntity {
                id: format!("warp:{index}"),
                kind: EntityKind::Warp,
                position,
                distance,
                interactable: distance == 0,
                name: warp.dest_map.clone(),
            });
        }
    }

    for spot in hidden {
        let position = Position {
            x: spot.x as i32,
            y: spot.y as i32,
        };
        let distance = player.manhattan(position);
        entities.push(NearbyEntity {
            id: format!("hidden:{}", spot.table_index),
            kind: EntityKind::HiddenItem,
            position,
            distance,
            interactable: !spot.obtained && distance <= INTERACT_REACH,
            name: Some(spot.item.clone()),
        });
    }

    entities.retain(|e| e.distance <= radius);
    entities.sort_by_key(|e| e.distance);
    entities
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::map_json::{MapHeaderJson, NpcJson, SignJson, WarpJson};

    fn npc_json(sprite_name: &str, x: u8, y: u8, is_trainer: bool, item_id: Option<u8>) -> NpcJson {
        NpcJson {
            sprite_id: 1,
            sprite_name: Some(sprite_name.to_string()),
            x,
            y,
            movement: "Stationary".to_string(),
            facing: "Down".to_string(),
            range: 0,
            text_id: 1,
            is_trainer,
            trainer_class: is_trainer.then(|| "Youngster".to_string()),
            trainer_set: None,
            item_id,
            end_battle_text: None,
        }
    }

    fn map_json(npcs: Vec<NpcJson>) -> MapJson {
        MapJson {
            id: 0,
            name: "TestMap".to_string(),
            header: MapHeaderJson {
                tileset: "Overworld".to_string(),
                music: "PalletTown".to_string(),
                connection_flags: 0,
                width: 10,
                height: 9,
                border_block: 0,
            },
            connections: Default::default(),
            warps: vec![
                WarpJson { x: 5, y: 5, dest_map: Some("DestA".to_string()), dest_warp_id: 0 },
                WarpJson { x: 12, y: 11, dest_map: Some("DestB".to_string()), dest_warp_id: 1 },
            ],
            npcs,
            signs: vec![SignJson { x: 7, y: 9, text_id: 2 }],
            text: Default::default(),
            wild: None,
        }
    }

    fn npc_obs(index: u8, x: u16, y: u16, visible: bool, defeated: bool) -> NpcObs {
        NpcObs {
            npc_index: index,
            x,
            y,
            visible,
            defeated,
        }
    }

    fn spot(index: usize, x: u8, y: u8, obtained: bool) -> HiddenItemSpot {
        HiddenItemSpot {
            table_index: index,
            x,
            y,
            item: "Potion".to_string(),
            obtained,
        }
    }

    #[test]
    fn manhattan_distance_is_step_units() {
        let a = Position { x: 10, y: 9 };
        assert_eq!(a.manhattan(Position { x: 8, y: 5 }), 6);
        assert_eq!(a.manhattan(a), 0);
    }

    #[test]
    fn npc_kinds_come_from_static_entry() {
        let map = map_json(vec![
            npc_json("Oak", 8, 5, false, None),
            npc_json("Youngster", 3, 8, true, None),
            npc_json("PokeBall", 11, 14, false, Some(0x14)), // Potion
        ]);
        let npcs = [
            npc_obs(0, 8, 5, true, false),
            npc_obs(1, 3, 8, true, false),
            npc_obs(2, 11, 14, true, false),
        ];
        let out = nearby_entities(Position { x: 10, y: 9 }, &npcs, Some(&map), &[], 99);
        let by_id = |id: &str| out.iter().find(|e| e.id == id).unwrap();

        let oak = by_id("npc:0");
        assert_eq!(oak.kind, EntityKind::Npc);
        assert_eq!(oak.name.as_deref(), Some("Oak"));
        assert_eq!(oak.distance, 6);

        let trainer = by_id("npc:1");
        assert_eq!(trainer.kind, EntityKind::Trainer);
        assert_eq!(trainer.name.as_deref(), Some("Youngster"));

        let item = by_id("npc:2");
        assert_eq!(item.kind, EntityKind::Item);
        assert_eq!(item.name.as_deref(), Some("Potion"));
    }

    #[test]
    fn interactable_thresholds() {
        let map = map_json(vec![
            npc_json("Oak", 10, 7, false, None),   // distance 2: adjacent
            npc_json("Girl", 10, 6, false, None),  // distance 3: out of reach
            npc_json("Fisher", 10, 7, false, None), // defeated at runtime
        ]);
        let npcs = [
            npc_obs(0, 10, 7, true, false),
            npc_obs(1, 10, 6, true, false),
            npc_obs(2, 10, 7, true, true),
            npc_obs(3, 10, 7, false, false), // invisible
        ];
        let hidden = [spot(0, 10, 8, false), spot(1, 10, 9, true)];
        let out = nearby_entities(Position { x: 10, y: 9 }, &npcs, Some(&map), &hidden, 99);
        let by_id = |id: &str| out.iter().find(|e| e.id == id).unwrap();

        assert!(by_id("npc:0").interactable, "adjacent visible NPC");
        assert!(!by_id("npc:1").interactable, "one tile too far");
        assert!(!by_id("npc:2").interactable, "defeated");
        assert!(!by_id("npc:3").interactable, "invisible");

        // Sign at (7, 9): distance 3 → not readable from here.
        assert!(!by_id("sign:0").interactable);
        // Warps are usable only on tile.
        assert!(!by_id("warp:0").interactable);
        assert!(!by_id("warp:1").interactable);
        // Hidden items: adjacency + not yet obtained.
        assert!(by_id("hidden:0").interactable, "adjacent unobtained");
        assert!(!by_id("hidden:1").interactable, "already obtained");
    }

    #[test]
    fn warp_on_tile_is_interactable() {
        let map = map_json(vec![]);
        let out = nearby_entities(Position { x: 5, y: 5 }, &[], Some(&map), &[], 99);
        let warp = out.iter().find(|e| e.id == "warp:0").unwrap();
        assert_eq!(warp.distance, 0);
        assert!(warp.interactable);
        assert_eq!(warp.name.as_deref(), Some("DestA"));
    }

    #[test]
    fn sorted_by_distance_and_radius_filtered() {
        let map = map_json(vec![npc_json("Oak", 8, 5, false, None)]);
        let npcs = [npc_obs(0, 8, 5, true, false)];
        // Player (10, 9): sign (7,9) d=3, warp:1 (12,11) d=4, Oak d=6, warp:0 (5,5) d=9.
        let out = nearby_entities(Position { x: 10, y: 9 }, &npcs, Some(&map), &[], 99);
        let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["sign:0", "warp:1", "npc:0", "warp:0"]);
        assert!(out.windows(2).all(|w| w[0].distance <= w[1].distance));

        let near = nearby_entities(Position { x: 10, y: 9 }, &npcs, Some(&map), &[], 4);
        let ids: Vec<&str> = near.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["sign:0", "warp:1"]);
    }

    #[test]
    fn missing_map_yields_npcs_only() {
        let npcs = [npc_obs(0, 1, 1, true, false)];
        let out = nearby_entities(Position { x: 0, y: 0 }, &npcs, None, &[], 99);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, EntityKind::Npc);
        assert_eq!(out[0].name, None);
    }

    #[test]
    fn hidden_item_spots_filter_map_and_decode_flags() {
        // Viridian Forest items are table indices 0 and 1.
        let spots = hidden_item_spots(MapId::ViridianForest, &[0b01]);
        assert_eq!(spots.len(), 2);
        assert_eq!(spots[0].table_index, 0);
        assert!(spots[0].obtained, "bit 0 set → first item obtained");
        assert!(!spots[1].obtained, "bit 1 clear → second item free");
        assert_eq!(spots[0].item, "Potion");
        // Empty flags: nothing obtained, no panic on short slice.
        let spots = hidden_item_spots(MapId::ViridianForest, &[]);
        assert!(spots.iter().all(|s| !s.obtained));
        // Other maps are excluded.
        assert!(hidden_item_spots(MapId::PalletTown, &[]).is_empty());
    }
}
