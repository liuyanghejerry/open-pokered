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

// Re-export the engine's per-tile NPC walk duration: classic GB walkers take
// $10 frames per tile (movement.asm) — double the player's 8-frame/tile pace
// in this port. Renderers must normalize by this, not by the player's step.
pub use dotzuki_engine::overworld::npc_movement::NPC_WALK_FRAMES;

/// Pixel offset (0..=15 px) of a walking NPC along its facing, for smooth
/// rendering. Classic GB walkers advance 1px/frame over their 16-frame step
/// (16px/tile); the tile commit lands only the final pixel. `walk_counter`
/// counts remaining frames (`NPC_WALK_FRAMES` at step start, 0 when idle).
pub fn npc_walk_pixel_offset(walk_counter: u8) -> i32 {
    if walk_counter == 0 {
        0 // Idle — the step has already been committed to the tile grid
    } else {
        (NPC_WALK_FRAMES.saturating_sub(walk_counter)) as i32
    }
}

/// Walking-animation phase (0..=3) of a walking NPC: 0/2 stand, 1 step,
/// 3 step (mirrored). The four phases spread evenly across the 16-frame
/// step — 4 frames each — matching the player's 4-phase cycle compressed
/// to 2 frames per phase at its 8-frame pace.
pub fn npc_walk_anim_phase(walk_counter: u8) -> u8 {
    if walk_counter == 0 {
        0 // Idle
    } else if walk_counter > NPC_WALK_FRAMES - 4 {
        0 // 13..=16: stand
    } else if walk_counter > NPC_WALK_FRAMES - 8 {
        1 // 9..=12: step
    } else if walk_counter > NPC_WALK_FRAMES - 12 {
        2 // 5..=8: stand
    } else {
        3 // 1..=4: step (mirrored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_walk_pixel_offset_covers_the_full_tile() {
        assert_eq!(npc_walk_pixel_offset(0), 0); // idle
        assert_eq!(npc_walk_pixel_offset(NPC_WALK_FRAMES), 0); // step start
        assert_eq!(npc_walk_pixel_offset(NPC_WALK_FRAMES - 1), 1);
        assert_eq!(npc_walk_pixel_offset(1), 15); // commit lands the last px
    }

    #[test]
    fn npc_walk_anim_phase_cycles_evenly_over_16_frames() {
        // Step start stands, then step/stand/step in 4-frame quarters.
        assert_eq!(npc_walk_anim_phase(0), 0); // idle
        for wc in 13..=16 {
            assert_eq!(npc_walk_anim_phase(wc), 0, "wc={wc}");
        }
        for wc in 9..=12 {
            assert_eq!(npc_walk_anim_phase(wc), 1, "wc={wc}");
        }
        for wc in 5..=8 {
            assert_eq!(npc_walk_anim_phase(wc), 2, "wc={wc}");
        }
        for wc in 1..=4 {
            assert_eq!(npc_walk_anim_phase(wc), 3, "wc={wc}");
        }
    }
}

// ── FLY arrival (bird) ─────────────────────────────────────────────

/// FLY arrival animation — `EnterMapAnim`'s `.flyAnimation`
/// (engine/overworld/player_animations.asm:53-70): the player sprite is
/// replaced by the BIRD sprite, which flies in from the top-right along
/// `FlyAnimationEnterScreenCoords`, flapping every step (`DoFlyAnimation`:
/// 12 iterations × Delay3 = 36 frames), with SFX_FLY. Game-specific (the
/// coordinate list and sprite are Pokémon's), so it lives here rather than
/// in the engine crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterMapFlyState {
    /// Elapsed frames; each coordinate step lasts [`FLY_ANIM_STEP_FRAMES`].
    pub frame: u16,
}

/// `wFlyAnimCounter` initial value (player_animations.asm:61).
pub const FLY_ANIM_STEPS: u16 = 12;
/// `DoFlyAnimation`'s Delay3 per step.
pub const FLY_ANIM_STEP_FRAMES: u16 = 3;
pub const FLY_ANIM_FRAMES: u16 = FLY_ANIM_STEPS * FLY_ANIM_STEP_FRAMES;

/// `FlyAnimationEnterScreenCoords` (player_animations.asm:66-79): (y, x)
/// screen-pixel pairs — the bird enters off the top-right and glides down to
/// the landing spot at (0x3C, 0x40).
pub const FLY_ANIM_COORDS: [(u16, u16); FLY_ANIM_STEPS as usize] = [
    (0x05, 0x98),
    (0x0F, 0x90),
    (0x18, 0x88),
    (0x20, 0x80),
    (0x27, 0x78),
    (0x2D, 0x70),
    (0x32, 0x68),
    (0x36, 0x60),
    (0x39, 0x58),
    (0x3B, 0x50),
    (0x3C, 0x48),
    (0x3C, 0x40),
];

impl EnterMapFlyState {
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    pub fn tick(&mut self) {
        if self.frame < FLY_ANIM_FRAMES {
            self.frame += 1;
        }
    }

    pub fn is_done(&self) -> bool {
        self.frame >= FLY_ANIM_FRAMES
    }

    /// Bird sprite screen position (y, x) in pixels for the current frame.
    pub fn bird_pos(&self) -> (u16, u16) {
        let step = (self.frame / FLY_ANIM_STEP_FRAMES) as usize;
        FLY_ANIM_COORDS[step.min(FLY_ANIM_STEPS as usize - 1)]
    }

    /// Wing flap toggles once per step (`DoFlyAnimation` XORs the sprite
    /// image index each iteration).
    pub fn flap_frame(&self) -> u8 {
        ((self.frame / FLY_ANIM_STEP_FRAMES) & 1) as u8
    }
}

impl Default for EnterMapFlyState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod fly_tests {
    use super::*;

    #[test]
    fn fly_anim_matches_asm_frame_count_and_coords() {
        // DoFlyAnimation: 12 iterations × Delay3 = 36 frames.
        assert_eq!(FLY_ANIM_FRAMES, 36);
        let mut fly = EnterMapFlyState::new();
        assert!(!fly.is_done());
        // First coord pair: (5, 0x98) — off the top-right.
        assert_eq!(fly.bird_pos(), (0x05, 0x98));
        for _ in 0..FLY_ANIM_FRAMES {
            fly.tick();
        }
        assert!(fly.is_done());
        // Last coord pair: (0x3C, 0x40) — the landing spot.
        assert_eq!(fly.bird_pos(), (0x3C, 0x40));
    }

    #[test]
    fn fly_anim_flap_toggles_per_step() {
        let mut fly = EnterMapFlyState::new();
        assert_eq!(fly.flap_frame(), 0);
        for _ in 0..FLY_ANIM_STEP_FRAMES {
            fly.tick();
        }
        assert_eq!(fly.flap_frame(), 1, "wing flips after each Delay3 step");
        for _ in 0..FLY_ANIM_STEP_FRAMES {
            fly.tick();
        }
        assert_eq!(fly.flap_frame(), 0);
    }

    #[test]
    fn fly_coord_list_matches_fly_animation_enter_screen_coords() {
        // player_animations.asm:66-79 — byte-for-byte.
        assert_eq!(
            FLY_ANIM_COORDS,
            [
                (0x05, 0x98),
                (0x0F, 0x90),
                (0x18, 0x88),
                (0x20, 0x80),
                (0x27, 0x78),
                (0x2D, 0x70),
                (0x32, 0x68),
                (0x36, 0x60),
                (0x39, 0x58),
                (0x3B, 0x50),
                (0x3C, 0x48),
                (0x3C, 0x40),
            ]
        );
    }
}
