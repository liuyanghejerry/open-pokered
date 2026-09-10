//! Overworld presentation-state machines — pure, frame-counted animation
//! logic for the visual effects classic JRPGs implement as blocking
//! animation routines. The renderer reads these each frame; the game's
//! update loop ticks them once per frame (freezing gameplay, mirroring the
//! originals' busy-wait structure).
//!
//! Everything here is game-agnostic: states depend only on
//! [`Direction`], injected tuning data (spin facing order, elevator shake
//! parameters), and frame counters. Sound cues are returned as typed enums
//! ([`TeleportSpinSfx`], [`EnterMapSpinSfx`], [`ElevatorShakeSfx`],
//! [`ShipDepartureSfx`]); the game maps them to its own audio ids.
//!
//! Covered effects:
//! - teleport / escape-item spin-out ([`TeleportSpinState`]) and the
//!   matching arrival spin-in ([`EnterMapSpinState`])
//! - elevator rumble ([`ElevatorShakeState`])
//! - looping water/flower background-tile animation ([`TileAnimState`])
//! - fishing-rod animation ([`FishingAnimState`])
//! - boulder-push dust puff ([`BoulderDustState`])
//! - ship-departure cutscene ([`ShipDepartureState`])
//! - the all-white palette flash frame count ([`FLASH_WHITE_FRAMES`])

use super::types::Direction;

// ── Teleport/escape-item spin-out ─────────────────────────────────

/// Phase of the leave-map spin animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeleportSpinPhase {
    /// Spin in place: 16 spins with frame delays 16,15,…,1 (136 frames
    /// total); a loop sound cue fires whenever the current spin's delay is
    /// a multiple of 4 (spins 0, 4, 8, 12).
    SpinInPlace,
    /// Spin while moving up: 5 spin steps of 16px each (3-frame step
    /// delay), a departure cue at the start.
    SpinUp,
    /// The extra 10-frame delay used when not standing on a warp pad.
    Delay,
    /// Animation finished; the caller starts the fade-out-to-white warp.
    Done,
}

/// Total frames of the spin-in-place phase: 16+15+…+1.
pub const SPIN_IN_PLACE_FRAMES: u16 = 136;
/// Frames between spin-up steps.
pub const SPIN_UP_STEP_DELAY: u16 = 3;
/// Number of 16px spin-up steps.
pub const SPIN_UP_STEPS: u16 = 5;
/// Extra frames after the spin-up when not on a warp pad.
pub const SPIN_POST_DELAY_FRAMES: u16 = 10;
/// Pixels the player sprite rises per spin-up step.
pub const SPIN_UP_STEP_PIXELS: i32 = 16;

/// Sound cues [`TeleportSpinState::tick`] can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeleportSpinSfx {
    /// The looping spin cue, fired on spins whose delay is a multiple of 4.
    SpinLoop,
    /// The departure cue, fired once at the start of the spin-up.
    Rise,
}

/// State of the teleport / escape-item spin-out animation.
///
/// Frame-driven: constructed when the leave-warp is triggered, ticked once
/// per frame by the update loop; the warp fade-out starts when
/// [`Self::is_done`] becomes true.
///
/// `spin_order` is the facing cycle the animation rotates through, starting
/// from the player's current facing (the classic order is
/// `[Down, Left, Up, Right]`).
#[derive(Debug, Clone, Copy)]
pub struct TeleportSpinState {
    /// Facing cycle, indexed by `(start_index + step) % 4`.
    spin_order: [Direction; 4],
    /// Index in `spin_order` of the facing shown at spin step 0 — the
    /// player's facing when the animation started.
    start_index: usize,
    /// Elapsed frames within the whole animation.
    frame: u16,
    phase: TeleportSpinPhase,
}

impl TeleportSpinState {
    pub fn new(current_facing: Direction, spin_order: [Direction; 4]) -> Self {
        // The spin starts by showing the current facing, then advances
        // through the cycle.
        let start_index = spin_order
            .iter()
            .position(|&d| d == current_facing)
            .unwrap_or(0);
        Self {
            spin_order,
            start_index,
            frame: 0,
            phase: TeleportSpinPhase::SpinInPlace,
        }
    }

    /// Spin-in-place index (0..=15) whose display window contains `frame`,
    /// or None once the phase is over. Spin i is shown for `16 - i` frames.
    fn spin_in_place_index(frame: u16) -> Option<usize> {
        let mut start = 0u16;
        for i in 0..16u16 {
            let dur = 16 - i;
            if frame < start + dur {
                return Some(i as usize);
            }
            start += dur;
        }
        None
    }

    /// Advance one frame. Returns the sound cue to play this frame, if any
    /// ([`TeleportSpinSfx::SpinLoop`] on spins whose delay is a multiple of
    /// 4, [`TeleportSpinSfx::Rise`] at the start of the spin-up).
    pub fn tick(&mut self) -> Option<TeleportSpinSfx> {
        if self.phase == TeleportSpinPhase::Done {
            return None;
        }
        let mut sfx = None;
        match self.phase {
            TeleportSpinPhase::SpinInPlace => {
                // First frame of a spin whose delay (16 - i) is a multiple of 4.
                if self.frame == 0
                    || Self::spin_in_place_index(self.frame)
                        != Self::spin_in_place_index(self.frame.wrapping_sub(1))
                {
                    if let Some(i) = Self::spin_in_place_index(self.frame) {
                        if (16 - i) % 4 == 0 {
                            sfx = Some(TeleportSpinSfx::SpinLoop);
                        }
                    }
                }
                if self.frame + 1 >= SPIN_IN_PLACE_FRAMES {
                    self.phase = TeleportSpinPhase::SpinUp;
                }
            }
            TeleportSpinPhase::SpinUp => {
                if self.frame == SPIN_IN_PLACE_FRAMES {
                    sfx = Some(TeleportSpinSfx::Rise);
                }
                // 5 steps: 4 with a 3-frame delay, the last ends immediately.
                let spin_up_frames = (SPIN_UP_STEPS - 1) * (1 + SPIN_UP_STEP_DELAY) + 1;
                if self.frame + 1 >= SPIN_IN_PLACE_FRAMES + spin_up_frames {
                    self.phase = TeleportSpinPhase::Delay;
                }
            }
            TeleportSpinPhase::Delay => {
                let spin_up_frames = (SPIN_UP_STEPS - 1) * (1 + SPIN_UP_STEP_DELAY) + 1;
                if self.frame + 1 >= SPIN_IN_PLACE_FRAMES + spin_up_frames + SPIN_POST_DELAY_FRAMES
                {
                    self.phase = TeleportSpinPhase::Done;
                }
            }
            TeleportSpinPhase::Done => {}
        }
        self.frame += 1;
        sfx
    }

    pub fn is_done(&self) -> bool {
        self.phase == TeleportSpinPhase::Done
    }

    pub fn phase(&self) -> TeleportSpinPhase {
        self.phase
    }

    /// Current player facing (the spin rotating through the facing cycle).
    pub fn facing(&self) -> Direction {
        let step = match self.phase {
            TeleportSpinPhase::SpinInPlace => {
                Self::spin_in_place_index(self.frame).unwrap_or(15)
            }
            TeleportSpinPhase::SpinUp => {
                let f = self.frame - SPIN_IN_PLACE_FRAMES;
                ((f / (1 + SPIN_UP_STEP_DELAY)) as usize).min((SPIN_UP_STEPS - 1) as usize) + 16
            }
            TeleportSpinPhase::Delay | TeleportSpinPhase::Done => 16 + (SPIN_UP_STEPS - 1) as usize,
        };
        self.spin_order[(self.start_index + step) % 4]
    }

    /// Vertical pixel offset of the player sprite (≤ 0; rises off screen
    /// during the spin-up phase).
    pub fn player_y_offset(&self) -> i32 {
        match self.phase {
            TeleportSpinPhase::SpinInPlace => 0,
            TeleportSpinPhase::SpinUp => {
                let f = self.frame - SPIN_IN_PLACE_FRAMES;
                let step = ((f / (1 + SPIN_UP_STEP_DELAY)) as i32 + 1).min(SPIN_UP_STEPS as i32);
                -step * SPIN_UP_STEP_PIXELS
            }
            TeleportSpinPhase::Delay | TeleportSpinPhase::Done => {
                -(SPIN_UP_STEPS as i32) * SPIN_UP_STEP_PIXELS
            }
        }
    }

    /// Whether the player sprite is still on screen (by the last step the
    /// sprite has risen fully above the visible area).
    pub fn player_visible(&self) -> bool {
        self.player_y_offset() > -(SPIN_UP_STEPS as i32) * SPIN_UP_STEP_PIXELS
    }
}

// ── Arrival spin-in ───────────────────────────────────────────────

/// Phase of the arrival spin, the counterpart of [`TeleportSpinState`]:
/// after a teleport-class warp arrival, the player descends from off the
/// top of the screen, then spins in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterMapSpinPhase {
    /// Spin while moving down: 5 spin steps of 16px each (3-frame step
    /// delay), a descent cue at the start, an arrival cue after the last
    /// step.
    SpinDown,
    /// Spin in place with delays 0,1,…,7 (8 spins, silent) — skipped when
    /// the player arrives ON a warp pad or hole.
    SpinInPlace,
    /// Finished; the caller restores the saved facing and Y position.
    Done,
}

/// Total frames of the spin-down phase: 5 spins with 3-frame delays between
/// (the last step ends immediately).
pub const ENTER_MAP_SPIN_DOWN_FRAMES: u16 =
    (ENTER_MAP_SPIN_DOWN_STEPS - 1) * (1 + ENTER_MAP_SPIN_DOWN_STEP_DELAY) + 1;
/// Number of 16px spin-down steps (from offscreen above to the standing
/// position).
pub const ENTER_MAP_SPIN_DOWN_STEPS: u16 = 5;
/// Frames between spin-down steps.
pub const ENTER_MAP_SPIN_DOWN_STEP_DELAY: u16 = 3;
/// Pixels the player sprite descends per spin-down step.
pub const ENTER_MAP_SPIN_DOWN_STEP_PIXELS: i32 = 16;
/// Total frames of the arrival spin-in-place phase: 8 spins whose delays are
/// 0,1,…,7 (8 + (1+2+…+7)).
pub const ENTER_MAP_SPIN_IN_PLACE_FRAMES: u16 = 36;

/// Sound cues [`EnterMapSpinState::tick`] can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterMapSpinSfx {
    /// Fired once at the start of the spin-down.
    Descend,
    /// Fired when the spin-down completes.
    Land,
}

/// State of the arrival spin animation.
///
/// Frame-driven: constructed when a teleport-class warp is committed;
/// ticked once per frame once the fade-in from white has completed (the
/// player stays hidden during the fade). `spin_in_place` mirrors the
/// standing-on-warp-pad check: arrivals on a warp pad/hole skip the final
/// spin-in-place.
#[derive(Debug, Clone, Copy)]
pub struct EnterMapSpinState {
    /// Facing cycle, indexed by `(start_index + step) % 4`.
    spin_order: [Direction; 4],
    /// Index in `spin_order` of the facing shown at spin step 0 — the
    /// player's facing at the destination.
    start_index: usize,
    /// Elapsed frames within the whole animation.
    frame: u16,
    phase: EnterMapSpinPhase,
    spin_in_place: bool,
}

impl EnterMapSpinState {
    pub fn new(current_facing: Direction, spin_order: [Direction; 4], spin_in_place: bool) -> Self {
        let start_index = spin_order
            .iter()
            .position(|&d| d == current_facing)
            .unwrap_or(0);
        Self {
            spin_order,
            start_index,
            frame: 0,
            phase: EnterMapSpinPhase::SpinDown,
            spin_in_place,
        }
    }

    /// Advance one frame. Returns the sound cue to play this frame, if any
    /// ([`EnterMapSpinSfx::Descend`] at the start of the spin-down,
    /// [`EnterMapSpinSfx::Land`] when it completes).
    pub fn tick(&mut self) -> Option<EnterMapSpinSfx> {
        if self.phase == EnterMapSpinPhase::Done {
            return None;
        }
        let mut sfx = None;
        match self.phase {
            EnterMapSpinPhase::SpinDown => {
                if self.frame == 0 {
                    sfx = Some(EnterMapSpinSfx::Descend);
                }
                if self.frame + 1 >= ENTER_MAP_SPIN_DOWN_FRAMES {
                    // Spin-down finished → land cue, then the spin-in-place
                    // unless the player arrived on a warp pad or hole.
                    sfx = Some(EnterMapSpinSfx::Land);
                    self.phase = if self.spin_in_place {
                        EnterMapSpinPhase::SpinInPlace
                    } else {
                        EnterMapSpinPhase::Done
                    };
                }
            }
            EnterMapSpinPhase::SpinInPlace => {
                if self.frame + 1
                    >= ENTER_MAP_SPIN_DOWN_FRAMES + ENTER_MAP_SPIN_IN_PLACE_FRAMES
                {
                    self.phase = EnterMapSpinPhase::Done;
                }
            }
            EnterMapSpinPhase::Done => {}
        }
        self.frame += 1;
        sfx
    }

    pub fn is_done(&self) -> bool {
        self.phase == EnterMapSpinPhase::Done
    }

    pub fn phase(&self) -> EnterMapSpinPhase {
        self.phase
    }

    /// Current player facing (the spin rotating through the facing cycle).
    pub fn facing(&self) -> Direction {
        let step = match self.phase {
            EnterMapSpinPhase::SpinDown => {
                (self.frame / (1 + ENTER_MAP_SPIN_DOWN_STEP_DELAY)) as usize
            }
            EnterMapSpinPhase::SpinInPlace => {
                let f = self.frame - ENTER_MAP_SPIN_DOWN_FRAMES;
                // 8 spins; spin i is shown for i+1 frames (delays 0..7).
                let spin = (f as usize).min(ENTER_MAP_SPIN_IN_PLACE_FRAMES as usize - 1);
                let mut start = 0usize;
                let mut idx = 0usize;
                for i in 0..8usize {
                    let dur = i + 1;
                    if spin < start + dur {
                        idx = i;
                        break;
                    }
                    start += dur;
                }
                ENTER_MAP_SPIN_DOWN_STEPS as usize + idx
            }
            // The animation ends by restoring the saved (destination) facing.
            EnterMapSpinPhase::Done => 0,
        };
        self.spin_order[(self.start_index + step) % 4]
    }

    /// Vertical pixel offset of the player sprite (≤ 0; the player descends
    /// from off the top of the screen into the standing position).
    pub fn player_y_offset(&self) -> i32 {
        match self.phase {
            EnterMapSpinPhase::SpinDown => {
                // Moves land on ticks 1, 5, 9, 13, 17 (a spin + 3-frame delay
                // each): Y rises 16px per move, -80 → 0. Frame 0 is the
                // pre-fade position (fully off the top).
                if self.frame == 0 {
                    -(ENTER_MAP_SPIN_DOWN_STEPS as i32) * ENTER_MAP_SPIN_DOWN_STEP_PIXELS
                } else {
                    let moves = ((self.frame - 1) / (1 + ENTER_MAP_SPIN_DOWN_STEP_DELAY) + 1)
                        .min(ENTER_MAP_SPIN_DOWN_STEPS);
                    -((ENTER_MAP_SPIN_DOWN_STEPS - moves) as i32)
                        * ENTER_MAP_SPIN_DOWN_STEP_PIXELS
                }
            }
            EnterMapSpinPhase::SpinInPlace | EnterMapSpinPhase::Done => 0,
        }
    }

    /// Whether the player sprite is still off the top of the screen.
    pub fn player_visible(&self) -> bool {
        self.player_y_offset() > -(ENTER_MAP_SPIN_DOWN_STEPS as i32)
            * ENTER_MAP_SPIN_DOWN_STEP_PIXELS
    }
}

// ── Elevator shake ────────────────────────────────────────────────

/// Tuning for the elevator rumble: how many up/down iterations and how far
/// the background scrolls each iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElevatorShakeParams {
    /// Number of shake iterations (each lasts 2 frames).
    pub iterations: u8,
    /// Background scroll magnitude per iteration, in pixels.
    pub pixel_offset: u8,
}

impl ElevatorShakeParams {
    /// Total frames of the shake (2 frames per iteration).
    pub const fn total_frames(&self) -> u16 {
        self.iterations as u16 * 2
    }
}

/// Sound cues [`ElevatorShakeState::tick`] can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorShakeSfx {
    /// The rattle, fired at the start of each 2-frame iteration.
    Rattle,
    /// The arrival "ding", fired on the final frame.
    Arrive,
}

/// Frame-driven elevator rumble: scrolls the background up/down by
/// ±`pixel_offset`, `iterations` × 2 frames, a rattle cue each iteration,
/// then a single arrival ding.
#[derive(Debug, Clone, Copy)]
pub struct ElevatorShakeState {
    /// Elapsed frames of the shake (0..params.total_frames()).
    frame: u16,
    params: ElevatorShakeParams,
}

impl ElevatorShakeState {
    pub fn new(params: ElevatorShakeParams) -> Self {
        Self { frame: 0, params }
    }

    pub fn params(&self) -> ElevatorShakeParams {
        self.params
    }

    /// Total frames of the shake.
    pub fn total_frames(&self) -> u16 {
        self.params.total_frames()
    }

    /// Advance one frame. Returns [`ElevatorShakeSfx::Rattle`] at the start
    /// of each 2-frame iteration and [`ElevatorShakeSfx::Arrive`] on the
    /// final frame.
    pub fn tick(&mut self) -> Option<ElevatorShakeSfx> {
        if self.is_done() {
            return None;
        }
        let sfx = if self.frame + 1 >= self.total_frames() {
            Some(ElevatorShakeSfx::Arrive)
        } else if self.frame % 2 == 0 {
            Some(ElevatorShakeSfx::Rattle)
        } else {
            None
        };
        self.frame += 1;
        sfx
    }

    pub fn is_done(&self) -> bool {
        self.frame >= self.total_frames()
    }

    /// Current background scroll offset (±pixel_offset). The first
    /// iteration scrolls negative.
    pub fn offset_y(&self) -> i32 {
        if self.is_done() {
            return 0;
        }
        let iteration = self.frame / 2;
        let px = self.params.pixel_offset as i32;
        if iteration % 2 == 0 {
            -px
        } else {
            px
        }
    }
}

// ── Water/flower tile animation ───────────────────────────────────

/// Which background tiles a tileset animates (the classic
/// water-rotation / flower-frame system).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileAnimKind {
    /// No animated tiles.
    None,
    /// Water tiles only.
    Water,
    /// Water + flower tiles.
    WaterFlower,
}

/// Per-frame water/flower background-tile animation.
///
/// - `counter1` increments every frame; at 20 the water tile rotates one
///   pixel; at 21 ([`TileAnimKind::WaterFlower`] only) the flower tile
///   advances and `counter1` resets. For water-only tilesets `counter1`
///   resets right after the water update.
/// - `counter2` increments on each water update (& 7); its bit 2 selects
///   the water rotation direction (right for 4 updates, then left for 4),
///   and `counter2 & 3` selects the flower frame (0/1 → 1, 2 → 2, 3 → 3).
#[derive(Debug, Clone, Copy)]
pub struct TileAnimState {
    counter1: u8,
    counter2: u8,
    /// Net horizontal water-tile rotation in pixels (0..=4, back and forth).
    water_shift: i8,
    /// Flower frame (1..=3) selected by the last flower update, if any.
    flower_frame: Option<u8>,
    /// Animation kind for the current tileset (None = animations disabled).
    kind: TileAnimKind,
}

impl TileAnimState {
    pub fn new() -> Self {
        Self {
            counter1: 0,
            counter2: 0,
            water_shift: 0,
            flower_frame: None,
            kind: TileAnimKind::None,
        }
    }

    /// Adopt a tileset's animation kind and reset `counter1` (`counter2`
    /// and the accumulated water shift persist, matching the classic WRAM
    /// behavior on map load).
    pub fn set_tileset(&mut self, kind: TileAnimKind) {
        self.kind = kind;
        self.counter1 = 0;
    }

    /// Advance one frame. No-op when the tileset has no animated tiles.
    pub fn tick(&mut self) {
        if self.kind == TileAnimKind::None {
            return;
        }
        self.counter1 = self.counter1.wrapping_add(1);
        if self.counter1 < 20 {
            return;
        }
        if self.counter1 == 21 {
            // flower update
            self.counter1 = 0;
            self.flower_frame = Some(match self.counter2 & 3 {
                0 | 1 => 1,
                2 => 2,
                _ => 3,
            });
            return;
        }
        // counter1 == 20: water update.
        self.counter2 = (self.counter2 + 1) & 7;
        // Shift the tile rows one pixel right (counter2 bit 2 clear) or
        // left (set).
        self.water_shift += if self.counter2 & 4 == 0 { 1 } else { -1 };
        // Water-only tilesets reset the counter immediately; WaterFlower
        // falls through to the flower frame on the next tick.
        if self.kind == TileAnimKind::Water {
            self.counter1 = 0;
        }
    }

    /// Current horizontal rotation of the water tile in pixels (positive =
    /// right). Sample source column `(x - shift) mod 8`.
    pub fn water_shift(&self) -> i8 {
        self.water_shift
    }

    /// Flower frame (1..=3) to display, or None before the first flower
    /// update (the tileset's base flower tile shows).
    pub fn flower_frame(&self) -> Option<u8> {
        self.flower_frame
    }

    /// Animation kind for the current tileset.
    pub fn kind(&self) -> TileAnimKind {
        self.kind
    }
}

impl Default for TileAnimState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Fishing rod animation ─────────────────────────────────────────

/// Initial pause before the rod appears.
pub const FISHING_CAST_DELAY_FRAMES: u16 = 10;
/// Frames the rod stays out waiting for a bite.
pub const FISHING_ROD_OUT_FRAMES: u16 = 100;
/// Shake iterations on a bite.
pub const FISHING_SHAKE_ITERATIONS: u16 = 10;
/// Frames per shake iteration.
pub const FISHING_SHAKE_STEP_FRAMES: u16 = 3;
/// Frames the "!" emotion bubble stays up.
pub const FISHING_BUBBLE_FRAMES: u16 = 60;

/// Total frames of the whole animation: 10 + 100 + 30 + 60.
pub const FISHING_ANIM_FRAMES: u16 = FISHING_CAST_DELAY_FRAMES
    + FISHING_ROD_OUT_FRAMES
    + FISHING_SHAKE_ITERATIONS * FISHING_SHAKE_STEP_FRAMES
    + FISHING_BUBBLE_FRAMES;

/// Phase of the player-side fishing rod animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishingAnimPhase {
    /// Initial pause — nothing drawn yet (the rod OAM and the fishing pose
    /// tiles are set up only after this delay).
    CastDelay,
    /// The rod is out and the player holds the fishing pose; the game's
    /// bite roll decides the outcome at the end of this phase.
    RodOut,
    /// Bite only: shake iterations toggling the player sprite's and rod's
    /// Y by ±1 px.
    Shake,
    /// Bite only: the "!" emotion bubble over the player. The rod is hidden
    /// during this phase when the player faces up (so it does not overlap
    /// the bubble), then unhidden.
    Bubble,
    /// Finished; the caller shows the result text.
    Done,
}

/// Frame-driven state of the rod animation. Constructed when a rod use
/// passes the game's eligibility gates; ticked once per frame by the update
/// loop (which freezes gameplay while it runs); when [`Self::is_done`] the
/// result text is queued.
#[derive(Debug, Clone, Copy)]
pub struct FishingAnimState {
    /// Elapsed ticks (0 before the first `tick`).
    frame: u16,
    /// Player facing when the anim started (selects the rod OAM entry).
    facing: Direction,
    /// A bite plays the shake + bubble.
    bite: bool,
    phase: FishingAnimPhase,
}

impl FishingAnimState {
    pub fn new(facing: Direction, bite: bool) -> Self {
        Self {
            frame: 0,
            facing,
            bite,
            phase: FishingAnimPhase::CastDelay,
        }
    }

    fn phase_for(frame: u16, bite: bool) -> FishingAnimPhase {
        let rod_end = FISHING_CAST_DELAY_FRAMES + FISHING_ROD_OUT_FRAMES;
        if frame < FISHING_CAST_DELAY_FRAMES {
            FishingAnimPhase::CastDelay
        } else if frame < rod_end {
            FishingAnimPhase::RodOut
        } else if !bite {
            FishingAnimPhase::Done
        } else if frame < rod_end + FISHING_SHAKE_ITERATIONS * FISHING_SHAKE_STEP_FRAMES {
            FishingAnimPhase::Shake
        } else if frame < rod_end
            + FISHING_SHAKE_ITERATIONS * FISHING_SHAKE_STEP_FRAMES
            + FISHING_BUBBLE_FRAMES
        {
            FishingAnimPhase::Bubble
        } else {
            FishingAnimPhase::Done
        }
    }

    /// Advance one frame.
    pub fn tick(&mut self) {
        self.frame = self.frame.saturating_add(1);
        self.phase = Self::phase_for(self.frame, self.bite);
    }

    pub fn phase(&self) -> FishingAnimPhase {
        self.phase
    }

    pub fn is_done(&self) -> bool {
        self.phase == FishingAnimPhase::Done
    }

    /// Facing captured at construction.
    pub fn facing(&self) -> Direction {
        self.facing
    }

    /// Whether the player is holding the fishing pose (pose + rod shown).
    /// False during the initial delay and after the anim ends.
    pub fn pose_active(&self) -> bool {
        matches!(
            self.phase,
            FishingAnimPhase::RodOut | FishingAnimPhase::Shake | FishingAnimPhase::Bubble
        )
    }

    /// Whether the rod OAM piece is drawn this frame. Hidden during the
    /// bubble for the up-facing player, and not yet present during the
    /// cast delay.
    pub fn rod_visible(&self) -> bool {
        self.pose_active()
            && !(self.phase == FishingAnimPhase::Bubble && self.facing == Direction::Up)
    }

    /// Whether the "!" emotion bubble is displayed above the player.
    pub fn bubble_active(&self) -> bool {
        self.phase == FishingAnimPhase::Bubble
    }

    /// Vertical offset (±1 px) of the player sprite and rod during the
    /// bite shake.
    pub fn player_shake_offset(&self) -> i32 {
        if self.phase != FishingAnimPhase::Shake {
            return 0;
        }
        let shake_start = FISHING_CAST_DELAY_FRAMES + FISHING_ROD_OUT_FRAMES;
        let iteration = (self.frame - shake_start) / FISHING_SHAKE_STEP_FRAMES;
        if iteration % 2 == 0 { 1 } else { 0 }
    }

    /// The rod's OAM piece for `facing`, expressed as an OFFSET from the
    /// player sprite's top-left. Returns `(dx, dy, tile index into the
    /// rod sprite sheet, x_flip)`; the sheet's tiles are 0 (DOWN/UP) and
    /// 1 (LEFT/RIGHT — X-flipped for RIGHT).
    pub fn rod_piece(facing: Direction) -> (i32, i32, u8, bool) {
        match facing {
            Direction::Down => (20, 35, 0, false),
            Direction::Up => (20, -12, 0, false),
            Direction::Left => (0, 16, 1, false),
            Direction::Right => (48, 16, 1, true),
        }
    }
}

// ── Boulder push dust ─────────────────────────────────────────────

/// Number of animation steps of the boulder-dust puff.
pub const BOULDER_DUST_STEPS: u8 = 8;
/// Frames each dust step lasts.
pub const BOULDER_DUST_STEP_FRAMES: u8 = 3;

/// The 2×2 smoke-puff block kicked up when a pushed boulder slides one
/// tile. The block is written once, anchored to the player's map tile at
/// push time (the animation outlives the push lockout, so the anchor must
/// not track the player afterward), then runs [`BOULDER_DUST_STEPS`] steps
/// of [`BOULDER_DUST_STEP_FRAMES`] frames each. Every step the block drifts
/// 1px against the boulder's slide direction and the sprite palette
/// toggles, flashing two gray shades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoulderDustState {
    /// The push direction — the player's facing when the boulder moved
    /// (also the dust's base offset and drift direction).
    facing: Direction,
    /// The player's map tile when the push started: the dust's world anchor.
    anchor_x: u16,
    anchor_y: u16,
    /// Current animation step (0..BOULDER_DUST_STEPS; == STEPS once done).
    step: u8,
    /// Frames elapsed within the current step.
    frame: u8,
}

impl BoulderDustState {
    /// A finished (inactive) state — no dust showing.
    pub const fn inactive() -> Self {
        Self {
            facing: Direction::Down,
            anchor_x: 0,
            anchor_y: 0,
            step: BOULDER_DUST_STEPS,
            frame: 0,
        }
    }

    /// Start the dust for a push in `facing` direction, anchored to the
    /// player's map tile at push time.
    pub const fn new(facing: Direction, anchor_x: u16, anchor_y: u16) -> Self {
        Self {
            facing,
            anchor_x,
            anchor_y,
            step: 0,
            frame: 0,
        }
    }

    /// Advance one frame. No-op once the animation has finished.
    pub fn tick(&mut self) {
        if self.step >= BOULDER_DUST_STEPS {
            return;
        }
        self.frame += 1;
        if self.frame >= BOULDER_DUST_STEP_FRAMES {
            self.frame = 0;
            self.step += 1;
        }
    }

    /// True while the puff is showing (steps 0..7).
    pub fn is_active(&self) -> bool {
        self.step < BOULDER_DUST_STEPS
    }

    /// The push direction.
    pub fn facing(&self) -> Direction {
        self.facing
    }

    /// The player's map tile at push time — the dust's world anchor.
    pub fn anchor(&self) -> (u16, u16) {
        (self.anchor_x, self.anchor_y)
    }

    /// Current animation step index (0..=7).
    pub fn step(&self) -> u8 {
        self.step.min(BOULDER_DUST_STEPS - 1)
    }

    /// Base pixel offset of the dust block's top-left corner from the
    /// player sprite's top-left (the puff appears at the boulder's base,
    /// "2 blocks away from the player").
    pub fn base_offset(&self) -> (i32, i32) {
        match self.facing {
            Direction::Down => (8, 52),
            Direction::Up => (8, -12),
            Direction::Left => (-24, 20),
            Direction::Right => (40, 20),
        }
    }

    /// Per-step pixel drift of the dust block — opposite to the boulder's
    /// slide direction. The puff lingers as the boulder slides away from it.
    pub fn drift_px(&self) -> (i32, i32) {
        match self.facing {
            Direction::Down => (0, -1),
            Direction::Up => (0, 1),
            Direction::Left => (1, 0),
            Direction::Right => (-1, 0),
        }
    }

    /// Per-step pixel delta of each of the block's four 8×8 tiles
    /// (upper-left, upper-right, lower-left, lower-right). Vertical pushes
    /// move the whole block; horizontal pushes move only the upper-right,
    /// lower-left and lower-right tiles — the upper-left tile stays in
    /// place.
    pub fn tile_drifts(&self) -> [(i32, i32); 4] {
        let (dx, dy) = self.drift_px();
        match self.facing {
            Direction::Left | Direction::Right => [(0, 0), (dx, 0), (dx, 0), (dx, 0)],
            _ => [(dx, dy); 4],
        }
    }

    /// True on odd steps: the palette toggles once per step, flashing the
    /// two gray shades of the smoke sprite.
    pub fn palette_flipped(&self) -> bool {
        self.step % 2 == 1
    }
}

// ── FLASH white-out ───────────────────────────────────────────────

/// Frames of the all-palettes-white flash when a dark area is lit up.
pub const FLASH_WHITE_FRAMES: u8 = 3;

// ── Ship departure cutscene ───────────────────────────────────────

/// Initial pause of the departure cutscene (after the departure music
/// starts).
pub const SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES: u16 = 120;
/// The water-fill commit (pushing the screen tile buffer's water fill to
/// the background). Kept for frame fidelity; the erase phase below is the
/// visible ship removal.
pub const SHIP_DEPARTURE_WATER_FILL_FRAMES: u16 = 3;
/// View-scroll iterations.
pub const SHIP_DEPARTURE_SCROLL_ITERATIONS: u16 = 8;
/// Frames per iteration: 16 smoke-drift substeps × an 8-frame delay each.
pub const SHIP_DEPARTURE_ITERATION_FRAMES: u16 = 16 * 8;
/// Final pause after the ship is erased.
pub const SHIP_DEPARTURE_ERASE_FRAMES: u16 = 120;
/// Pixels the view scrolls east per iteration (the ship slides left).
pub const SHIP_DEPARTURE_SCROLL_PX_PER_ITERATION: i32 = 2 * 8;
/// Smoke-puff spawn spacing: a new 2×2 puff block is emitted above the
/// smokestack every iteration, 16px left of the previous one.
pub const SHIP_DEPARTURE_PUFF_SPACING_PX: i32 = 16;
/// Smoke-puff drift: every 8-frame substep all live puffs drift +2px right.
pub const SHIP_DEPARTURE_PUFF_DRIFT_PX_PER_SUBSTEP: i32 = 2;
/// Substeps per iteration.
pub const SHIP_DEPARTURE_SUBSTEPS_PER_ITERATION: u16 = 16;
/// Frames per drift substep.
pub const SHIP_DEPARTURE_SUBSTEP_FRAMES: u16 = 8;
/// The smokestack's map position (tile units) — the puffs' world anchor.
pub const SHIP_DEPARTURE_SMOKESTACK_TILE_X: u16 = 16;
pub const SHIP_DEPARTURE_SMOKESTACK_TILE_Y: f32 = 10.5;
/// The first puff's screen X at departure start.
pub const SHIP_DEPARTURE_PUFF_START_SCREEN_X: i32 = 88;

/// Phase of the ship-departure cutscene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipDeparturePhase {
    /// Initial pause — the ship sits at the dock while the music plays.
    InitialPause,
    /// Water-fill commit. The visible erase happens in [`Self::Erase`].
    WaterFill,
    /// The scroll loop: view-scroll iterations with a smoke puff emitted
    /// per iteration and all live puffs drifting right.
    Scroll,
    /// The ship's map blocks become water, the dock→ship warp is removed,
    /// the horn plays again, and a final pause closes the cutscene.
    Erase,
    /// Finished; the caller proceeds to whatever follows the cutscene.
    Done,
}

/// Total frames of the whole cutscene: 120 + 3 + 8×128 + 120.
pub const SHIP_DEPARTURE_TOTAL_FRAMES: u16 = SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES
    + SHIP_DEPARTURE_WATER_FILL_FRAMES
    + SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_ITERATION_FRAMES
    + SHIP_DEPARTURE_ERASE_FRAMES;

/// Sound cues [`ShipDepartureState::tick`] can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipDepartureSfx {
    /// The ship's horn — played when the scroll begins (blocking) and
    /// again when the erase phase begins (non-blocking).
    Horn,
}

/// Frame-driven state of the ship-departure cutscene. Constructed when the
/// game's departure script fires; ticked once per frame by the update loop
/// (freezing gameplay, matching the classic blocking structure). The
/// renderer reads the scroll offset and puff positions; the game applies
/// the ship erase + warp removal at the erase transition.
#[derive(Debug, Clone, Copy)]
pub struct ShipDepartureState {
    /// Elapsed ticks (0 before the first `tick`).
    frame: u16,
    phase: ShipDeparturePhase,
}

impl ShipDepartureState {
    pub fn new() -> Self {
        Self {
            frame: 0,
            phase: ShipDeparturePhase::InitialPause,
        }
    }

    fn phase_for(frame: u16) -> ShipDeparturePhase {
        let scroll_end = SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES
            + SHIP_DEPARTURE_WATER_FILL_FRAMES
            + SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_ITERATION_FRAMES;
        if frame < SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES {
            ShipDeparturePhase::InitialPause
        } else if frame < SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES {
            ShipDeparturePhase::WaterFill
        } else if frame < scroll_end {
            ShipDeparturePhase::Scroll
        } else if frame < SHIP_DEPARTURE_TOTAL_FRAMES {
            ShipDeparturePhase::Erase
        } else {
            ShipDeparturePhase::Done
        }
    }

    /// Advance one frame. Returns the sound cue to play this frame, if any:
    /// [`ShipDepartureSfx::Horn`] on the first scroll frame and on the
    /// first erase frame.
    pub fn tick(&mut self) -> Option<ShipDepartureSfx> {
        if self.phase == ShipDeparturePhase::Done {
            return None;
        }
        let mut sfx = None;
        let next = self.frame + 1;
        let next_phase = Self::phase_for(next);
        if next_phase != self.phase {
            match next_phase {
                ShipDeparturePhase::Scroll => {
                    sfx = Some(ShipDepartureSfx::Horn);
                }
                ShipDeparturePhase::Erase => {
                    sfx = Some(ShipDepartureSfx::Horn);
                }
                _ => {}
            }
        }
        self.frame = next;
        self.phase = next_phase;
        sfx
    }

    pub fn is_done(&self) -> bool {
        self.phase == ShipDeparturePhase::Done
    }

    pub fn phase(&self) -> ShipDeparturePhase {
        self.phase
    }

    /// Elapsed frames within the whole animation.
    pub fn frame(&self) -> u16 {
        self.frame
    }

    /// True once the ship should be shown as water: the erase phase has
    /// begun (the game's map-block mutation lands at the same time; this
    /// flag covers the same frame for renderers that draw before the
    /// mutation takes effect).
    pub fn ship_erased(&self) -> bool {
        matches!(
            self.phase,
            ShipDeparturePhase::Erase | ShipDeparturePhase::Done
        )
    }

    /// Current iteration of the view scroll (0..=7), or 7 once the scroll
    /// finished (the erase phase keeps the final scrolled position).
    pub fn scroll_iteration(&self) -> u16 {
        match self.phase {
            ShipDeparturePhase::Scroll => {
                (self.frame
                    - SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES
                    - SHIP_DEPARTURE_WATER_FILL_FRAMES)
                    / SHIP_DEPARTURE_ITERATION_FRAMES
            }
            ShipDeparturePhase::Erase | ShipDeparturePhase::Done => {
                SHIP_DEPARTURE_SCROLL_ITERATIONS - 1
            }
            _ => 0,
        }
    }

    /// Current drift substep within the scroll (0..=127 across all 8
    /// iterations), or 127 once the scroll finished. Each substep lasts
    /// 8 frames.
    pub fn scroll_substep(&self) -> u16 {
        match self.phase {
            ShipDeparturePhase::Scroll => {
                (self.frame
                    - SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES
                    - SHIP_DEPARTURE_WATER_FILL_FRAMES)
                    / SHIP_DEPARTURE_SUBSTEP_FRAMES
            }
            ShipDeparturePhase::Erase | ShipDeparturePhase::Done => {
                SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_SUBSTEPS_PER_ITERATION - 1
            }
            _ => 0,
        }
    }

    /// Horizontal scroll of the map view in pixels (0..=128). The view
    /// advances [`SHIP_DEPARTURE_SCROLL_PX_PER_ITERATION`] per iteration
    /// and ramps one more pixel per 8-frame substep — net movement
    /// 16i + (substep+1).
    pub fn scroll_px(&self) -> i32 {
        match self.phase {
            ShipDeparturePhase::Scroll => {
                self.scroll_iteration() as i32 * SHIP_DEPARTURE_SCROLL_PX_PER_ITERATION
                    + (self.scroll_substep() as i32
                        % SHIP_DEPARTURE_SUBSTEPS_PER_ITERATION as i32)
                    + 1
            }
            ShipDeparturePhase::Erase | ShipDeparturePhase::Done => {
                SHIP_DEPARTURE_SCROLL_ITERATIONS as i32
                    * SHIP_DEPARTURE_SCROLL_PX_PER_ITERATION
            }
            _ => 0,
        }
    }

    /// Number of smoke puffs emitted so far (1 per iteration, 8 total).
    /// Puff i spawns at the start of iteration i, at the smokestack's
    /// current screen position.
    pub fn puff_count(&self) -> usize {
        match self.phase {
            ShipDeparturePhase::InitialPause | ShipDeparturePhase::WaterFill => 0,
            ShipDeparturePhase::Scroll | ShipDeparturePhase::Erase | ShipDeparturePhase::Done => {
                (self.scroll_iteration() + 1) as usize
            }
        }
    }

    /// Screen-x offset (px) of puff `i` from the smokestack's position at
    /// departure start: spawns at X = 88 − 16i and drifts +2px per substep
    /// from its spawn substep (the spawn iteration's own drift loop moves
    /// it immediately). Renderers rebase onto their own view by adding
    /// `(smokestack_screen_x - SHIP_DEPARTURE_PUFF_START_SCREEN_X)`.
    pub fn puff_x_offset(&self, i: usize) -> i32 {
        let i = i as i32;
        let spawn = SHIP_DEPARTURE_PUFF_START_SCREEN_X - SHIP_DEPARTURE_PUFF_SPACING_PX * i;
        let s = self.scroll_substep() as i32;
        let substeps_live = (s - SHIP_DEPARTURE_SUBSTEPS_PER_ITERATION as i32 * i).max(0);
        spawn + SHIP_DEPARTURE_PUFF_DRIFT_PX_PER_SUBSTEP * (substeps_live + 1)
    }

    /// Screen y (px) of every puff at departure start — the smokestack row
    /// (10.5 tiles).
    pub fn puff_screen_y(&self) -> i32 {
        (SHIP_DEPARTURE_SMOKESTACK_TILE_Y * 8.0) as i32
    }
}

impl Default for ShipDepartureState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// The classic facing cycle: DOWN, LEFT, UP, RIGHT.
    const SPIN_ORDER: [Direction; 4] = [
        Direction::Down,
        Direction::Left,
        Direction::Up,
        Direction::Right,
    ];

    // ── TeleportSpinState ─────────────────────────────────────────

    #[test]
    fn spin_starts_on_current_facing_then_cycles() {
        // The spin starts by showing the current facing, then advances
        // through the injected cycle.
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
            let mut spin = TeleportSpinState::new(start, SPIN_ORDER);
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
    fn spin_honors_a_custom_facing_cycle() {
        // A game with a different spin cycle gets its own order back.
        const REVERSED: [Direction; 4] = [
            Direction::Down,
            Direction::Right,
            Direction::Up,
            Direction::Left,
        ];
        let mut spin = TeleportSpinState::new(Direction::Down, REVERSED);
        assert_eq!(spin.facing(), Direction::Down);
        for _ in 0..16 {
            spin.tick();
        }
        assert_eq!(spin.facing(), Direction::Right, "second spin of the custom cycle");
        // A start facing absent from the cycle falls back to the cycle start.
        let spin = TeleportSpinState::new(Direction::Up, [Direction::Down; 4]);
        assert_eq!(spin.facing(), Direction::Down);
    }

    #[test]
    fn spin_sfx_schedule() {
        // SpinLoop when the current delay is a multiple of 4 (spins 0, 4, 8,
        // 12 → 4 plays); Rise once at the start of the spin-up.
        let mut spin = TeleportSpinState::new(Direction::Down, SPIN_ORDER);
        let mut loops = 0;
        let mut rises = 0;
        let mut frames = 0;
        while !spin.is_done() {
            match spin.tick() {
                Some(TeleportSpinSfx::SpinLoop) => loops += 1,
                Some(TeleportSpinSfx::Rise) => {
                    rises += 1;
                    assert_eq!(frames, SPIN_IN_PLACE_FRAMES, "rise at spin-up start");
                }
                None => {}
            }
            frames += 1;
            assert!(frames < 1000, "spin must terminate");
        }
        assert_eq!(loops, 4);
        assert_eq!(rises, 1);
        // 136 in-place + 17 spin-up (4×(1+3) + 1) + 10 delay.
        assert_eq!(frames, 136 + 17 + 10);
    }

    #[test]
    fn spin_rises_off_screen_and_hides() {
        let mut spin = TeleportSpinState::new(Direction::Down, SPIN_ORDER);
        assert_eq!(spin.player_y_offset(), 0);
        assert!(spin.player_visible());
        for _ in 0..SPIN_IN_PLACE_FRAMES - 1 {
            spin.tick();
        }
        assert_eq!(spin.player_y_offset(), 0, "still grounded on the last spin frame");
        spin.tick(); // first spin-up step applies the -16px delta immediately
        assert_eq!(spin.player_y_offset(), -16);
        // Remaining 4 steps (each 1 spin + 3 delay frames, the last has no delay).
        for _ in 0..16 {
            spin.tick();
        }
        assert_eq!(spin.player_y_offset(), -80);
        assert!(!spin.player_visible(), "sprite fully above the screen");
    }

    // ── EnterMapSpinState ─────────────────────────────────────────

    #[test]
    fn enter_map_spin_starts_hidden_and_descends_in_five_steps() {
        let mut anim = EnterMapSpinState::new(Direction::Down, SPIN_ORDER, true);
        // The state is created at warp commit; the player is off the top of
        // the screen while the fade-in plays — offset -80, not visible.
        assert_eq!(anim.phase(), EnterMapSpinPhase::SpinDown);
        assert!(!anim.player_visible(), "hidden until the spin-down descends");
        assert_eq!(anim.player_y_offset(), -80);

        // 5 moves of 16px on ticks 1, 5, 9, 13, 17 (a spin + 3-frame delay
        // each). The offset after tick 17 is the standing position.
        let mut last = -80;
        for frame in 1..=17 {
            anim.tick();
            if frame % 4 == 1 {
                assert!(anim.player_y_offset() > last, "move {frame} descends");
                last = anim.player_y_offset();
            }
        }
        assert_eq!(anim.player_y_offset(), 0, "standing position after the 5th move");
        assert!(anim.player_visible());
    }

    #[test]
    fn enter_map_spin_timing_and_sfx() {
        let mut anim = EnterMapSpinState::new(Direction::Left, SPIN_ORDER, true);
        // Descend cue on the very first frame.
        assert_eq!(anim.tick(), Some(EnterMapSpinSfx::Descend));
        // 16 more frames of the spin-down, then the Land cue.
        for _ in 1..ENTER_MAP_SPIN_DOWN_FRAMES - 1 {
            assert_eq!(anim.tick(), None);
        }
        assert_eq!(
            anim.tick(),
            Some(EnterMapSpinSfx::Land),
            "land when the spin-down completes"
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
        // Arrival on a warp pad/hole skips the final spin-in-place.
        let mut anim = EnterMapSpinState::new(Direction::Down, SPIN_ORDER, false);
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
        // The facing advances through the cycle from the start facing (the
        // start facing shows while the first 16px move's delay runs).
        let mut anim = EnterMapSpinState::new(Direction::Down, SPIN_ORDER, true);
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

    // ── ElevatorShakeState ────────────────────────────────────────

    const SHAKE: ElevatorShakeParams = ElevatorShakeParams {
        iterations: 100,
        pixel_offset: 1,
    };

    #[test]
    fn elevator_shake_alternates_offset_each_iteration() {
        let mut shake = ElevatorShakeState::new(SHAKE);
        // First iteration scrolls negative.
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
        let mut shake = ElevatorShakeState::new(SHAKE);
        let mut rattles = 0;
        let mut dings = 0;
        let mut frames = 0;
        while !shake.is_done() {
            match shake.tick() {
                Some(ElevatorShakeSfx::Rattle) => rattles += 1,
                Some(ElevatorShakeSfx::Arrive) => dings += 1,
                None => {}
            }
            frames += 1;
        }
        assert_eq!(frames, SHAKE.total_frames());
        assert_eq!(rattles, 100, "rattle once per iteration");
        assert_eq!(dings, 1, "arrival ding at the end");
        assert_eq!(shake.offset_y(), 0, "scroll restored after the shake");
    }

    #[test]
    fn elevator_shake_honors_custom_params() {
        let params = ElevatorShakeParams {
            iterations: 4,
            pixel_offset: 2,
        };
        let mut shake = ElevatorShakeState::new(params);
        assert_eq!(shake.total_frames(), 8);
        assert_eq!(shake.offset_y(), -2);
        shake.tick();
        shake.tick();
        assert_eq!(shake.offset_y(), 2);
        let mut frames = 2;
        while !shake.is_done() {
            shake.tick();
            frames += 1;
        }
        assert_eq!(frames, 8);
    }

    // ── TileAnimState ─────────────────────────────────────────────

    #[test]
    fn tile_anim_disabled_for_none_tilesets() {
        let mut anim = TileAnimState::new();
        anim.set_tileset(TileAnimKind::None);
        for _ in 0..100 {
            anim.tick();
        }
        assert_eq!(anim.water_shift(), 0);
        assert_eq!(anim.flower_frame(), None);
    }

    #[test]
    fn tile_anim_water_rotates_every_20_frames() {
        let mut anim = TileAnimState::new();
        anim.set_tileset(TileAnimKind::Water);
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
        // counter2 increments per water update; direction is right while bit 2
        // is clear (counter2 = 1,2,3,0), left while set (4,5,6,7). Net shift
        // sequence over the first 8 updates: 1,2,3,2,1,0,-1,0.
        let mut anim = TileAnimState::new();
        anim.set_tileset(TileAnimKind::Water);
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
        // Flower update one frame after each water update; frame from
        // counter2&3: 0/1 → flower1, 2 → flower2, 3 → flower3.
        let mut anim = TileAnimState::new();
        anim.set_tileset(TileAnimKind::WaterFlower);
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
        // A tileset swap resets counter1 only.
        let mut anim = TileAnimState::new();
        anim.set_tileset(TileAnimKind::WaterFlower);
        for _ in 0..10 {
            anim.tick();
        }
        anim.set_tileset(TileAnimKind::Water);
        for _ in 0..19 {
            anim.tick();
        }
        assert_eq!(anim.water_shift(), 0);
        anim.tick();
        assert_eq!(anim.water_shift(), 1);
        assert_eq!(anim.flower_frame(), None, "water-only tilesets never flower");
    }

    // ── FishingAnimState ──────────────────────────────────────────

    /// Tick `anim` `n` times in place.
    fn tick_n(anim: &mut FishingAnimState, n: u16) {
        for _ in 0..n {
            anim.tick();
        }
    }

    #[test]
    fn fishing_anim_no_bite_finishes_after_rod_out() {
        // No bite → straight to the result text: no shake, no bubble.
        for facing in [Direction::Down, Direction::Up, Direction::Left, Direction::Right] {
            let mut anim = FishingAnimState::new(facing, false);
            assert_eq!(anim.phase(), FishingAnimPhase::CastDelay);
            // CastDelay covers the first 10 frames: ticks 1..9 are still
            // casting, the 10th tick shows the rod.
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

        // 10 iterations × 3 frames.
        for i in 0..FISHING_SHAKE_ITERATIONS {
            assert_eq!(
                anim.phase(),
                FishingAnimPhase::Shake,
                "shake iteration {i} still active"
            );
            // Offset toggles between +1 (even iteration) and 0 (odd).
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
        assert_eq!(anim.phase(), FishingAnimPhase::Done, "bubble ends → result text");
        assert!(anim.is_done());
        assert!(!anim.bubble_active());
    }

    #[test]
    fn fishing_anim_total_duration() {
        let mut no_bite = FishingAnimState::new(Direction::Left, false);
        let mut bite = FishingAnimState::new(Direction::Left, true);
        for _ in 0..FISHING_ANIM_FRAMES {
            no_bite.tick();
            bite.tick();
        }
        // 10 + 100 (+ 30 + 60 on a bite) — no-bite is done at 110.
        assert!(no_bite.is_done(), "no-bite finishes at 10 + 100 = 110 frames");
        assert!(bite.is_done(), "bite finishes at 10 + 100 + 30 + 60 = 200 frames");
        let mut late = FishingAnimState::new(Direction::Down, true);
        for _ in 0..FISHING_ANIM_FRAMES - 1 {
            late.tick();
        }
        assert_eq!(late.phase(), FishingAnimPhase::Bubble, "frame 199 is still the bubble");
    }

    #[test]
    fn fishing_anim_pose_and_rod_visibility_follow_phase_and_facing() {
        // CastDelay: nothing drawn. Done: pose gone.
        let mut anim = FishingAnimState::new(Direction::Up, true);
        assert!(!anim.pose_active());
        assert!(!anim.rod_visible());
        tick_n(&mut anim, FISHING_CAST_DELAY_FRAMES);
        assert!(anim.pose_active(), "fishing pose while the rod is out");
        assert!(anim.rod_visible());
        assert_eq!(anim.facing(), Direction::Up);

        // Facing up: the rod is hidden during the bubble so it does not
        // overlap the "!", then the pose is restored.
        let shake_bubble = FISHING_CAST_DELAY_FRAMES
            + FISHING_ROD_OUT_FRAMES
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
    fn fishing_anim_rod_piece_offsets() {
        assert_eq!(FishingAnimState::rod_piece(Direction::Down), (20, 35, 0, false));
        assert_eq!(FishingAnimState::rod_piece(Direction::Up), (20, -12, 0, false));
        assert_eq!(FishingAnimState::rod_piece(Direction::Left), (0, 16, 1, false));
        assert_eq!(FishingAnimState::rod_piece(Direction::Right), (48, 16, 1, true));
    }

    // ── BoulderDustState ──────────────────────────────────────────

    #[test]
    fn boulder_dust_inactive_constant_is_inert() {
        let mut dust = BoulderDustState::inactive();
        assert!(!dust.is_active());
        dust.tick();
        assert!(!dust.is_active());
    }

    #[test]
    fn boulder_dust_base_offsets() {
        for (facing, expected) in [
            (Direction::Down, (8, 52)),
            (Direction::Up, (8, -12)),
            (Direction::Left, (-24, 20)),
            (Direction::Right, (40, 20)),
        ] {
            let dust = BoulderDustState::new(facing, 5, 5);
            assert_eq!(dust.base_offset(), expected, "facing {:?}", facing);
        }
    }

    #[test]
    fn boulder_dust_drifts_against_the_push_direction() {
        for (facing, expected) in [
            (Direction::Down, (0, -1)),
            (Direction::Up, (0, 1)),
            (Direction::Left, (1, 0)),
            (Direction::Right, (-1, 0)),
        ] {
            let dust = BoulderDustState::new(facing, 5, 5);
            assert_eq!(dust.drift_px(), expected, "facing {:?}", facing);
        }
    }

    #[test]
    fn boulder_dust_horizontal_push_moves_three_of_four_tiles() {
        // Horizontal pushes leave the upper-left tile in place.
        let dust = BoulderDustState::new(Direction::Left, 5, 5);
        assert_eq!(dust.tile_drifts(), [(0, 0), (1, 0), (1, 0), (1, 0)]);
        let dust = BoulderDustState::new(Direction::Down, 5, 5);
        assert_eq!(dust.tile_drifts(), [(0, -1); 4], "vertical pushes move all tiles");
    }

    #[test]
    fn boulder_dust_runs_8_steps_of_3_frames_then_ends() {
        let mut dust = BoulderDustState::new(Direction::Down, 5, 5);
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
        let mut dust = BoulderDustState::new(Direction::Right, 5, 5);
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
        let mut dust = BoulderDustState::new(Direction::Up, 12, 7);
        assert_eq!(dust.anchor(), (12, 7));
        for _ in 0..24 {
            dust.tick();
        }
        assert_eq!(dust.anchor(), (12, 7), "anchor survives the animation");
    }

    // ── ShipDepartureState ────────────────────────────────────────

    #[test]
    fn ship_departure_phase_boundaries() {
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
        // Ticking a finished state is a no-op (returns no cue).
        assert_eq!(dep.tick(), None);
        assert_eq!(dep.frame(), SHIP_DEPARTURE_TOTAL_FRAMES);
    }

    #[test]
    fn ship_departure_horn_timing() {
        // The horn plays twice: when the scroll begins and when the erase
        // phase begins.
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
                (
                    SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES,
                    ShipDepartureSfx::Horn
                ),
                (
                    SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES
                        + SHIP_DEPARTURE_WATER_FILL_FRAMES
                        + SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_ITERATION_FRAMES,
                    ShipDepartureSfx::Horn
                ),
            ]
        );
    }

    #[test]
    fn ship_departure_scroll_px_ramps() {
        // The view advances 16px per iteration and adds one more px per
        // 8-frame substep: iteration 0 sweeps 1..=16, the total reachable
        // offset is 128px = 16 tiles.
        let mut dep = ShipDepartureState::new();
        for _ in 0..SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES {
            dep.tick();
        }
        let mut seen = Vec::new();
        for _ in 0..SHIP_DEPARTURE_ITERATION_FRAMES {
            seen.push(dep.scroll_px());
            dep.tick();
        }
        let expected: Vec<i32> = (0..16).flat_map(|d| core::iter::repeat(d + 1).take(8)).collect();
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
        // (Tick one frame short of the end — the final frame is Done.)
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
    fn ship_departure_puff_positions() {
        // Puffs spawn 16px apart and drift +2px per substep.
        let mut dep = ShipDepartureState::new();
        for _ in 0..SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES + SHIP_DEPARTURE_WATER_FILL_FRAMES {
            dep.tick();
        }
        // 128 scroll frames = one full iteration; at substep 16 (iteration 1)
        // puff 1 has just been emitted, and puff 0 has drifted 2px × (16+1) —
        // its spawn iteration's drift loop moved it immediately after emission.
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
        // Puff Y is the smokestack row in screen px (10.5 tiles × 8).
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
}
