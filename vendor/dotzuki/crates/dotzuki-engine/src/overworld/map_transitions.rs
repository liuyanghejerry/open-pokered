use crate::map::MapTrait;
use crate::overworld::types::{Direction, MapData};
use crate::tileset::TilesetTrait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionTransition<M: MapTrait> {
    pub new_map: M,
    pub new_x: u16,
    pub new_y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarpTransition<M: MapTrait> {
    pub new_map: M,
    pub dest_warp_id: u8,
    pub is_last_map: bool,
}

/// Provider trait for map data needed by transition calculations.
///
/// This trait allows the transition functions to resolve map names
/// to typed IDs and query map dimensions without depending on any
/// specific game's data loading mechanism.
pub trait MapTransitionProvider<M: MapTrait> {
    /// Resolve a map name string to a typed map ID.
    fn resolve_map_id(&self, name: &str) -> Option<M>;

    /// Get the dimensions `(width, height)` of a map in blocks.
    fn get_map_dimensions(&self, map: M) -> (u8, u8);
}

/// Calculate the transition when the player walks across a map boundary
/// (connection). Returns the new map and the player's position within it.
pub fn calculate_connection_transition<P, M, T, Mus>(
    map_data: &MapData<M, T, Mus>,
    provider: &P,
    px: u16,
    py: u16,
    direction: Direction,
) -> Option<ConnectionTransition<M>>
where
    M: MapTrait,
    T: TilesetTrait,
    P: MapTransitionProvider<M>,
{
    let conns = &map_data.connections;
    let current_w = map_data.width as u16 * 2;
    let current_h = map_data.height as u16 * 2;

    match direction {
        Direction::Up => {
            if py != 0 {
                return None;
            }
            let conn = conns.north.as_ref()?;
            let (_, dest_h) = provider.get_map_dimensions(conn.target_map);
            let new_y = (dest_h as u16) * 2 - 1;
            let new_x = apply_offset(px, conn.offset);
            Some(ConnectionTransition {
                new_map: conn.target_map,
                new_x,
                new_y,
            })
        }
        Direction::Down => {
            if py != current_h - 1 {
                return None;
            }
            let conn = conns.south.as_ref()?;
            let new_y = 0;
            let new_x = apply_offset(px, conn.offset);
            Some(ConnectionTransition {
                new_map: conn.target_map,
                new_x,
                new_y,
            })
        }
        Direction::Left => {
            if px != 0 {
                return None;
            }
            let conn = conns.west.as_ref()?;
            let (dest_w, _) = provider.get_map_dimensions(conn.target_map);
            let new_x = (dest_w as u16) * 2 - 1;
            let new_y = apply_offset(py, conn.offset);
            Some(ConnectionTransition {
                new_map: conn.target_map,
                new_x,
                new_y,
            })
        }
        Direction::Right => {
            if px != current_w - 1 {
                return None;
            }
            let conn = conns.east.as_ref()?;
            let new_x = 0;
            let new_y = apply_offset(py, conn.offset);
            Some(ConnectionTransition {
                new_map: conn.target_map,
                new_x,
                new_y,
            })
        }
    }
}

fn apply_offset(coord: u16, offset: i8) -> u16 {
    let adjusted = coord as i32 - (offset as i32 * 2);
    adjusted.max(0) as u16
}

/// Check if the player is standing on a warp point and return the
/// transition info (target map, warp ID, and whether it's a last-map warp).
pub fn check_warp_at<M: MapTrait, T: TilesetTrait, Mus>(
    map_data: &MapData<M, T, Mus>,
    px: u8,
    py: u8,
) -> Option<WarpTransition<M>> {
    for warp in &map_data.warps {
        if px == warp.x && py == warp.y {
            return Some(WarpTransition {
                new_map: warp.target_map,
                dest_warp_id: warp.target_warp_id,
                is_last_map: warp.is_last_map,
            });
        }
    }
    None
}
