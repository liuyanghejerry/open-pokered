use jrpg_engine::map::MapProvider;
use jrpg_engine::tileset::{TilesetProvider, TilesetTrait};
use pokered_data::map_data_loader::get_map_json;
use pokered_data::map_flags;
use pokered_data::map_names::{self, MapNameId};
use pokered_data::maps::{MapId, NUM_MAPS};
use pokered_data::music::MusicId;
use pokered_data::tilesets::TilesetId;

pub fn get_map_dimensions(map: MapId) -> (u8, u8) {
    map.dimensions()
}

pub fn get_map_dimensions_via_provider(provider: &dyn MapProvider<MapId>, map: MapId) -> (u8, u8) {
    provider.dimensions(map)
}

pub fn get_map_tileset_with_provider<T: TilesetTrait>(map: MapId, provider: &dyn TilesetProvider<T>) -> T {
    let json = get_map_json(map);
    json.and_then(|j| provider.tileset_by_name(&j.header.tileset))
        .or_else(|| provider.tileset_by_name("Overworld"))
        .unwrap()
}

pub fn get_map_tileset(map: MapId) -> TilesetId {
    get_map_json(map)
        .and_then(|j| TilesetId::from_name(&j.header.tileset))
        .unwrap_or(TilesetId::Overworld)
}

pub fn get_map_music(map: MapId) -> MusicId {
    get_map_json(map)
        .and_then(|j| MusicId::from_name(&j.header.music))
        .unwrap_or(MusicId::PalletTown)
}

pub fn has_connection(map: MapId, direction: super::Direction) -> bool {
    let Some(j) = get_map_json(map) else {
        return false;
    };
    match direction {
        super::Direction::Up => j.connections.north.is_some(),
        super::Direction::Down => j.connections.south.is_some(),
        super::Direction::Left => j.connections.west.is_some(),
        super::Direction::Right => j.connections.east.is_some(),
    }
}

pub fn connection_count(map: MapId) -> u8 {
    let Some(j) = get_map_json(map) else {
        return 0;
    };
    let c = &j.connections;
    c.north.is_some() as u8
        + c.south.is_some() as u8
        + c.west.is_some() as u8
        + c.east.is_some() as u8
}

pub fn get_map_block_count(map: MapId) -> u16 {
    let (w, h) = map.dimensions();
    w as u16 * h as u16
}

pub fn is_valid_map_id(id: u8) -> bool {
    (id as usize) < NUM_MAPS
}

pub fn maps_with_tileset(tileset: TilesetId) -> Vec<MapId> {
    (0..NUM_MAPS as u8)
        .filter_map(|i| {
            let map = MapId::from_u8(i)?;
            let j = get_map_json(map)?;
            let ts = TilesetId::from_name(&j.header.tileset)?;
            if ts == tileset {
                Some(map)
            } else {
                None
            }
        })
        .collect()
}

pub fn maps_with_music(music: MusicId) -> Vec<MapId> {
    (0..NUM_MAPS as u8)
        .filter_map(|i| {
            let map = MapId::from_u8(i)?;
            let j = get_map_json(map)?;
            let m = MusicId::from_name(&j.header.music)?;
            if m == music {
                Some(map)
            } else {
                None
            }
        })
        .collect()
}

pub fn connected_outdoor_maps() -> Vec<MapId> {
    (0..NUM_MAPS as u8)
        .filter_map(|i| {
            let map = MapId::from_u8(i)?;
            let j = get_map_json(map)?;
            let c = &j.connections;
            if c.north.is_some() || c.south.is_some() || c.west.is_some() || c.east.is_some() {
                Some(map)
            } else {
                None
            }
        })
        .collect()
}

pub fn get_map_name(map: MapId) -> &'static str {
    map_names::map_name_for_map(map, false)
}

pub fn get_map_name_id(map: MapId) -> MapNameId {
    map_names::map_to_name_id(map)
}

pub fn is_dungeon_map(map: MapId) -> bool {
    map_flags::is_dungeon_map(map)
}

pub fn check_force_bike_surf(map: MapId, x: u8, y: u8) -> bool {
    map_flags::check_force_bike_surf(map, x, y)
}

pub fn is_safari_rest_house(map: MapId) -> bool {
    map_flags::is_safari_rest_house(map)
}

pub fn badge_for_gym(map: MapId) -> Option<u8> {
    map_flags::badge_for_gym(map)
}
