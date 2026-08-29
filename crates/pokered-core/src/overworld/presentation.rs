//! Overworld presentation-state machines — thin pokered shell over
//! [`dotzuki_engine::overworld::presentation`]. The generic frame-counted
//! state machines (teleport spin-out/in, elevator shake, water/flower tile
//! animation, fishing rod, boulder dust, ship departure, FLASH white-out)
//! live in the engine; this module re-exports them and binds pokered's
//! data: the Gen-1 facing cycle, the elevator shake parameters, the Gen-1
//! tile ids, and the `SFX_*` audio ids the engine's typed sound cues map to.
//!
//! Gen-1 references:
//! - engine/overworld/player_animations.asm — `_LeaveMapAnim` (TELEPORT/DIG/
//!   ESCAPE ROPE spin-out), `PlayerSpinInPlace`, `PlayerSpinWhileMovingUpOrDown`
//! - engine/overworld/elevator.asm — `ShakeElevator`
//! - home/vcopy.asm — `UpdateMovingBgTiles` (water/flower tile animation)
//! - home/fade.asm — `LoadGBPal` / `GBPalWhiteOutWithDelay3` (dark cave, FLASH)

use dotzuki_engine::overworld::Direction;
use pokered_data::tileset_data::TileAnimation;

// Re-export the engine's state machines, phase enums, sound cues, and
// frame-count constants so existing `presentation::*` paths keep working.
pub use dotzuki_engine::overworld::presentation::{
    BoulderDustState, ElevatorShakeParams, ElevatorShakeSfx, ElevatorShakeState, EnterMapSpinPhase,
    EnterMapSpinSfx, EnterMapSpinState, FishingAnimPhase, FishingAnimState, ShipDeparturePhase,
    ShipDepartureSfx, ShipDepartureState, TileAnimKind, TileAnimState, TeleportSpinPhase,
    TeleportSpinSfx, TeleportSpinState, BOULDER_DUST_STEPS, BOULDER_DUST_STEP_FRAMES,
    ENTER_MAP_SPIN_DOWN_FRAMES, ENTER_MAP_SPIN_DOWN_STEPS, ENTER_MAP_SPIN_DOWN_STEP_DELAY,
    ENTER_MAP_SPIN_DOWN_STEP_PIXELS, ENTER_MAP_SPIN_IN_PLACE_FRAMES, FISHING_ANIM_FRAMES,
    FISHING_BUBBLE_FRAMES, FISHING_CAST_DELAY_FRAMES, FISHING_ROD_OUT_FRAMES,
    FISHING_SHAKE_ITERATIONS, FISHING_SHAKE_STEP_FRAMES, FLASH_WHITE_FRAMES,
    SHIP_DEPARTURE_ERASE_FRAMES, SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES,
    SHIP_DEPARTURE_ITERATION_FRAMES, SHIP_DEPARTURE_PUFF_DRIFT_PX_PER_SUBSTEP,
    SHIP_DEPARTURE_PUFF_SPACING_PX, SHIP_DEPARTURE_PUFF_START_SCREEN_X,
    SHIP_DEPARTURE_SCROLL_ITERATIONS, SHIP_DEPARTURE_SCROLL_PX_PER_ITERATION,
    SHIP_DEPARTURE_SMOKESTACK_TILE_X, SHIP_DEPARTURE_SMOKESTACK_TILE_Y,
    SHIP_DEPARTURE_SUBSTEPS_PER_ITERATION, SHIP_DEPARTURE_SUBSTEP_FRAMES,
    SHIP_DEPARTURE_TOTAL_FRAMES, SHIP_DEPARTURE_WATER_FILL_FRAMES, SPIN_IN_PLACE_FRAMES,
    SPIN_POST_DELAY_FRAMES, SPIN_UP_STEPS, SPIN_UP_STEP_DELAY, SPIN_UP_STEP_PIXELS,
};

/// pokered's teleport spin facing cycle — `PlayerSpinningFacingOrder`
/// (TELEPORT_SPIN_ORDER in fly_warp_data.asm): DOWN, LEFT, UP, RIGHT.
pub const TELEPORT_SPIN_FACINGS: [Direction; 4] = [
    Direction::Down,
    Direction::Left,
    Direction::Up,
    Direction::Right,
];

/// Total frames of the elevator shake with pokered's params (100 iterations
/// × 2 frames, ShakeElevator).
pub const ELEVATOR_SHAKE_FRAMES: u16 = super::doors_elevators::elevator_shake_params()
    .total_frames();

/// Water tile ID ($14 — `vTileset tile $14` in UpdateMovingBgTiles).
pub const ANIM_WATER_TILE: u8 = 0x14;
/// Flower tile ID ($03 — `vTileset tile $03` in UpdateMovingBgTiles).
pub const ANIM_FLOWER_TILE: u8 = 0x03;

/// Convert pokered's tileset-header animation byte into the engine's
/// generic [`TileAnimKind`].
pub fn tile_anim_kind(animation: TileAnimation) -> TileAnimKind {
    match animation {
        TileAnimation::None => TileAnimKind::None,
        TileAnimation::Water => TileAnimKind::Water,
        TileAnimation::WaterFlower => TileAnimKind::WaterFlower,
    }
}

/// Map the engine's teleport spin-out cues to pokered's audio ids.
pub fn teleport_spin_sfx(sfx: TeleportSpinSfx) -> &'static str {
    match sfx {
        TeleportSpinSfx::SpinLoop => "SFX_TELEPORT_EXIT_2",
        TeleportSpinSfx::Rise => "SFX_TELEPORT_EXIT_1",
    }
}

/// Map the engine's arrival spin-in cues to pokered's audio ids.
pub fn enter_map_spin_sfx(sfx: EnterMapSpinSfx) -> &'static str {
    match sfx {
        EnterMapSpinSfx::Descend => "SFX_TELEPORT_ENTER_1",
        EnterMapSpinSfx::Land => "SFX_TELEPORT_ENTER_2",
    }
}

/// Map the engine's elevator shake cues to pokered's audio ids.
pub fn elevator_shake_sfx(sfx: ElevatorShakeSfx) -> &'static str {
    match sfx {
        ElevatorShakeSfx::Rattle => "SFX_COLLISION",
        ElevatorShakeSfx::Arrive => "SFX_SAFARI_ZONE_PA",
    }
}

/// Map the engine's ship departure cues to pokered's audio ids.
pub fn ship_departure_sfx(sfx: ShipDepartureSfx) -> &'static str {
    match sfx {
        ShipDepartureSfx::Horn => "SFX_SS_ANNE_HORN",
    }
}
