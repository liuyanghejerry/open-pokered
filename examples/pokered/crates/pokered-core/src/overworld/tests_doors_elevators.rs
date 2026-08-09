use pokered_data::elevator_data::{ElevatorFloor, ElevatorId};
use pokered_data::fly_warp_data::LinkWarpType;
use pokered_data::maps::MapId;
use pokered_data::tilesets::TilesetId;

use super::doors_elevators::*;
use super::Direction;

// ── Door Detection Tests ──────────────────────────────────────────────

#[test]
fn overworld_door_tile_0x1b() {
    assert!(is_standing_on_door(TilesetId::Overworld, 0x1B));
}

#[test]
fn overworld_door_tile_0x58() {
    assert!(is_standing_on_door(TilesetId::Overworld, 0x58));
}

#[test]
fn overworld_non_door_tile() {
    assert!(!is_standing_on_door(TilesetId::Overworld, 0x00));
}

#[test]
fn forest_door_tile() {
    assert!(is_standing_on_door(TilesetId::Forest, 0x3A));
}

#[test]
fn forest_non_door_tile() {
    assert!(!is_standing_on_door(TilesetId::Forest, 0x3B));
}

#[test]
fn mart_door_tile() {
    assert!(is_standing_on_door(TilesetId::Mart, 0x5E));
}

#[test]
fn house_door_tile() {
    assert!(is_standing_on_door(TilesetId::House, 0x54));
}

#[test]
fn forest_gate_door_tile() {
    assert!(is_standing_on_door(TilesetId::ForestGate, 0x3B));
}

#[test]
fn museum_door_tile() {
    assert!(is_standing_on_door(TilesetId::Museum, 0x3B));
}

#[test]
fn gate_door_tile() {
    assert!(is_standing_on_door(TilesetId::Gate, 0x3B));
}

#[test]
fn ship_door_tile() {
    assert!(is_standing_on_door(TilesetId::Ship, 0x1E));
}

#[test]
fn lobby_door_tiles() {
    assert!(is_standing_on_door(TilesetId::Lobby, 0x1C));
    assert!(is_standing_on_door(TilesetId::Lobby, 0x38));
    assert!(is_standing_on_door(TilesetId::Lobby, 0x1A));
}

#[test]
fn mansion_door_tiles() {
    assert!(is_standing_on_door(TilesetId::Mansion, 0x1A));
    assert!(is_standing_on_door(TilesetId::Mansion, 0x1C));
    assert!(is_standing_on_door(TilesetId::Mansion, 0x53));
}

#[test]
fn lab_door_tile() {
    assert!(is_standing_on_door(TilesetId::Lab, 0x34));
}

#[test]
fn facility_door_tiles() {
    assert!(is_standing_on_door(TilesetId::Facility, 0x43));
    assert!(is_standing_on_door(TilesetId::Facility, 0x58));
    assert!(is_standing_on_door(TilesetId::Facility, 0x1B));
}

#[test]
fn plateau_door_tiles() {
    assert!(is_standing_on_door(TilesetId::Plateau, 0x3B));
    assert!(is_standing_on_door(TilesetId::Plateau, 0x1B));
}

#[test]
fn tileset_without_doors_returns_false() {
    assert!(!is_standing_on_door(TilesetId::Gym, 0x00));
    assert!(!is_standing_on_door(TilesetId::Gym, 0xFF));
}

// ── Elevator Detection Tests ──────────────────────────────────────────

#[test]
fn rocket_hideout_elevator_map() {
    let result = get_elevator_for_map(MapId::RocketHideoutElevator);
    assert!(result.is_some());
    let (id, _data) = result.unwrap();
    assert_eq!(id, ElevatorId::RocketHideout);
}

#[test]
fn celadon_mart_elevator_map() {
    let result = get_elevator_for_map(MapId::CeladonMartElevator);
    assert!(result.is_some());
    let (id, _data) = result.unwrap();
    assert_eq!(id, ElevatorId::CeladonMart);
}

#[test]
fn silph_co_elevator_map() {
    let result = get_elevator_for_map(MapId::SilphCoElevator);
    assert!(result.is_some());
    let (id, _data) = result.unwrap();
    assert_eq!(id, ElevatorId::SilphCo);
}

#[test]
fn non_elevator_map_returns_none() {
    assert!(get_elevator_for_map(MapId::PalletTown).is_none());
}

// ── Elevator Floor Selection Tests ────────────────────────────────────

#[test]
fn rocket_hideout_has_3_floors() {
    assert_eq!(elevator_floor_count(ElevatorId::RocketHideout), 3);
}

#[test]
fn celadon_mart_has_5_floors() {
    assert_eq!(elevator_floor_count(ElevatorId::CeladonMart), 5);
}

#[test]
fn silph_co_has_11_floors() {
    assert_eq!(elevator_floor_count(ElevatorId::SilphCo), 11);
}

#[test]
fn rocket_hideout_floor_list() {
    let floors = elevator_floors(ElevatorId::RocketHideout);
    assert_eq!(
        floors,
        &[ElevatorFloor::B1F, ElevatorFloor::B2F, ElevatorFloor::B4F]
    );
}

#[test]
fn select_rocket_hideout_b1f() {
    let result = select_elevator_floor(ElevatorId::RocketHideout, 0).unwrap();
    assert_eq!(result.floor, ElevatorFloor::B1F);
    assert_eq!(result.map_id, MapId::RocketHideoutB1F);
    assert_eq!(result.warp_id, 4);
}

#[test]
fn select_rocket_hideout_b2f() {
    let result = select_elevator_floor(ElevatorId::RocketHideout, 1).unwrap();
    assert_eq!(result.floor, ElevatorFloor::B2F);
    assert_eq!(result.map_id, MapId::RocketHideoutB2F);
    assert_eq!(result.warp_id, 4);
}

#[test]
fn select_rocket_hideout_b4f() {
    let result = select_elevator_floor(ElevatorId::RocketHideout, 2).unwrap();
    assert_eq!(result.floor, ElevatorFloor::B4F);
    assert_eq!(result.map_id, MapId::RocketHideoutB4F);
    assert_eq!(result.warp_id, 2);
}

#[test]
fn select_celadon_mart_f1() {
    let result = select_elevator_floor(ElevatorId::CeladonMart, 0).unwrap();
    assert_eq!(result.floor, ElevatorFloor::F1);
    assert_eq!(result.map_id, MapId::CeladonMart1F);
    assert_eq!(result.warp_id, 5);
}

#[test]
fn select_celadon_mart_f5() {
    let result = select_elevator_floor(ElevatorId::CeladonMart, 4).unwrap();
    assert_eq!(result.floor, ElevatorFloor::F5);
    assert_eq!(result.map_id, MapId::CeladonMart5F);
    assert_eq!(result.warp_id, 2);
}

#[test]
fn select_silph_co_f1() {
    let result = select_elevator_floor(ElevatorId::SilphCo, 0).unwrap();
    assert_eq!(result.floor, ElevatorFloor::F1);
    assert_eq!(result.map_id, MapId::SilphCo1F);
    assert_eq!(result.warp_id, 3);
}

#[test]
fn select_silph_co_f11() {
    let result = select_elevator_floor(ElevatorId::SilphCo, 10).unwrap();
    assert_eq!(result.floor, ElevatorFloor::F11);
    assert_eq!(result.map_id, MapId::SilphCo11F);
    assert_eq!(result.warp_id, 1);
}

#[test]
fn select_elevator_floor_out_of_bounds() {
    assert!(select_elevator_floor(ElevatorId::RocketHideout, 3).is_none());
    assert!(select_elevator_floor(ElevatorId::CeladonMart, 5).is_none());
    assert!(select_elevator_floor(ElevatorId::SilphCo, 11).is_none());
}

// ── Elevator Floor Values (Item Constants) ────────────────────────────

#[test]
fn elevator_floor_values_match_item_constants() {
    assert_eq!(ElevatorFloor::B4F as u8, 0x61);
    assert_eq!(ElevatorFloor::B2F as u8, 0x54);
    assert_eq!(ElevatorFloor::B1F as u8, 0x55);
    assert_eq!(ElevatorFloor::F1 as u8, 0x56);
    assert_eq!(ElevatorFloor::F2 as u8, 0x57);
    assert_eq!(ElevatorFloor::F3 as u8, 0x58);
    assert_eq!(ElevatorFloor::F4 as u8, 0x59);
    assert_eq!(ElevatorFloor::F5 as u8, 0x5A);
    assert_eq!(ElevatorFloor::F6 as u8, 0x5B);
    assert_eq!(ElevatorFloor::F7 as u8, 0x5C);
    assert_eq!(ElevatorFloor::F8 as u8, 0x5D);
    assert_eq!(ElevatorFloor::F9 as u8, 0x5E);
    assert_eq!(ElevatorFloor::F10 as u8, 0x5F);
    assert_eq!(ElevatorFloor::F11 as u8, 0x60);
}

// ── Fly Destination Tests ─────────────────────────────────────────────

#[test]
fn fly_destination_pallet_town() {
    let dest = resolve_fly_destination(MapId::PalletTown).unwrap();
    assert_eq!(dest.map_id, MapId::PalletTown);
    assert_eq!(dest.coords.map_width, 10);
    assert_eq!(dest.coords.x, 5);
    assert_eq!(dest.coords.y, 6);
}

#[test]
fn fly_destination_viridian_city() {
    let dest = resolve_fly_destination(MapId::ViridianCity).unwrap();
    assert_eq!(dest.map_id, MapId::ViridianCity);
    assert_eq!(dest.coords.map_width, 20);
}

#[test]
fn fly_destination_cinnabar_island() {
    let dest = resolve_fly_destination(MapId::CinnabarIsland).unwrap();
    assert_eq!(dest.map_id, MapId::CinnabarIsland);
    assert_eq!(dest.coords.map_width, 10);
    assert_eq!(dest.coords.x, 11);
    assert_eq!(dest.coords.y, 12);
}

#[test]
fn fly_destination_indigo_plateau() {
    let dest = resolve_fly_destination(MapId::IndigoPlateau).unwrap();
    assert_eq!(dest.map_id, MapId::IndigoPlateau);
}

#[test]
fn fly_destination_route_4() {
    let dest = resolve_fly_destination(MapId::Route4).unwrap();
    assert_eq!(dest.map_id, MapId::Route4);
    assert_eq!(dest.coords.map_width, 45);
}

#[test]
fn fly_destination_route_10() {
    let dest = resolve_fly_destination(MapId::Route10).unwrap();
    assert_eq!(dest.map_id, MapId::Route10);
    assert_eq!(dest.coords.map_width, 10);
}

#[test]
fn fly_destination_count_is_13() {
    let valid_destinations: Vec<_> = [
        MapId::PalletTown,
        MapId::ViridianCity,
        MapId::PewterCity,
        MapId::CeruleanCity,
        MapId::LavenderTown,
        MapId::VermilionCity,
        MapId::CeladonCity,
        MapId::FuchsiaCity,
        MapId::CinnabarIsland,
        MapId::IndigoPlateau,
        MapId::SaffronCity,
        MapId::Route4,
        MapId::Route10,
    ]
    .iter()
    .filter_map(|&m| resolve_fly_destination(m))
    .collect();
    assert_eq!(valid_destinations.len(), 13);
}

#[test]
fn fly_destination_invalid_map_returns_none() {
    assert!(resolve_fly_destination(MapId::RedsHouse1F).is_none());
    assert!(resolve_fly_destination(MapId::RocketHideoutB1F).is_none());
}

// ── Teleport Spin Direction Tests ─────────────────────────────────────

#[test]
fn teleport_spin_step_0_is_down() {
    assert_eq!(teleport_spin_direction(0), Direction::Down);
}

#[test]
fn teleport_spin_step_1_is_left() {
    assert_eq!(teleport_spin_direction(1), Direction::Left);
}

#[test]
fn teleport_spin_step_2_is_up() {
    assert_eq!(teleport_spin_direction(2), Direction::Up);
}

#[test]
fn teleport_spin_step_3_is_right() {
    assert_eq!(teleport_spin_direction(3), Direction::Right);
}

#[test]
fn teleport_spin_wraps_around() {
    assert_eq!(teleport_spin_direction(4), Direction::Down);
    assert_eq!(teleport_spin_direction(5), Direction::Left);
    assert_eq!(teleport_spin_direction(7), Direction::Right);
}

// ── Dungeon Warp Tests ────────────────────────────────────────────────

#[test]
fn dungeon_warp_seafoam_b1f_warp1() {
    let entry = resolve_dungeon_warp(MapId::SeafoamIslandsB1F, 1).unwrap();
    assert_eq!(entry.map_id, MapId::SeafoamIslandsB1F);
    assert_eq!(entry.warp_id, 1);
    assert_eq!(entry.coords.map_width, 15);
    assert_eq!(entry.coords.x, 18);
}

#[test]
fn dungeon_warp_seafoam_b1f_warp2() {
    let entry = resolve_dungeon_warp(MapId::SeafoamIslandsB1F, 2).unwrap();
    assert_eq!(entry.warp_id, 2);
    assert_eq!(entry.coords.x, 23);
}

#[test]
fn dungeon_warp_seafoam_b4f_warp1() {
    let entry = resolve_dungeon_warp(MapId::SeafoamIslandsB4F, 1).unwrap();
    assert_eq!(entry.coords.x, 4);
    assert_eq!(entry.coords.y, 14);
}

#[test]
fn dungeon_warp_victory_road_2f() {
    let entry = resolve_dungeon_warp(MapId::VictoryRoad2F, 2).unwrap();
    assert_eq!(entry.coords.x, 22);
    assert_eq!(entry.coords.y, 16);
}

#[test]
fn dungeon_warp_pokemon_mansion_1f_warp1() {
    let entry = resolve_dungeon_warp(MapId::PokemonMansion1F, 1).unwrap();
    assert_eq!(entry.coords.x, 16);
    assert_eq!(entry.coords.y, 14);
}

#[test]
fn dungeon_warp_pokemon_mansion_2f() {
    let entry = resolve_dungeon_warp(MapId::PokemonMansion2F, 3).unwrap();
    assert_eq!(entry.coords.x, 18);
    assert_eq!(entry.coords.y, 14);
}

#[test]
fn dungeon_warp_nonexistent_returns_none() {
    assert!(resolve_dungeon_warp(MapId::PalletTown, 1).is_none());
    assert!(resolve_dungeon_warp(MapId::SeafoamIslandsB1F, 99).is_none());
}

#[test]
fn is_dungeon_warp_map_positive() {
    assert!(is_dungeon_warp_map(MapId::SeafoamIslandsB1F));
    assert!(is_dungeon_warp_map(MapId::SeafoamIslandsB2F));
    assert!(is_dungeon_warp_map(MapId::SeafoamIslandsB3F));
    assert!(is_dungeon_warp_map(MapId::SeafoamIslandsB4F));
    assert!(is_dungeon_warp_map(MapId::VictoryRoad2F));
    assert!(is_dungeon_warp_map(MapId::PokemonMansion1F));
    assert!(is_dungeon_warp_map(MapId::PokemonMansion2F));
}

#[test]
fn is_dungeon_warp_map_negative() {
    assert!(!is_dungeon_warp_map(MapId::PalletTown));
    assert!(!is_dungeon_warp_map(MapId::RocketHideoutB1F));
}

// ── Special Warp Tests ────────────────────────────────────────────────

#[test]
fn new_game_warp_is_reds_house_2f() {
    let warp = new_game_warp();
    assert_eq!(warp.map_id, MapId::RedsHouse2F);
    assert_eq!(warp.coords.map_width, 4);
    assert_eq!(warp.coords.x, 3);
    assert_eq!(warp.coords.y, 6);
    assert_eq!(warp.tileset_id, 2);
}

#[test]
fn link_warp_trade_center_player() {
    let warp = resolve_link_warp(LinkWarpType::TradeCenterPlayer);
    assert_eq!(warp.map_id, MapId::TradeCenter);
    assert_eq!(warp.coords.x, 3);
    assert_eq!(warp.coords.y, 4);
    assert_eq!(warp.tileset_id, 8);
}

#[test]
fn link_warp_trade_center_friend() {
    let warp = resolve_link_warp(LinkWarpType::TradeCenterFriend);
    assert_eq!(warp.map_id, MapId::TradeCenter);
    assert_eq!(warp.coords.x, 6);
}

#[test]
fn link_warp_colosseum_player() {
    let warp = resolve_link_warp(LinkWarpType::ColosseumPlayer);
    assert_eq!(warp.map_id, MapId::Colosseum);
    assert_eq!(warp.coords.x, 3);
}

#[test]
fn link_warp_colosseum_friend() {
    let warp = resolve_link_warp(LinkWarpType::ColosseumFriend);
    assert_eq!(warp.map_id, MapId::Colosseum);
    assert_eq!(warp.coords.x, 6);
}

// ── Elevator Shake Params Tests ───────────────────────────────────────

#[test]
fn elevator_shake_params_values() {
    let params = elevator_shake_params();
    assert_eq!(params.iterations, 100);
    assert_eq!(params.pixel_offset, 1);
}
