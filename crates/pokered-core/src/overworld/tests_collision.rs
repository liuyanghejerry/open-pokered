use super::collision::*;
use super::*;
use pokered_data::collision;
use pokered_data::tileset_data;
use pokered_data::tilesets::TilesetId;
use pokered_data::maps::MapId;


fn provider() -> PokemonCollisionProvider {
    PokemonCollisionProvider::new(MapId::PalletTown, TilesetId::Overworld)
}
#[test]
fn test_overworld_passable_tiles() {
    assert!(collision::is_tile_passable(TilesetId::Overworld, 0x00));
    assert!(collision::is_tile_passable(TilesetId::Overworld, 0x52));
    assert!(!collision::is_tile_passable(TilesetId::Overworld, 0xFF));
    assert!(!collision::is_tile_passable(TilesetId::Overworld, 0x01));
}

#[test]
fn test_forest_passable_tiles() {
    assert!(collision::is_tile_passable(TilesetId::Forest, 0x20));
    assert!(collision::is_tile_passable(TilesetId::Forest, 0x52));
    assert!(!collision::is_tile_passable(TilesetId::Forest, 0x00));
}

#[test]
fn test_cavern_passable_tiles() {
    assert!(collision::is_tile_passable(TilesetId::Cavern, 0x05));
    assert!(collision::is_tile_passable(TilesetId::Cavern, 0x15));
    assert!(!collision::is_tile_passable(TilesetId::Cavern, 0x00));
}

#[test]
fn test_mart_passable_tiles() {
    assert!(collision::is_tile_passable(TilesetId::Mart, 0x11));
    assert!(collision::is_tile_passable(TilesetId::Mart, 0x1A));
    assert!(!collision::is_tile_passable(TilesetId::Mart, 0x00));
}

#[test]
fn test_all_tilesets_have_passable_tiles() {
    for i in 0..24u8 {
        let ts = TilesetId::from_u8(i).unwrap();
        let tiles = collision::get_passable_tiles(ts);
        assert!(!tiles.is_empty(), "tileset {} has no passable tiles", i);
    }
}

#[test]
fn test_tile_pair_collision_land_forest() {
    assert!(collision::check_tile_pair_collision(
        TilesetId::Forest,
        0x30,
        0x2E,
        false
    ));
    assert!(collision::check_tile_pair_collision(
        TilesetId::Forest,
        0x2E,
        0x30,
        false
    ));
    assert!(!collision::check_tile_pair_collision(
        TilesetId::Forest,
        0x30,
        0x2E,
        true
    ));
}

#[test]
fn test_tile_pair_collision_land_cavern() {
    assert!(collision::check_tile_pair_collision(
        TilesetId::Cavern,
        0x20,
        0x05,
        false
    ));
    assert!(collision::check_tile_pair_collision(
        TilesetId::Cavern,
        0x05,
        0x20,
        false
    ));
    assert!(collision::check_tile_pair_collision(
        TilesetId::Cavern,
        0x05,
        0x21,
        false
    ));
}

#[test]
fn test_tile_pair_collision_water() {
    assert!(collision::check_tile_pair_collision(
        TilesetId::Forest,
        0x14,
        0x2E,
        true
    ));
    assert!(collision::check_tile_pair_collision(
        TilesetId::Cavern,
        0x14,
        0x05,
        true
    ));
    assert!(!collision::check_tile_pair_collision(
        TilesetId::Cavern,
        0x14,
        0x05,
        false
    ));
}

#[test]
fn test_tile_pair_no_collision_wrong_tileset() {
    assert!(!collision::check_tile_pair_collision(
        TilesetId::Overworld,
        0x20,
        0x05,
        false
    ));
    assert!(!collision::check_tile_pair_collision(
        TilesetId::Mart,
        0x30,
        0x2E,
        false
    ));
}

#[test]
fn test_ledge_jump_down() {
    assert!(collision::check_ledge_jump(
        collision::SPRITE_FACING_DOWN,
        0x2C,
        0x37,
        collision::PAD_DOWN
    ));
    assert!(collision::check_ledge_jump(
        collision::SPRITE_FACING_DOWN,
        0x39,
        0x37,
        collision::PAD_DOWN
    ));
}

#[test]
fn test_ledge_jump_left() {
    assert!(collision::check_ledge_jump(
        collision::SPRITE_FACING_LEFT,
        0x2C,
        0x27,
        collision::PAD_LEFT
    ));
}

#[test]
fn test_ledge_jump_right() {
    assert!(collision::check_ledge_jump(
        collision::SPRITE_FACING_RIGHT,
        0x2C,
        0x0D,
        collision::PAD_RIGHT
    ));
}

#[test]
fn test_ledge_jump_wrong_direction() {
    assert!(!collision::check_ledge_jump(
        collision::SPRITE_FACING_UP,
        0x2C,
        0x37,
        collision::PAD_UP
    ));
}

#[test]
fn test_ledge_jump_wrong_input() {
    assert!(!collision::check_ledge_jump(
        collision::SPRITE_FACING_DOWN,
        0x2C,
        0x37,
        collision::PAD_LEFT
    ));
}

#[test]
fn test_ledge_jump_wrong_tile() {
    assert!(!collision::check_ledge_jump(
        collision::SPRITE_FACING_DOWN,
        0x2C,
        0x00,
        collision::PAD_DOWN
    ));
}

#[test]
fn test_direction_to_sprite_facing() {
    assert_eq!(
        direction_to_sprite_facing(Direction::Down),
        collision::SPRITE_FACING_DOWN
    );
    assert_eq!(
        direction_to_sprite_facing(Direction::Up),
        collision::SPRITE_FACING_UP
    );
    assert_eq!(
        direction_to_sprite_facing(Direction::Left),
        collision::SPRITE_FACING_LEFT
    );
    assert_eq!(
        direction_to_sprite_facing(Direction::Right),
        collision::SPRITE_FACING_RIGHT
    );
}

#[test]
fn test_get_target_coords_within_bounds() {
    assert_eq!(get_target_coords(5, 5, Direction::Up, 10, 9), Some((5, 4)));
    assert_eq!(
        get_target_coords(5, 5, Direction::Down, 10, 9),
        Some((5, 6))
    );
    assert_eq!(
        get_target_coords(5, 5, Direction::Left, 10, 9),
        Some((4, 5))
    );
    assert_eq!(
        get_target_coords(5, 5, Direction::Right, 10, 9),
        Some((6, 5))
    );
}

#[test]
fn test_get_target_coords_at_edges() {
    assert_eq!(get_target_coords(0, 0, Direction::Up, 10, 9), None);
    assert_eq!(get_target_coords(0, 0, Direction::Left, 10, 9), None);
    assert_eq!(get_target_coords(19, 17, Direction::Right, 10, 9), None);
    assert_eq!(get_target_coords(19, 17, Direction::Down, 10, 9), None);
}

#[test]
fn test_get_target_coords_boundary() {
    assert_eq!(get_target_coords(0, 1, Direction::Up, 10, 9), Some((0, 0)));
    assert_eq!(
        get_target_coords(1, 0, Direction::Left, 10, 9),
        Some((0, 0))
    );
    assert_eq!(
        get_target_coords(18, 17, Direction::Right, 10, 9),
        Some((19, 17))
    );
    assert_eq!(
        get_target_coords(19, 16, Direction::Down, 10, 9),
        Some((19, 17))
    );
}

#[test]
fn test_is_facing_map_edge() {
    assert!(is_facing_map_edge(0, 0, Direction::Up, 10, 9));
    assert!(is_facing_map_edge(0, 0, Direction::Left, 10, 9));
    assert!(!is_facing_map_edge(5, 5, Direction::Up, 10, 9));
    assert!(is_facing_map_edge(19, 17, Direction::Right, 10, 9));
}

#[test]
fn test_sprite_collision_hit() {
    let npcs = vec![SpritePosition { x: 3, y: 5 }, SpritePosition { x: 7, y: 2 }];
    assert!(check_sprite_collision(3, 5, &npcs));
    assert!(check_sprite_collision(7, 2, &npcs));
}

#[test]
fn test_sprite_collision_miss() {
    let npcs = vec![SpritePosition { x: 3, y: 5 }];
    assert!(!check_sprite_collision(4, 5, &npcs));
    assert!(!check_sprite_collision(3, 6, &npcs));
    assert!(!check_sprite_collision(0, 0, &npcs));
}

#[test]
fn test_sprite_collision_empty() {
    assert!(!check_sprite_collision(5, 5, &[]));
}

#[test]
fn test_get_block_at() {
    let blocks = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    // 10x1 blocks, tile (0,0) and (1,0) = block 0
    assert_eq!(get_block_at(0, 0, 10, &blocks), Some(1));
    assert_eq!(get_block_at(1, 0, 10, &blocks), Some(1));
    assert_eq!(get_block_at(2, 0, 10, &blocks), Some(2));
    assert_eq!(get_block_at(18, 0, 10, &blocks), Some(10));
    assert_eq!(get_block_at(20, 0, 10, &blocks), None);
}

#[test]
fn test_check_movement_collision_passable() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0x00,
        TransportMode::Walking,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::Passable);
}

#[test]
fn test_check_movement_collision_tile_blocked() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0xFF,
        TransportMode::Walking,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::TileBlocked);
}

#[test]
fn test_check_movement_collision_map_edge() {
    let result = check_movement_collision(
        0,
        0,
        Direction::Up,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0x00,
        TransportMode::Walking,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::MapEdge);
}

#[test]
fn test_check_movement_collision_sprite_blocked() {
    let npcs = vec![SpritePosition { x: 5, y: 6 }];
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0x00,
        TransportMode::Walking,
        &npcs,
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::SpriteBlocked);
}

#[test]
fn test_check_movement_collision_ledge_jump() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x2C,
        0x37,
        TransportMode::Walking,
        &[],
        collision::PAD_DOWN,
        &provider(),
    );
    assert_eq!(result, CollisionResult::LedgeJump);
}

#[test]
fn test_check_movement_collision_tile_pair() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Forest,
        10,
        9,
        0x30,
        0x2E,
        TransportMode::Walking,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::TilePairBlocked);
}

#[test]
fn test_check_movement_collision_counter_tile() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Mart,
        10,
        9,
        0x11,
        0x18,
        TransportMode::Walking,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::CounterTile);
}

#[test]
fn test_tileset_header_overworld() {
    let h = tileset_data::get_tileset_header(TilesetId::Overworld);
    assert_eq!(h.grass_tile, Some(0x52));
    assert_eq!(h.animation, tileset_data::TileAnimation::WaterFlower);
    assert_eq!(h.counter_tiles, [None, None, None]);
}

#[test]
fn test_tileset_header_mart() {
    let h = tileset_data::get_tileset_header(TilesetId::Mart);
    assert_eq!(h.counter_tiles, [Some(0x18), Some(0x19), Some(0x1E)]);
    assert_eq!(h.grass_tile, None);
    assert!(h.is_counter_tile(0x18));
    assert!(h.is_counter_tile(0x19));
    assert!(h.is_counter_tile(0x1E));
    assert!(!h.is_counter_tile(0x00));
}

#[test]
fn test_tileset_header_forest() {
    let h = tileset_data::get_tileset_header(TilesetId::Forest);
    assert_eq!(h.grass_tile, Some(0x20));
    assert_eq!(h.animation, tileset_data::TileAnimation::Water);
    assert!(h.is_grass_tile(0x20));
    assert!(!h.is_grass_tile(0x52));
}

#[test]
fn test_tileset_header_cavern() {
    let h = tileset_data::get_tileset_header(TilesetId::Cavern);
    assert_eq!(h.grass_tile, None);
    assert_eq!(h.animation, tileset_data::TileAnimation::Water);
}

#[test]
fn test_tileset_header_plateau() {
    let h = tileset_data::get_tileset_header(TilesetId::Plateau);
    assert_eq!(h.grass_tile, Some(0x45));
    assert_eq!(h.animation, tileset_data::TileAnimation::Water);
}

#[test]
fn test_tileset_header_dojo_counter() {
    let h = tileset_data::get_tileset_header(TilesetId::Dojo);
    assert_eq!(h.counter_tiles, [Some(0x3A), None, None]);
    assert!(h.is_counter_tile(0x3A));
}

#[test]
fn test_get_grass_tile() {
    assert_eq!(
        tileset_data::get_grass_tile(TilesetId::Overworld),
        Some(0x52)
    );
    assert_eq!(tileset_data::get_grass_tile(TilesetId::Forest), Some(0x20));
    assert_eq!(tileset_data::get_grass_tile(TilesetId::Plateau), Some(0x45));
    assert_eq!(tileset_data::get_grass_tile(TilesetId::Mart), None);
    assert_eq!(tileset_data::get_grass_tile(TilesetId::Cavern), None);
}

#[test]
fn test_has_water_animation() {
    assert!(tileset_data::has_water_animation(TilesetId::Overworld));
    assert!(tileset_data::has_water_animation(TilesetId::Forest));
    assert!(tileset_data::has_water_animation(TilesetId::Cavern));
    assert!(tileset_data::has_water_animation(TilesetId::Ship));
    assert!(!tileset_data::has_water_animation(TilesetId::Mart));
    assert!(!tileset_data::has_water_animation(TilesetId::House));
}

#[test]
fn test_check_warp_at_position() {
    use pokered_data::maps::MapId;
    use pokered_data::music::MusicId;
    let map = MapData::new(
        MapId::PalletTown,
        10,
        9,
        TilesetId::Overworld,
        MusicId::PalletTown,
        vec![0; 90],
        vec![
            WarpPoint::new(3, 4, MapId::RedsHouse1F, 0),
            WarpPoint::new(7, 4, MapId::BluesHouse, 0),
        ],
        vec![],
        vec![],
        MapConnections::default(),
    );
    assert_eq!(check_warp_at_position(3, 4, &map), Some(0));
    assert_eq!(check_warp_at_position(7, 4, &map), Some(1));
    assert_eq!(check_warp_at_position(0, 0, &map), None);
    assert_eq!(check_warp_at_position(4, 4, &map), None);
    assert_eq!(check_warp_at_position(3, 5, &map), None);
}

// ── Surfing movement (CollisionCheckOnWater, home/overworld.asm) ──────

#[test]
fn test_surf_onto_water_keeps_surfing() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x14, // standing on water
        0x14, // water ahead
        TransportMode::Surfing,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::Passable);
}

#[test]
fn test_surf_onto_shore_tile_keeps_surfing() {
    // Eastern shoreline ($32) counts as water except on SHIP_PORT.
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x14,
        0x32,
        TransportMode::Surfing,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::Passable);
}

#[test]
fn test_surf_onto_passable_land_stops_surfing() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x14, // standing on water
        0x00, // passable land ahead
        TransportMode::Surfing,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::StopSurfing);
}

#[test]
fn test_surf_into_wall_blocked() {
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x14,
        0xFF,
        TransportMode::Surfing,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::TileBlocked);
}

#[test]
fn test_surf_water_tile_pair_blocked() {
    // FOREST TilePairCollisionsWater: $14 -> $2E is blocked even while surfing.
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Forest,
        10,
        9,
        0x14,
        0x2E,
        TransportMode::Surfing,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::TilePairBlocked);
}

#[test]
fn test_walk_into_water_blocked() {
    // Water ($14) is not in the overworld passable list, so walking into it
    // is an ordinary tile collision — surfing is the only way across.
    let result = check_movement_collision(
        5,
        5,
        Direction::Down,
        TilesetId::Overworld,
        10,
        9,
        0x00,
        0x14,
        TransportMode::Walking,
        &[],
        0,
        &provider(),
    );
    assert_eq!(result, CollisionResult::TileBlocked);
}
