//! Tests for HM overworld effects — Cut, Fly, Surf, Strength, Flash, Boulder Push.

use super::hm_effects::*;
use super::Direction;
use pokered_data::maps::MapId;
use pokered_data::tileset_data::{CUT_GRASS_TILE, CUT_TREE_TILE_GYM, CUT_TREE_TILE_OVERWORLD};
use pokered_data::tilesets::TilesetId;

// ── Badge helpers ────────────────────────────────────────────────────

const NO_BADGES: u8 = 0;
const CASCADE_BADGE: u8 = 1 << BIT_CASCADEBADGE;
const THUNDER_BADGE: u8 = 1 << BIT_THUNDERBADGE;
const RAINBOW_BADGE: u8 = 1 << BIT_RAINBOWBADGE;
const SOUL_BADGE: u8 = 1 << BIT_SOULBADGE;
const BOULDER_BADGE: u8 = 1 << BIT_BOULDERBADGE;
const ALL_BADGES: u8 = 0xFF;

// ══════════════════════════════════════════════════════════════════════
//  CUT TESTS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn cut_no_badge() {
    assert_eq!(
        use_cut(
            NO_BADGES,
            TilesetId::Overworld,
            CUT_TREE_TILE_OVERWORLD,
            0x32
        ),
        CutResult::NoBadge
    );
}

#[test]
fn cut_overworld_tree() {
    let result = use_cut(
        CASCADE_BADGE,
        TilesetId::Overworld,
        CUT_TREE_TILE_OVERWORLD,
        0x32,
    );
    assert_eq!(
        result,
        CutResult::CutTree {
            replacement_block: 0x6D
        }
    );
}

#[test]
fn cut_overworld_tree_block_33() {
    let result = use_cut(
        CASCADE_BADGE,
        TilesetId::Overworld,
        CUT_TREE_TILE_OVERWORLD,
        0x33,
    );
    assert_eq!(
        result,
        CutResult::CutTree {
            replacement_block: 0x6C
        }
    );
}

#[test]
fn cut_overworld_tree_block_34() {
    let result = use_cut(
        CASCADE_BADGE,
        TilesetId::Overworld,
        CUT_TREE_TILE_OVERWORLD,
        0x34,
    );
    assert_eq!(
        result,
        CutResult::CutTree {
            replacement_block: 0x6F
        }
    );
}

#[test]
fn cut_overworld_tree_block_3d() {
    // Block 0x3D itself is in the swap table
    let result = use_cut(
        CASCADE_BADGE,
        TilesetId::Overworld,
        CUT_TREE_TILE_OVERWORLD,
        0x3D,
    );
    assert_eq!(
        result,
        CutResult::CutTree {
            replacement_block: 0x36
        }
    );
}

#[test]
fn cut_overworld_grass() {
    assert_eq!(
        use_cut(CASCADE_BADGE, TilesetId::Overworld, CUT_GRASS_TILE, 0x00),
        CutResult::CutGrass
    );
}

#[test]
fn cut_gym_tree() {
    let result = use_cut(CASCADE_BADGE, TilesetId::Gym, CUT_TREE_TILE_GYM, 0x3C);
    assert_eq!(
        result,
        CutResult::CutTree {
            replacement_block: 0x35
        }
    );
}

#[test]
fn cut_gym_no_grass() {
    // Gym tileset has no grass cutting
    assert_eq!(
        use_cut(CASCADE_BADGE, TilesetId::Gym, CUT_GRASS_TILE, 0x00),
        CutResult::NothingToCut
    );
}

#[test]
fn cut_wrong_tileset() {
    assert_eq!(
        use_cut(
            CASCADE_BADGE,
            TilesetId::Cavern,
            CUT_TREE_TILE_OVERWORLD,
            0x32
        ),
        CutResult::NothingToCut
    );
}

#[test]
fn cut_overworld_wrong_tile() {
    assert_eq!(
        use_cut(CASCADE_BADGE, TilesetId::Overworld, 0x10, 0x32),
        CutResult::NothingToCut
    );
}

#[test]
fn cut_overworld_tree_unknown_block() {
    // Valid tree tile but block not in swap table
    assert_eq!(
        use_cut(
            CASCADE_BADGE,
            TilesetId::Overworld,
            CUT_TREE_TILE_OVERWORLD,
            0xFF
        ),
        CutResult::NothingToCut
    );
}

#[test]
fn cut_with_all_badges() {
    let result = use_cut(
        ALL_BADGES,
        TilesetId::Overworld,
        CUT_TREE_TILE_OVERWORLD,
        0x35,
    );
    assert_eq!(
        result,
        CutResult::CutTree {
            replacement_block: 0x4C
        }
    );
}

// ══════════════════════════════════════════════════════════════════════
//  FLY TESTS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn fly_no_badge() {
    assert_eq!(
        use_fly(NO_BADGES, TilesetId::Overworld, Some(MapId::PalletTown)),
        FlyResult::NoBadge
    );
}

#[test]
fn fly_indoors() {
    assert_eq!(
        use_fly(THUNDER_BADGE, TilesetId::House, Some(MapId::PalletTown)),
        FlyResult::CannotFlyHere
    );
}

#[test]
fn fly_in_cave() {
    assert_eq!(
        use_fly(THUNDER_BADGE, TilesetId::Cavern, Some(MapId::PalletTown)),
        FlyResult::CannotFlyHere
    );
}

#[test]
fn fly_success_overworld() {
    assert_eq!(
        use_fly(
            THUNDER_BADGE,
            TilesetId::Overworld,
            Some(MapId::CeruleanCity)
        ),
        FlyResult::ChoseDestination {
            destination: MapId::CeruleanCity
        }
    );
}

#[test]
fn fly_success_plateau() {
    assert_eq!(
        use_fly(
            THUNDER_BADGE,
            TilesetId::Plateau,
            Some(MapId::IndigoPlateau)
        ),
        FlyResult::ChoseDestination {
            destination: MapId::IndigoPlateau
        }
    );
}

#[test]
fn fly_cancelled() {
    assert_eq!(
        use_fly(THUNDER_BADGE, TilesetId::Overworld, None),
        FlyResult::Cancelled
    );
}

// ══════════════════════════════════════════════════════════════════════
//  FLY DESTINATIONS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn fly_destinations_count() {
    assert_eq!(FLY_DESTINATIONS.len(), NUM_FLY_DESTINATIONS);
    assert_eq!(FLY_DESTINATIONS.len(), 13);
}

#[test]
fn fly_destination_pallet_town() {
    let d = fly_destination_for_map(MapId::PalletTown).unwrap();
    assert_eq!(d.x, 5);
    assert_eq!(d.y, 6);
}

#[test]
fn fly_destination_saffron() {
    let d = fly_destination_for_map(MapId::SaffronCity).unwrap();
    assert_eq!(d.x, 9);
    assert_eq!(d.y, 30);
}

#[test]
fn fly_destination_route4() {
    let d = fly_destination_for_map(MapId::Route4).unwrap();
    assert_eq!(d.x, 11);
    assert_eq!(d.y, 6);
}

#[test]
fn fly_destination_route10() {
    let d = fly_destination_for_map(MapId::Route10).unwrap();
    assert_eq!(d.x, 11);
    assert_eq!(d.y, 20);
}

#[test]
fn fly_destination_not_found() {
    assert!(fly_destination_for_map(MapId::Route1).is_none());
}

#[test]
fn fly_destination_indigo_plateau() {
    let d = fly_destination_for_map(MapId::IndigoPlateau).unwrap();
    assert_eq!(d.x, 9);
    assert_eq!(d.y, 6);
}

// ══════════════════════════════════════════════════════════════════════
//  SURF TESTS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn surf_no_badge() {
    assert_eq!(
        use_surf(
            NO_BADGES,
            TilesetId::Overworld,
            true,
            false,
            false,
            MapId::PalletTown,
            false,
            0,
            0
        ),
        SurfResult::NoBadge
    );
}

#[test]
fn surf_already_surfing() {
    assert_eq!(
        use_surf(
            SOUL_BADGE,
            TilesetId::Overworld,
            true,
            true,
            false,
            MapId::PalletTown,
            false,
            0,
            0
        ),
        SurfResult::AlreadySurfing
    );
}

#[test]
fn surf_forced_bike() {
    assert_eq!(
        use_surf(
            SOUL_BADGE,
            TilesetId::Overworld,
            true,
            false,
            true,
            MapId::Route17,
            false,
            0,
            0
        ),
        SurfResult::ForcedToRideBike
    );
}

#[test]
fn surf_not_facing_water() {
    assert_eq!(
        use_surf(
            SOUL_BADGE,
            TilesetId::Overworld,
            false,
            false,
            false,
            MapId::PalletTown,
            false,
            0,
            0
        ),
        SurfResult::NotFacingWater
    );
}

#[test]
fn surf_success() {
    assert_eq!(
        use_surf(
            SOUL_BADGE,
            TilesetId::Overworld,
            true,
            false,
            false,
            MapId::PalletTown,
            false,
            0,
            0
        ),
        SurfResult::StartedSurfing
    );
}

#[test]
fn surf_seafoam_b4f_blocked() {
    assert_eq!(
        use_surf(
            SOUL_BADGE,
            TilesetId::Cavern,
            true,
            false,
            false,
            MapId::SeafoamIslandsB4F,
            false,
            SEAFOAM_B4F_STAIRS_X,
            SEAFOAM_B4F_STAIRS_Y
        ),
        SurfResult::CurrentTooFast
    );
}

#[test]
fn surf_seafoam_b4f_boulders_done() {
    assert_eq!(
        use_surf(
            SOUL_BADGE,
            TilesetId::Cavern,
            true,
            false,
            false,
            MapId::SeafoamIslandsB4F,
            true,
            SEAFOAM_B4F_STAIRS_X,
            SEAFOAM_B4F_STAIRS_Y
        ),
        SurfResult::StartedSurfing
    );
}

#[test]
fn surf_seafoam_b4f_different_position() {
    // Not at the specific blocked position, should be fine
    assert_eq!(
        use_surf(
            SOUL_BADGE,
            TilesetId::Cavern,
            true,
            false,
            false,
            MapId::SeafoamIslandsB4F,
            false,
            0,
            0
        ),
        SurfResult::StartedSurfing
    );
}

// ══════════════════════════════════════════════════════════════════════
//  STRENGTH TESTS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn strength_no_badge() {
    assert_eq!(use_strength(NO_BADGES, false), StrengthResult::NoBadge);
}

#[test]
fn strength_activate() {
    assert_eq!(
        use_strength(RAINBOW_BADGE, false),
        StrengthResult::Activated
    );
}

#[test]
fn strength_already_active() {
    assert_eq!(
        use_strength(RAINBOW_BADGE, true),
        StrengthResult::AlreadyActive
    );
}

#[test]
fn strength_all_badges() {
    assert_eq!(use_strength(ALL_BADGES, false), StrengthResult::Activated);
}

// ══════════════════════════════════════════════════════════════════════
//  FLASH TESTS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn flash_no_badge() {
    assert_eq!(use_flash(NO_BADGES, true), FlashResult::NoBadge);
}

#[test]
fn flash_already_lit() {
    assert_eq!(use_flash(BOULDER_BADGE, false), FlashResult::AlreadyLit);
}

#[test]
fn flash_success() {
    assert_eq!(use_flash(BOULDER_BADGE, true), FlashResult::LitUpCave);
}

#[test]
fn flash_all_badges() {
    assert_eq!(use_flash(ALL_BADGES, true), FlashResult::LitUpCave);
}

// ══════════════════════════════════════════════════════════════════════
//  BOULDER PUSH TESTS (simple API)
// ══════════════════════════════════════════════════════════════════════

#[test]
fn boulder_strength_not_active() {
    assert_eq!(
        try_push_boulder(false, false, Some(1), true, true, true, false),
        BoulderPushResult::StrengthNotActive
    );
}

#[test]
fn boulder_dust_active() {
    assert_eq!(
        try_push_boulder(true, true, Some(1), true, true, true, false),
        BoulderPushResult::StrengthNotActive
    );
}

#[test]
fn boulder_no_sprite_in_front() {
    assert_eq!(
        try_push_boulder(true, false, None, true, true, true, false),
        BoulderPushResult::NoBoulderInFront
    );
}

#[test]
fn boulder_not_a_boulder() {
    assert_eq!(
        try_push_boulder(true, false, Some(1), false, true, true, false),
        BoulderPushResult::NotABoulder
    );
}

#[test]
fn boulder_need_push_again() {
    assert_eq!(
        try_push_boulder(true, false, Some(1), true, false, true, false),
        BoulderPushResult::NeedPushAgain
    );
}

#[test]
fn boulder_wrong_direction() {
    assert_eq!(
        try_push_boulder(true, false, Some(1), true, true, false, false),
        BoulderPushResult::NotPushingCorrectDirection
    );
}

#[test]
fn boulder_blocked() {
    assert_eq!(
        try_push_boulder(true, false, Some(1), true, true, true, true),
        BoulderPushResult::BoulderBlocked
    );
}

#[test]
fn boulder_pushed_success() {
    assert_eq!(
        try_push_boulder(true, false, Some(1), true, true, true, false),
        BoulderPushResult::Pushed {
            direction: Direction::Down
        }
    );
}

// ══════════════════════════════════════════════════════════════════════
//  BOULDER PUSH WITH DIRECTION TESTS
// ══════════════════════════════════════════════════════════════════════

#[test]
fn boulder_dir_strength_not_active() {
    assert_eq!(
        try_push_boulder_with_direction(
            false,
            false,
            Some(1),
            true,
            true,
            Direction::Up,
            Some(Direction::Up),
            false
        ),
        BoulderPushResult::StrengthNotActive
    );
}

#[test]
fn boulder_dir_matching_direction() {
    assert_eq!(
        try_push_boulder_with_direction(
            true,
            false,
            Some(1),
            true,
            true,
            Direction::Left,
            Some(Direction::Left),
            false
        ),
        BoulderPushResult::Pushed {
            direction: Direction::Left
        }
    );
}

#[test]
fn boulder_dir_mismatched_direction() {
    assert_eq!(
        try_push_boulder_with_direction(
            true,
            false,
            Some(1),
            true,
            true,
            Direction::Up,
            Some(Direction::Down),
            false
        ),
        BoulderPushResult::NotPushingCorrectDirection
    );
}

#[test]
fn boulder_dir_no_held_direction() {
    assert_eq!(
        try_push_boulder_with_direction(
            true,
            false,
            Some(1),
            true,
            true,
            Direction::Up,
            None,
            false
        ),
        BoulderPushResult::NotPushingCorrectDirection
    );
}

#[test]
fn boulder_dir_blocked() {
    assert_eq!(
        try_push_boulder_with_direction(
            true,
            false,
            Some(1),
            true,
            true,
            Direction::Right,
            Some(Direction::Right),
            true
        ),
        BoulderPushResult::BoulderBlocked
    );
}

#[test]
fn boulder_dir_need_push_again() {
    assert_eq!(
        try_push_boulder_with_direction(
            true,
            false,
            Some(1),
            true,
            false,
            Direction::Down,
            Some(Direction::Down),
            false
        ),
        BoulderPushResult::NeedPushAgain
    );
}

#[test]
fn boulder_dir_push_all_four_directions() {
    for dir in [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ] {
        assert_eq!(
            try_push_boulder_with_direction(
                true,
                false,
                Some(1),
                true,
                true,
                dir,
                Some(dir),
                false
            ),
            BoulderPushResult::Pushed { direction: dir }
        );
    }
}

// ══════════════════════════════════════════════════════════════════════
//  FIELD-MOVE TABLE (data/moves/field_moves.asm)
// ══════════════════════════════════════════════════════════════════════

#[test]
fn field_move_table_recognizes_the_eight_gen1_field_moves() {
    use pokered_data::moves::MoveId;
    // FieldMoveDisplayData (data/moves/field_moves.asm): the seven HMs plus
    // SOFTBOILED — the 9th entry, a healing effect with no badge gate.
    for m in [
        MoveId::Cut,
        MoveId::Fly,
        MoveId::Surf,
        MoveId::Strength,
        MoveId::Flash,
        MoveId::Dig,
        MoveId::Teleport,
        MoveId::Softboiled,
    ] {
        assert!(is_field_move(m), "{:?} is a field move", m);
    }
    assert!(!is_field_move(MoveId::Tackle));
    assert!(!is_field_move(MoveId::None));
}

#[test]
fn field_moves_of_preserves_moveset_order() {
    use pokered_data::moves::MoveId;
    let moves = [MoveId::Surf, MoveId::Tackle, MoveId::Cut, MoveId::None];
    assert_eq!(field_moves_of(&moves), vec![MoveId::Surf, MoveId::Cut]);
    assert!(field_moves_of(&[MoveId::None; 4]).is_empty());
}

#[test]
fn field_move_badge_requirements_match_gen1() {
    use pokered_data::moves::MoveId;
    // start_sub_menus.asm .outOfBattleMovePointers badge checks.
    assert_eq!(field_move_required_badge(MoveId::Cut), Some(BIT_CASCADEBADGE));
    assert_eq!(field_move_required_badge(MoveId::Fly), Some(BIT_THUNDERBADGE));
    assert_eq!(field_move_required_badge(MoveId::Surf), Some(BIT_SOULBADGE));
    assert_eq!(
        field_move_required_badge(MoveId::Strength),
        Some(BIT_RAINBOWBADGE)
    );
    assert_eq!(
        field_move_required_badge(MoveId::Flash),
        Some(BIT_BOULDERBADGE)
    );
    // DIG, TELEPORT and SOFTBOILED have no badge gate in the original
    // (start_sub_menus.asm .dig / .teleport / .softboiled).
    assert_eq!(field_move_required_badge(MoveId::Dig), None);
    assert_eq!(field_move_required_badge(MoveId::Teleport), None);
    assert_eq!(field_move_required_badge(MoveId::Softboiled), None);
    assert_eq!(field_move_required_badge(MoveId::Tackle), None);
}

#[test]
fn field_move_menu_leftmost_matches_display_data() {
    use pokered_data::moves::MoveId;
    // FieldMoveDisplayData "leftmost tile": short names $0C, STRENGTH and
    // TELEPORT $0A (8-letter names), SOFTBOILED $08 (10 letters).
    assert_eq!(field_move_menu_leftmost(MoveId::Cut), Some(0x0C));
    assert_eq!(field_move_menu_leftmost(MoveId::Strength), Some(0x0A));
    assert_eq!(field_move_menu_leftmost(MoveId::Teleport), Some(0x0A));
    assert_eq!(field_move_menu_leftmost(MoveId::Softboiled), Some(0x08));
    assert_eq!(field_move_menu_leftmost(MoveId::Tackle), None);
}
