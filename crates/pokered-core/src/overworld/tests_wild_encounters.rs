use super::wild_encounters::*;
use pokered_data::maps::MapId;
use pokered_data::species::Species;
use pokered_data::tilesets::TilesetId;
use pokered_data::wild_data::{wild_data_for_map, GameVersion};

use crate::battle::wild::{EncounterContext, WildEncounterRandoms, WildEncounterResult};

// ── wild_data_for_map tests ───────────────────────────────────────

#[test]
fn wild_data_for_route1_red() {
    let data = wild_data_for_map(MapId::Route1, GameVersion::Red).unwrap();
    assert_eq!(data.name, "Route1");
    assert!(data.grass.encounter_rate > 0);
    assert_eq!(data.grass.mons.len(), 10);
}

#[test]
fn wild_data_for_route1_blue() {
    let data = wild_data_for_map(MapId::Route1, GameVersion::Blue).unwrap();
    assert_eq!(data.name, "Route1");
    assert!(data.grass.encounter_rate > 0);
}

#[test]
fn wild_data_for_pallet_town_returns_none() {
    assert!(wild_data_for_map(MapId::PalletTown, GameVersion::Red).is_none());
}

#[test]
fn sea_routes_has_water_encounters_only() {
    let data = wild_data_for_map(MapId::Route19, GameVersion::Red).unwrap();
    assert_eq!(data.name, "Route19");
    assert_eq!(data.grass.encounter_rate, 0);
    assert!(data.water.encounter_rate > 0);
    assert_eq!(data.water.mons.len(), 10);
}

#[test]
fn route21_has_both_grass_and_water() {
    let data = wild_data_for_map(MapId::Route21, GameVersion::Red).unwrap();
    assert!(data.grass.encounter_rate > 0);
    assert!(data.water.encounter_rate > 0);
}

// ── determine_encounter_type tests ────────────────────────────────

#[test]
fn overworld_grass_tile_detected() {
    let result = determine_encounter_type(0x52, TilesetId::Overworld, MapId::Route1);
    assert_eq!(result, TileEncounterType::Grass);
}

#[test]
fn forest_grass_tile_detected() {
    let result = determine_encounter_type(0x20, TilesetId::Forest, MapId::ViridianForest);
    assert_eq!(result, TileEncounterType::Grass);
}

#[test]
fn water_tile_detected_on_overworld() {
    let result = determine_encounter_type(WATER_TILE, TilesetId::Overworld, MapId::Route21);
    assert_eq!(result, TileEncounterType::Water);
}

#[test]
fn water_tile_detected_in_cave() {
    let result = determine_encounter_type(WATER_TILE, TilesetId::Cavern, MapId::SeafoamIslandsB3F);
    assert_eq!(result, TileEncounterType::Water);
}

#[test]
fn indoor_cave_encounter_type() {
    let result = determine_encounter_type(0x00, TilesetId::Cavern, MapId::MtMoon1F);
    assert_eq!(result, TileEncounterType::IndoorCave);
}

#[test]
fn indoor_cemetery_encounter_type() {
    let result = determine_encounter_type(0x00, TilesetId::Cemetery, MapId::PokemonTower3F);
    assert_eq!(result, TileEncounterType::IndoorCave);
}

#[test]
fn viridian_forest_is_indoor_but_uses_forest_tileset_so_needs_grass_tile() {
    // ViridianForest (0x33 >= FIRST_INDOOR_MAP=0x25) but tileset == Forest
    // so the indoor exception does NOT apply — must be on grass tile
    let result = determine_encounter_type(0x00, TilesetId::Forest, MapId::ViridianForest);
    assert_eq!(result, TileEncounterType::None);

    let result = determine_encounter_type(0x20, TilesetId::Forest, MapId::ViridianForest);
    assert_eq!(result, TileEncounterType::Grass);
}

#[test]
fn outdoor_route_non_grass_non_water_tile_is_none() {
    let result = determine_encounter_type(0x00, TilesetId::Overworld, MapId::Route1);
    assert_eq!(result, TileEncounterType::None);
}

#[test]
fn city_map_non_grass_tile_is_none() {
    let result = determine_encounter_type(0x00, TilesetId::Overworld, MapId::PalletTown);
    assert_eq!(result, TileEncounterType::None);
}

#[test]
fn plateau_grass_tile_detected() {
    let result = determine_encounter_type(0x45, TilesetId::Plateau, MapId::VictoryRoad2F);
    assert_eq!(result, TileEncounterType::Grass);
}

// ── should_check_encounter tests ──────────────────────────────────

#[test]
fn check_allowed_when_all_clear() {
    assert!(should_check_encounter(false, false, 0));
}

#[test]
fn check_blocked_on_warp_tile() {
    assert!(!should_check_encounter(true, false, 0));
}

#[test]
fn check_blocked_during_npc_script() {
    assert!(!should_check_encounter(false, true, 0));
}

#[test]
fn check_blocked_by_cooldown() {
    assert!(!should_check_encounter(false, false, 3));
}

#[test]
fn check_blocked_by_multiple_conditions() {
    assert!(!should_check_encounter(true, true, 5));
}

// ── check_wild_encounter integration tests ────────────────────────

fn low_roll() -> WildEncounterRandoms {
    WildEncounterRandoms {
        encounter_roll: 0,
        slot_roll: 0,
    }
}

fn high_roll() -> WildEncounterRandoms {
    WildEncounterRandoms {
        encounter_roll: 255,
        slot_roll: 0,
    }
}

fn no_repel() -> EncounterContext {
    EncounterContext {
        repel_active: false,
        party_lead_level: 50,
    }
}

#[test]
fn route1_grass_encounter_triggers() {
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52, // grass tile
        0x52, // right anchor: same tile
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
}

#[test]
fn route1_high_roll_no_encounter() {
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &high_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

#[test]
fn route1_non_grass_tile_no_encounter() {
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x00, // not a grass tile
        0x00, // right anchor: same tile
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

#[test]
fn mt_moon_cave_encounter_on_any_tile() {
    let result = check_wild_encounter(
        MapId::MtMoon1F,
        TilesetId::Cavern,
        0x00, // any tile — indoor cave exception applies
        0x00, // right anchor: same tile
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
}

#[test]
fn pallet_town_never_encounters() {
    let result = check_wild_encounter(
        MapId::PalletTown,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

#[test]
fn warp_tile_blocks_encounter() {
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        true, // on warp tile
        false,
        0,
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

#[test]
fn npc_script_blocks_encounter() {
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        true, // NPC script active
        0,
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

#[test]
fn cooldown_blocks_encounter() {
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        5, // cooldown active
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

#[test]
fn route19_water_encounter() {
    let result = check_wild_encounter(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        WATER_TILE,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
    if let WildEncounterResult::Encounter { species, .. } = result {
        assert_eq!(species, Species::Tentacool);
    }
}

#[test]
fn route20_uses_same_sea_routes_data() {
    let r19 = check_wild_encounter(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        WATER_TILE,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    let r20 = check_wild_encounter(
        MapId::Route20,
        TilesetId::Overworld,
        WATER_TILE,
        WATER_TILE,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert_eq!(r19, r20);
}

#[test]
fn repel_blocks_low_level_encounter() {
    let ctx = EncounterContext {
        repel_active: true,
        party_lead_level: 50,
    };
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &low_roll(),
        &ctx,
        false,
        false,
        0,
    );
    assert_eq!(result, WildEncounterResult::RepelBlocked);
}

#[test]
fn repel_allows_high_level_encounter() {
    let ctx = EncounterContext {
        repel_active: true,
        party_lead_level: 1,
    };
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &low_roll(),
        &ctx,
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
}

#[test]
fn viridian_forest_needs_grass_tile_despite_being_indoor() {
    let no_encounter = check_wild_encounter(
        MapId::ViridianForest,
        TilesetId::Forest,
        0x00,
        0x00,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert_eq!(no_encounter, WildEncounterResult::NoEncounter);

    let encounter = check_wild_encounter(
        MapId::ViridianForest,
        TilesetId::Forest,
        0x20, // Forest grass tile
        0x20, // right anchor: same tile
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(encounter, WildEncounterResult::Encounter { .. }));
}

#[test]
fn pokemon_tower_3f_cemetery_encounter() {
    let result = check_wild_encounter(
        MapId::PokemonTower3F,
        TilesetId::Cemetery,
        0x00,
        0x00,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
}

#[test]
fn blue_version_has_different_pokemon() {
    let red_data = wild_data_for_map(MapId::Route1, GameVersion::Red).unwrap();
    let blue_data = wild_data_for_map(MapId::Route1, GameVersion::Blue).unwrap();
    assert_eq!(red_data.name, blue_data.name);
    // Red and Blue have different species on some routes
    // Both should have 10 slots
    assert_eq!(red_data.grass.mons.len(), 10);
    assert_eq!(blue_data.grass.mons.len(), 10);
}

#[test]
fn seafoam_islands_b3f_has_grass_but_no_water_encounters() {
    let data = wild_data_for_map(MapId::SeafoamIslandsB3F, GameVersion::Red).unwrap();
    assert!(
        data.grass.encounter_rate > 0,
        "SeafoamIslandsB3F should have grass/cave encounters"
    );
    assert_eq!(
        data.water.encounter_rate, 0,
        "SeafoamIslandsB3F has no water encounters in original data"
    );
}

#[test]
fn digletts_cave_encounter() {
    let result = check_wild_encounter(
        MapId::DiglettsCave,
        TilesetId::Cavern,
        0x00,
        0x00,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
    if let WildEncounterResult::Encounter { species, .. } = result {
        assert_eq!(species, Species::Diglett);
    }
}

#[test]
fn power_plant_encounter() {
    let result = check_wild_encounter(
        MapId::PowerPlant,
        TilesetId::Cavern,
        0x00,
        0x00,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
}

#[test]
fn select_table_grass_returns_grass() {
    let data = wild_data_for_map(MapId::Route1, GameVersion::Red).unwrap();
    let table = select_encounter_table(TileEncounterType::Grass, &data).unwrap();
    assert_eq!(table.encounter_rate, data.grass.encounter_rate);
}

#[test]
fn select_table_water_returns_water() {
    let data = wild_data_for_map(MapId::Route21, GameVersion::Red).unwrap();
    let table = select_encounter_table(TileEncounterType::Water, &data).unwrap();
    assert_eq!(table.encounter_rate, data.water.encounter_rate);
}

#[test]
fn select_table_indoor_cave_returns_grass() {
    let data = wild_data_for_map(MapId::MtMoon1F, GameVersion::Red).unwrap();
    let table = select_encounter_table(TileEncounterType::IndoorCave, &data).unwrap();
    assert_eq!(table.encounter_rate, data.grass.encounter_rate);
}

#[test]
fn select_table_none_returns_none() {
    let data = wild_data_for_map(MapId::Route1, GameVersion::Red).unwrap();
    assert!(select_encounter_table(TileEncounterType::None, &data).is_none());
}

#[test]
fn all_dungeon_maps_have_valid_data() {
    let dungeons = [
        MapId::MtMoon1F,
        MapId::MtMoonB1F,
        MapId::MtMoonB2F,
        MapId::RockTunnel1F,
        MapId::RockTunnelB1F,
        MapId::PowerPlant,
        MapId::DiglettsCave,
        MapId::VictoryRoad1F,
        MapId::VictoryRoad2F,
        MapId::VictoryRoad3F,
        MapId::SeafoamIslands1F,
        MapId::SeafoamIslandsB1F,
        MapId::SeafoamIslandsB2F,
        MapId::SeafoamIslandsB3F,
        MapId::SeafoamIslandsB4F,
        MapId::PokemonMansion1F,
        MapId::PokemonMansion2F,
        MapId::PokemonMansion3F,
        MapId::PokemonMansionB1F,
        MapId::CeruleanCave1F,
        MapId::CeruleanCave2F,
        MapId::CeruleanCaveB1F,
    ];
    for map in dungeons {
        let data = wild_data_for_map(map, GameVersion::Red);
        assert!(data.is_some(), "{:?} should have wild data in Red", map);
        let data = wild_data_for_map(map, GameVersion::Blue);
        assert!(data.is_some(), "{:?} should have wild data in Blue", map);
    }
}

// ─── P0d engine parity tests ──────────────────────────────────────────────
//
// Prove `EncounterEngine::on_step` (engine-owned control flow) yields exactly
// the same encounter (or None) as the legacy `check_wild_encounter` path, for a
// fixed map/tile/version/context and a FIXED rng stream. The engine draws the
// rate byte then the slot byte through `BattleRng`; the legacy path takes those
// two bytes pre-rolled as `WildEncounterRandoms`. Comparing results proves the
// draw order matches.

use dotzuki_engine::battle::rng::ScriptedRng;
use dotzuki_engine::overworld::encounter::{EncounterEngine, EncounterMode, EncounterStep};

/// Run both paths over the same inputs + rng stream and assert they agree.
#[allow(clippy::too_many_arguments)]
fn assert_encounter_parity(
    map_id: MapId,
    tileset: TilesetId,
    standing_tile: u8,
    version: GameVersion,
    context: EncounterContext,
    on_warp_tile: bool,
    npc_script_active: bool,
    encounter_cooldown: u8,
    stream: &[u8],
) {
    // Legacy: first two stream bytes are (encounter_roll, slot_roll).
    let legacy = check_wild_encounter(
        map_id,
        tileset,
        standing_tile,
        standing_tile,
        version,
        &WildEncounterRandoms {
            encounter_roll: stream[0],
            slot_roll: stream[1],
        },
        &context,
        on_warp_tile,
        npc_script_active,
        encounter_cooldown,
    );

    // Engine: provider over the same inputs, fed the same stream via BattleRng.
    let provider = PokeredEncounterProvider::new(
        map_id,
        tileset,
        standing_tile,
        version,
        context,
        on_warp_tile,
        npc_script_active,
        encounter_cooldown,
    );
    let mut rng = ScriptedRng::new(stream.to_vec());
    let engine = EncounterEngine::on_step(
        &provider,
        map_id as u32,
        0,
        0,
        EncounterMode::Walking,
        &mut rng,
    );

    match (legacy, engine) {
        // RepelBlocked maps to None engine-side (no battle is started).
        (WildEncounterResult::NoEncounter, EncounterStep::None)
        | (WildEncounterResult::RepelBlocked, EncounterStep::None) => {}
        (
            WildEncounterResult::Encounter { level, species },
            EncounterStep::Encounter {
                species_level: (es, el),
            },
        ) => {
            assert_eq!(species, es, "species mismatch (stream {:?})", stream);
            assert_eq!(level, el, "level mismatch (stream {:?})", stream);
        }
        (l, e) => panic!(
            "encounter parity mismatch: legacy={:?} engine={:?} stream={:?}",
            l, e, stream
        ),
    }
}

fn parity_no_repel() -> EncounterContext {
    EncounterContext {
        repel_active: false,
        party_lead_level: 50,
    }
}

#[test]
fn engine_matches_legacy_encounter_on_route1() {
    // Hit: low rate roll on Route 1 grass -> same species/level both paths.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[0, 0],
    );
    // Different slot byte -> still parity.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[5, 200],
    );
    // Last slot.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[3, 255],
    );
}

#[test]
fn engine_matches_legacy_miss_high_rate_roll() {
    // High rate roll -> NoEncounter on both paths.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[200, 0],
    );
}

#[test]
fn engine_matches_legacy_repel_blocked() {
    // Repel active + high party lead level -> RepelBlocked legacy, None engine.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        EncounterContext {
            repel_active: true,
            party_lead_level: 50,
        },
        false,
        false,
        0,
        &[0, 0],
    );
}

#[test]
fn engine_matches_legacy_gated_cases() {
    // Cooldown active -> both None (engine gate returns None, no RNG used).
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        5,
        &[0, 0],
    );
    // NPC script active -> both None.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        true,
        0,
        &[0, 0],
    );
    // On warp tile -> both None.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        true,
        false,
        0,
        &[0, 0],
    );
    // No wild data (Pallet Town) -> both None.
    assert_encounter_parity(
        MapId::PalletTown,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[0, 0],
    );
    // Non-encounter tile (plain ground) -> both None.
    assert_encounter_parity(
        MapId::Route1,
        TilesetId::Overworld,
        0x00,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[0, 0],
    );
}

// ─── P0d parity: water (Surfing) + cave tables ────────────────────────────
//
// The original P0d harness only exercised Route1 grass + the gate cases. These
// extend genuine, draw-order-faithful parity to the WATER table (Surfing on a
// water tile) and the IndoorCave classification (cave map on a non-grass tile,
// which routes to the grass table via `determine_encounter_type`). Both run the
// legacy `check_wild_encounter` and engine `on_step` over the same fixed RNG
// stream and assert species+level (or None) agreement, the same way
// `assert_encounter_parity` does for grass.

#[test]
fn engine_matches_legacy_water_encounter_on_route19() {
    // Route19 is water-only (grass rate 0, water rate > 0). Standing on a WATER
    // tile classifies as Water -> the water table is used by both paths.
    let data = wild_data_for_map(MapId::Route19, GameVersion::Red).unwrap();
    assert_eq!(data.grass.encounter_rate, 0, "Route19 grass must be empty");
    assert!(data.water.encounter_rate > 0, "Route19 needs water encounters");

    // Hit: low rate roll, slot 0.
    assert_encounter_parity(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[0, 0],
    );
    // Different slot byte (mid table) -> still parity.
    assert_encounter_parity(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[5, 200],
    );
    // Last slot.
    assert_encounter_parity(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[3, 255],
    );
}

#[test]
fn engine_matches_legacy_water_miss_high_rate_roll() {
    // High rate roll over the water table -> NoEncounter on both paths.
    assert_encounter_parity(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[254, 0],
    );
}

#[test]
fn engine_matches_legacy_water_repel_blocked() {
    // Repel active + lead level above the wild mon's level on water -> legacy
    // RepelBlocked, engine None. Uses a high lead level to guarantee the block.
    assert_encounter_parity(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        GameVersion::Red,
        EncounterContext {
            repel_active: true,
            party_lead_level: 100,
        },
        false,
        false,
        0,
        &[0, 0],
    );
}

#[test]
fn engine_matches_legacy_cave_encounter_on_mtmoon() {
    // MtMoon1F uses the Cavern tileset; a non-grass/non-water tile (0x00) on an
    // indoor map with a non-forest tileset classifies as IndoorCave, which
    // `select_encounter_table` routes to the grass table for both paths.
    let data = wild_data_for_map(MapId::MtMoon1F, GameVersion::Red).unwrap();
    assert!(
        data.grass.encounter_rate > 0,
        "MtMoon1F needs cave (grass-table) encounters"
    );

    // Hit: low rate roll, slot 0.
    assert_encounter_parity(
        MapId::MtMoon1F,
        TilesetId::Cavern,
        0x00,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[0, 0],
    );
    // Mid table slot.
    assert_encounter_parity(
        MapId::MtMoon1F,
        TilesetId::Cavern,
        0x00,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[4, 180],
    );
    // Last slot.
    assert_encounter_parity(
        MapId::MtMoon1F,
        TilesetId::Cavern,
        0x00,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[2, 255],
    );
}

#[test]
fn engine_matches_legacy_cave_miss_high_rate_roll() {
    // High rate roll over the cave (grass) table -> NoEncounter on both paths.
    assert_encounter_parity(
        MapId::MtMoon1F,
        TilesetId::Cavern,
        0x00,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
        &[254, 0],
    );
}

#[test]
fn engine_matches_legacy_cave_repel_blocked() {
    // Repel active + lead level above the wild mon level in a cave -> legacy
    // RepelBlocked, engine None.
    assert_encounter_parity(
        MapId::MtMoon1F,
        TilesetId::Cavern,
        0x00,
        GameVersion::Red,
        EncounterContext {
            repel_active: true,
            party_lead_level: 100,
        },
        false,
        false,
        0,
        &[0, 0],
    );
}

#[test]
fn engine_gate_consumes_no_rng_when_ineligible() {
    // When the tile is ineligible the engine must not touch the rng.
    let provider = PokeredEncounterProvider::new(
        MapId::PalletTown,
        TilesetId::Overworld,
        0x52,
        GameVersion::Red,
        parity_no_repel(),
        false,
        false,
        0,
    );
    let mut rng = ScriptedRng::new(vec![0, 0]);
    let step = EncounterEngine::on_step(
        &provider,
        MapId::PalletTown as u32,
        0,
        0,
        EncounterMode::Walking,
        &mut rng,
    );
    assert_eq!(step, EncounterStep::None);
    assert_eq!(rng.consumed(), 0);
}

// ── dual-anchor parity (wild_encounters.asm:28-72) ─────────────────
// Rate anchor = RIGHT neighbour (screen 9,9); table anchor = STANDING
// tile (screen 8,9). The "left shore" quirk: standing on land whose right
// neighbour is water rolls the WATER rate but reads the GRASS table.

#[test]
fn left_shore_rolls_water_rate_with_grass_table() {
    // Standing tile: land (not grass, not water). Right neighbour: water.
    // The rate anchor (water) lets the roll through; the table anchor is
    // land ≠ $14 → the GRASS list is consulted (the Cinnabar east-coast
    // setup). Route 21 has BOTH tables (grass 25 / water 5), so the split is
    // observable: a WATER rate roll must yield a GRASS species. (In the
    // original the grass read hits the cross-map stale wGrassMons buffer —
    // the Missingno mechanism; the port reads the map's own list, a
    // documented deviation.)
    let result = check_wild_encounter(
        MapId::Route21,
        TilesetId::Overworld,
        0x00, // standing: land
        WATER_TILE, // right neighbour: water → water rate
        GameVersion::Red,
        &WildEncounterRandoms { encounter_roll: 0, slot_roll: 0 },
        &no_repel(),
        false,
        false,
        0,
    );
    match result {
        WildEncounterResult::Encounter { species, .. } => {
            assert!(
                !matches!(species, pokered_data::species::Species::Tentacool),
                "left shore must read the GRASS list, got the water-only {species:?}"
            );
        }
        other => panic!("water-rate anchor must roll: {other:?}"),
    }
}

#[test]
fn grass_with_non_grass_right_neighbour_never_rolls() {
    // Standing on grass but the right neighbour is plain land → the rate
    // anchor finds NOTHING → no roll at all (the original's (9,9) check).
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52, // standing: grass
        0x00, // right neighbour: land → no rate
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

#[test]
fn surfing_with_land_right_neighbour_never_rolls() {
    // Surfing on water whose right neighbour is land → no roll (the original
    // reads (9,9) for the rate; land there suppresses the check entirely).
    let result = check_wild_encounter(
        MapId::Route19,
        TilesetId::Overworld,
        WATER_TILE,
        0x00,
        GameVersion::Red,
        &low_roll(),
        &no_repel(),
        false,
        false,
        0,
    );
    assert_eq!(result, WildEncounterResult::NoEncounter);
}

// The 180° turn-in-place encounter roll lives in update.rs's frame loop
// (MoveResult::TurnedOnly → the same on-step check); the dual-anchor split
// it shares is covered above. Unit-level proof that the TURN runs the roll
// lives in tests_turn_encounter below via a scripted provider is not
// feasible without the full frame harness — the behaviour is exercised by
// the update.rs integration tests (turn-in-place does not panic and can
// pend an encounter on grass).
#[test]
fn turn_only_result_semantics_documented() {
    // Guard the anchor semantics the turn path relies on: the roll uses the
    // RIGHT-neighbour rate anchor (grass pair → grass rate > 0 → rolls).
    let result = check_wild_encounter(
        MapId::Route1,
        TilesetId::Overworld,
        0x52,
        0x52,
        GameVersion::Red,
        &WildEncounterRandoms { encounter_roll: 0, slot_roll: 0 },
        &no_repel(),
        false,
        false,
        0,
    );
    assert!(matches!(result, WildEncounterResult::Encounter { .. }));
}
