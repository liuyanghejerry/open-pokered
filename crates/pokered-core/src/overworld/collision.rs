use jrpg_engine::overworld::collision::CollisionProvider as CollisionProviderTrait;
use jrpg_engine::overworld::types::Direction;
use jrpg_engine::tileset::TilesetTrait;
use pokered_data::blockset_data;
use pokered_data::collision as pokered_collision;
use pokered_data::map_connections::get_map_connections;
use pokered_data::map_data_loader::get_block_data;
use pokered_data::maps::MapId;
use pokered_data::tileset_data;
use pokered_data::tilesets::TilesetId;

pub use jrpg_engine::overworld::collision::{
    check_movement_collision, check_sprite_collision, check_warp_at_position,
    direction_to_pad_input, direction_to_sprite_facing, get_block_at, get_target_coords,
    is_facing_map_edge, CollisionProvider, CollisionResult, SpritePosition, PAD_DOWN, PAD_LEFT,
    PAD_RIGHT, PAD_UP, SPRITE_FACING_DOWN, SPRITE_FACING_LEFT, SPRITE_FACING_RIGHT,
    SPRITE_FACING_UP,
};

pub struct PokemonCollisionProvider {
    map_id: MapId,
    warp_front_check: bool,
    is_ssanne_bow: bool,
}

/// Match `jrpg_engine::overworld::map_transitions::apply_offset`: arrival
/// coordinates on the connected map are shifted by `-2 * offset` (the offset
/// is in blocks, clamped at 0).
fn apply_connection_offset(coord: u16, offset: i8) -> u16 {
    (coord as i32 - offset as i32 * 2).max(0) as u16
}

impl PokemonCollisionProvider {
    pub fn new(map_id: MapId, tileset: TilesetId) -> Self {
        let warp_front_check = if map_id == MapId::SSAnne3F {
            false
        } else {
            matches!(
                map_id,
                MapId::RocketHideoutB1F
                    | MapId::RocketHideoutB2F
                    | MapId::RocketHideoutB4F
                    | MapId::RockTunnel1F
            ) || matches!(tileset.name(), "overworld" | "ship" | "ship_port" | "plateau")
        };

        Self {
            map_id,
            warp_front_check,
            is_ssanne_bow: map_id == MapId::SSAnneBow,
        }
    }
}

impl CollisionProviderTrait<TilesetId> for PokemonCollisionProvider {
    fn is_tile_passable(&self, tileset: TilesetId, tile_id: u8) -> bool {
        pokered_collision::is_tile_passable(tileset, tile_id)
    }

    fn check_tile_pair_collision(
        &self,
        tileset: TilesetId,
        standing_tile: u8,
        target_tile: u8,
        on_water: bool,
    ) -> bool {
        pokered_collision::check_tile_pair_collision(tileset, standing_tile, target_tile, on_water)
    }

    fn is_water_tile(&self, tileset: TilesetId, tile_id: u8) -> bool {
        // CollisionCheckOnWater (home/overworld.asm): a surfer may move onto
        // water ($14), the Safari Zone coastline ($48), and the eastern shore
        // ($32) while staying on the water. $32 on the SHIP_PORT tileset is
        // the S.S. Anne boarding platform instead — passable land there
        // (ShipPort_Coll), so the generic dismount rule handles it.
        tile_id == 0x14 || tile_id == 0x48 || (tile_id == 0x32 && tileset != TilesetId::ShipPort)
    }

    fn get_connection_edge_tile(
        &self,
        tileset: TilesetId,
        map_width_blocks: u8,
        map_height_blocks: u8,
        x: u16,
        y: u16,
        direction: Direction,
    ) -> Option<u8> {
        // The tile the player would step onto when crossing a map boundary.
        // The original reads it from the connection strip drawn in the
        // tilemap (LoadTileBlockMap's .northConnection/.southConnection/...)
        // and checks it with the CURRENT map's tileset, so the connected
        // map's block is expanded with `tileset` — exactly like the
        // renderer's resolve_block_with_connections does.
        let entry = get_map_connections(self.map_id);
        let conn = match direction {
            Direction::Up => entry.north?,
            Direction::Down => entry.south?,
            Direction::Left => entry.west?,
            Direction::Right => entry.east?,
        };

        let current_w = map_width_blocks as u16 * 2;
        let current_h = map_height_blocks as u16 * 2;
        let (dest_w, dest_h) = conn.target_map.dimensions();
        // Mirror calculate_connection_transition's arrival math (the new_x /
        // new_y the player gets when the map swap completes).
        let (arrival_x, arrival_y) = match direction {
            Direction::Up if y == 0 => (
                apply_connection_offset(x, conn.offset),
                dest_h as u16 * 2 - 1,
            ),
            Direction::Down if y == current_h - 1 => {
                (apply_connection_offset(x, conn.offset), 0)
            }
            Direction::Left if x == 0 => (
                dest_w as u16 * 2 - 1,
                apply_connection_offset(y, conn.offset),
            ),
            Direction::Right if x == current_w - 1 => {
                (0, apply_connection_offset(y, conn.offset))
            }
            _ => return None,
        };

        let target_blocks = get_block_data(conn.target_map);
        let bx = (arrival_x / 2) as usize;
        let by = (arrival_y / 2) as usize;
        if bx >= dest_w as usize || by >= dest_h as usize || target_blocks.is_empty() {
            return None;
        }
        let block_id = target_blocks[by * dest_w as usize + bx];
        let sub_x = (arrival_x % 2) as usize;
        let sub_y = (arrival_y % 2) as usize;
        blockset_data::block_tiles(tileset, block_id)
            .map(|t| t[(sub_y * 2 + 1) * 4 + sub_x * 2])
    }

    fn check_ledge_jump(
        &self,
        tileset: TilesetId,
        sprite_facing: u8,
        standing_tile: u8,
        target_tile: u8,
        held_input: u8,
    ) -> bool {
        pokered_collision::check_ledge_jump(sprite_facing, standing_tile, target_tile, held_input)
    }

    fn is_counter_tile(&self, tileset: TilesetId, tile_id: u8) -> bool {
        tileset_data::get_tileset_header(tileset).is_counter_tile(tile_id)
    }

    fn get_tile_at_position(
        &self,
        tileset: TilesetId,
        blocks: &[u8],
        map_width: u8,
        x: u16,
        y: u16,
    ) -> u8 {
        let block_x = (x / 2) as usize;
        let block_y = (y / 2) as usize;
        let sub_x = (x % 2) as usize;
        let sub_y = (y % 2) as usize;

        if block_x < map_width as usize {
            let block_idx = block_y * (map_width as usize) + block_x;
            if block_idx < blocks.len() {
                let block_id = blocks[block_idx];
                return blockset_data::block_tiles(tileset, block_id)
                    .map(|t| t[(sub_y * 2 + 1) * 4 + sub_x * 2])
                    .unwrap_or(0);
            }
        }
        0
    }

    fn is_door_tile(&self, tileset: TilesetId, tile_id: u8) -> bool {
        tileset_data::is_door_tile(tileset, tile_id)
    }

    fn is_warp_tile(&self, tileset: TilesetId, tile_id: u8) -> bool {
        tileset_data::is_warp_tile(tileset, tile_id)
    }

    fn is_warp_carpet_tile_in_front(
        &self,
        tileset: TilesetId,
        facing_idx: u8,
        tile_id: u8,
    ) -> bool {
        tileset_data::is_warp_carpet_tile_in_front(facing_idx, tile_id)
    }

    fn uses_warp_tile_in_front_check(&self, _tileset: TilesetId) -> bool {
        self.warp_front_check
    }

    fn check_extra_warp_special(&self, _tileset: TilesetId, tile_in_front: u8) -> Option<bool> {
        if self.is_ssanne_bow {
            Some(tile_in_front == 0x15)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overworld::screen::MapData;
    use jrpg_engine::overworld::types::MapConnections;
    use pokered_data::music::MusicId;

    /// The sub-tile that `get_tile_at_position` reads for sub_x=sub_y=0 is
    /// `block_tiles[(0*2+1)*4 + 0*2] = block_tiles[4]`. Resolve the passable
    /// tile a given block contributes at the top-left sub-position.
    fn standing_tile_for_block(tileset: TilesetId, block_id: u8) -> Option<u8> {
        blockset_data::block_tiles(tileset, block_id).map(|t| t[4])
    }

    /// Scan the Overworld blockset for one block that resolves (at the player's
    /// standing sub-tile) to a passable tile and one that resolves to an
    /// impassable tile. Returns (passable_block, wall_block).
    fn find_passable_and_wall_blocks(tileset: TilesetId) -> (u8, u8) {
        let mut passable = None;
        let mut wall = None;
        for block_id in 0u8..=u8::MAX {
            let Some(tile) = standing_tile_for_block(tileset, block_id) else {
                break; // ran off the end of the blockset
            };
            if pokered_collision::is_tile_passable(tileset, tile) {
                passable.get_or_insert(block_id);
            } else {
                wall.get_or_insert(block_id);
            }
            if passable.is_some() && wall.is_some() {
                break;
            }
        }
        (
            passable.expect("Overworld tileset should have a passable block"),
            wall.expect("Overworld tileset should have an impassable block"),
        )
    }

    /// Prove collision follows a runtime `set_block`: standing on a tile whose
    /// block is passable, then swapping that block for a wall block makes the
    /// same coordinate read as impassable — with no cache to invalidate.
    #[test]
    fn collision_follows_set_block() {
        let tileset = TilesetId::Overworld;
        let (passable_block, wall_block) = find_passable_and_wall_blocks(tileset);

        // 2x2-block map; player tile coords (0,0) live in block (0,0) at idx 0.
        let provider = PokemonCollisionProvider::new(MapId::PalletTown, tileset);
        let mut map: MapData = MapData::new(
            MapId::PalletTown,
            2, // width in blocks
            2, // height in blocks
            tileset,
            MusicId::PalletTown,
            vec![passable_block, passable_block, passable_block, passable_block],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            MapConnections::default(),
        );

        // Before: tile at (0,0) is passable.
        let tile_before = provider.get_tile_at_position(tileset, &map.blocks, map.width, 0, 0);
        assert!(
            pokered_collision::is_tile_passable(tileset, tile_before),
            "precondition: standing tile must be passable before set_block"
        );

        // Swap block (0,0) for a wall block at runtime.
        assert!(map.set_block(0, 0, wall_block));

        // After: same coordinate now resolves to an impassable tile, read live
        // from `map.blocks` with no re-render or cache invalidation.
        let tile_after = provider.get_tile_at_position(tileset, &map.blocks, map.width, 0, 0);
        assert!(
            !pokered_collision::is_tile_passable(tileset, tile_after),
            "collision must follow the runtime block swap"
        );
    }
}
