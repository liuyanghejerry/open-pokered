use jrpg_engine::tileset::TilesetTrait;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::tileset_data::{
    cut_tree_replacement, is_outside_tileset, is_water_tileset, CUT_GRASS_TILE, CUT_TREE_TILE_GYM,
    CUT_TREE_TILE_OVERWORLD,
};
use pokered_data::tilesets::TilesetId;

use super::Direction;

pub const BIT_BOULDERBADGE: u8 = 0;
pub const BIT_CASCADEBADGE: u8 = 1;
pub const BIT_THUNDERBADGE: u8 = 2;
pub const BIT_RAINBOWBADGE: u8 = 3;
pub const BIT_SOULBADGE: u8 = 4;

fn has_badge(obtained_badges: u8, bit: u8) -> bool {
    obtained_badges & (1 << bit) != 0
}

// ── Field-move table ────────────────────────────────────────────────
//
// The eight out-of-battle moves listed in the Gen-1 party menu, from
// data/moves/field_moves.asm (FieldMoveDisplayData). SOFTBOILED (the 9th
// entry, name index 9, leftmost tile $08) is a healing effect: it runs
// through the party menu like the overworld moves but heals a chosen party
// member instead of acting on the map (start_sub_menus.asm `.softboiled` +
// ItemUseMedicine's pseudo-item path).
// `menu_leftmost_x` is the original's per-move "leftmost tile" used to
// size/position the party-menu action box.
struct FieldMoveEntry {
    move_id: MoveId,
    /// Badge bit required to *use* the move (None = no badge gate),
    /// checked on selection in engine/menus/start_sub_menus.asm.
    badge_bit: Option<u8>,
    /// FieldMoveDisplayData leftmost tile column for the move's name.
    menu_leftmost_x: u8,
}

const FIELD_MOVE_TABLE: &[FieldMoveEntry] = &[
    FieldMoveEntry { move_id: MoveId::Cut, badge_bit: Some(BIT_CASCADEBADGE), menu_leftmost_x: 0x0C },
    FieldMoveEntry { move_id: MoveId::Fly, badge_bit: Some(BIT_THUNDERBADGE), menu_leftmost_x: 0x0C },
    FieldMoveEntry { move_id: MoveId::Surf, badge_bit: Some(BIT_SOULBADGE), menu_leftmost_x: 0x0C },
    FieldMoveEntry { move_id: MoveId::Strength, badge_bit: Some(BIT_RAINBOWBADGE), menu_leftmost_x: 0x0A },
    FieldMoveEntry { move_id: MoveId::Flash, badge_bit: Some(BIT_BOULDERBADGE), menu_leftmost_x: 0x0C },
    FieldMoveEntry { move_id: MoveId::Dig, badge_bit: None, menu_leftmost_x: 0x0C },
    FieldMoveEntry { move_id: MoveId::Teleport, badge_bit: None, menu_leftmost_x: 0x0A },
    // .softboiled (start_sub_menus.asm:236-274) has no badge check.
    FieldMoveEntry { move_id: MoveId::Softboiled, badge_bit: None, menu_leftmost_x: 0x08 },
];

/// Is this move usable out of battle (shown in the Gen-1 party menu)?
pub fn is_field_move(m: MoveId) -> bool {
    FIELD_MOVE_TABLE.iter().any(|e| e.move_id == m)
}

/// The field moves a Pokémon can show in the party menu, in moveset order.
/// Mirrors GetMonFieldMoves (engine/menus/text_box.asm): every known move
/// that appears in FieldMoveDisplayData is listed — the badge check happens
//  on selection, not on display.
pub fn field_moves_of(moves: &[MoveId; 4]) -> Vec<MoveId> {
    moves.iter().copied().filter(|m| is_field_move(*m)).collect()
}

/// Badge bit required to use this field move, if any
/// (start_sub_menus.asm .cut/.fly/.surf/.strength/.flash badge checks).
pub fn field_move_required_badge(m: MoveId) -> Option<u8> {
    FIELD_MOVE_TABLE
        .iter()
        .find(|e| e.move_id == m)
        .and_then(|e| e.badge_bit)
}

/// FieldMoveDisplayData "leftmost tile" for the move's name — the party-menu
/// action box is sized from the minimum of these across the listed moves.
pub fn field_move_menu_leftmost(m: MoveId) -> Option<u8> {
    FIELD_MOVE_TABLE
        .iter()
        .find(|e| e.move_id == m)
        .map(|e| e.menu_leftmost_x)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CutResult {
    NoBadge,
    NothingToCut,
    CutTree { replacement_block: u8 },
    CutGrass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlyResult {
    NoBadge,
    CannotFlyHere,
    ChoseDestination { destination: MapId },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfResult {
    NoBadge,
    AlreadySurfing,
    NotFacingWater,
    ForcedToRideBike,
    CurrentTooFast,
    StartedSurfing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrengthResult {
    NoBadge,
    Activated,
    AlreadyActive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashResult {
    NoBadge,
    AlreadyLit,
    LitUpCave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlyDestination {
    pub map: MapId,
    pub x: u8,
    pub y: u8,
}

pub const FLY_DESTINATIONS: &[FlyDestination] = &[
    FlyDestination {
        map: MapId::PalletTown,
        x: 5,
        y: 6,
    },
    FlyDestination {
        map: MapId::ViridianCity,
        x: 23,
        y: 26,
    },
    FlyDestination {
        map: MapId::PewterCity,
        x: 13,
        y: 26,
    },
    FlyDestination {
        map: MapId::CeruleanCity,
        x: 19,
        y: 18,
    },
    FlyDestination {
        map: MapId::LavenderTown,
        x: 3,
        y: 6,
    },
    FlyDestination {
        map: MapId::VermilionCity,
        x: 11,
        y: 4,
    },
    FlyDestination {
        map: MapId::CeladonCity,
        x: 41,
        y: 10,
    },
    FlyDestination {
        map: MapId::FuchsiaCity,
        x: 19,
        y: 28,
    },
    FlyDestination {
        map: MapId::CinnabarIsland,
        x: 11,
        y: 12,
    },
    FlyDestination {
        map: MapId::IndigoPlateau,
        x: 9,
        y: 6,
    },
    FlyDestination {
        map: MapId::SaffronCity,
        x: 9,
        y: 30,
    },
    FlyDestination {
        map: MapId::Route4,
        x: 11,
        y: 6,
    },
    FlyDestination {
        map: MapId::Route10,
        x: 11,
        y: 20,
    },
];

pub const NUM_FLY_DESTINATIONS: usize = 13;

pub fn fly_destination_for_map(map: MapId) -> Option<&'static FlyDestination> {
    FLY_DESTINATIONS.iter().find(|d| d.map == map)
}

/// `EscapeRopeTilesets` (data/tilesets/escape_rope_tilesets.asm): FOREST,
/// CEMETERY, CAVERN, FACILITY, INTERIOR — the only tilesets where the
/// ESCAPE ROPE item / DIG field move may be used (`ItemUseEscapeRope`,
/// engine/items/item_effects.asm:1499-1507). Every other tileset — SHIP
/// (SS Anne), GATE (gates + Underground Paths), POKECENTER, GYM, … —
/// refuses with "This isn't the time to use that!" in the original.
/// Custom tilesets are resolved to their base first.
pub fn is_escape_rope_tileset(tileset: TilesetId) -> bool {
    matches!(
        tileset.base(),
        TilesetId::Forest
            | TilesetId::Cemetery
            | TilesetId::Cavern
            | TilesetId::Facility
            | TilesetId::Interior
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoulderPushResult {
    StrengthNotActive,
    NoBoulderInFront,
    NotABoulder,
    BoulderBlocked,
    NotPushingCorrectDirection,
    NeedPushAgain,
    Pushed { direction: Direction },
}

pub const SEAFOAM_B4F_STAIRS_X: u8 = 7;
pub const SEAFOAM_B4F_STAIRS_Y: u8 = 11;

pub fn use_cut<T: TilesetTrait>(
    obtained_badges: u8,
    tileset: T,
    tile_in_front: u8,
    current_block: u8,
) -> CutResult {
    if !has_badge(obtained_badges, BIT_CASCADEBADGE) {
        return CutResult::NoBadge;
    }

    match tileset.name() {
        "overworld" => {
            if tile_in_front == CUT_TREE_TILE_OVERWORLD {
                if let Some(replacement) = cut_tree_replacement(current_block) {
                    return CutResult::CutTree {
                        replacement_block: replacement,
                    };
                }
            }
            if tile_in_front == CUT_GRASS_TILE {
                return CutResult::CutGrass;
            }
            CutResult::NothingToCut
        }
        "gym" => {
            if tile_in_front == CUT_TREE_TILE_GYM {
                if let Some(replacement) = cut_tree_replacement(current_block) {
                    return CutResult::CutTree {
                        replacement_block: replacement,
                    };
                }
            }
            CutResult::NothingToCut
        }
        _ => CutResult::NothingToCut,
    }
}

pub fn use_fly<T: TilesetTrait>(
    obtained_badges: u8,
    tileset: T,
    chosen_destination: Option<MapId>,
) -> FlyResult {
    if !has_badge(obtained_badges, BIT_THUNDERBADGE) {
        return FlyResult::NoBadge;
    }

    let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
    if !is_outside_tileset(concrete) {
        return FlyResult::CannotFlyHere;
    }

    match chosen_destination {
        Some(dest) => FlyResult::ChoseDestination { destination: dest },
        None => FlyResult::Cancelled,
    }
}

pub fn use_surf<T: TilesetTrait>(
    obtained_badges: u8,
    tileset: T,
    is_facing_water: bool,
    already_surfing: bool,
    forced_bike: bool,
    current_map: MapId,
    seafoam_b4f_boulders_done: bool,
    player_x: u8,
    player_y: u8,
) -> SurfResult {
    if !has_badge(obtained_badges, BIT_SOULBADGE) {
        return SurfResult::NoBadge;
    }

    if already_surfing {
        return SurfResult::AlreadySurfing;
    }

    if forced_bike {
        return SurfResult::ForcedToRideBike;
    }

    if current_map == MapId::SeafoamIslandsB4F
        && !seafoam_b4f_boulders_done
        && player_x == SEAFOAM_B4F_STAIRS_X
        && player_y == SEAFOAM_B4F_STAIRS_Y
    {
        return SurfResult::CurrentTooFast;
    }

    if !is_facing_water {
        return SurfResult::NotFacingWater;
    }

    let concrete = pokered_data::tilesets::resolve_concrete(&tileset);
    let _ = is_water_tileset(concrete);

    SurfResult::StartedSurfing
}

pub fn use_strength(obtained_badges: u8, strength_already_active: bool) -> StrengthResult {
    if !has_badge(obtained_badges, BIT_RAINBOWBADGE) {
        return StrengthResult::NoBadge;
    }

    if strength_already_active {
        return StrengthResult::AlreadyActive;
    }

    StrengthResult::Activated
}

pub fn use_flash(obtained_badges: u8, cave_is_dark: bool) -> FlashResult {
    if !has_badge(obtained_badges, BIT_BOULDERBADGE) {
        return FlashResult::NoBadge;
    }

    if !cave_is_dark {
        return FlashResult::AlreadyLit;
    }

    FlashResult::LitUpCave
}

pub fn try_push_boulder(
    strength_active: bool,
    boulder_dust_active: bool,
    sprite_in_front: Option<u8>,
    is_boulder: bool,
    already_tried_push: bool,
    pushing_correct_direction: bool,
    boulder_blocked: bool,
) -> BoulderPushResult {
    if !strength_active {
        return BoulderPushResult::StrengthNotActive;
    }

    if boulder_dust_active {
        return BoulderPushResult::StrengthNotActive;
    }

    if sprite_in_front.is_none() {
        return BoulderPushResult::NoBoulderInFront;
    }

    if !is_boulder {
        return BoulderPushResult::NotABoulder;
    }

    if !already_tried_push {
        return BoulderPushResult::NeedPushAgain;
    }

    if !pushing_correct_direction {
        return BoulderPushResult::NotPushingCorrectDirection;
    }

    if boulder_blocked {
        return BoulderPushResult::BoulderBlocked;
    }

    BoulderPushResult::Pushed {
        direction: Direction::Down,
    }
}

pub fn try_push_boulder_with_direction(
    strength_active: bool,
    boulder_dust_active: bool,
    sprite_in_front: Option<u8>,
    is_boulder: bool,
    already_tried_push: bool,
    player_facing: Direction,
    held_direction: Option<Direction>,
    boulder_blocked: bool,
) -> BoulderPushResult {
    if !strength_active {
        return BoulderPushResult::StrengthNotActive;
    }

    if boulder_dust_active {
        return BoulderPushResult::StrengthNotActive;
    }

    if sprite_in_front.is_none() {
        return BoulderPushResult::NoBoulderInFront;
    }

    if !is_boulder {
        return BoulderPushResult::NotABoulder;
    }

    if !already_tried_push {
        return BoulderPushResult::NeedPushAgain;
    }

    match held_direction {
        Some(dir) if dir == player_facing => {}
        _ => return BoulderPushResult::NotPushingCorrectDirection,
    }

    if boulder_blocked {
        return BoulderPushResult::BoulderBlocked;
    }

    BoulderPushResult::Pushed {
        direction: player_facing,
    }
}
