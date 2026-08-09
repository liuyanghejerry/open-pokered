use dotzuki_engine::tileset::TilesetTrait;
use pokered_data::elevator_data::{
    elevator_data, elevator_for_map, ElevatorData, ElevatorFloor, ElevatorId,
};
use pokered_data::fly_warp_data::{
    dungeon_warp_for, fly_destination_for_map, link_warp, DungeonWarpEntry, FlyDestination,
    LinkWarpType, SpecialWarpEntry, NEW_GAME_WARP, TELEPORT_SPIN_ORDER,
};
use pokered_data::maps::MapId;
use pokered_data::tileset_data::is_door_tile;
use pokered_data::tilesets::TilesetId;

use super::Direction;

pub fn is_standing_on_door<T: TilesetTrait>(tileset: T, standing_tile: u8) -> bool {
    let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
    is_door_tile(concrete, standing_tile)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElevatorWarpResult {
    pub warp_id: u8,
    pub map_id: MapId,
    pub floor: ElevatorFloor,
}

pub fn get_elevator_for_map(map_id: MapId) -> Option<(ElevatorId, ElevatorData)> {
    elevator_for_map(map_id).map(|id| (id, elevator_data(id)))
}

pub fn select_elevator_floor(
    elevator_id: ElevatorId,
    floor_index: usize,
) -> Option<ElevatorWarpResult> {
    let data = elevator_data(elevator_id);
    if floor_index >= data.floors.len() {
        return None;
    }
    let floor = data.floors[floor_index];
    let warp = data.warp_maps[floor_index];
    Some(ElevatorWarpResult {
        warp_id: warp.warp_id,
        map_id: warp.map_id,
        floor,
    })
}

pub fn elevator_floor_count(elevator_id: ElevatorId) -> usize {
    elevator_data(elevator_id).floors.len()
}

pub fn elevator_floors(elevator_id: ElevatorId) -> &'static [ElevatorFloor] {
    elevator_data(elevator_id).floors
}

pub fn resolve_fly_destination(map_id: MapId) -> Option<&'static FlyDestination> {
    fly_destination_for_map(map_id)
}

pub fn teleport_spin_direction(step: usize) -> Direction {
    let facing_byte = TELEPORT_SPIN_ORDER[step % 4];
    match facing_byte {
        0 => Direction::Down,
        4 => Direction::Up,
        8 => Direction::Left,
        12 => Direction::Right,
        _ => Direction::Down,
    }
}

pub fn resolve_dungeon_warp(map_id: MapId, warp_id: u8) -> Option<&'static DungeonWarpEntry> {
    dungeon_warp_for(map_id, warp_id)
}

pub fn is_dungeon_warp_map(map_id: MapId) -> bool {
    pokered_data::fly_warp_data::DUNGEON_WARPS
        .iter()
        .any(|e| e.map_id == map_id)
}

pub fn new_game_warp() -> &'static SpecialWarpEntry {
    &NEW_GAME_WARP
}

pub fn resolve_link_warp(warp_type: LinkWarpType) -> SpecialWarpEntry {
    link_warp(warp_type)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElevatorShakeParams {
    pub iterations: u8,
    pub pixel_offset: u8,
}

pub const fn elevator_shake_params() -> ElevatorShakeParams {
    ElevatorShakeParams {
        iterations: 100,
        pixel_offset: 1,
    }
}
