use pokered_data::maps::MapId;
use pokered_data::tileset_data::WarpPadOrHoleType;
use pokered_data::tilesets::TilesetId;

use super::special_terrain::*;
use super::Direction;

// ── Ledge Jump Tests ─────────────────────────────────────────────────

#[test]
fn ledge_jump_down_standing_2c_ledge_37() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Down,
        0x2C,
        0x37,
        Some(Direction::Down),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Down
        })
    );
}

#[test]
fn ledge_jump_down_standing_39_ledge_36() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Down,
        0x39,
        0x36,
        Some(Direction::Down),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Down
        })
    );
}

#[test]
fn ledge_jump_down_standing_39_ledge_37() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Down,
        0x39,
        0x37,
        Some(Direction::Down),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Down
        })
    );
}

#[test]
fn ledge_jump_left_standing_2c_ledge_27() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Left,
        0x2C,
        0x27,
        Some(Direction::Left),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Left
        })
    );
}

#[test]
fn ledge_jump_left_standing_39_ledge_27() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Left,
        0x39,
        0x27,
        Some(Direction::Left),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Left
        })
    );
}

#[test]
fn ledge_jump_right_standing_2c_ledge_0d() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Right,
        0x2C,
        0x0D,
        Some(Direction::Right),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Right
        })
    );
}

#[test]
fn ledge_jump_right_standing_2c_ledge_1d() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Right,
        0x2C,
        0x1D,
        Some(Direction::Right),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Right
        })
    );
}

#[test]
fn ledge_jump_right_standing_39_ledge_0d() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Right,
        0x39,
        0x0D,
        Some(Direction::Right),
    );
    assert_eq!(
        result,
        Some(LedgeJumpResult {
            direction: Direction::Right
        })
    );
}

#[test]
fn ledge_jump_wrong_tileset_returns_none() {
    let result = check_ledge_jump(
        TilesetId::Forest,
        Direction::Down,
        0x2C,
        0x37,
        Some(Direction::Down),
    );
    assert_eq!(result, None);
}

#[test]
fn ledge_jump_wrong_facing_returns_none() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Up,
        0x2C,
        0x37,
        Some(Direction::Up),
    );
    assert_eq!(result, None);
}

#[test]
fn ledge_jump_wrong_input_direction_returns_none() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Down,
        0x2C,
        0x37,
        Some(Direction::Left),
    );
    assert_eq!(result, None);
}

#[test]
fn ledge_jump_no_input_held_returns_none() {
    let result = check_ledge_jump(TilesetId::Overworld, Direction::Down, 0x2C, 0x37, None);
    assert_eq!(result, None);
}

#[test]
fn ledge_jump_wrong_standing_tile_returns_none() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Down,
        0x00,
        0x37,
        Some(Direction::Down),
    );
    assert_eq!(result, None);
}

#[test]
fn ledge_jump_wrong_ledge_tile_returns_none() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Down,
        0x2C,
        0xFF,
        Some(Direction::Down),
    );
    assert_eq!(result, None);
}

#[test]
fn ledge_jump_up_direction_never_jumps() {
    let result = check_ledge_jump(
        TilesetId::Overworld,
        Direction::Up,
        0x2C,
        0x37,
        Some(Direction::Up),
    );
    assert_eq!(result, None);
}

// ── Warp Pad / Hole Tests ────────────────────────────────────────────

#[test]
fn warp_pad_facility_tile_20() {
    assert_eq!(
        check_warp_pad_or_hole(TilesetId::Facility, 0x20),
        WarpPadOrHoleType::WarpPad
    );
}

#[test]
fn hole_facility_tile_11() {
    assert_eq!(
        check_warp_pad_or_hole(TilesetId::Facility, 0x11),
        WarpPadOrHoleType::Hole
    );
}

#[test]
fn hole_cavern_tile_22() {
    assert_eq!(
        check_warp_pad_or_hole(TilesetId::Cavern, 0x22),
        WarpPadOrHoleType::Hole
    );
}

#[test]
fn warp_pad_interior_tile_55() {
    assert_eq!(
        check_warp_pad_or_hole(TilesetId::Interior, 0x55),
        WarpPadOrHoleType::WarpPad
    );
}

#[test]
fn warp_pad_wrong_tileset_returns_none() {
    assert_eq!(
        check_warp_pad_or_hole(TilesetId::Overworld, 0x20),
        WarpPadOrHoleType::None
    );
}

#[test]
fn warp_pad_wrong_tile_returns_none() {
    assert_eq!(
        check_warp_pad_or_hole(TilesetId::Facility, 0xFF),
        WarpPadOrHoleType::None
    );
}

// ── Spinner Rotation Tests ───────────────────────────────────────────

#[test]
fn spinner_facility_down_to_left() {
    let result = handle_spinner_rotation(TilesetId::Facility, Direction::Down);
    assert_eq!(
        result,
        Some(SpinnerResult {
            new_facing: Direction::Left
        })
    );
}

#[test]
fn spinner_facility_up_to_right() {
    let result = handle_spinner_rotation(TilesetId::Facility, Direction::Up);
    assert_eq!(
        result,
        Some(SpinnerResult {
            new_facing: Direction::Right
        })
    );
}

#[test]
fn spinner_facility_left_to_up() {
    let result = handle_spinner_rotation(TilesetId::Facility, Direction::Left);
    assert_eq!(
        result,
        Some(SpinnerResult {
            new_facing: Direction::Up
        })
    );
}

#[test]
fn spinner_facility_right_to_down() {
    let result = handle_spinner_rotation(TilesetId::Facility, Direction::Right);
    assert_eq!(
        result,
        Some(SpinnerResult {
            new_facing: Direction::Down
        })
    );
}

#[test]
fn spinner_gym_down_to_left() {
    let result = handle_spinner_rotation(TilesetId::Gym, Direction::Down);
    assert_eq!(
        result,
        Some(SpinnerResult {
            new_facing: Direction::Left
        })
    );
}

#[test]
fn spinner_full_cycle_returns_to_start() {
    let mut facing = Direction::Down;
    for _ in 0..4 {
        let result = handle_spinner_rotation(TilesetId::Facility, facing).unwrap();
        facing = result.new_facing;
    }
    assert_eq!(facing, Direction::Down);
}

#[test]
fn spinner_wrong_tileset_returns_none() {
    assert_eq!(
        handle_spinner_rotation(TilesetId::Overworld, Direction::Down),
        None
    );
    assert_eq!(
        handle_spinner_rotation(TilesetId::Forest, Direction::Left),
        None
    );
    assert_eq!(
        handle_spinner_rotation(TilesetId::Cavern, Direction::Up),
        None
    );
}

// ── Dark Cave State Tests ────────────────────────────────────────────

#[test]
fn dark_cave_starts_not_dark() {
    let state = DarkCaveState::new();
    assert!(!state.is_dark());
}

#[test]
fn dark_cave_enter_rock_tunnel_1f() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::RockTunnel1F);
    assert!(state.is_dark());
    assert_eq!(state.map_pal_offset, 0x06);
}

#[test]
fn dark_cave_enter_rock_tunnel_b1f() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::RockTunnelB1F);
    assert!(state.is_dark());
}

#[test]
fn dark_cave_enter_normal_map_not_dark() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::PalletTown);
    assert!(!state.is_dark());
}

#[test]
fn dark_cave_flash_clears_darkness() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::RockTunnel1F);
    assert!(state.is_dark());
    let used = state.use_flash();
    assert!(used);
    assert!(!state.is_dark());
    assert_eq!(state.map_pal_offset, 0);
}

#[test]
fn dark_cave_flash_on_lit_map_returns_false() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::PalletTown);
    assert!(!state.use_flash());
}

#[test]
fn dark_cave_leave_clears_state() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::RockTunnel1F);
    state.leave_map();
    assert!(!state.is_dark());
}

#[test]
fn dark_cave_should_skip_fade_when_dark() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::RockTunnel1F);
    assert!(state.should_skip_fade_after_battle());
}

#[test]
fn dark_cave_should_not_skip_fade_when_lit() {
    let state = DarkCaveState::new();
    assert!(!state.should_skip_fade_after_battle());
}

#[test]
fn dark_cave_re_enter_dark_after_leaving() {
    let mut state = DarkCaveState::new();
    state.enter_map(MapId::RockTunnel1F);
    state.leave_map();
    state.enter_map(MapId::RockTunnelB1F);
    assert!(state.is_dark());
}

// ── Outside Map Check Tests ──────────────────────────────────────────

#[test]
fn overworld_is_outside() {
    assert!(is_outside_map(TilesetId::Overworld));
}

#[test]
fn plateau_is_outside() {
    assert!(is_outside_map(TilesetId::Plateau));
}

#[test]
fn forest_is_not_outside() {
    assert!(!is_outside_map(TilesetId::Forest));
}

#[test]
fn cavern_is_not_outside() {
    assert!(!is_outside_map(TilesetId::Cavern));
}

// ── Extra Warp Check Tests ───────────────────────────────────────────

#[test]
fn ss_anne_3f_uses_facing_edge() {
    assert_eq!(
        extra_warp_check_type(MapId::SSAnne3F, TilesetId::Ship),
        ExtraWarpCheckType::FacingEdge
    );
}

#[test]
fn rocket_hideout_uses_warp_tile() {
    assert_eq!(
        extra_warp_check_type(MapId::RocketHideoutB1F, TilesetId::Facility),
        ExtraWarpCheckType::WarpTileInFront
    );
}

#[test]
fn overworld_tileset_uses_warp_tile() {
    assert_eq!(
        extra_warp_check_type(MapId::PalletTown, TilesetId::Overworld),
        ExtraWarpCheckType::WarpTileInFront
    );
}

#[test]
fn house_tileset_uses_facing_edge() {
    assert_eq!(
        extra_warp_check_type(MapId::RedsHouse1F, TilesetId::House),
        ExtraWarpCheckType::FacingEdge
    );
}

#[test]
fn rock_tunnel_1f_uses_warp_tile() {
    assert_eq!(
        extra_warp_check_type(MapId::RockTunnel1F, TilesetId::Cavern),
        ExtraWarpCheckType::WarpTileInFront
    );
}
