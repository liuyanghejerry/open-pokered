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

// ── Ledge jump ───────────────────────────────────────────────────────

/// Frame-exact presentation state for the two simulated joypad steps used by
/// `HandleLedges`. The original spends 40 visible frames from the ledge flag
/// being set until control is restored: setup, two 16 px steps at 2 px every
/// other frame, then the three-frame landing delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgeJumpState {
    pub origin_x: u16,
    pub origin_y: u16,
    pub direction: Direction,
    /// Visible frame index. Frame 0 is the first frame with the ledge flag set.
    pub frame: u8,
}

pub const LEDGE_JUMP_LAST_ACTIVE_FRAME: u8 = 38;

impl LedgeJumpState {
    pub fn new(origin_x: u16, origin_y: u16, direction: Direction) -> Self {
        Self {
            origin_x,
            origin_y,
            direction,
            frame: 0,
        }
    }

    /// Advance one rendered frame. `true` means the landing delay has ended.
    pub fn tick(&mut self) -> bool {
        self.frame = self.frame.saturating_add(1);
        self.frame > LEDGE_JUMP_LAST_ACTIVE_FRAME
    }

    /// Number of whole map tiles committed by this frame (0, then 1, then 2).
    /// The commits intentionally precede the matching final 2 px camera tick,
    /// exactly as the original updates wYCoord at frames 20 and 36.
    pub fn committed_steps(&self) -> i32 {
        if self.frame >= 34 {
            2
        } else if self.frame >= 18 {
            1
        } else {
            0
        }
    }

    pub fn player_position(&self) -> (u16, u16) {
        let (dx, dy) = super::player_movement::direction_delta(self.direction);
        let steps = self.committed_steps();
        (
            (self.origin_x as i32 + dx as i32 * steps).max(0) as u16,
            (self.origin_y as i32 + dy as i32 * steps).max(0) as u16,
        )
    }

    pub fn landing_position(&self) -> (u16, u16) {
        let (dx, dy) = super::player_movement::direction_delta(self.direction);
        (
            (self.origin_x as i32 + dx as i32 * 2).max(0) as u16,
            (self.origin_y as i32 + dy as i32 * 2).max(0) as u16,
        )
    }

    /// Total camera travel from the jump origin. Motion begins after setup and
    /// advances 2 px every other frame, yielding 16 evenly distributed steps.
    pub fn camera_progress_px(&self) -> i32 {
        if self.frame < 5 {
            0
        } else {
            (2 * (1 + (self.frame as i32 - 5) / 2)).min(32)
        }
    }

    /// Camera offset relative to the currently committed logical tile.
    pub fn camera_residual_px(&self) -> (i32, i32) {
        let residual = self.camera_progress_px() - self.committed_steps() * 16;
        match self.direction {
            Direction::Down => (0, residual),
            Direction::Up => (0, -residual),
            Direction::Left => (-residual, 0),
            Direction::Right => (residual, 0),
        }
    }

    /// Original `PlayerJumpingYScreenCoords`, expressed relative to the 60 px
    /// standing baseline. The first three frames are setup; table index 1 is
    /// then held for three frames before the regular two-frame cadence.
    pub fn player_y_offset(&self) -> i32 {
        const OFFSETS: [i32; 15] = [
            -4, -6, -8, -10, -11, -12, -12, -12, -11, -10, -9, -8, -6, -4, 0,
        ];
        let index = if self.frame < 3 {
            return 0;
        } else if self.frame <= 5 {
            1
        } else {
            2 + (self.frame - 6) / 2
        };
        OFFSETS[(index.saturating_sub(1) as usize).min(OFFSETS.len() - 1)]
    }

    /// `wWalkCounter` values exposed while the two simulated tile steps run.
    pub fn walk_counter(&self) -> u8 {
        match self.frame {
            0..=2 => 0,
            3..=5 => 7,
            6..=17 => 6 - (self.frame - 6) / 2,
            18..=19 => 0,
            20..=21 => 7,
            22..=33 => 6 - (self.frame - 22) / 2,
            _ => 0,
        }
    }
}

/// One-tile scripted field-move step (SURF mount/dismount). The original
/// movement loop is sampled every other display frame: eight 2 px camera
/// advances, followed by the frame that clears scripted movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldMoveStepState {
    pub origin_x: u16,
    pub origin_y: u16,
    pub direction: Direction,
    pub frame: u8,
}

/// Blocking screen restoration between a successful party-menu SURF action
/// and the queued simulated step. The original spends 60 visible frames in
/// `GBPalWhiteOutWithDelay3`, sprite/font tile reloads, and
/// `CloseTextDisplay`: 37 all-white frames followed by 23 map-only frames
/// while OAM graphics are still being restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldMoveRestoreState {
    pub frame: u8,
}

pub const FIELD_MOVE_RESTORE_FRAMES: u8 = 60;
pub const FIELD_MOVE_RESTORE_WHITE_FRAMES: u8 = 37;

impl FieldMoveRestoreState {
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    pub fn tick(&mut self) -> bool {
        self.frame = self.frame.saturating_add(1);
        self.frame >= FIELD_MOVE_RESTORE_FRAMES
    }

    pub fn force_white(&self) -> bool {
        self.frame < FIELD_MOVE_RESTORE_WHITE_FRAMES
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutAnimKind {
    Tree,
    Grass,
}

/// CUT's temporary four-sprite OAM block. Tree CUT holds the intact 2×2
/// shape during setup, then pulls the top and bottom rows apart over eight
/// updates (`AnimCut`), for 18 raw visible frames in total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutAnimState {
    pub facing: Direction,
    pub kind: CutAnimKind,
    pub frame: u8,
}

pub const CUT_ANIM_FRAMES: u8 = 18;

impl CutAnimState {
    pub fn new(facing: Direction, kind: CutAnimKind) -> Self {
        Self {
            facing,
            kind,
            frame: 0,
        }
    }

    pub fn tick(&mut self) -> bool {
        self.frame = self.frame.saturating_add(1);
        self.frame >= CUT_ANIM_FRAMES
    }

    /// Horizontal separation applied to each row of a cut tree (0..=8 px).
    pub fn tree_spread_px(&self) -> i32 {
        self.frame.saturating_sub(8).min(8) as i32
    }

    pub fn palette_flipped(&self) -> bool {
        self.tree_spread_px() & 1 != 0
    }

    /// Screen offset from the player's sprite top-left after accounting for
    /// Game Boy OAM's +8 X / +16 Y hardware coordinate bias.
    pub fn base_offset(&self) -> (i32, i32) {
        match self.facing {
            Direction::Down => (0, 20),
            Direction::Up => (0, -12),
            Direction::Left => (-16, 4),
            Direction::Right => (16, 4),
        }
    }
}

pub const FIELD_MOVE_STEP_LAST_ACTIVE_FRAME: u8 = 17;

impl FieldMoveStepState {
    pub fn new(origin_x: u16, origin_y: u16, direction: Direction) -> Self {
        Self {
            origin_x,
            origin_y,
            direction,
            frame: 0,
        }
    }

    pub fn tick(&mut self) -> bool {
        self.frame = self.frame.saturating_add(1);
        self.frame > FIELD_MOVE_STEP_LAST_ACTIVE_FRAME
    }

    pub fn committed(&self) -> bool {
        self.frame >= 16
    }

    pub fn player_position(&self) -> (u16, u16) {
        if !self.committed() {
            return (self.origin_x, self.origin_y);
        }
        self.landing_position()
    }

    pub fn landing_position(&self) -> (u16, u16) {
        let (dx, dy) = super::player_movement::direction_delta(self.direction);
        (
            (self.origin_x as i32 + dx as i32).max(0) as u16,
            (self.origin_y as i32 + dy as i32).max(0) as u16,
        )
    }

    pub fn camera_progress_px(&self) -> i32 {
        if self.frame < 3 {
            0
        } else {
            (2 * (1 + (self.frame as i32 - 3) / 2)).min(16)
        }
    }

    pub fn camera_residual_px(&self) -> (i32, i32) {
        let residual = self.camera_progress_px() - if self.committed() { 16 } else { 0 };
        match self.direction {
            Direction::Down => (0, residual),
            Direction::Up => (0, -residual),
            Direction::Left => (-residual, 0),
            Direction::Right => (residual, 0),
        }
    }

    pub fn walk_counter(&self) -> u8 {
        match self.frame {
            0 => 0,
            1..=3 => 7,
            4..=15 => 6 - (self.frame - 4) / 2,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_prelude::*;

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

    #[test]
    fn ledge_jump_matches_original_camera_and_coordinate_cadence() {
        let mut jump = LedgeJumpState::new(10, 4, Direction::Down);
        let mut camera_steps = Vec::new();
        let mut positions = Vec::new();
        let mut previous_progress = 0;

        for _ in 0..=LEDGE_JUMP_LAST_ACTIVE_FRAME {
            let progress = jump.camera_progress_px();
            if progress != previous_progress {
                camera_steps.push(progress - previous_progress);
                previous_progress = progress;
            }
            if positions.last().copied() != Some(jump.player_position()) {
                positions.push(jump.player_position());
            }
            jump.tick();
        }

        assert_eq!(camera_steps, vec![2; 16]);
        assert_eq!(positions, vec![(10, 4), (10, 5), (10, 6)]);
        assert_eq!(jump.landing_position(), (10, 6));
        assert!(jump.tick(), "frame 39 restores control");
    }

    #[test]
    fn ledge_jump_uses_original_vertical_arc() {
        let jump = |frame| {
            let mut state = LedgeJumpState::new(10, 4, Direction::Down);
            state.frame = frame;
            state.player_y_offset()
        };
        assert_eq!(jump(2), 0);
        assert_eq!(jump(3), -4);
        assert_eq!(jump(6), -6);
        assert_eq!(jump(14), -12);
        assert_eq!(jump(20), -11);
        assert_eq!(jump(32), 0);
    }

    #[test]
    fn field_move_step_spreads_one_tile_over_eighteen_frames() {
        let mut step = FieldMoveStepState::new(5, 13, Direction::Down);
        let mut progress = Vec::new();
        let mut samples = Vec::new();
        let mut previous = 0;
        for _ in 0..=FIELD_MOVE_STEP_LAST_ACTIVE_FRAME {
            let current = step.camera_progress_px();
            samples.push((current, step.walk_counter(), step.player_position()));
            if current != previous {
                progress.push(current - previous);
                previous = current;
            }
            step.tick();
        }
        assert_eq!(progress, vec![2; 8]);
        assert_eq!(
            samples.iter().map(|sample| sample.0).collect::<Vec<_>>(),
            vec![0, 0, 0, 2, 2, 4, 4, 6, 6, 8, 8, 10, 10, 12, 12, 14, 14, 16]
        );
        assert_eq!(
            samples.iter().map(|sample| sample.1).collect::<Vec<_>>(),
            vec![0, 7, 7, 7, 6, 6, 5, 5, 4, 4, 3, 3, 2, 2, 1, 1, 0, 0]
        );
        assert_eq!(step.player_position(), (5, 14));
        assert!(step.tick(), "the eighteenth visible frame restores control");
    }

    #[test]
    fn field_move_restore_matches_white_and_map_only_windows() {
        let mut restore = FieldMoveRestoreState::new();
        let mut white = 0;
        let mut map_only = 0;
        loop {
            if restore.force_white() {
                white += 1;
            } else {
                map_only += 1;
            }
            if restore.tick() {
                break;
            }
        }
        assert_eq!(white, 37);
        assert_eq!(map_only, 23);
    }

    #[test]
    fn cut_tree_holds_then_separates_for_eighteen_frames() {
        let mut cut = CutAnimState::new(Direction::Down, CutAnimKind::Tree);
        let mut spreads = Vec::new();
        while !cut.tick() {
            spreads.push(cut.tree_spread_px());
        }
        assert_eq!(spreads.len(), CUT_ANIM_FRAMES as usize - 1);
        assert_eq!(&spreads[..8], &[0; 8]);
        assert_eq!(&spreads[8..], &[1, 2, 3, 4, 5, 6, 7, 8, 8]);
    }
}

// ── FLY departure / arrival (bird) ─────────────────────────────────

/// Trigger-to-fade portion of `_LeaveMapAnim` for FLY. The first eight
/// frames remain on the town map (owned by the app); then the original spends
/// 49 frames on its menu transition before returning to the overworld. The
/// bird flaps in place, crosses right, waits off-screen, and crosses back out
/// through the top-left before the final white fade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaveMapFlyState {
    pub frame: u16,
}

pub const FLY_DEPARTURE_FRAMES: u16 = 205;
pub const FLY_DEPARTURE_TOWN_MAP_FRAMES: u8 = 8;
pub const FLY_DEPARTURE_WHITE_START: u16 = 8;
pub const FLY_DEPARTURE_WHITE_END: u16 = 57;
pub const FLY_DEPARTURE_BIRD_START: u16 = 72;
pub const FLY_DEPARTURE_FIRST_PATH_START: u16 = 98;
pub const FLY_DEPARTURE_HOLD_START: u16 = 132;
pub const FLY_DEPARTURE_SECOND_PATH_START: u16 = 174;

/// The departure's final GBFadeOutToWhite starts at raw t+205 and reaches the
/// map commit at t+228. The generic fade includes a separate BlackScreen
/// frame, hence 22 countdown frames here.
pub const FLY_DEPARTURE_FADE_FRAMES: u8 = 22;

/// Delay from FLY's map commit through EnterMapAnim's initial Delay3,
/// GBFadeInFromWhite, and bird-graphics copy. Combined with the generic
/// 24-frame fade-in, this places the first arrival-bird frame 69 frames after
/// the commit, matching the recorded original.
pub const FLY_ARRIVAL_POST_FADE_DELAY_FRAMES: u8 = 45;

pub const FLY_DEPARTURE_COORDS_1: [(u16, u16); 12] = [
    (0x3C, 0x48),
    (0x3C, 0x50),
    (0x3B, 0x58),
    (0x3A, 0x60),
    (0x39, 0x68),
    (0x37, 0x70),
    (0x37, 0x78),
    (0x33, 0x80),
    (0x30, 0x88),
    (0x2D, 0x90),
    (0x2A, 0x98),
    (0x27, 0xA0),
];

pub const FLY_DEPARTURE_COORDS_2: [(u16, u16); 11] = [
    (0x1A, 0x90),
    (0x19, 0x80),
    (0x17, 0x70),
    (0x15, 0x60),
    (0x12, 0x50),
    (0x0F, 0x40),
    (0x0C, 0x30),
    (0x09, 0x20),
    (0x05, 0x10),
    (0x00, 0x00),
    (0xF0, 0x00),
];

impl LeaveMapFlyState {
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    pub fn tick(&mut self) -> bool {
        self.frame = self.frame.saturating_add(1);
        self.frame >= FLY_DEPARTURE_FRAMES
    }

    pub fn force_white(&self) -> bool {
        (FLY_DEPARTURE_WHITE_START..FLY_DEPARTURE_WHITE_END).contains(&self.frame)
    }

    pub fn player_visible(&self) -> bool {
        self.frame < FLY_DEPARTURE_BIRD_START
    }

    pub fn bird_pose(&self) -> Option<(u16, u16, u8)> {
        let (y, x) = if (FLY_DEPARTURE_BIRD_START..FLY_DEPARTURE_FIRST_PATH_START)
            .contains(&self.frame)
        {
            (0x3C, 0x40)
        } else if (FLY_DEPARTURE_FIRST_PATH_START..FLY_DEPARTURE_HOLD_START)
            .contains(&self.frame)
        {
            let index = ((self.frame - FLY_DEPARTURE_FIRST_PATH_START) / 3) as usize;
            FLY_DEPARTURE_COORDS_1[index.min(FLY_DEPARTURE_COORDS_1.len() - 1)]
        } else if (FLY_DEPARTURE_SECOND_PATH_START..FLY_DEPARTURE_FRAMES)
            .contains(&self.frame)
        {
            let index = ((self.frame - FLY_DEPARTURE_SECOND_PATH_START) / 3) as usize;
            FLY_DEPARTURE_COORDS_2[index.min(FLY_DEPARTURE_COORDS_2.len() - 1)]
        } else {
            return None;
        };
        let flap = (((self.frame - FLY_DEPARTURE_BIRD_START) / 3) + 1) as u8 & 1;
        Some((y, x, flap))
    }
}

impl Default for LeaveMapFlyState {
    fn default() -> Self {
        Self::new()
    }
}

/// FLY arrival animation — `EnterMapAnim`'s `.flyAnimation`
/// (engine/overworld/player_animations.asm:53-70): the player sprite is
/// replaced by the BIRD sprite, which flies in from the top-right along
/// `FlyAnimationEnterScreenCoords`, flapping every step (`DoFlyAnimation`:
/// 12 iterations × Delay3 = 36 frames), followed by the observed 11-frame
/// player-graphics restore while the bird remains at the landing point.
/// Game-specific (the
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
pub const FLY_ANIM_MOTION_FRAMES: u16 = FLY_ANIM_STEPS * FLY_ANIM_STEP_FRAMES;
pub const FLY_ANIM_RESTORE_FRAMES: u16 = 11;
pub const FLY_ANIM_FRAMES: u16 = FLY_ANIM_MOTION_FRAMES + FLY_ANIM_RESTORE_FRAMES;

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
        let motion_frame = self.frame.min(FLY_ANIM_MOTION_FRAMES - 1);
        ((motion_frame / FLY_ANIM_STEP_FRAMES) & 1) as u8
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
    fn fly_departure_matches_recorded_phase_boundaries() {
        let mut fly = LeaveMapFlyState::new();
        fly.frame = 7;
        assert!(!fly.force_white());
        fly.frame = 8;
        assert!(fly.force_white());
        fly.frame = 56;
        assert!(fly.force_white());
        fly.frame = 57;
        assert!(!fly.force_white());
        assert!(fly.player_visible());
        fly.frame = FLY_DEPARTURE_BIRD_START;
        assert_eq!(fly.bird_pose(), Some((0x3C, 0x40, 1)));
        assert!(!fly.player_visible());
        fly.frame = FLY_DEPARTURE_FIRST_PATH_START;
        assert_eq!(fly.bird_pose().map(|pose| (pose.0, pose.1)), Some((0x3C, 0x48)));
        fly.frame = FLY_DEPARTURE_HOLD_START;
        assert!(fly.bird_pose().is_none());
        fly.frame = FLY_DEPARTURE_SECOND_PATH_START;
        assert_eq!(fly.bird_pose().map(|pose| (pose.0, pose.1)), Some((0x1A, 0x90)));
        fly.frame = FLY_DEPARTURE_FRAMES - 1;
        assert_eq!(fly.bird_pose().map(|pose| (pose.0, pose.1)), Some((0xF0, 0x00)));
        assert!(fly.tick());
    }

    #[test]
    fn fly_anim_matches_asm_frame_count_and_coords() {
        // DoFlyAnimation is 36 frames, followed by the measured graphics-
        // restore hold through the first stable player frame.
        assert_eq!(FLY_ANIM_MOTION_FRAMES, 36);
        assert_eq!(FLY_ANIM_FRAMES, 47);
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
