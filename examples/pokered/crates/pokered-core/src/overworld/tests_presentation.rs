//! Unit + integration tests for the overworld presentation-state machines
//! (`presentation.rs`): the TELEPORT/DIG/ESCAPE ROPE spin-out
//! (`_LeaveMapAnim`), `ShakeElevator`, `UpdateMovingBgTiles` water/flower
//! animation, and the FLASH white-out.

use super::presentation::{
    ElevatorShakeState, TileAnimState, TeleportSpinState, ELEVATOR_SHAKE_FRAMES,
    SPIN_IN_PLACE_FRAMES,
};
use super::screen::{OverworldScreen, WarpFadeState};
use super::{Direction, MovementState};
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::tileset_data::TileAnimation;

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

// ── TeleportSpinState (_LeaveMapAnim escape path) ─────────────────

#[test]
fn spin_starts_on_current_facing_then_cycles_down_left_up_right() {
    // InitFacingDirectionList points the facing list at the current facing;
    // PlayerSpinningFacingOrder is DOWN, LEFT, UP, RIGHT.
    for (start, order) in [
        (
            Direction::Down,
            [
                Direction::Down,
                Direction::Left,
                Direction::Up,
                Direction::Right,
            ],
        ),
        (
            Direction::Up,
            [
                Direction::Up,
                Direction::Right,
                Direction::Down,
                Direction::Left,
            ],
        ),
    ] {
        let mut spin = TeleportSpinState::new(start);
        for (i, want) in order.iter().enumerate() {
            assert_eq!(&spin.facing(), want, "start {:?} spin {}", start, i);
            // Advance to the next spin: spin i lasts (16 - i) frames.
            for _ in 0..(16 - i) {
                spin.tick();
            }
        }
    }
}

#[test]
fn spin_sfx_schedule_matches_original() {
    // PlayerSpinInPlace: SFX_TELEPORT_EXIT_2 when the current delay is a
    // multiple of 4 (spins 0, 4, 8, 12 → 4 plays); SFX_TELEPORT_EXIT_1 once
    // at the start of the spin-up.
    let mut spin = TeleportSpinState::new(Direction::Down);
    let mut exit2 = 0;
    let mut exit1 = 0;
    let mut frames = 0;
    while !spin.is_done() {
        match spin.tick() {
            Some("SFX_TELEPORT_EXIT_2") => exit2 += 1,
            Some("SFX_TELEPORT_EXIT_1") => {
                exit1 += 1;
                assert_eq!(frames, SPIN_IN_PLACE_FRAMES, "exit-1 at spin-up start");
            }
            _ => {}
        }
        frames += 1;
        assert!(frames < 1000, "spin must terminate");
    }
    assert_eq!(exit2, 4);
    assert_eq!(exit1, 1);
    // 136 in-place + 17 spin-up (4×(1+3) + 1) + 10 delay.
    assert_eq!(frames, 136 + 17 + 10);
}

#[test]
fn spin_rises_off_screen_and_hides() {
    let mut spin = TeleportSpinState::new(Direction::Down);
    assert_eq!(spin.player_y_offset(), 0);
    assert!(spin.player_visible());
    for _ in 0..SPIN_IN_PLACE_FRAMES - 1 {
        spin.tick();
    }
    assert_eq!(spin.player_y_offset(), 0, "still grounded on the last spin frame");
    spin.tick(); // first spin-up step applies the -$10 delta immediately
    assert_eq!(spin.player_y_offset(), -16);
    // Remaining 4 steps (each 1 spin + 3 delay frames, the last has no delay).
    for _ in 0..16 {
        spin.tick();
    }
    assert_eq!(spin.player_y_offset(), -80);
    assert!(!spin.player_visible(), "sprite fully above the screen at $ec");
}

// ── ElevatorShakeState (ShakeElevator) ────────────────────────────

#[test]
fn elevator_shake_uses_core_params() {
    assert_eq!(
        ELEVATOR_SHAKE_FRAMES as u32,
        super::doors_elevators::elevator_shake_params().iterations as u32 * 2
    );
    assert_eq!(super::doors_elevators::elevator_shake_params().pixel_offset, 1);
}

#[test]
fn elevator_shake_alternates_offset_each_iteration() {
    let mut shake = ElevatorShakeState::new();
    // First iteration scrolls to -1 (e starts at 1, XORed with $fe first).
    assert_eq!(shake.offset_y(), -1);
    shake.tick();
    assert_eq!(shake.offset_y(), -1, "one iteration lasts 2 frames");
    shake.tick();
    assert_eq!(shake.offset_y(), 1);
    shake.tick();
    assert_eq!(shake.offset_y(), 1);
    shake.tick();
    assert_eq!(shake.offset_y(), -1);
}

#[test]
fn elevator_shake_sfx_and_duration() {
    let mut shake = ElevatorShakeState::new();
    let mut collisions = 0;
    let mut dings = 0;
    let mut frames = 0;
    while !shake.is_done() {
        match shake.tick() {
            Some("SFX_COLLISION") => collisions += 1,
            Some("SFX_SAFARI_ZONE_PA") => dings += 1,
            None => {}
            other => panic!("unexpected sfx {:?}", other),
        }
        frames += 1;
    }
    assert_eq!(frames, ELEVATOR_SHAKE_FRAMES);
    assert_eq!(collisions, 100, "SFX_COLLISION once per iteration");
    assert_eq!(dings, 1, "arrival ding at the end");
    assert_eq!(shake.offset_y(), 0, "scroll restored after the shake");
}

// ── TileAnimState (UpdateMovingBgTiles) ───────────────────────────

#[test]
fn tile_anim_disabled_for_none_tilesets() {
    let mut anim = TileAnimState::new();
    anim.set_tileset(TileAnimation::None);
    for _ in 0..100 {
        anim.tick();
    }
    assert_eq!(anim.water_shift(), 0);
    assert_eq!(anim.flower_frame(), None);
}

#[test]
fn tile_anim_water_rotates_every_20_frames() {
    let mut anim = TileAnimState::new();
    anim.set_tileset(TileAnimation::Water);
    for _ in 0..19 {
        anim.tick();
    }
    assert_eq!(anim.water_shift(), 0, "no update before counter1 hits 20");
    anim.tick();
    assert_eq!(anim.water_shift(), 1, "first update shifts right one pixel");
    for _ in 0..20 {
        anim.tick();
    }
    assert_eq!(anim.water_shift(), 2);
}

#[test]
fn tile_anim_water_direction_follows_counter2_bit2() {
    // counter2 increments per water update; direction is right while bit 2 is
    // clear (counter2 = 1,2,3,0), left while set (4,5,6,7). Net shift sequence
    // over the first 8 updates: 1,2,3,2,1,0,-1,0.
    let mut anim = TileAnimState::new();
    anim.set_tileset(TileAnimation::Water);
    let expected = [1, 2, 3, 2, 1, 0, -1, 0];
    for want in expected {
        for _ in 0..20 {
            anim.tick();
        }
        assert_eq!(anim.water_shift(), want);
    }
}

#[test]
fn tile_anim_flower_frames_cycle_1_2_3_1() {
    // Flower update one frame after each water update; frame from counter2&3:
    // 0/1 → flower1, 2 → flower2, 3 → flower3.
    let mut anim = TileAnimState::new();
    anim.set_tileset(TileAnimation::WaterFlower);
    assert_eq!(anim.flower_frame(), None, "base tile until the first update");
    let expected = [1, 2, 3, 1, 1, 2];
    for want in expected {
        for _ in 0..21 {
            anim.tick();
        }
        assert_eq!(anim.flower_frame(), Some(want));
    }
}

#[test]
fn tile_anim_map_load_resets_counter_but_keeps_water_phase() {
    // LoadTilesetHeader resets hMovingBGTilesCounter1 only.
    let mut anim = TileAnimState::new();
    anim.set_tileset(TileAnimation::WaterFlower);
    for _ in 0..10 {
        anim.tick();
    }
    anim.set_tileset(TileAnimation::Water);
    for _ in 0..19 {
        anim.tick();
    }
    assert_eq!(anim.water_shift(), 0);
    anim.tick();
    assert_eq!(anim.water_shift(), 1);
    assert_eq!(anim.flower_frame(), None, "water-only tilesets never flower");
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
    assert_eq!(outdoor.tile_anim.kind(), TileAnimation::WaterFlower);
    let cave = screen_on(MapId::MtMoon1F); // CAVERN: WATER
    assert_eq!(cave.tile_anim.kind(), TileAnimation::Water);
    let indoor = screen_on(MapId::RedsHouse1F); // REDS_HOUSE_1: NONE
    assert_eq!(indoor.tile_anim.kind(), TileAnimation::None);
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

// ── FishingAnimState (FishingAnim, player_animations.asm:378-469) ──

use super::presentation::{
    FishingAnimPhase, FishingAnimState, FISHING_ANIM_FRAMES, FISHING_BUBBLE_FRAMES,
    FISHING_CAST_DELAY_FRAMES, FISHING_ROD_OUT_FRAMES, FISHING_SHAKE_ITERATIONS,
    FISHING_SHAKE_STEP_FRAMES,
};

/// Tick `anim` `n` times in place.
fn tick_n(anim: &mut FishingAnimState, n: u16) {
    for _ in 0..n {
        anim.tick();
    }
}

#[test]
fn fishing_anim_no_bite_finishes_after_rod_out() {
    // `wRodResponse == 0` → NoNibbleText: no shake, no bubble.
    for facing in [Direction::Down, Direction::Up, Direction::Left, Direction::Right] {
        let mut anim = FishingAnimState::new(facing, false);
        assert_eq!(anim.phase(), FishingAnimPhase::CastDelay);
        // CastDelay covers the first 10 frames (DelayFrames(10)): ticks 1..9
        // are still casting, the 10th tick shows the rod.
        for _ in 1..FISHING_CAST_DELAY_FRAMES {
            anim.tick();
            assert_eq!(anim.phase(), FishingAnimPhase::CastDelay);
        }
        anim.tick();
        assert_eq!(anim.phase(), FishingAnimPhase::RodOut, "rod appears at frame 10");
        for _ in 0..FISHING_ROD_OUT_FRAMES - 1 {
            anim.tick();
            assert_eq!(anim.phase(), FishingAnimPhase::RodOut);
        }
        anim.tick();
        assert_eq!(anim.phase(), FishingAnimPhase::Done, "no bite → straight to text");
        assert!(anim.is_done());
    }
}

#[test]
fn fishing_anim_bite_plays_shake_then_bubble_then_done() {
    let mut anim = FishingAnimState::new(Direction::Down, true);
    tick_n(&mut anim, FISHING_CAST_DELAY_FRAMES + FISHING_ROD_OUT_FRAMES);
    assert_eq!(anim.phase(), FishingAnimPhase::Shake, "bite starts the shake");
    assert!(!anim.bubble_active());

    // 10 iterations × 3 frames (Delay3).
    for i in 0..FISHING_SHAKE_ITERATIONS {
        assert_eq!(
            anim.phase(),
            FishingAnimPhase::Shake,
            "shake iteration {i} still active"
        );
        // Offset toggles between +1 (even iteration) and 0 (odd) — the
        // original's `xor $1` on the Y coordinate.
        assert_eq!(anim.player_shake_offset(), if i % 2 == 0 { 1 } else { 0 });
        tick_n(&mut anim, FISHING_SHAKE_STEP_FRAMES);
    }
    assert_eq!(anim.phase(), FishingAnimPhase::Bubble, "shake ends → bubble");
    assert!(anim.bubble_active());
    assert_eq!(anim.player_shake_offset(), 0, "no shake during the bubble");

    for f in 1..FISHING_BUBBLE_FRAMES {
        anim.tick();
        assert_eq!(anim.phase(), FishingAnimPhase::Bubble, "bubble frame {f}");
        assert!(anim.bubble_active());
    }
    anim.tick();
    assert_eq!(anim.phase(), FishingAnimPhase::Done, "bubble ends → ItsABiteText");
    assert!(anim.is_done());
    assert!(!anim.bubble_active());
}

#[test]
fn fishing_anim_total_duration_matches_asm() {
    let mut no_bite = FishingAnimState::new(Direction::Left, false);
    let mut bite = FishingAnimState::new(Direction::Left, true);
    for _ in 0..FISHING_ANIM_FRAMES {
        no_bite.tick();
        bite.tick();
    }
    // 10 + 100 (+ 30 + 60 on a bite) — no-bite is done at 110.
    assert!(
        no_bite.is_done(),
        "no-bite finishes at 10 + 100 = 110 frames"
    );
    assert!(
        bite.is_done(),
        "bite finishes at 10 + 100 + 30 + 60 = 200 frames"
    );
    let mut late = FishingAnimState::new(Direction::Down, true);
    for _ in 0..FISHING_ANIM_FRAMES - 1 {
        late.tick();
    }
    assert_eq!(
        late.phase(),
        FishingAnimPhase::Bubble,
        "frame 199 is still the bubble"
    );
}

#[test]
fn fishing_anim_pose_and_rod_visibility_follow_phase_and_facing() {
    // CastDelay: nothing drawn (the rod OAM is set up after the 10-frame
    // delay). Done: pose gone.
    let mut anim = FishingAnimState::new(Direction::Up, true);
    assert!(!anim.pose_active());
    assert!(!anim.rod_visible());
    tick_n(&mut anim, FISHING_CAST_DELAY_FRAMES);
    assert!(anim.pose_active(), "fishing pose while the rod is out");
    assert!(anim.rod_visible());
    assert_eq!(anim.facing(), Direction::Up);

    // Facing up: the rod is hidden during the bubble so it does not overlap
    // the "!" (player_animations.asm:421-428), then the pose is restored.
    let shake_bubble = FISHING_CAST_DELAY_FRAMES + FISHING_ROD_OUT_FRAMES
        + FISHING_SHAKE_ITERATIONS * FISHING_SHAKE_STEP_FRAMES;
    tick_n(&mut anim, shake_bubble);
    assert_eq!(anim.phase(), FishingAnimPhase::Bubble);
    assert!(!anim.rod_visible(), "up-facing rod hidden under the bubble");
    assert!(anim.pose_active());
    tick_n(&mut anim, FISHING_BUBBLE_FRAMES);
    assert!(anim.is_done());
    assert!(!anim.pose_active(), "pose restored after the anim");

    // Other facings keep the rod visible during the bubble.
    let mut left = FishingAnimState::new(Direction::Left, true);
    tick_n(&mut left, shake_bubble);
    assert_eq!(left.phase(), FishingAnimPhase::Bubble);
    assert!(left.rod_visible());
}

#[test]
fn fishing_anim_rod_piece_matches_fishing_rod_oam_table() {
    // FishingRodOAM (player_animations.asm:471-476): dbsprite x,y → OAM
    // (y*16+ypx, x*16+xpx); screen Y = OAM Y - OAM_Y_OFS(16). The entries
    // are absolute screen coords for the original's bottom-anchored player
    // (top-left at (128,128), feet at the 144px bottom edge):
    //   down: dbsprite 9,11,4,3 → (148, 163) tile $fd — below the screen
    //   up:   dbsprite 9,8,4,4  → (148, 116) tile $fd
    //   left: dbsprite 8,10,0,0 → (128, 144) tile $fe
    //   right:dbsprite 11,10,0,0→ (176, 144) tile $fe OAM_XFLIP — off-screen
    // `rod_piece` returns the offsets from the player's top-left so the
    // port's centered player (screen (72,64)) keeps the same relationship.
    assert_eq!(
        FishingAnimState::rod_piece(Direction::Down),
        (20, 35, 0, false),
        "down rod hangs 19px below the player's feet"
    );
    assert_eq!(
        FishingAnimState::rod_piece(Direction::Up),
        (20, -12, 0, false),
        "up rod sits 12px above the player's head"
    );
    assert_eq!(
        FishingAnimState::rod_piece(Direction::Left),
        (0, 16, 1, false),
        "left rod at the player's bottom-left"
    );
    assert_eq!(
        FishingAnimState::rod_piece(Direction::Right),
        (48, 16, 1, true),
        "right rod at the player's bottom-right, X-flipped"
    );
}

// ── EnterMapSpinState (arrival spin-in, player_animations.asm:1-91) ──

use super::presentation::{
    EnterMapSpinPhase, EnterMapSpinState, ENTER_MAP_SPIN_DOWN_FRAMES,
    ENTER_MAP_SPIN_IN_PLACE_FRAMES,
};

#[test]
fn enter_map_spin_starts_hidden_and_descends_in_five_steps() {
    let mut anim = EnterMapSpinState::new(Direction::Down, true);
    // The state is created at warp commit; the player is off the top of the
    // screen (Y=$ec) while the fade-in plays — offset -80, not visible.
    assert_eq!(anim.phase(), EnterMapSpinPhase::SpinDown);
    assert!(!anim.player_visible(), "hidden until the spin-down descends");
    assert_eq!(anim.player_y_offset(), -80);

    // 5 moves of 16px on ticks 1, 5, 9, 13, 17 (a spin + 3-frame delay
    // each). The offset after tick 17 is the standing position.
    let mut last = -80;
    for frame in 1..=17 {
        anim.tick();
        if frame % 4 == 1 {
            assert!(
                anim.player_y_offset() > last,
                "move {frame} descends"
            );
            last = anim.player_y_offset();
        }
    }
    assert_eq!(
        anim.player_y_offset(),
        0,
        "standing position ($3c) after the 5th move"
    );
    assert!(anim.player_visible());
}

#[test]
fn enter_map_spin_timing_and_sfx() {
    let mut anim = EnterMapSpinState::new(Direction::Left, true);
    // SFX_TELEPORT_ENTER_1 on the very first frame (player_animations.asm:12).
    assert_eq!(anim.tick(), Some("SFX_TELEPORT_ENTER_1"));
    // 16 more frames of the spin-down, then SFX_TELEPORT_ENTER_2 (asm:20-21).
    for _ in 1..ENTER_MAP_SPIN_DOWN_FRAMES - 1 {
        assert_eq!(anim.tick(), None);
    }
    assert_eq!(
        anim.tick(),
        Some("SFX_TELEPORT_ENTER_2"),
        "enter-2 when the spin-down completes"
    );
    assert_eq!(anim.phase(), EnterMapSpinPhase::SpinInPlace);

    // The spin-in-place is 8 spins of 0,1,…,7 frames = 36 frames, silent.
    let mut ticks = 0;
    while !anim.is_done() {
        anim.tick();
        ticks += 1;
        assert!(ticks <= ENTER_MAP_SPIN_IN_PLACE_FRAMES, "bounded");
    }
    assert_eq!(ticks, ENTER_MAP_SPIN_IN_PLACE_FRAMES);
    assert!(anim.is_done());
}

#[test]
fn enter_map_spin_skips_spin_in_place_on_warp_pad() {
    // IsPlayerStandingOnWarpPadOrHole → arrival on a warp pad/hole skips
    // the final spin-in-place (player_animations.asm:22-25).
    let mut anim = EnterMapSpinState::new(Direction::Down, false);
    let mut ticks = 0;
    while !anim.is_done() && ticks < 200 {
        anim.tick();
        ticks += 1;
    }
    assert_eq!(ticks, ENTER_MAP_SPIN_DOWN_FRAMES, "no spin-in-place phase");
    assert!(anim.is_done());
}

#[test]
fn enter_map_spin_facing_cycles_and_restores() {
    // The facing advances DOWN→LEFT→UP→RIGHT from the start facing
    // (mirroring the departure spin's "step 0 = current facing" window: the
    // start facing shows while the first 16px move's delay runs).
    let mut anim = EnterMapSpinState::new(Direction::Down, true);
    anim.tick(); // frame 1 → first move, start facing still shown
    assert_eq!(anim.facing(), Direction::Down);
    for _ in 0..4 {
        anim.tick(); // frames 2..5 → second move starts
    }
    assert_eq!(anim.facing(), Direction::Left);
    // Deep into the spin-in-place the list has wrapped several times.
    for _ in 0..ENTER_MAP_SPIN_DOWN_FRAMES + 5 {
        anim.tick();
    }
    assert_eq!(anim.phase(), EnterMapSpinPhase::SpinInPlace);
    // f = 27 - 17 = 10 → spin-in-place index 4 (durations 1,2,3,4,5):
    // 5 + 4 = 9 steps from start → 9 mod 4 = 1 → LEFT.
    assert_eq!(anim.facing(), Direction::Left);
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

// ── BoulderDustState (AnimateBoulderDust, dust_smoke.asm) ─────────

#[test]
fn boulder_dust_inactive_by_default() {
    let screen = screen_on(MapId::PalletTown);
    assert!(!screen.boulder_dust.is_active(), "no dust without a push");
}

#[test]
fn boulder_dust_base_offsets_match_asm() {
    // BoulderDustAnimationOffsets (engine/overworld/cut.asm:170-176):
    // "2 blocks away from the player", in px from the player sprite's
    // top-left (OAM Y +16 cancels out of the visible position).
    for (facing, expected) in [
        (Direction::Down, (8, 52)),
        (Direction::Up, (8, -12)),
        (Direction::Left, (-24, 20)),
        (Direction::Right, (40, 20)),
    ] {
        let dust = super::presentation::BoulderDustState::new(facing, 5, 5);
        assert_eq!(dust.base_offset(), expected, "facing {:?}", facing);
    }
}

#[test]
fn boulder_dust_drifts_against_the_push_direction() {
    // MoveBoulderDustFunctionPointerTable (dust_smoke.asm:59-63): the dust
    // drifts opposite to the boulder's slide direction, 1px per step.
    for (facing, expected) in [
        (Direction::Down, (0, -1)),
        (Direction::Up, (0, 1)),
        (Direction::Left, (1, 0)),
        (Direction::Right, (-1, 0)),
    ] {
        let dust = super::presentation::BoulderDustState::new(facing, 5, 5);
        assert_eq!(dust.drift_px(), expected, "facing {:?}", facing);
    }
}

#[test]
fn boulder_dust_horizontal_push_moves_three_of_four_tiles() {
    // The OAM-adjust loop starts at the upper-right sprite
    // (wShadowOAMSprite36 + 1, dust_smoke.asm:46-51), so horizontal pushes
    // leave the upper-left tile in place.
    let dust = super::presentation::BoulderDustState::new(Direction::Left, 5, 5);
    assert_eq!(
        dust.tile_drifts(),
        [(0, 0), (1, 0), (1, 0), (1, 0)],
        "upper-left tile stays put"
    );
    let dust = super::presentation::BoulderDustState::new(Direction::Down, 5, 5);
    assert_eq!(dust.tile_drifts(), [(0, -1); 4], "vertical pushes move all tiles");
}

#[test]
fn boulder_dust_runs_8_steps_of_3_frames_then_ends() {
    // AnimateBoulderDust: `ld c, 8` steps, each ending in `Delay3`.
    let mut dust = super::presentation::BoulderDustState::new(Direction::Down, 5, 5);
    assert!(dust.is_active());
    assert_eq!(dust.step(), 0);
    for tick in 1..=24 {
        dust.tick();
        if tick == 24 {
            assert!(!dust.is_active(), "tick 24 completes the animation");
            continue;
        }
        assert!(dust.is_active(), "tick {}", tick);
        if tick % 3 == 0 {
            assert_eq!(dust.step(), tick / 3, "step advances every 3rd tick");
        } else {
            assert_eq!(dust.step(), (tick - 1) / 3, "tick {}", tick);
        }
    }
    assert!(!dust.is_active(), "8 steps × 3 frames = 24 frames");
    // Ticking a finished state is a no-op.
    dust.tick();
    assert!(!dust.is_active());
}

#[test]
fn boulder_dust_palette_flashes_every_step() {
    // rOBP1 XOR %01100100 once per step (dust_smoke.asm:21-23).
    let mut dust = super::presentation::BoulderDustState::new(Direction::Right, 5, 5);
    let mut seen = Vec::new();
    for _ in 0..24 {
        seen.push(dust.palette_flipped());
        dust.tick();
    }
    let expected: Vec<bool> = (0..8).map(|s| s % 2 == 1).flat_map(|f| [f, f, f]).collect();
    assert_eq!(seen, expected, "palette toggles on each odd step");
}

#[test]
fn boulder_dust_keeps_its_world_anchor() {
    // The OAM block is written once at animation start from the player's
    // position — the state must remember that spot.
    let mut dust = super::presentation::BoulderDustState::new(Direction::Up, 12, 7);
    assert_eq!(dust.anchor(), (12, 7));
    for _ in 0..24 {
        dust.tick();
    }
    assert_eq!(dust.anchor(), (12, 7), "anchor survives the animation");
}

// ── ShipDepartureState (VermilionDockSSAnneLeavesScript) ───────────

use super::presentation::{
    ShipDeparturePhase, ShipDepartureState, SHIP_DEPARTURE_ERASE_FRAMES,
    SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES, SHIP_DEPARTURE_ITERATION_FRAMES,
    SHIP_DEPARTURE_SCROLL_ITERATIONS, SHIP_DEPARTURE_TOTAL_FRAMES,
    SHIP_DEPARTURE_WATER_FILL_FRAMES,
};

#[test]
fn ship_departure_phase_boundaries_match_asm() {
    // VermilionDock.asm: DelayFrames(120) → Delay3 transfer → 8×128-frame
    // scroll loop → EraseSSAnne's DelayFrames(120).
    let mut dep = ShipDepartureState::new();
    assert_eq!(dep.phase(), ShipDeparturePhase::InitialPause);
    for _ in 0..SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES {
        dep.tick();
    }
    assert_eq!(dep.phase(), ShipDeparturePhase::WaterFill);
    for _ in 0..SHIP_DEPARTURE_WATER_FILL_FRAMES {
        dep.tick();
    }
    assert_eq!(dep.phase(), ShipDeparturePhase::Scroll);
    for _ in 0..SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_ITERATION_FRAMES {
        dep.tick();
    }
    assert_eq!(dep.phase(), ShipDeparturePhase::Erase);
    for _ in 0..SHIP_DEPARTURE_ERASE_FRAMES {
        dep.tick();
    }
    assert_eq!(dep.phase(), ShipDeparturePhase::Done);
    assert!(dep.is_done());
    assert_eq!(dep.frame(), SHIP_DEPARTURE_TOTAL_FRAMES);
    // Ticking a finished state is a no-op (returns no SFX).
    assert_eq!(dep.tick(), None);
    assert_eq!(dep.frame(), SHIP_DEPARTURE_TOTAL_FRAMES);
}

#[test]
fn ship_departure_horn_timing_matches_asm() {
    // SFX_SS_ANNE_HORN plays twice: PlaySoundWaitForCurrent before
    // .shift_columns_up (VermilionDock.asm:74-75) and PlaySound inside
    // VermilionDock_EraseSSAnne (VermilionDock.asm:218-219).
    let mut dep = ShipDepartureState::new();
    let mut horns = Vec::new();
    for _ in 0..SHIP_DEPARTURE_TOTAL_FRAMES {
        if let Some(sfx) = dep.tick() {
            horns.push((dep.frame(), sfx));
        }
    }
    assert_eq!(
        horns,
        vec![
            (SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES, "SFX_SS_ANNE_HORN"),
            (
                SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES
                    + SHIP_DEPARTURE_WATER_FILL_FRAMES
                    + SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_ITERATION_FRAMES,
                "SFX_SS_ANNE_HORN"
            ),
        ]
    );
}

#[test]
fn ship_departure_scroll_px_ramps_like_scx() {
    // The view advances 16px per iteration and the LY-split SCX adds one
    // more px per 8-frame substep: iteration 0 sweeps 1..=16, the total
    // reachable offset is 128px = 16 tiles.
    let mut dep = ShipDepartureState::new();
    for _ in 0..SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES {
        dep.tick();
    }
    let mut seen = Vec::new();
    for _ in 0..SHIP_DEPARTURE_ITERATION_FRAMES {
        seen.push(dep.scroll_px());
        dep.tick();
    }
    let expected: Vec<i32> = (0..16).flat_map(|d| std::iter::repeat(d + 1).take(8)).collect();
    assert_eq!(seen, expected, "scroll ramps 1..=16 across iteration 0");

    // Mid-animation: frame 123 + 40 substeps → iteration 2, substep 40 →
    // 2*16 + 40%16 + 1 = 41.
    for _ in 0..(40 * 8 - SHIP_DEPARTURE_ITERATION_FRAMES) {
        dep.tick();
    }
    assert_eq!(dep.scroll_iteration(), 2);
    assert_eq!(dep.scroll_substep(), 40);
    assert_eq!(dep.scroll_px(), 41);

    // The erase phase keeps the fully scrolled position (16 tiles).
    // (Tick one frame short of the end — frame 1267 is Done.)
    for _ in 0..(SHIP_DEPARTURE_TOTAL_FRAMES - 1 - (123 + 40 * 8)) {
        dep.tick();
    }
    assert_eq!(dep.phase(), ShipDeparturePhase::Erase);
    assert_eq!(dep.scroll_px(), 128);
    assert_eq!(dep.scroll_iteration(), SHIP_DEPARTURE_SCROLL_ITERATIONS - 1);
    assert_eq!(dep.scroll_substep(), 127);
    assert!(dep.ship_erased());
}

#[test]
fn ship_departure_puff_positions_match_asm() {
    // wSSAnneSmokeX starts at 88 and drops 16px per iteration; each puff
    // then drifts +2px per substep (VermilionDock_AnimSmokePuffDriftRight).
    let mut dep = ShipDepartureState::new();
    for _ in 0..SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES {
        dep.tick();
    }
    // 128 scroll frames = one full iteration (frames 123..250); at frame
    // 251 (substep 16, iteration 1) puff 1 has just been emitted, and
    // puff 0 has drifted 2px × (16+1) — its spawn iteration's drift loop
    // moved it immediately after emission.
    for _ in 0..16 * 8 {
        dep.tick();
    }
    assert_eq!(dep.puff_count(), 2);
    assert_eq!(dep.puff_x_offset(0), 88 + 2 * (16 + 1));
    assert_eq!(dep.puff_x_offset(1), 72 + 2 * (16 - 16 + 1));

    // After one more iteration (substep 32, iteration 2): puffs 0..2 live.
    for _ in 0..SHIP_DEPARTURE_ITERATION_FRAMES {
        dep.tick();
    }
    assert_eq!(dep.puff_count(), 3);
    assert_eq!(dep.puff_x_offset(0), 88 + 2 * (32 + 1));
    assert_eq!(dep.puff_x_offset(1), 72 + 2 * (32 - 16 + 1));
    assert_eq!(dep.puff_x_offset(2), 56 + 2 * (32 - 32 + 1));

    // At the very end (substep 127) all 8 puffs are live.
    for _ in 0..6 * SHIP_DEPARTURE_ITERATION_FRAMES {
        dep.tick();
    }
    assert_eq!(dep.phase(), ShipDeparturePhase::Erase);
    assert_eq!(dep.puff_count(), 8);
    assert_eq!(dep.puff_x_offset(0), 88 + 2 * 128);
    assert_eq!(dep.puff_x_offset(7), 88 - 16 * 7 + 2 * (127 - 16 * 7 + 1));
    // Puff Y is the smokestack row in the original's screen px (OAM 100-16).
    assert_eq!(dep.puff_screen_y(), 84);
}

#[test]
fn ship_departure_erase_flag_only_in_erase_phase() {
    let mut dep = ShipDepartureState::new();
    assert!(!dep.ship_erased());
    for _ in 0..SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES {
        dep.tick();
    }
    assert!(!dep.ship_erased(), "hull still visible during the scroll");
    for _ in 0..SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_ITERATION_FRAMES {
        dep.tick();
    }
    assert!(dep.ship_erased(), "erase phase shows the ship as water");
}
