use jrpg_engine::tileset::TilesetTrait;
use pokered_data::maps::MapId;
use pokered_data::tileset_data::{
    check_warp_pad_or_hole as data_check_warp_pad_or_hole, spinner_next_facing, PadInput,
    SpriteFacing, WarpPadOrHoleType, DARK_CAVE_PAL_OFFSET, LEDGE_TILES,
};
use pokered_data::tilesets::TilesetId;

use super::Direction;

fn direction_to_sprite_facing(dir: Direction) -> SpriteFacing {
    match dir {
        Direction::Down => SpriteFacing::Down,
        Direction::Up => SpriteFacing::Up,
        Direction::Left => SpriteFacing::Left,
        Direction::Right => SpriteFacing::Right,
    }
}

fn sprite_facing_to_direction(sf: SpriteFacing) -> Direction {
    match sf {
        SpriteFacing::Down => Direction::Down,
        SpriteFacing::Up => Direction::Up,
        SpriteFacing::Left => Direction::Left,
        SpriteFacing::Right => Direction::Right,
    }
}

fn direction_to_pad_input(dir: Direction) -> Option<PadInput> {
    match dir {
        Direction::Down => Some(PadInput::Down),
        Direction::Left => Some(PadInput::Left),
        Direction::Right => Some(PadInput::Right),
        Direction::Up => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgeJumpResult {
    pub direction: Direction,
}

/// Check if the player should jump a ledge.
/// Ledge jumping only works on the OVERWORLD tileset.
/// Requires: correct facing + standing tile + tile in front + held input direction.
/// From engine/overworld/ledges.asm HandleLedges.
pub fn check_ledge_jump<T: TilesetTrait>(
    tileset: T,
    facing: Direction,
    standing_tile: u8,
    tile_in_front: u8,
    held_input: Option<Direction>,
) -> Option<LedgeJumpResult> {
    if tileset.name() != "overworld" {
        return None;
    }

    let sprite_facing = direction_to_sprite_facing(facing);

    for entry in LEDGE_TILES {
        if entry.facing == sprite_facing
            && entry.standing_tile == standing_tile
            && entry.ledge_tile == tile_in_front
        {
            if let Some(held_dir) = held_input {
                if let Some(held_pad) = direction_to_pad_input(held_dir) {
                    if held_pad == entry.required_input {
                        return Some(LedgeJumpResult { direction: facing });
                    }
                }
            }
            return None;
        }
    }

    None
}

pub fn check_warp_pad_or_hole<T: TilesetTrait>(
    tileset: T,
    standing_tile: u8,
) -> WarpPadOrHoleType {
    let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
    data_check_warp_pad_or_hole(concrete, standing_tile)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpinnerResult {
    pub new_facing: Direction,
}

/// Is the player standing on an arrow/spinner floor tile? (Facility/Gym tilesets.)
/// When true, the overworld forces continuous movement (rotating via
/// `handle_spinner_rotation`) until the player slides onto a non-spinner tile.
pub fn is_spinner_tile<T: TilesetTrait>(tileset: T, standing_tile: u8) -> bool {
    let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
    pokered_data::tileset_data::is_spinner_tile(concrete, standing_tile)
}

/// Handle spinner tile rotation.
/// Spinners only exist in Facility and Gym tilesets.
/// Returns the new facing direction after being spun, or None if not on a spinner tileset.
pub fn handle_spinner_rotation<T: TilesetTrait>(
    tileset: T,
    current_facing: Direction,
) -> Option<SpinnerResult> {
    let sprite_facing = direction_to_sprite_facing(current_facing);
    let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
    spinner_next_facing(concrete, sprite_facing).map(|new_sf| SpinnerResult {
        new_facing: sprite_facing_to_direction(new_sf),
    })
}

/// Maps that start as dark caves (require Flash to see).
/// From home/overworld.asm: Rock Tunnel sets wMapPalOffset to 6.
const DARK_CAVE_MAPS: &[MapId] = &[MapId::RockTunnel1F, MapId::RockTunnelB1F];

#[derive(Debug, Clone)]
pub struct DarkCaveState {
    pub map_pal_offset: u8,
}

impl DarkCaveState {
    pub fn new() -> Self {
        Self { map_pal_offset: 0 }
    }

    pub fn is_dark(&self) -> bool {
        self.map_pal_offset != 0
    }

    /// Enter a map — sets dark cave palette if the map requires it.
    pub fn enter_map(&mut self, map_id: MapId) {
        if DARK_CAVE_MAPS.contains(&map_id) {
            self.map_pal_offset = DARK_CAVE_PAL_OFFSET;
        } else {
            self.map_pal_offset = 0;
        }
    }

    /// Use Flash to light up a dark cave.
    /// From engine/menus/start_sub_menus.asm: sets wMapPalOffset to 0.
    pub fn use_flash(&mut self) -> bool {
        if self.is_dark() {
            self.map_pal_offset = 0;
            true
        } else {
            false
        }
    }

    /// Leave a map — clears dark state.
    /// From home/overworld.asm .goBackOutside: `xor a` / `ld [wMapPalOffset], a`
    pub fn leave_map(&mut self) {
        self.map_pal_offset = 0;
    }

    /// Check if returning from battle to a dark map should use LoadGBPal instead of fade.
    /// From home/overworld.asm MapEntryAfterBattle.
    pub fn should_skip_fade_after_battle(&self) -> bool {
        self.is_dark()
    }
}

impl Default for DarkCaveState {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a tileset represents an "outside" map.
/// Outside maps use OVERWORLD or PLATEAU tilesets.
/// From home/overworld.asm CheckIfInOutsideMap.
pub fn is_outside_map<T: TilesetTrait>(tileset: T) -> bool {
    matches!(tileset.name(), "overworld" | "plateau")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraWarpCheckType {
    /// Check if player is facing the edge of the map.
    FacingEdge,
    /// Check if the tile in front is a warp tile.
    WarpTileInFront,
}

/// Determine which extra warp check function to use for a given map.
/// From home/overworld.asm ExtraWarpCheck.
pub fn extra_warp_check_type<T: TilesetTrait>(
    map_id: MapId,
    tileset: T,
) -> ExtraWarpCheckType {
    match map_id {
        MapId::SSAnne3F => ExtraWarpCheckType::FacingEdge,
        MapId::RocketHideoutB1F
        | MapId::RocketHideoutB2F
        | MapId::RocketHideoutB4F
        | MapId::RockTunnel1F => ExtraWarpCheckType::WarpTileInFront,
        _ => match tileset.name() {
            "overworld" | "ship" | "ship_port" | "plateau" => {
                ExtraWarpCheckType::WarpTileInFront
            }
            _ => ExtraWarpCheckType::FacingEdge,
        },
    }
}
