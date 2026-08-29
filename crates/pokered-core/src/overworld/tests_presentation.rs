//! Integration tests for the overworld presentation-state machines: the
//! pure frame-machine unit tests moved to
//! `dotzuki_engine::overworld::presentation` with the machines themselves;
//! what remains here is the pokered glue — FLASH white-out wiring, elevator
//! shake wiring, tileset-header → tile animation binding, and the warp-fade
//! interplay.

use super::presentation::{TileAnimKind, ELEVATOR_SHAKE_FRAMES};
use super::screen::{OverworldScreen, WarpFadeState};
use super::MovementState;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;

const BOULDER: u8 = 1 << 0; // BIT_BOULDERBADGE

fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(map, None, PokemonRedData)
}

fn test_mon() -> crate::battle::state::Pokemon {
    crate::pokemon::stats::create_pokemon(pokered_data::species::Species::Squirtle, 5, [0xFF, 0xFF])
        .unwrap()
}

fn idle_input() -> super::OverworldInput {
    super::OverworldInput::new(false, false, false, false, false, false, false, false)
}

// ── Elevator shake params binding (doors_elevators → presentation) ─

#[test]
fn elevator_shake_uses_core_params() {
    assert_eq!(
        ELEVATOR_SHAKE_FRAMES as u32,
        super::doors_elevators::elevator_shake_params().iterations as u32 * 2
    );
    assert_eq!(super::doors_elevators::elevator_shake_params().pixel_offset, 1);
}

// ── FLASH white-out (GBPalWhiteOutWithDelay3) ─────────────────────

#[test]
fn flash_white_out_starts_when_message_dismissed() {
    let mut screen = screen_on(MapId::RockTunnel1F);
    assert!(screen.dark_cave.is_dark());
    let mon = test_mon();
    screen.use_field_move(MoveId::Flash, &mon, BOULDER, MapId::PalletTown);
    assert!(!screen.dark_cave.is_dark(), "wMapPalOffset cleared");
    assert_eq!(screen.flash_lit_frames, 0, "white-out waits for the text");

    screen.pending_dialogue = None;
    let input = idle_input();
    screen.update_frame(input);
    assert_eq!(screen.flash_lit_frames, 3, "GBPalWhiteOutWithDelay3 = 3 frames");
    screen.update_frame(input);
    assert_eq!(screen.flash_lit_frames, 2);
    screen.update_frame(input);
    screen.update_frame(input);
    assert_eq!(screen.flash_lit_frames, 0);
}

#[test]
fn flash_outside_cave_has_no_white_out() {
    let mut screen = screen_on(MapId::PalletTown);
    let mon = test_mon();
    screen.use_field_move(MoveId::Flash, &mon, BOULDER, MapId::PalletTown);
    screen.pending_dialogue = None;
    screen.update_frame(idle_input());
    assert_eq!(screen.flash_lit_frames, 0);
}

// ── Naming screen white flash (GBPalWhiteOutWithDelay3) ───────────

#[test]
fn naming_screen_open_and_submit_flash_white() {
    let mut screen = screen_on(MapId::PalletTown);
    // The script effect opens the naming screen and starts the entry flash
    // together (DisplayNamingScreen, naming_screen.asm:84-120).
    screen.pending_naming_screen = Some(crate::naming_screen::NamingScreenState::new(
        crate::naming_screen::NamingScreenType::Pokemon,
    ));
    screen.naming_flash_frames = crate::naming_screen::NAMING_FLASH_FRAMES;

    // Input is ignored while the entry flash plays.
    screen.update_naming_input(
        crate::naming_screen::NamingInput {
            a: true,
            ..crate::naming_screen::NamingInput::none()
        },
        false,
    );
    assert_eq!(screen.pending_naming_screen.as_ref().unwrap().name(), "");

    // Gameplay freezes while the flash ticks down (blocking Delay3).
    let input = idle_input();
    screen.update_frame(input);
    assert_eq!(screen.naming_flash_frames, 2);
    screen.update_frame(input);
    screen.update_frame(input);
    assert_eq!(screen.naming_flash_frames, 0);

    // Input reaches the naming screen after the flash.
    screen.update_naming_input(
        crate::naming_screen::NamingInput {
            a: true,
            ..crate::naming_screen::NamingInput::none()
        },
        false,
    );
    assert_eq!(screen.pending_naming_screen.as_ref().unwrap().name(), "A");

    // Submitting (.submitNickname, naming_screen.asm:158-175) closes the
    // screen and flashes white again.
    screen.update_naming_input(
        crate::naming_screen::NamingInput {
            start: true,
            ..crate::naming_screen::NamingInput::none()
        },
        false,
    );
    assert!(screen.pending_naming_screen.is_none());
    assert_eq!(
        screen.naming_flash_frames,
        crate::naming_screen::NAMING_FLASH_FRAMES
    );
    for _ in 0..crate::naming_screen::NAMING_FLASH_FRAMES {
        screen.update_frame(input);
    }
    assert_eq!(screen.naming_flash_frames, 0);
}

// ── Elevator shake wiring ─────────────────────────────────────────

#[test]
fn elevator_shake_starts_after_floor_choice() {
    let mut screen = screen_on(MapId::PalletTown);
    screen.script_awaiting_elevator = true;
    screen.resume_script_after_elevator(2);
    // The shake is pending until the frame loop picks it up.
    let input = idle_input();
    screen.update_frame(input);
    assert!(screen.elevator_shake.is_some(), "shake starts");
    // Gameplay is frozen for the full shake, then the state clears.
    while screen.elevator_shake.is_some() {
        screen.update_frame(input);
    }
}

#[test]
fn elevator_cancel_does_not_shake() {
    let mut screen = screen_on(MapId::PalletTown);
    screen.script_awaiting_elevator = true;
    screen.resume_script_after_elevator(-1);
    screen.update_frame(idle_input());
    assert!(screen.elevator_shake.is_none());
}

// ── Tile animation enable follows the tileset header ─────────────

#[test]
fn tile_anim_follows_tileset_header_on_screen_creation() {
    let outdoor = screen_on(MapId::PalletTown); // OVERWORLD: WATER_FLOWER
    assert_eq!(outdoor.tile_anim.kind(), TileAnimKind::WaterFlower);
    let cave = screen_on(MapId::MtMoon1F); // CAVERN: WATER
    assert_eq!(cave.tile_anim.kind(), TileAnimKind::Water);
    let indoor = screen_on(MapId::RedsHouse1F); // REDS_HOUSE_1: NONE
    assert_eq!(indoor.tile_anim.kind(), TileAnimKind::None);
}

#[test]
fn warp_fade_to_white_resets_after_fade() {
    let mut screen = screen_on(MapId::MtMoon1F);
    let mon = test_mon();
    // DIG (ESCAPE ROPE flow): warps to the last PokéCenter's fly point —
    // here Pallet Town (no heal recorded → wLastBlackoutMap defaults there).
    screen.use_field_move(MoveId::Dig, &mon, 0, MapId::PalletTown);
    assert!(screen.warp_fade_to_white);
    let input = idle_input();
    // Run through the spin + full fade (24 out + 1 black + 24 in frames).
    for _ in 0..(136 + 17 + 10 + 24 + 1 + 24) {
        screen.update_frame(input);
    }
    assert!(matches!(screen.warp_fade_state, WarpFadeState::Idle));
    assert!(!screen.warp_fade_to_white, "next warp defaults to black");
    assert_eq!(screen.state.current_map, MapId::PalletTown, "warp committed");
}

// ── FLASH white-out freezes movement (GBPalWhiteOutWithDelay3) ─────

#[test]
fn flash_white_out_freezes_movement_for_three_frames() {
    let mut screen = screen_on(MapId::RockTunnel1F);
    // Stand on open cave floor (block 1), facing a passable tile above.
    screen.state.player.x = 10;
    screen.state.player.y = 10;
    let mon = test_mon();
    screen.use_field_move(MoveId::Flash, &mon, BOULDER, MapId::PalletTown);
    screen.pending_dialogue = None;
    let input = idle_input();
    screen.update_frame(input);
    assert_eq!(screen.flash_lit_frames, 3, "white-out starts");

    // Movement input during the white-out must be ignored (the original's
    // Delay3 blocks the whole overworld loop).
    let up = super::OverworldInput::new(true, false, false, false, false, false, false, false);
    screen.update_frame(up);
    screen.update_frame(up);
    screen.update_frame(up);
    assert_eq!(
        screen.state.player.movement_state,
        MovementState::Idle,
        "player cannot start moving while the FLASH white-out plays"
    );
    assert_eq!(screen.flash_lit_frames, 0, "3 frames elapsed");

    // Movement resumes once the white-out clears.
    screen.update_frame(up);
    assert_eq!(
        screen.state.player.movement_state,
        MovementState::Walking,
        "movement resumes after the freeze"
    );
}

// ── BoulderDustState wiring (AnimateBoulderDust, dust_smoke.asm) ───

#[test]
fn boulder_dust_inactive_by_default() {
    let screen = screen_on(MapId::PalletTown);
    assert!(!screen.boulder_dust.is_active(), "no dust without a push");
}
