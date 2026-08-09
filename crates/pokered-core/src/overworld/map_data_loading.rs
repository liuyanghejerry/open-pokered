use dotzuki_engine::overworld::{
    Direction, MapConnection, MapConnections, NpcDefinition, NpcMovementType, Sign, WarpPoint,
};
use dotzuki_engine::tileset::{TilesetProvider, TilesetTrait};
use pokered_data::map_data_loader::{get_block_data, get_map_json, resolve_map_id};
use pokered_data::map_json::{ConnectionEntryJson, ConnectionsJson, NpcJson, SignJson, WarpJson};
use pokered_data::maps::MapId;
use pokered_data::music::MusicId;
use pokered_data::tilesets::TilesetId;

use super::{MapData, PokemonNpcData};

/// Load map data and return both the engine-level [`MapData`] and
/// Pokémon-specific NPC data in parallel vectors.
pub fn load_full_map_data<T: TilesetTrait>(
    map_id: MapId,
    provider: &dyn TilesetProvider<T>,
) -> (MapData<T>, Vec<PokemonNpcData>) {
    let map_json = get_map_json(map_id).unwrap_or_else(|| {
        panic!(
            "No map.json found for {:?} — ensure map data is generated",
            map_id
        )
    });

    let (width, height) = map_id.dimensions();

    let blocks = get_block_data(map_id).to_vec();

    let warps: Vec<WarpPoint<MapId>> = map_json
        .warps
        .iter()
        .map(|w| convert_warp(w, map_id))
        .collect();

    let (npcs, npc_pokemon_data): (Vec<NpcDefinition>, Vec<PokemonNpcData>) =
        map_json.npcs.iter().map(convert_npc).unzip();

    let signs: Vec<Sign> = map_json.signs.iter().map(convert_sign).collect();

    let connections = convert_connections(&map_json.connections);

    let tileset = provider
        .tileset_by_name(&map_json.header.tileset)
        .or_else(|| provider.tileset_by_name("Overworld"))
        .unwrap_or_else(|| {
            panic!("No tileset found for '{}'", map_json.header.tileset)
        });

    let music = MusicId::from_name(&map_json.header.music).unwrap_or_else(|| {
        log::warn!(
            "Unknown music '{}' for map {:?}, defaulting to PalletTown",
            map_json.header.music,
            map_id
        );
        MusicId::PalletTown
    });

    (
        MapData::new(
            map_id,
            width,
            height,
            tileset,
            music,
            blocks,
            warps,
            npcs,
            signs,
            connections,
        ),
        npc_pokemon_data,
    )
}

pub fn load_full_map_data_concrete(map_id: MapId) -> (MapData<TilesetId>, Vec<PokemonNpcData>) {
    let map_json = get_map_json(map_id).unwrap_or_else(|| {
        panic!(
            "No map.json found for {:?} — ensure map data is generated",
            map_id
        )
    });

    let (width, height) = map_id.dimensions();

    let blocks = get_block_data(map_id).to_vec();

    let warps: Vec<WarpPoint<MapId>> = map_json
        .warps
        .iter()
        .map(|w| convert_warp(w, map_id))
        .collect();

    let (npcs, npc_pokemon_data): (Vec<NpcDefinition>, Vec<PokemonNpcData>) =
        map_json.npcs.iter().map(convert_npc).unzip();

    let signs: Vec<Sign> = map_json.signs.iter().map(convert_sign).collect();

    let connections = convert_connections(&map_json.connections);

    let tileset = TilesetId::from_name(&map_json.header.tileset).unwrap_or_else(|| {
        log::warn!(
            "Unknown tileset '{}' for map {:?}, defaulting to Overworld",
            map_json.header.tileset,
            map_id
        );
        TilesetId::Overworld
    });
    let music = MusicId::from_name(&map_json.header.music).unwrap_or_else(|| {
        log::warn!(
            "Unknown music '{}' for map {:?}, defaulting to PalletTown",
            map_json.header.music,
            map_id
        );
        MusicId::PalletTown
    });

    (
        MapData::new(
            map_id,
            width,
            height,
            tileset,
            music,
            blocks,
            warps,
            npcs,
            signs,
            connections,
        ),
        npc_pokemon_data,
    )
}

fn convert_npc(npc: &NpcJson) -> (NpcDefinition, PokemonNpcData) {
    let def = NpcDefinition::new(
        npc.sprite_id,
        npc.x,
        npc.y,
        parse_movement_type(&npc.movement),
        parse_direction(&npc.facing),
        npc.range,
        npc.text_id,
    );
    let extra = PokemonNpcData {
        is_trainer: npc.is_trainer,
        trainer_class: npc
            .trainer_class
            .as_ref()
            .map(|name| parse_trainer_class(name))
            .unwrap_or(0),
        trainer_set: npc.trainer_set.unwrap_or(0),
        item_id: npc.item_id.unwrap_or(0),
        end_battle_text: npc.end_battle_text.clone(),
    };
    (def, extra)
}

fn convert_warp(warp: &WarpJson, current_map: MapId) -> WarpPoint<MapId> {
    let is_last_map = warp.dest_map.is_none();
    let target_map = warp
        .dest_map
        .as_ref()
        .and_then(|name| resolve_map_id(name))
        .unwrap_or(current_map);
    let mut wp = WarpPoint::new(warp.x, warp.y, target_map, warp.dest_warp_id);
    wp.is_last_map = is_last_map;
    wp
}

fn convert_sign(sign: &SignJson) -> Sign {
    Sign::new(sign.x, sign.y, sign.text_id)
}

fn convert_connections(conns: &ConnectionsJson) -> MapConnections<MapId> {
    let mut result = MapConnections::default();
    result.north = conns
        .north
        .as_ref()
        .and_then(|c| convert_connection_entry(c, Direction::Up));
    result.south = conns
        .south
        .as_ref()
        .and_then(|c| convert_connection_entry(c, Direction::Down));
    result.west = conns
        .west
        .as_ref()
        .and_then(|c| convert_connection_entry(c, Direction::Left));
    result.east = conns
        .east
        .as_ref()
        .and_then(|c| convert_connection_entry(c, Direction::Right));
    result
}

fn convert_connection_entry(
    entry: &ConnectionEntryJson,
    direction: Direction,
) -> Option<MapConnection<MapId>> {
    let target_map = resolve_map_id(&entry.target_map)?;
    Some(MapConnection::new(direction, target_map, entry.offset))
}

fn parse_movement_type(s: &str) -> NpcMovementType {
    match s {
        "Stationary" => NpcMovementType::Stationary,
        "Wander" => NpcMovementType::Wander,
        "FixedPath" => NpcMovementType::FixedPath,
        "FacePlayer" => NpcMovementType::FacePlayer,
        _ => {
            log::warn!(
                "Unknown NPC movement type '{}', defaulting to Stationary",
                s
            );
            NpcMovementType::Stationary
        }
    }
}

fn parse_direction(s: &str) -> Direction {
    match s {
        "Down" => Direction::Down,
        "Up" => Direction::Up,
        "Left" => Direction::Left,
        "Right" => Direction::Right,
        _ => {
            log::warn!("Unknown direction '{}', defaulting to Down", s);
            Direction::Down
        }
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
        _ => {
            log::warn!("Unknown trainer class '{}', defaulting to 0", name);
            0
        }
    }
}
