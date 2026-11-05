//! Native stationary map-view tiles, sampled from the exact raster map source.
//! This is the map view, not a party/text buffer or an original WRAM snapshot.
//! Capturing it neither copies VRAM nor acknowledges a video/IRQ request.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct MapTileSnapshot {
    pub map: u8,
    pub tileset: String,
    pub world_tile_origin: (i32, i32),
    pub tiles: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) enum MapTileSnapshotError {
    MovingViewport,
    MissingMap,
    MissingTileset,
    InvalidLiveGrid,
    MissingBlockTile,
}

pub(crate) fn stationary_map_tile_snapshot(
    screen: &OverworldScreen,
) -> Result<MapTileSnapshot, MapTileSnapshotError> {
    map_tile_snapshot(screen, false)
}

pub(crate) fn surrounding_map_tile_snapshot(screen: &OverworldScreen) -> Result<MapTileSnapshot, MapTileSnapshotError> {
    map_tile_snapshot(screen, true)
}

fn map_tile_snapshot(screen: &OverworldScreen, surrounding: bool) -> Result<MapTileSnapshot, MapTileSnapshotError> {
    use MapTileSnapshotError::*;
    let map = screen.state.current_map;
    let player = (screen.state.player.x, screen.state.player.y);
    if screen.state.player.movement_state != MovementState::Idle
        || screen.ledge_jump.is_some()
        || screen.field_move_step.is_some()
        || screen.elevator_shake.is_some()
        || screen.ship_departure.is_some()
        || screen
            .field_viewport
            .residual(map, player)
            .is_some_and(|xy| xy != (0, 0))
    {
        return Err(MovingViewport);
    }
    let handle = get_map_json(map).ok_or(MissingMap)?;
    let json = &*handle;
    let tileset = PokemonTilesetData
        .tileset_by_name(&json.header.tileset)
        .ok_or(MissingTileset)?;
    let (width, height) = map.dimensions();
    let blocks = match screen.map_data.as_ref() {
        Some(live) => {
            if live.width != width || live.height != height {
                return Err(InvalidLiveGrid);
            }
            &live.blocks[..]
        }
        None => get_block_data(map),
    };
    let source = BackgroundMapTiles {
        map_json: Some(json),
        width,
        height,
        blocks,
        border: json.header.border_block,
        blockset: blockset_data::blockset_for_tileset(tileset),
    };
    let origin = (
        i32::from(player.0) * 2 - PLAYER_SCREEN_X / TILE_SIZE as i32,
        i32::from(player.1) * 2 - (PLAYER_SCREEN_Y - ACTOR_CELL_Y_OFFSET) / TILE_SIZE as i32,
    );
    let (columns, rows) = if surrounding { (24, 20) } else { (20, 18) };
    let sampled_origin = if surrounding { (origin.0.div_euclid(4) * 4, origin.1.div_euclid(4) * 4) } else { origin };
    let mut tiles = Vec::with_capacity((columns * rows) as usize);
    for y in 0..rows {
        let mut cache = None;
        for x in 0..columns {
            tiles.push(
                source
                    .tile(sampled_origin.0 + x, sampled_origin.1 + y, &mut cache)
                    .ok_or(MissingBlockTile)?,
            );
        }
    }
    Ok(MapTileSnapshot {
        map: map as u8,
        tileset: json.header.tileset.clone(),
        world_tile_origin: origin,
        tiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::impl_traits::PokemonRedData;

    fn compare_raster(screen: &mut OverworldScreen) -> MapTileSnapshot {
        let snapshot = stationary_map_tile_snapshot(screen).unwrap();
        assert_eq!(snapshot.tiles.len(), 360);
        let mut resources = Some(ResourceManager::new(
            pokered_renderer::resource::AssetRoot::auto_detect().unwrap(),
        ));
        let mut raster = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        draw_overworld_background(
            screen,
            &mut resources,
            &mut raster,
            pokered_core::game_state::Lang::En,
        );
        let json = get_map_json(screen.state.current_map).unwrap();
        let tileset = PokemonTilesetData
            .tileset_by_name(&json.header.tileset)
            .unwrap();
        let tiles = resources
            .as_mut()
            .unwrap()
            .load_tileset(tileset.tileset_name())
            .unwrap();
        let mut reconstructed = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        for (i, &tile) in snapshot.tiles.iter().enumerate() {
            reconstructed.blit_gb_tile_indices(
                (i % 20 * 8) as i32,
                (i / 20 * 8) as i32,
                tiles.tileset.get(usize::from(tile)),
                false,
                false,
                false,
            );
        }
        // Compare palette indices, not a fabricated original-font image or
        // a lit/dark display palette. These are the actual native map cells.
        assert_eq!(raster.packed(), reconstructed.packed());
        snapshot
    }

    #[test]
    fn stationary_map_requests_match_real_background_raster_interior_border_and_connection() {
        for (map, x, y) in [
            (MapId::RockTunnel1F, 15, 3),
            (MapId::SilphCo5F, 15, 9),
            (MapId::PalletTown, 0, 0),
            (MapId::ViridianCity, 10, 0),
        ] {
            let mut field = OverworldScreen::new(map, None, PokemonRedData);
            field.state.player.x = x;
            field.state.player.y = y;
            let first = compare_raster(&mut field);
            assert_eq!(
                first,
                compare_raster(&mut field),
                "capture does not advance a producer"
            );
        }
    }

    #[test]
    fn map_requests_follow_live_block_replacement_instead_of_the_static_map() {
        let mut field = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        field.state.player.x = 15;
        field.state.player.y = 3;
        let before = compare_raster(&mut field);
        let json = get_map_json(field.state.current_map).unwrap();
        let tileset = PokemonTilesetData
            .tileset_by_name(&json.header.tileset)
            .unwrap();
        let blocks = blockset_data::blockset_for_tileset(tileset);
        let live = field.map_data.as_mut().unwrap();
        let index = usize::from(field.state.player.y / 2) * usize::from(live.width)
            + usize::from(field.state.player.x / 2);
        let old = live.blocks[index];
        let replacement = blocks
            .chunks_exact(16)
            .position(|b| b != &blocks[usize::from(old) * 16..usize::from(old) * 16 + 16])
            .unwrap() as u8;
        live.blocks[index] = replacement;
        let after = compare_raster(&mut field);
        assert_ne!(before.tiles, after.tiles);
        assert_eq!(before.world_tile_origin, after.world_tile_origin);
    }

    #[test]
    fn unsupported_camera_and_invalid_live_grid_do_not_invent_stationary_tiles() {
        let mut field = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        field.state.player.movement_state = MovementState::Walking;
        assert_eq!(
            stationary_map_tile_snapshot(&field),
            Err(MapTileSnapshotError::MovingViewport)
        );
        field.state.player.movement_state = MovementState::Idle;
        field.map_data.as_mut().unwrap().width = 1;
        assert_eq!(
            stationary_map_tile_snapshot(&field),
            Err(MapTileSnapshotError::InvalidLiveGrid)
        );
    }
}
