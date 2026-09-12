//! M1 semantic observation layer: `PokemonGame::agent_snapshot` /
//! `agent_nearby` against a live Pallet Town overworld.

use pokered_agent::{
    AgentMode, EntityKind, ObservationLevel, ObservationProfile, Position,
};
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_core::game_state::GameScreen;
use pokered_core::overworld::{BedroomDialogue, Direction, OverworldScreen};
use pokered_core::pokemon::stats::create_pokemon;
use pokered_core::save::SaveData;
use pokered_data::{impl_traits::PokemonRedData, items::ItemId, maps::MapId, species::Species};

/// A fresh game standing in the Pallet Town overworld at (x, y), facing
/// down, with an empty save (same construction idiom as the other app
/// integration tests).
fn game_at_pallet(x: u16, y: u16) -> PokemonGame {
    let mut game = PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.save_data = SaveData::new();
    game.state.screen = GameScreen::Overworld;
    game.overworld = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    game.overworld.state.player.x = x;
    game.overworld.state.player.y = y;
    game.overworld.state.player.facing = Direction::Down;
    game
}

#[test]
fn snapshot_reports_overworld_mode_map_and_position() {
    let game = game_at_pallet(10, 9);
    let snap = game.agent_snapshot(&ObservationProfile::default());

    assert_eq!(snap.mode, AgentMode::Overworld);
    let map = snap.map.expect("map is always populated");
    assert_eq!(map.name, "PalletTown");
    assert_eq!(map.id, MapId::PalletTown as u8);
    assert_eq!(snap.position, Some(Position { x: 10, y: 9 }));
    assert_eq!(snap.facing.as_deref(), Some("Down"));

    // Empty save: no party/bag, no badges, no dialogue/battle.
    assert!(snap.party.is_empty());
    assert!(snap.bag.is_empty());
    assert_eq!(snap.badges.expect("badges section at level 3").count, 0);
    assert!(snap.dialogue.is_none());
    assert!(snap.battle.is_none());
    assert!(snap.nearby.is_some(), "level 3 includes nearby entities");
}

#[test]
fn snapshot_summarizes_party_bag_and_badges() {
    let mut game = game_at_pallet(10, 9);
    game.save_data
        .party
        .add(create_pokemon(Species::Charmander, 5, [0x9a, 0x78]).unwrap())
        .unwrap();
    game.save_data
        .game_data
        .bag
        .add_item(ItemId::Potion, 3)
        .unwrap();
    game.save_data.game_data.obtained_badges = 0b0000_0011;

    let snap = game.agent_snapshot(&ObservationProfile::default());
    assert_eq!(snap.party.len(), 1);
    let mon = &snap.party[0];
    assert_eq!(mon.species, "Charmander");
    assert_eq!(mon.level, 5);
    assert_eq!(mon.hp, mon.max_hp);
    assert_eq!(mon.status, "None");

    assert_eq!(snap.bag.len(), 1);
    assert_eq!(snap.bag[0].item, "Potion");
    assert_eq!(snap.bag[0].qty, 3);

    let badges = snap.badges.unwrap();
    assert_eq!(badges.count, 2);
    assert_eq!(badges.names, ["Boulder", "Cascade"]);
}

#[test]
fn nearby_contains_pallet_town_entities_with_distances() {
    // Player at (10, 9): every Pallet Town warp and the Oak NPC fall
    // inside the default radius 10; the (3, 5) sign at distance 11 does not.
    let game = game_at_pallet(10, 9);
    let entities = game.agent_nearby(pokered_agent::DEFAULT_NEARBY_RADIUS);
    let by_id = |id: &str| {
        entities
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing entity {id}"))
    };

    // Oak (static npc index 0 at (8, 5)) — Manhattan distance |10-8|+|9-5| = 6.
    let oak = by_id("npc:0");
    assert_eq!(oak.kind, EntityKind::Npc);
    assert_eq!(oak.position, Position { x: 8, y: 5 });
    assert_eq!(oak.distance, 6);
    assert_eq!(oak.name.as_deref(), Some("Oak"));
    assert!(!oak.interactable, "too far to talk to");
    // Warps carry their destination map names.
    let reds_house = by_id("warp:0");
    assert_eq!(reds_house.kind, EntityKind::Warp);
    assert_eq!(reds_house.position, Position { x: 5, y: 5 });
    assert_eq!(reds_house.distance, 9);
    assert_eq!(reds_house.name.as_deref(), Some("RedsHouse1F"));

    let blues_house = by_id("warp:1");
    assert_eq!(blues_house.name.as_deref(), Some("BluesHouse"));
    assert_eq!(blues_house.distance, 7);

    let oaks_lab = by_id("warp:2");
    assert_eq!(oaks_lab.name.as_deref(), Some("OaksLab"));
    assert_eq!(oaks_lab.distance, 4);
    assert!(!oaks_lab.interactable, "warps are usable only on tile");

    // The wandering NPCs and readable signs are present too.
    assert_eq!(by_id("npc:1").name.as_deref(), Some("Girl"));
    assert_eq!(by_id("npc:2").name.as_deref(), Some("Fisher"));
    assert_eq!(by_id("sign:1").kind, EntityKind::Sign);
    assert_eq!(by_id("sign:1").distance, 3);

    // Sorted nearest-first; the far sign (distance 11) is radius-filtered.
    assert!(entities.windows(2).all(|w| w[0].distance <= w[1].distance));
    assert!(entities.iter().all(|e| e.id != "sign:2"));

    // Radius shrinks the set: only the sign (3) and Oak's lab warp (4).
    let near = game.agent_nearby(4);
    let ids: Vec<&str> = near.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["sign:1", "warp:2"]);
}

#[test]
fn interactable_heuristics_track_distance_and_tile() {
    // Adjacent to the Fisher (11, 14) from (11, 12): distance 2 → talkable.
    let game = game_at_pallet(11, 12);
    let entities = game.agent_nearby(10);
    let fisher = entities.iter().find(|e| e.id == "npc:2").unwrap();
    assert_eq!(fisher.distance, 2);
    assert!(fisher.interactable);

    // Oak spawns hidden (`defaultHidden` in the Pallet Town script
    // config: he appears for the north-exit event) — even at talk
    // distance he is not interactable while invisible.
    let game = game_at_pallet(8, 7);
    let entities = game.agent_nearby(10);
    let oak = entities.iter().find(|e| e.id == "npc:0").unwrap();
    assert_eq!(oak.distance, 2);
    assert!(!oak.interactable, "hidden NPC is not interactable");

    // Standing on the Red's house warp tile (5, 5) makes it usable.
    let game = game_at_pallet(5, 5);
    let entities = game.agent_nearby(10);
    let warp = entities.iter().find(|e| e.id == "warp:0").unwrap();
    assert_eq!(warp.distance, 0);
    assert!(warp.interactable);
}

#[test]
fn open_dialogue_flips_mode_and_fills_dialogue_section() {
    let mut game = game_at_pallet(10, 9);
    game.overworld.pending_dialogue = Some(BedroomDialogue::from_message("OAK: It's unsafe!"));
    let snap = game.agent_snapshot(&ObservationProfile::default());

    assert_eq!(snap.mode, AgentMode::Dialogue);
    let dialogue = snap.dialogue.expect("level 3 includes dialogue detail");
    assert_eq!(dialogue.text, "OAK: It's unsafe!");
    assert_eq!(dialogue.page, 1);
    assert!(dialogue.choice.is_none());
}

#[test]
fn observation_levels_gate_sections() {
    let mut game = game_at_pallet(10, 9);
    game.overworld.pending_dialogue = Some(BedroomDialogue::from_message("Hello"));

    // Level 1: runtime state only.
    let level1 = ObservationProfile::for_level(ObservationLevel::RuntimeState);
    let snap = game.agent_snapshot(&level1);
    assert_eq!(snap.mode, AgentMode::Dialogue, "mode is always classified");
    assert!(snap.map.is_some() && snap.position.is_some());
    assert!(snap.badges.is_some());
    assert!(snap.dialogue.is_none(), "level 1 has no dialogue detail");
    assert!(snap.nearby.is_none(), "level 1 has no nearby entities");

    // Level 2: + dialogue/battle, still no nearby.
    let level2 = ObservationProfile::for_level(ObservationLevel::Interaction);
    let snap = game.agent_snapshot(&level2);
    assert!(snap.dialogue.is_some());
    assert!(snap.nearby.is_none());

    // Level 4 allowance flag only; sections match level 3.
    let level4 = ObservationProfile::for_level(ObservationLevel::WorldModel);
    assert!(level4.allow_world_data);
    let snap = game.agent_snapshot(&level4);
    assert!(snap.nearby.is_some());
}

#[test]
fn snapshot_serializes_to_sparse_json() {
    let game = game_at_pallet(10, 9);
    let snap = game.agent_snapshot(&ObservationProfile::default());
    let json = serde_json::to_value(&snap).unwrap();

    assert_eq!(json["mode"], "overworld");
    assert_eq!(json["map"]["name"], "PalletTown");
    assert_eq!(json["position"]["x"], 10);
    assert_eq!(json["facing"], "Down");
    // Sparse: empty party/bag and absent dialogue/battle are omitted.
    assert!(json.get("party").is_none());
    assert!(json.get("bag").is_none());
    assert!(json.get("dialogue").is_none());
    assert!(json.get("battle").is_none());
    let nearby = json["nearby"].as_array().expect("level 3 nearby");
    assert!(nearby.iter().any(|e| e["name"] == "Oak"));
    assert!(nearby.iter().any(|e| e["name"] == "OaksLab"));
}
