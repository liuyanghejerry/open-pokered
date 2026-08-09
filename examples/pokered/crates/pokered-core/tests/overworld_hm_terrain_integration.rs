//! Integration tests for HM effects, wild encounters, and special terrain.
//! Part of M9.3: 事件/脚本触发测试.

use pokered_core::overworld::hm_effects::*;
use pokered_core::overworld::special_terrain::*;
use pokered_core::overworld::wild_encounters::*;
use pokered_core::overworld::Direction;

use pokered_data::maps::MapId;
use pokered_data::tileset_data::WarpPadOrHoleType;
use pokered_data::tilesets::TilesetId;

// ── HM: Cut ──────────────────────────────────────────────────────────

#[test]
fn cut_requires_cascade_badge() {
    let result = use_cut(0, TilesetId::Overworld, 0x3D, 0x32);
    assert_eq!(result, CutResult::NoBadge);
}

#[test]
fn cut_tree_on_overworld() {
    let badges = 1 << BIT_CASCADEBADGE;
    let result = use_cut(badges, TilesetId::Overworld, 0x3D, 0x32);
    assert!(
        matches!(result, CutResult::CutTree { .. }),
        "Should cut a tree on overworld tileset, got {:?}",
        result
    );
}

#[test]
fn cut_grass_on_overworld() {
    let badges = 1 << BIT_CASCADEBADGE;
    let result = use_cut(badges, TilesetId::Overworld, 0x52, 0x00);
    assert_eq!(result, CutResult::CutGrass);
}

#[test]
fn cut_nothing_wrong_tile() {
    let badges = 1 << BIT_CASCADEBADGE;
    let result = use_cut(badges, TilesetId::Overworld, 0x01, 0x00);
    assert_eq!(result, CutResult::NothingToCut);
}

// ── HM: Fly ──────────────────────────────────────────────────────────

#[test]
fn fly_requires_thunder_badge() {
    let result = use_fly(0, TilesetId::Overworld, Some(MapId::PalletTown));
    assert_eq!(result, FlyResult::NoBadge);
}

#[test]
fn fly_only_works_outside() {
    let badges = 1 << BIT_THUNDERBADGE;
    let result = use_fly(badges, TilesetId::Interior, Some(MapId::PalletTown));
    assert_eq!(result, FlyResult::CannotFlyHere);
}

#[test]
fn fly_choose_destination() {
    let badges = 1 << BIT_THUNDERBADGE;
    let result = use_fly(badges, TilesetId::Overworld, Some(MapId::CeruleanCity));
    assert_eq!(
        result,
        FlyResult::ChoseDestination {
            destination: MapId::CeruleanCity
        }
    );
}

#[test]
fn fly_cancelled() {
    let badges = 1 << BIT_THUNDERBADGE;
    let result = use_fly(badges, TilesetId::Overworld, None);
    assert_eq!(result, FlyResult::Cancelled);
}

#[test]
fn fly_destinations_table_has_13_entries() {
    assert_eq!(FLY_DESTINATIONS.len(), NUM_FLY_DESTINATIONS);
    assert_eq!(NUM_FLY_DESTINATIONS, 13);
}

// ── HM: Surf ─────────────────────────────────────────────────────────

#[test]
fn surf_requires_soul_badge() {
    let result = use_surf(
        0,
        TilesetId::Overworld,
        true,
        false,
        false,
        MapId::PalletTown,
        false,
        0,
        0,
    );
    assert_eq!(result, SurfResult::NoBadge);
}

#[test]
fn surf_already_surfing() {
    let badges = 1 << BIT_SOULBADGE;
    let result = use_surf(
        badges,
        TilesetId::Overworld,
        true,
        true,
        false,
        MapId::PalletTown,
        false,
        0,
        0,
    );
    assert_eq!(result, SurfResult::AlreadySurfing);
}

#[test]
fn surf_not_facing_water() {
    let badges = 1 << BIT_SOULBADGE;
    let result = use_surf(
        badges,
        TilesetId::Overworld,
        false,
        false,
        false,
        MapId::PalletTown,
        false,
        0,
        0,
    );
    assert_eq!(result, SurfResult::NotFacingWater);
}

#[test]
fn surf_success() {
    let badges = 1 << BIT_SOULBADGE;
    let result = use_surf(
        badges,
        TilesetId::Overworld,
        true,
        false,
        false,
        MapId::PalletTown,
        false,
        0,
        0,
    );
    assert_eq!(result, SurfResult::StartedSurfing);
}

#[test]
fn surf_seafoam_b4f_blocked() {
    let badges = 1 << BIT_SOULBADGE;
    let result = use_surf(
        badges,
        TilesetId::Cavern,
        true,
        false,
        false,
        MapId::SeafoamIslandsB4F,
        false,
        SEAFOAM_B4F_STAIRS_X,
        SEAFOAM_B4F_STAIRS_Y,
    );
    assert_eq!(result, SurfResult::CurrentTooFast);
}

#[test]
fn surf_seafoam_b4f_boulders_done() {
    let badges = 1 << BIT_SOULBADGE;
    let result = use_surf(
        badges,
        TilesetId::Cavern,
        true,
        false,
        false,
        MapId::SeafoamIslandsB4F,
        true,
        SEAFOAM_B4F_STAIRS_X,
        SEAFOAM_B4F_STAIRS_Y,
    );
    assert_eq!(result, SurfResult::StartedSurfing);
}

// ── HM: Strength & Flash ────────────────────────────────────────────

#[test]
fn strength_requires_rainbow_badge() {
    assert_eq!(use_strength(0, false), StrengthResult::NoBadge);
}

#[test]
fn strength_activates() {
    let badges = 1 << BIT_RAINBOWBADGE;
    assert_eq!(use_strength(badges, false), StrengthResult::Activated);
}

#[test]
fn strength_already_active() {
    let badges = 1 << BIT_RAINBOWBADGE;
    assert_eq!(use_strength(badges, true), StrengthResult::AlreadyActive);
}

#[test]
fn flash_requires_boulder_badge() {
    assert_eq!(use_flash(0, true), FlashResult::NoBadge);
}

#[test]
fn flash_lights_dark_cave() {
    let badges = 1 << BIT_BOULDERBADGE;
    assert_eq!(use_flash(badges, true), FlashResult::LitUpCave);
}

#[test]
fn flash_already_lit() {
    let badges = 1 << BIT_BOULDERBADGE;
    assert_eq!(use_flash(badges, false), FlashResult::AlreadyLit);
}

// ── Boulder Pushing ─────────────────────────────────────────────────

#[test]
fn boulder_push_requires_strength() {
    let result = try_push_boulder(false, false, Some(1), true, true, true, false);
    assert_eq!(result, BoulderPushResult::StrengthNotActive);
}

#[test]
fn boulder_push_success() {
    let result = try_push_boulder(true, false, Some(1), true, true, true, false);
    assert!(matches!(result, BoulderPushResult::Pushed { .. }));
}

#[test]
fn boulder_push_need_second_press() {
    let result = try_push_boulder(true, false, Some(1), true, false, true, false);
    assert_eq!(result, BoulderPushResult::NeedPushAgain);
}

#[test]
fn boulder_push_blocked() {
    let result = try_push_boulder(true, false, Some(1), true, true, true, true);
    assert_eq!(result, BoulderPushResult::BoulderBlocked);
}

// ── Ledge Jumping ───────────────────────────────────────────────────

#[test]
fn ledge_jump_down_on_overworld() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Down,
        0x2C,
        0x37,
        Some(Direction::Down),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().direction, Direction::Down);
}

#[test]
fn ledge_jump_wrong_tileset() {
    let result = check_ledge_jump(
        TilesetId::Interior,
        Direction::Down,
        0x2C,
        0x37,
        Some(Direction::Down),
    );
    assert!(result.is_none());
}

#[test]
fn ledge_jump_wrong_direction() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Up,
        0x2C,
        0x37,
        Some(Direction::Up),
    );
    assert!(result.is_none());
}

#[test]
fn ledge_jump_not_holding_input() {
    let result = check_ledge_jump(TilesetId::Overworld, Direction::Down, 0x2C, 0x37, None);
    assert!(result.is_none());
}

#[test]
fn ledge_jump_right() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Right,
        0x2C,
        0x0D,
        Some(Direction::Right),
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().direction, Direction::Right);
}

// ── Warp Pads & Holes ───────────────────────────────────────────────

#[test]
fn facility_warp_pad_detected() {
    let result = check_warp_pad_or_hole(TilesetId::Facility, 0x20);
    assert_eq!(result, WarpPadOrHoleType::WarpPad);
}

#[test]
fn facility_hole_detected() {
    let result = check_warp_pad_or_hole(TilesetId::Facility, 0x11);
    assert_eq!(result, WarpPadOrHoleType::Hole);
}

#[test]
fn cavern_hole_detected() {
    let result = check_warp_pad_or_hole(TilesetId::Cavern, 0x22);
    assert_eq!(result, WarpPadOrHoleType::Hole);
}

#[test]
fn non_special_tile_returns_none() {
    let result = check_warp_pad_or_hole(TilesetId::Overworld, 0x00);
    assert_eq!(result, WarpPadOrHoleType::None);
}

// ── Spinners ────────────────────────────────────────────────────────

#[test]
fn spinner_rotates_in_facility() {
    let result = handle_spinner_rotation(TilesetId::Facility, Direction::Down);
    assert!(result.is_some());
    assert_eq!(result.unwrap().new_facing, Direction::Left);
}

#[test]
fn spinner_no_effect_on_overworld() {
    let result = handle_spinner_rotation(TilesetId::Overworld, Direction::Down);
    assert!(result.is_none());
}

#[test]
fn is_spinner_tile_detects_arrow_tiles() {
    // Facility arrow tiles ($20/$21/$30/$31) and Gym arrow tiles ($3c/$3d/$4c/$4d).
    assert!(is_spinner_tile(TilesetId::Facility, 0x20));
    assert!(is_spinner_tile(TilesetId::Facility, 0x31));
    assert!(is_spinner_tile(TilesetId::Gym, 0x3c));
    assert!(is_spinner_tile(TilesetId::Gym, 0x4d));
    // Non-arrow tile, wrong tileset's id, and a non-spinner tileset are all false.
    assert!(!is_spinner_tile(TilesetId::Facility, 0x00));
    assert!(!is_spinner_tile(TilesetId::Gym, 0x20));
    assert!(!is_spinner_tile(TilesetId::Overworld, 0x20));
}

// ── Dark Cave ───────────────────────────────────────────────────────

#[test]
fn dark_cave_state_lifecycle() {
    let mut state = DarkCaveState::new();
    assert!(!state.is_dark());

    state.enter_map(MapId::RockTunnel1F);
    assert!(state.is_dark());

    assert!(state.use_flash());
    assert!(!state.is_dark());
}

#[test]
fn dark_cave_not_dark_in_normal_map() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::PalletTown);
    assert!(!state.is_dark());
}

#[test]
fn dark_cave_leave_clears() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::RockTunnelB1F);
    assert!(state.is_dark());
    state.leave_map();
    assert!(!state.is_dark());
}

// ── Outside Map Check ───────────────────────────────────────────────

#[test]
fn overworld_is_outside() {
    assert!(is_outside_map(TilesetId::Overworld));
}

#[test]
fn plateau_is_outside() {
    assert!(is_outside_map(TilesetId::Plateau));
}

#[test]
fn interior_is_not_outside() {
    assert!(!is_outside_map(TilesetId::Interior));
}

// ── Wild Encounters ─────────────────────────────────────────────────

#[test]
fn encounter_type_grass() {
    let enc = determine_encounter_type(0x52, TilesetId::Overworld, MapId::Route1);
    assert_eq!(enc, TileEncounterType::Grass);
}

#[test]
fn encounter_type_water() {
    let enc = determine_encounter_type(WATER_TILE, TilesetId::Overworld, MapId::Route21);
    assert_eq!(enc, TileEncounterType::Water);
}

#[test]
fn encounter_suppressed_on_warp() {
    assert!(!should_check_encounter(true, false, 0));
}

#[test]
fn encounter_suppressed_during_script() {
    assert!(!should_check_encounter(false, true, 0));
}

#[test]
fn encounter_suppressed_during_cooldown() {
    assert!(!should_check_encounter(false, false, 5));
}

#[test]
fn encounter_allowed_normally() {
    assert!(should_check_encounter(false, false, 0));
}

// ── Combined: HM Badge Progression ─────────────────────────────────

#[test]
fn hm_badge_progression_gates() {
    let no_badges: u8 = 0;
    assert_eq!(use_flash(no_badges, true), FlashResult::NoBadge);
    assert_eq!(
        use_cut(no_badges, TilesetId::Overworld, 0x3D, 0x32),
        CutResult::NoBadge
    );
    assert_eq!(
        use_fly(no_badges, TilesetId::Overworld, Some(MapId::PalletTown)),
        FlyResult::NoBadge
    );
    assert_eq!(use_strength(no_badges, false), StrengthResult::NoBadge);
    assert_eq!(
        use_surf(
            no_badges,
            TilesetId::Overworld,
            true,
            false,
            false,
            MapId::PalletTown,
            false,
            0,
            0
        ),
        SurfResult::NoBadge,
    );

    let all_badges: u8 = 0xFF;
    assert_eq!(use_flash(all_badges, true), FlashResult::LitUpCave);
    assert_eq!(use_strength(all_badges, false), StrengthResult::Activated);
    assert_eq!(
        use_surf(
            all_badges,
            TilesetId::Overworld,
            true,
            false,
            false,
            MapId::PalletTown,
            false,
            0,
            0
        ),
        SurfResult::StartedSurfing,
    );
}
