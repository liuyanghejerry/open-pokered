//! Tests for M4.3 — map connections, warp data, and transition logic.

use pokered_data::blockset_data;
use pokered_data::impl_traits::PokemonMapData;
use pokered_data::map_connections::{get_map_connections, MAP_CONNECTIONS};
use pokered_data::map_objects::get_map_warps;
use pokered_data::maps::MapId;
use pokered_data::music::MusicId;
use pokered_data::tilesets::TilesetId;

use dotzuki_engine::overworld::collision::CollisionProvider;
use dotzuki_engine::overworld::collision::CollisionResult;
use dotzuki_engine::overworld::map_transitions::{
    calculate_connection_transition, check_warp_at, ConnectionTransition,
};
use dotzuki_engine::overworld::types::TransportMode;
use dotzuki_engine::overworld::MapData;
use pokered_data::collision::is_tile_passable;
use crate::overworld::{Direction, MapConnection, MapConnections, WarpPoint};
use super::update::{execute_warp, resolve_warp_destination};

// ── Helpers ───────────────────────────────────────────────────────────

fn make_connections(map_id: MapId) -> MapConnections<MapId> {
    let entry = get_map_connections(map_id);
    let mut conns = MapConnections::default();
    conns.north = entry.north.map(|c| MapConnection::new(Direction::Up, c.target_map, c.offset));
    conns.south = entry.south.map(|c| MapConnection::new(Direction::Down, c.target_map, c.offset));
    conns.west = entry.west.map(|c| MapConnection::new(Direction::Left, c.target_map, c.offset));
    conns.east = entry.east.map(|c| MapConnection::new(Direction::Right, c.target_map, c.offset));
    conns
}

fn make_map_data(map_id: MapId) -> MapData<MapId, TilesetId, MusicId> {
    let w = map_id.width();
    let h = map_id.height();
    let warps: Vec<WarpPoint<MapId>> = get_map_warps(map_id).iter().map(|wd| {
        let (target_map, is_last_map) = match wd.dest_map {
            Some(m) => (m, false),
            None => (MapId::PalletTown, true),
        };
        let mut wp = WarpPoint::new(wd.x, wd.y, target_map, wd.dest_warp_id);
        wp.is_last_map = is_last_map;
        wp
    }).collect();
    MapData::new(
        map_id, w, h,
        TilesetId::Overworld,
        MusicId::PalletTown,
        vec![0; w as usize * h as usize],
        warps,
        vec![], vec![],
        make_connections(map_id),
    )
}

// ── Connection Data Integrity Tests ─────────────────────────────────

#[test]
fn test_pallet_town_connections() {
    let conns = get_map_connections(MapId::PalletTown);
    assert!(
        conns.north.is_some(),
        "PalletTown should have north connection"
    );
    assert!(
        conns.south.is_some(),
        "PalletTown should have south connection"
    );
    assert!(
        conns.west.is_none(),
        "PalletTown should not have west connection"
    );
    assert!(
        conns.east.is_none(),
        "PalletTown should not have east connection"
    );

    let north = conns.north.unwrap();
    assert_eq!(north.target_map, MapId::Route1);
    assert_eq!(north.offset, 0);

    let south = conns.south.unwrap();
    assert_eq!(south.target_map, MapId::Route21);
    assert_eq!(south.offset, 0);
}

#[test]
fn test_viridian_city_connections() {
    let conns = get_map_connections(MapId::ViridianCity);
    assert!(conns.north.is_some());
    assert!(conns.south.is_some());
    assert!(conns.west.is_some());
    assert!(conns.east.is_none());

    assert_eq!(conns.north.unwrap().target_map, MapId::Route2);
    assert_eq!(conns.south.unwrap().target_map, MapId::Route1);
    assert_eq!(conns.west.unwrap().target_map, MapId::Route22);
}

#[test]
fn test_cerulean_city_all_four_connections() {
    let conns = get_map_connections(MapId::CeruleanCity);
    assert!(conns.north.is_some());
    assert!(conns.south.is_some());
    assert!(conns.west.is_some());
    assert!(conns.east.is_some());
    assert_eq!(conns.north.unwrap().target_map, MapId::Route24);
    assert_eq!(conns.south.unwrap().target_map, MapId::Route5);
    assert_eq!(conns.west.unwrap().target_map, MapId::Route4);
    assert_eq!(conns.east.unwrap().target_map, MapId::Route9);
}

#[test]
fn test_indoor_maps_have_no_connections() {
    // Indoor maps (RedsHouse1F, OaksLab, etc.) should have no connections
    let indoor_maps = [
        MapId::RedsHouse1F,
        MapId::RedsHouse2F,
        MapId::OaksLab,
        MapId::ViridianPokecenter,
        MapId::ViridianMart,
        MapId::PewterGym,
    ];
    for map in &indoor_maps {
        let conns = get_map_connections(*map);
        assert!(
            conns.north.is_none()
                && conns.south.is_none()
                && conns.west.is_none()
                && conns.east.is_none(),
            "{:?} should have no connections",
            map
        );
    }
}

#[test]
fn test_connection_symmetry_pallet_route1() {
    // PalletTown connects north to Route1
    let pallet = get_map_connections(MapId::PalletTown);
    assert_eq!(pallet.north.unwrap().target_map, MapId::Route1);

    // Route1 connects south to PalletTown
    let route1 = get_map_connections(MapId::Route1);
    assert_eq!(route1.south.unwrap().target_map, MapId::PalletTown);
}

#[test]
fn test_connection_count_across_all_maps() {
    // Count total connections
    let total: u8 = MAP_CONNECTIONS.iter().map(|c| c.connection_count()).sum();
    // We know from data extraction there are 78 total connections across 36 maps
    assert_eq!(total, 78, "Expected 78 total connections");
}

#[test]
fn test_maps_with_connections_count() {
    let maps_with_conns = MAP_CONNECTIONS
        .iter()
        .filter(|c| c.connection_count() > 0)
        .count();
    assert_eq!(maps_with_conns, 36, "Expected 36 maps with connections");
}

// ── Warp Data Integrity Tests ───────────────────────────────────────

#[test]
fn test_pallet_town_warps() {
    let warps = get_map_warps(MapId::PalletTown);
    assert_eq!(warps.len(), 3, "PalletTown has 3 warps");

    // Warp 0: Red's House
    assert_eq!(warps[0].x, 5);
    assert_eq!(warps[0].y, 5);
    assert_eq!(warps[0].dest_map, Some(MapId::RedsHouse1F));
    assert_eq!(warps[0].dest_warp_id, 0);

    // Warp 1: Blue's House
    assert_eq!(warps[1].x, 13);
    assert_eq!(warps[1].y, 5);
    assert_eq!(warps[1].dest_map, Some(MapId::BluesHouse));
    assert_eq!(warps[1].dest_warp_id, 0);

    // Warp 2: Oak's Lab
    assert_eq!(warps[2].x, 12);
    assert_eq!(warps[2].y, 11);
    assert_eq!(warps[2].dest_map, Some(MapId::OaksLab));
    assert_eq!(warps[2].dest_warp_id, 1);
}

#[test]
fn test_reds_house_1f_warps() {
    let warps = get_map_warps(MapId::RedsHouse1F);
    assert_eq!(warps.len(), 3);

    // Warps 0,1 go to LAST_MAP (None) — they are the exit doors
    assert_eq!(warps[0].dest_map, None); // LAST_MAP
    assert_eq!(warps[1].dest_map, None); // LAST_MAP

    // Warp 2 goes to RedsHouse2F (stairs up)
    assert_eq!(warps[2].dest_map, Some(MapId::RedsHouse2F));
    assert_eq!(warps[2].dest_warp_id, 0);
}

#[test]
fn test_reds_house_2f_warp_back() {
    let warps = get_map_warps(MapId::RedsHouse2F);
    assert_eq!(warps.len(), 1);
    assert_eq!(warps[0].dest_map, Some(MapId::RedsHouse1F));
    assert_eq!(warps[0].dest_warp_id, 2);
}

#[test]
fn test_maps_without_warps() {
    // Some maps have no warps (usually routes without buildings)
    let warps = get_map_warps(MapId::Route1);
    // Route1 has no warps in the warp table (no buildings directly on the route)
    // (Route1 has gate connections, but those are in different map IDs)
    assert_eq!(warps.len(), 0, "Route1 has no direct warps");
}

#[test]
fn test_viridian_city_warps() {
    let warps = get_map_warps(MapId::ViridianCity);
    assert_eq!(warps.len(), 5);
    // First warp leads to ViridianPokecenter
    assert_eq!(warps[0].dest_map, Some(MapId::ViridianPokecenter));
}

#[test]
fn test_silph_co_elevator_last_map_warps() {
    // SilphCo Elevator warps target UNUSED_MAP_ED (the placeholder overwritten
    // by the elevator select flow at runtime), like the reference
    // data/maps/objects/SilphCoElevator.asm.
    let warps = get_map_warps(MapId::SilphCoElevator);
    assert_eq!(warps.len(), 2);
    assert_eq!(warps[0].dest_map, Some(MapId::UnusedMapED));
    assert_eq!(warps[1].dest_map, Some(MapId::UnusedMapED));
}

#[test]
fn test_total_warp_count() {
    // Count total warps across all maps
    let total: usize = (0..248u8)
        .filter_map(MapId::from_u8)
        .map(|m| get_map_warps(m).len())
        .sum();
    // We know from extraction: 805 total warps across 212 maps
    assert_eq!(total, 805, "Expected 805 total warps");
}

// ── Connection Transition Tests ─────────────────────────────────────

#[test]
fn test_walk_north_from_pallet_to_route1() {
    // PalletTown: 10×9 blocks = 20×18 tiles
    // Walking north from Y=0 → Route1, offset=0
    // Route1: 10×18 blocks = 20×36 tiles → new Y = 35 (bottom)
    let map_data = make_map_data(MapId::PalletTown);
    let result = calculate_connection_transition(
        &map_data,
        &PokemonMapData,
        10, // player X (mid-map)
        0,  // player Y (top edge)
        Direction::Up,
    );
    assert!(result.is_some());
    let t = result.unwrap();
    assert_eq!(t.new_map, MapId::Route1);
    assert_eq!(t.new_y, 35); // bottom of Route1 (18*2-1)
    assert_eq!(t.new_x, 10); // offset=0 → same X
}

#[test]
fn test_walk_south_from_pallet_to_route21() {
    // PalletTown: 20×18 tiles, walking south from Y=17
    // Route21: 10×45 blocks = 20×90 tiles → new Y = 0 (top)
    let map_data = make_map_data(MapId::PalletTown);
    let result = calculate_connection_transition(
        &map_data,
        &PokemonMapData,
        10,
        17, // bottom edge (18-1)
        Direction::Down,
    );
    assert!(result.is_some());
    let t = result.unwrap();
    assert_eq!(t.new_map, MapId::Route21);
    assert_eq!(t.new_y, 0);
    assert_eq!(t.new_x, 10); // offset=0
}

#[test]
fn test_walk_south_from_route1_to_pallet() {
    // Route1: 10×18 blocks = 20×36 tiles
    // Route1 south → PalletTown, offset=0
    let map_data = make_map_data(MapId::Route1);
    let result = calculate_connection_transition(
        &map_data,
        &PokemonMapData,
        10,
        35, // bottom edge
        Direction::Down,
    );
    assert!(result.is_some());
    let t = result.unwrap();
    assert_eq!(t.new_map, MapId::PalletTown);
    assert_eq!(t.new_y, 0);
}

#[test]
fn test_walk_north_from_route1_to_viridian() {
    // Route1 north → ViridianCity, offset=-5
    // Route1 top edge Y=0
    let map_data = make_map_data(MapId::Route1);
    let result = calculate_connection_transition(&map_data, &PokemonMapData, 10, 0, Direction::Up);
    assert!(result.is_some());
    let t = result.unwrap();
    assert_eq!(t.new_map, MapId::ViridianCity);
    // ViridianCity: height is looked up from MapId, offset=-5 (blocks)
    // ASM: XAlignment = offset * -2 = (-5) * -2 = 10; new_x = px + XAlignment = 10 + 10 = 20
    assert_eq!(t.new_x, 20);
}

#[test]
fn test_connection_with_positive_offset() {
    // ViridianCity north → Route2, offset=5
    // ViridianCity: Y=0 top edge
    let map_data = make_map_data(MapId::ViridianCity);
    let result = calculate_connection_transition(&map_data, &PokemonMapData, 0, 0, Direction::Up);
    assert!(result.is_some());
    let t = result.unwrap();
    assert_eq!(t.new_map, MapId::Route2);
    // new_x = 0 - 5*2 = -10, clamped to 0
    assert_eq!(t.new_x, 0);
}

#[test]
fn test_connection_west_viridian_to_route22() {
    // ViridianCity west → Route22, offset=4
    // ViridianCity: X=0, moving left
    let map_data = make_map_data(MapId::ViridianCity);
    let result = calculate_connection_transition(&map_data, &PokemonMapData, 0, 10, Direction::Left);
    assert!(result.is_some());
    let t = result.unwrap();
    assert_eq!(t.new_map, MapId::Route22);
    // Route22 width: need to look up... dest_w*2-1
    let route22_w = MapId::Route22.width() as u16 * 2;
    assert_eq!(t.new_x, route22_w - 1);
    // new_y = 10 - 4*2 = 2
    assert_eq!(t.new_y, 2);
}

#[test]
fn test_no_connection_returns_none() {
    // PalletTown has no west connection
    let map_data = make_map_data(MapId::PalletTown);
    let result = calculate_connection_transition(&map_data, &PokemonMapData, 0, 10, Direction::Left);
    assert!(result.is_none());
}

#[test]
fn test_not_at_edge_returns_none() {
    // Player is in the middle of the map, not at edge → no transition
    let map_data = make_map_data(MapId::PalletTown);
    let result = calculate_connection_transition(&map_data, &PokemonMapData, 10, 10, Direction::Up);
    assert!(result.is_none());
}

#[test]
fn test_indoor_map_no_connections() {
    // Indoor map should never have connection transitions
    let map_data = make_map_data(MapId::RedsHouse1F);
    let result = calculate_connection_transition(&map_data, &PokemonMapData, 0, 0, Direction::Up);
    assert!(result.is_none());
}

// ── Warp Transition Tests ───────────────────────────────────────────

#[test]
fn test_check_warp_pallet_town_reds_house() {
    let map_data = make_map_data(MapId::PalletTown);
    let warp = check_warp_at(&map_data, 5, 5);
    assert!(warp.is_some());
    let w = warp.unwrap();
    assert_eq!(w.new_map, MapId::RedsHouse1F);
    assert_eq!(w.dest_warp_id, 0);
    assert!(!w.is_last_map);
}

#[test]
fn test_check_warp_oaks_lab() {
    let map_data = make_map_data(MapId::PalletTown);
    let warp = check_warp_at(&map_data, 12, 11);
    assert!(warp.is_some());
    let w = warp.unwrap();
    assert_eq!(w.new_map, MapId::OaksLab);
    assert_eq!(w.dest_warp_id, 1);
}

#[test]
fn test_check_warp_no_warp() {
    // Position with no warp
    let map_data = make_map_data(MapId::PalletTown);
    let warp = check_warp_at(&map_data, 0, 0);
    assert!(warp.is_none());
}

#[test]
fn test_check_warp_last_map() {
    let map_data = make_map_data(MapId::RedsHouse1F);
    let warp = check_warp_at(&map_data, 2, 7);
    assert!(warp.is_some());
    let w = warp.unwrap();
    assert!(w.is_last_map);
}

#[test]
fn test_resolve_warp_destination() {
    let pos = resolve_warp_destination(MapId::RedsHouse1F, 0);
    assert_eq!(pos, Some((2, 7)));

    let pos = resolve_warp_destination(MapId::RedsHouse1F, 2);
    assert_eq!(pos, Some((7, 1)));
}

#[test]
fn test_resolve_warp_out_of_bounds() {
    let pos = resolve_warp_destination(MapId::RedsHouse1F, 99);
    assert!(pos.is_none());
}

#[test]
fn test_execute_warp_full() {
    let map_data = make_map_data(MapId::PalletTown);
    let result = execute_warp(&map_data, 5, 5, None);
    assert!(result.is_some());
    let (map, x, y) = result.unwrap();
    assert_eq!(map, MapId::RedsHouse1F);
    assert_eq!(x, 2);
    assert_eq!(y, 7);
}

#[test]
fn test_execute_warp_last_map() {
    let map_data = make_map_data(MapId::RedsHouse1F);
    let result = execute_warp(&map_data, 2, 7, Some(MapId::PalletTown));
    assert!(result.is_some());
    let (map, x, y) = result.unwrap();
    assert_eq!(map, MapId::PalletTown);
    assert_eq!(x, 5);
    assert_eq!(y, 5);
}

#[test]
fn test_execute_warp_last_map_none_returns_none() {
    // LAST_MAP warp with no last_map info → should return None
    let map_data = make_map_data(MapId::RedsHouse1F);
    let result = execute_warp(&map_data, 2, 7, None);
    assert!(result.is_none());
}

#[test]
fn test_execute_warp_stairs_reds_house() {
    let map_data = make_map_data(MapId::RedsHouse1F);
    let result = execute_warp(&map_data, 7, 1, None);
    assert!(result.is_some());
    let (map, x, y) = result.unwrap();
    assert_eq!(map, MapId::RedsHouse2F);
    assert_eq!(x, 7);
    assert_eq!(y, 1);
}

#[test]
fn test_execute_warp_stairs_down() {
    let map_data = make_map_data(MapId::RedsHouse2F);
    let result = execute_warp(&map_data, 7, 1, None);
    assert!(result.is_some());
    let (map, x, y) = result.unwrap();
    assert_eq!(map, MapId::RedsHouse1F);
    assert_eq!(x, 7);
    assert_eq!(y, 1);
}

#[test]
fn test_no_warp_returns_none() {
    let map_data = make_map_data(MapId::PalletTown);
    let result = execute_warp(&map_data, 0, 0, None);
    assert!(result.is_none());
}

// ── PalletTown south edge: on-foot entry into Route21 is blocked ────

/// Resolve the collision tile the player reads at tile coords (x, y):
/// block = (x/2, y/2), sub-tile = t[(sub_y*2+1)*4 + sub_x*2].
fn resolve_tile(blocks: &[u8], width_blocks: u8, x: u16, y: u16) -> Option<u8> {
    let bx = (x / 2) as usize;
    let by = (y / 2) as usize;
    let idx = by * (width_blocks as usize) + bx;
    let block_id = *blocks.get(idx)?;
    blockset_data::block_tiles(TilesetId::Overworld, block_id)
        .map(|t| t[((y % 2) as usize * 2 + 1) * 4 + (x % 2) as usize * 2])
}

/// `check_movement_collision` at a map edge with real map block data.
fn edge_collision(
    map_id: MapId,
    x: u16,
    y: u16,
    direction: Direction,
    transport: TransportMode,
) -> CollisionResult {
    use dotzuki_engine::overworld::collision::{check_movement_collision, SpritePosition};

    let blocks = pokered_data::map_data_loader::get_block_data(map_id);
    let (w, h) = map_id.dimensions();
    let provider =
        crate::overworld::collision::PokemonCollisionProvider::new(map_id, TilesetId::Overworld);
    let standing = resolve_tile(blocks, w, x, y).expect("standing tile");
    let edge = provider.get_connection_edge_tile(
        TilesetId::Overworld,
        w,
        h,
        x,
        y,
        direction,
    );
    check_movement_collision(
        x,
        y,
        direction,
        TilesetId::Overworld,
        w,
        h,
        standing,
        edge.unwrap_or(0),
        transport,
        &[] as &[SpritePosition],
        0,
        &provider,
    )
}

#[test]
fn pallet_town_south_edge_walk_down_blocked() {
    // Regression: walking down from PalletTown (3,17) must NOT enter
    // Route21 (3,0) on foot. The seam tile is Route21's top row — a tree
    // block ($63 → $3a at the arrival position), impassable on land. The
    // original blocks this via CheckTilePassable on the connection strip;
    // only Surf may cross PalletTown → Route21.
    let result = edge_collision(
        MapId::PalletTown,
        3,
        17,
        Direction::Down,
        TransportMode::Walking,
    );
    assert_eq!(result, CollisionResult::TileBlocked);
}

#[test]
fn pallet_town_south_edge_surf_crosses_water() {
    // A surfer may cross the same seam where the far tile is water:
    // PalletTown (4,17) → Route21 (4,0) = $32 (surfable coast).
    let result = edge_collision(
        MapId::PalletTown,
        4,
        17,
        Direction::Down,
        TransportMode::Surfing,
    );
    assert_eq!(result, CollisionResult::MapEdge);
}

#[test]
fn pallet_town_south_edge_surf_blocked_by_tree() {
    // Surfing straight into the tree seam stays blocked.
    let result = edge_collision(
        MapId::PalletTown,
        3,
        17,
        Direction::Down,
        TransportMode::Surfing,
    );
    assert_eq!(result, CollisionResult::TileBlocked);
}

#[test]
fn pallet_town_north_edge_walk_crosses_passable_seam() {
    // Walking north from PalletTown (2,0) → Route1 (2,35) is grass ($2c):
    // the seam is passable on foot, so the crossing reports MapEdge.
    let result = edge_collision(
        MapId::PalletTown,
        2,
        0,
        Direction::Up,
        TransportMode::Walking,
    );
    assert_eq!(result, CollisionResult::MapEdge);
}

#[test]
fn route21_north_edge_surf_onto_pallet_beach_dismounts() {
    // Surfing north from Route21's north edge onto PalletTown's beach grass
    // ($2c) is allowed (CollisionCheckOnWater .stopSurfing); the game layer
    // dismounts after the map swap.
    let result = edge_collision(
        MapId::Route21,
        2,
        0,
        Direction::Up,
        TransportMode::Surfing,
    );
    assert_eq!(result, CollisionResult::MapEdge);
}

#[test]
fn get_connection_edge_tile_resolves_seams() {
    let provider =
        crate::overworld::collision::PokemonCollisionProvider::new(MapId::PalletTown, TilesetId::Overworld);

    // South seam at (3,17): Route21's top row, tree block → impassable tile.
    let tile = provider
        .get_connection_edge_tile(TilesetId::Overworld, 10, 9, 3, 17, Direction::Down)
        .expect("PalletTown south seam tile");
    assert!(
        !is_tile_passable(TilesetId::Overworld, tile),
        "Route21 (3,0) seam tile ${tile:02x} must be impassable on foot"
    );

    // North seam at (2,0): Route1's bottom row, grass → passable tile.
    let tile = provider
        .get_connection_edge_tile(TilesetId::Overworld, 10, 9, 2, 0, Direction::Up)
        .expect("PalletTown north seam tile");
    assert!(is_tile_passable(TilesetId::Overworld, tile));

    // No connection in a direction → None (plain map-edge behavior).
    let none = provider.get_connection_edge_tile(TilesetId::Overworld, 10, 9, 3, 17, Direction::Left);
    assert!(none.is_none(), "PalletTown has no west connection");

    // Indoor map with no connections at all → None.
    let indoor = crate::overworld::collision::PokemonCollisionProvider::new(
        MapId::RedsHouse1F,
        TilesetId::RedsHouse1,
    );
    assert!(indoor
        .get_connection_edge_tile(TilesetId::RedsHouse1, 10, 8, 0, 0, Direction::Up)
        .is_none());
}

// ── End-to-end (full update_frame loop) ─────────────────────────────

use super::screen::OverworldScreen;
use super::OverworldInput;
use pokered_data::impl_traits::PokemonRedData;

fn down_input() -> OverworldInput {
    OverworldInput::new(false, true, false, false, false, false, false, false)
}

fn up_input() -> OverworldInput {
    OverworldInput::new(true, false, false, false, false, false, false, false)
}

/// Regression: holding DOWN on PalletTown's beach grass at (3,17) must NOT
/// carry the player into Route21 (3,0) on foot — the seam is Route21's
/// tree-lined top row, impassable without Surf (the original bumps).
#[test]
fn e2e_pallet_south_edge_down_is_blocked() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    screen.state.player.x = 3;
    screen.state.player.y = 17;
    screen.state.player.facing = Direction::Down;

    for _ in 0..120 {
        screen.update_frame(down_input());
    }

    assert_eq!(
        screen.state.current_map,
        MapId::PalletTown,
        "player must not leave PalletTown on foot"
    );
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (3, 17),
        "player must stay on the beach at (3,17)"
    );
}

/// Control: holding UP on a passable seam still crosses into the connected
/// map (PalletTown (2,0) → Route1 bottom, both grass).
#[test]
fn e2e_pallet_north_edge_up_crosses_to_route1() {
    let mut screen = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
    screen.state.player.x = 2;
    screen.state.player.y = 0;
    screen.state.player.facing = Direction::Up;

    for _ in 0..120 {
        screen.update_frame(up_input());
        if screen.state.current_map == MapId::Route1 {
            break;
        }
    }

    assert_eq!(screen.state.current_map, MapId::Route1);
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        (2, MapId::Route1.height() as u16 * 2 - 1),
        "should arrive at Route1's bottom edge"
    );
}
