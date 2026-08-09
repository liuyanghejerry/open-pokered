//! Overworld presentation-state machines — pure logic for visual effects that
//! the original implements as blocking animation routines. The renderer reads
//! these each frame; `update.rs` ticks them.
//!
//! Gen-1 references:
//! - engine/overworld/player_animations.asm — `_LeaveMapAnim` (TELEPORT/DIG/
//!   ESCAPE ROPE spin-out), `PlayerSpinInPlace`, `PlayerSpinWhileMovingUpOrDown`
//! - engine/overworld/elevator.asm — `ShakeElevator`
//! - home/vcopy.asm — `UpdateMovingBgTiles` (water/flower tile animation)
//! - home/fade.asm — `LoadGBPal` / `GBPalWhiteOutWithDelay3` (dark cave, FLASH)

use super::doors_elevators::{elevator_shake_params, teleport_spin_direction};
use super::Direction;

// ── Teleport/Dig/Escape-Rope spin-out ─────────────────────────────

/// Phase of the leave-map spin animation (`_LeaveMapAnim` escape-warp path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeleportSpinPhase {
    /// `PlayerSpinInPlace`: 16 spins with frame delays 16,15,…,1 (136 frames
    /// total), SFX_TELEPORT_EXIT_2 whenever the current delay is a multiple
    /// of 4 (spins 0, 4, 8, 12).
    SpinInPlace,
    /// `PlayerSpinWhileMovingUpOrDown` (deltaY=-$10, maxY=$ec, delay 3 — the
    /// DMG value from `GetPlayerTeleportAnimFrameDelay`): 5 spin steps of
    /// 16px each, SFX_TELEPORT_EXIT_1 at the start.
    SpinUp,
    /// The extra 10-frame delay when not standing on a warp pad (field-move
    /// use is never on a warp pad).
    Delay,
    /// Animation finished; the caller starts the fade-out-to-white warp.
    Done,
}

/// Total frames of the spin-in-place phase: 16+15+…+1.
pub const SPIN_IN_PLACE_FRAMES: u16 = 136;
/// Frames between spin-up steps (DMG `GetPlayerTeleportAnimFrameDelay`).
pub const SPIN_UP_STEP_DELAY: u16 = 3;
/// Number of 16px spin-up steps ($3C → $ec in OAM Y).
pub const SPIN_UP_STEPS: u16 = 5;
/// Extra frames after the spin-up when not on a warp pad.
pub const SPIN_POST_DELAY_FRAMES: u16 = 10;
/// Pixels the player sprite rises per spin-up step.
pub const SPIN_UP_STEP_PIXELS: i32 = 16;

/// State of the TELEPORT/DIG/ESCAPE ROPE spin-out animation.
///
/// Frame-driven: constructed when the leave-warp is triggered, ticked once per
/// frame by `update_frame`; the warp fade-out starts when [`Self::is_done`]
/// becomes true (mirroring `_LeaveMapAnim` falling through to
/// `GBFadeOutToWhite`).
#[derive(Debug, Clone, Copy)]
pub struct TeleportSpinState {
    /// Index in TELEPORT_SPIN_ORDER of the facing shown at spin step 0 — the
    /// player's facing when the animation started (`InitFacingDirectionList`
    /// points the facing list at the current direction).
    start_index: usize,
    /// Elapsed frames within the whole animation.
    frame: u16,
    phase: TeleportSpinPhase,
}

impl TeleportSpinState {
    pub fn new(current_facing: Direction) -> Self {
        // PlayerSpinningFacingOrder is DOWN, LEFT, UP, RIGHT; the spin starts
        // by showing the current facing, then advances through the list.
        let start_index = match current_facing {
            Direction::Down => 0,
            Direction::Left => 1,
            Direction::Up => 2,
            Direction::Right => 3,
        };
        Self {
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

    /// Advance one frame. Returns the SFX to play this frame, if any
    /// (SFX_TELEPORT_EXIT_2 on spins whose delay is a multiple of 4,
    /// SFX_TELEPORT_EXIT_1 at the start of the spin-up).
    pub fn tick(&mut self) -> Option<&'static str> {
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
                            sfx = Some("SFX_TELEPORT_EXIT_2");
                        }
                    }
                }
                if self.frame + 1 >= SPIN_IN_PLACE_FRAMES {
                    self.phase = TeleportSpinPhase::SpinUp;
                }
            }
            TeleportSpinPhase::SpinUp => {
                if self.frame == SPIN_IN_PLACE_FRAMES {
                    sfx = Some("SFX_TELEPORT_EXIT_1");
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

    /// Current player facing (`SpinPlayerSprite` rotating the facing list).
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
        teleport_spin_direction(self.start_index + step)
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

    /// Whether the player sprite is still on screen (the original moves the
    /// sprite to Y=$ec, fully above the visible area, by the last step).
    pub fn player_visible(&self) -> bool {
        self.player_y_offset() > -(SPIN_UP_STEPS as i32) * SPIN_UP_STEP_PIXELS
    }
}

// ── Arrival spin-in (`EnterMapAnim`) ──────────────────────────────

/// Phase of the arrival spin (`EnterMapAnim`, player_animations.asm:1-91),
/// the counterpart of [`TeleportSpinState`]: after a FLY / TELEPORT / DIG /
/// ESCAPE ROPE / dungeon-warp arrival, the player descends from off the top
/// of the screen, then spins in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterMapSpinPhase {
    /// `PlayerSpinWhileMovingDown` (deltaY=+$10, maxY=$3c, delay 3 — the DMG
    /// value from `GetPlayerTeleportAnimFrameDelay`): 5 spin steps of 16px
    /// each, SFX_TELEPORT_ENTER_1 at the start, SFX_TELEPORT_ENTER_2 after
    /// the last step.
    SpinDown,
    /// `PlayerSpinInPlace` with delay 0 / delta +1 / end value 8, sound $ff
    /// (none): 8 spins of 0,1,…,7 frames — skipped when the player arrives
    /// ON a warp pad or hole (`IsPlayerStandingOnWarpPadOrHole`).
    SpinInPlace,
    /// Finished; `RestoreFacingDirectionAndYScreenPos` already restored the
    /// facing and Y (the renderer reads the final values).
    Done,
}

/// Total frames of the spin-down phase: 5 spins with 3-frame delays between
/// (the last step ends immediately).
pub const ENTER_MAP_SPIN_DOWN_FRAMES: u16 = (ENTER_MAP_SPIN_DOWN_STEPS - 1) * (1 + ENTER_MAP_SPIN_DOWN_STEP_DELAY) + 1;
/// Number of 16px spin-down steps ($ec → $3c in OAM Y — from offscreen above
/// to the standing position).
pub const ENTER_MAP_SPIN_DOWN_STEPS: u16 = 5;
/// Frames between spin-down steps (DMG `GetPlayerTeleportAnimFrameDelay`).
pub const ENTER_MAP_SPIN_DOWN_STEP_DELAY: u16 = 3;
/// Pixels the player sprite descends per spin-down step.
pub const ENTER_MAP_SPIN_DOWN_STEP_PIXELS: i32 = 16;
/// Total frames of the arrival spin-in-place phase: 8 spins whose delays are
/// 0,1,…,7 (8 + (1+2+…+7)).
pub const ENTER_MAP_SPIN_IN_PLACE_FRAMES: u16 = 36;

/// State of the arrival spin animation (`EnterMapAnim`).
///
/// Frame-driven: constructed when a FLY/teleport-class warp is committed;
/// ticked once per frame by `update_frame` once the fade-in from white has
/// completed (the player stays hidden during the fade, mirroring the
/// original's Y=$ec before `PlayerSpinWhileMovingDown`). The original's
/// pre-fade `Delay3` is folded into the fade-in (both are full-white frames).
/// `spin_in_place` mirrors `IsPlayerStandingOnWarpPadOrHole`: arrivals on a
/// warp pad/hole skip the final spin-in-place.
#[derive(Debug, Clone, Copy)]
pub struct EnterMapSpinState {
    /// Index in TELEPORT_SPIN_ORDER of the facing shown at spin step 0 — the
    /// player's facing at the destination (`InitFacingDirectionList` points
    /// the facing list at the current direction).
    start_index: usize,
    /// Elapsed frames within the whole animation.
    frame: u16,
    phase: EnterMapSpinPhase,
    spin_in_place: bool,
}

impl EnterMapSpinState {
    pub fn new(current_facing: Direction, spin_in_place: bool) -> Self {
        let start_index = match current_facing {
            Direction::Down => 0,
            Direction::Left => 1,
            Direction::Up => 2,
            Direction::Right => 3,
        };
        Self {
            start_index,
            frame: 0,
            phase: EnterMapSpinPhase::SpinDown,
            spin_in_place,
        }
    }

    /// Advance one frame. Returns the SFX to play this frame, if any
    /// (SFX_TELEPORT_ENTER_1 at the start of the spin-down,
    /// SFX_TELEPORT_ENTER_2 when it completes).
    pub fn tick(&mut self) -> Option<&'static str> {
        if self.phase == EnterMapSpinPhase::Done {
            return None;
        }
        let mut sfx = None;
        match self.phase {
            EnterMapSpinPhase::SpinDown => {
                if self.frame == 0 {
                    sfx = Some("SFX_TELEPORT_ENTER_1");
                }
                if self.frame + 1 >= ENTER_MAP_SPIN_DOWN_FRAMES {
                    // PlayerSpinWhileMovingDown finished → SFX_TELEPORT_ENTER_2
                    // (player_animations.asm:20-21), then the spin-in-place
                    // unless the player arrived on a warp pad or hole.
                    sfx = Some("SFX_TELEPORT_ENTER_2");
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

    /// Current player facing (`SpinPlayerSprite` rotating the facing list).
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
            // `RestoreFacingDirectionAndYScreenPos`: the animation ends by
            // restoring the saved (destination) facing.
            EnterMapSpinPhase::Done => 0,
        };
        teleport_spin_direction(self.start_index + step)
    }

    /// Vertical pixel offset of the player sprite (≤ 0; the player descends
    /// from off the top of the screen into the standing position).
    pub fn player_y_offset(&self) -> i32 {
        match self.phase {
            EnterMapSpinPhase::SpinDown => {
                // Moves land on ticks 1, 5, 9, 13, 17 (a spin + 3-frame delay
                // each): Y rises 16px per move, -80 → 0. Frame 0 is the
                // pre-fade position (Y=$ec, fully off the top).
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

    /// Whether the player sprite is still off the top of the screen (the
    /// original keeps Y=$ec until `PlayerSpinWhileMovingDown` descends).
    pub fn player_visible(&self) -> bool {
        self.player_y_offset() > -(ENTER_MAP_SPIN_DOWN_STEPS as i32)
            * ENTER_MAP_SPIN_DOWN_STEP_PIXELS
    }
}

// ── Elevator shake ────────────────────────────────────────────────

/// Total frames of the shake (100 iterations × 2 frames each, DelayFrames(2)
/// per iteration in ShakeElevator).
pub const ELEVATOR_SHAKE_FRAMES: u16 = 200;

/// `ShakeElevator` (engine/overworld/elevator.asm): scrolls the BG up/down by
/// ±1px, 100 iterations of 2 frames, SFX_COLLISION each iteration, then
/// SFX_SAFARI_ZONE_PA (the arrival "ding").
#[derive(Debug, Clone, Copy)]
pub struct ElevatorShakeState {
    /// Elapsed frames of the shake (0..ELEVATOR_SHAKE_FRAMES).
    frame: u16,
}

impl ElevatorShakeState {
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    /// Advance one frame. Returns SFX_COLLISION at the start of each 2-frame
    /// iteration and SFX_SAFARI_ZONE_PA on the final frame.
    pub fn tick(&mut self) -> Option<&'static str> {
        if self.is_done() {
            return None;
        }
        let sfx = if self.frame + 1 >= ELEVATOR_SHAKE_FRAMES {
            Some("SFX_SAFARI_ZONE_PA")
        } else if self.frame % 2 == 0 {
            Some("SFX_COLLISION")
        } else {
            None
        };
        self.frame += 1;
        sfx
    }

    pub fn is_done(&self) -> bool {
        self.frame >= ELEVATOR_SHAKE_FRAMES
    }

    /// Current BG scroll offset (±pixel_offset). The first iteration scrolls
    /// to -1 (`e` starts at 1 and is XORed with $fe before being applied).
    pub fn offset_y(&self) -> i32 {
        if self.is_done() {
            return 0;
        }
        let iteration = self.frame / 2;
        let px = elevator_shake_params().pixel_offset as i32;
        if iteration % 2 == 0 {
            -px
        } else {
            px
        }
    }
}

impl Default for ElevatorShakeState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Water/flower tile animation ───────────────────────────────────

use pokered_data::tileset_data::TileAnimation;

/// Water tile ID ($14 — `vTileset tile $14` in UpdateMovingBgTiles).
pub const ANIM_WATER_TILE: u8 = 0x14;
/// Flower tile ID ($03 — `vTileset tile $03` in UpdateMovingBgTiles).
pub const ANIM_FLOWER_TILE: u8 = 0x03;

/// `UpdateMovingBgTiles` (home/vcopy.asm): per-frame water/flower tile
/// animation driven by `hMovingBGTilesCounter1`/`wMovingBGTilesCounter2`.
///
/// - counter1 increments every frame; at 20 the water tile rotates one pixel;
///   at 21 (WATER_FLOWER tilesets only) the flower tile advances and counter1
///   resets. For WATER-only tilesets counter1 resets right after the water
///   update (`hTileAnimations` bit 0).
/// - counter2 increments on each water update (& 7); its bit 2 selects the
///   water rotation direction (right for 4 updates, then left for 4), and
///   `counter2 & 3` selects the flower frame (0/1 → flower1, 2 → flower2,
///   3 → flower3).
#[derive(Debug, Clone, Copy)]
pub struct TileAnimState {
    counter1: u8,
    counter2: u8,
    /// Net horizontal water-tile rotation in pixels (0..=4, back and forth).
    water_shift: i8,
    /// Flower frame (1..=3) selected by the last flower update, if any.
    flower_frame: Option<u8>,
    /// hTileAnimations for the current tileset (None = animations disabled).
    kind: TileAnimation,
}

impl TileAnimState {
    pub fn new() -> Self {
        Self {
            counter1: 0,
            counter2: 0,
            water_shift: 0,
            flower_frame: None,
            kind: TileAnimation::None,
        }
    }

    /// `LoadTilesetHeader`: adopt the tileset's animation byte and reset
    /// counter1 (counter2 and the accumulated water shift persist, matching
    /// the original's WRAM behavior).
    pub fn set_tileset(&mut self, kind: TileAnimation) {
        self.kind = kind;
        self.counter1 = 0;
    }

    /// Advance one frame (UpdateMovingBgTiles). No-op when the tileset has no
    /// animated tiles (`hTileAnimations == 0` → `ret z`).
    pub fn tick(&mut self) {
        if self.kind == TileAnimation::None {
            return;
        }
        self.counter1 = self.counter1.wrapping_add(1);
        if self.counter1 < 20 {
            return;
        }
        if self.counter1 == 21 {
            // .flower
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
        // rrca/rlca per byte: shift the tile rows one pixel right (counter2
        // bit 2 clear) or left (set).
        self.water_shift += if self.counter2 & 4 == 0 { 1 } else { -1 };
        // `ldh a,[hTileAnimations]; rrca; ret nc` — WATER (bit 0 set) resets
        // the counter immediately; WATER_FLOWER falls through to the flower
        // frame on the next tick.
        if self.kind == TileAnimation::Water {
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

    /// hTileAnimations for the current tileset.
    pub fn kind(&self) -> TileAnimation {
        self.kind
    }
}

impl Default for TileAnimState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Fishing rod animation (FishingAnim) ──────────────────────────

/// Initial pause of `FishingAnim` before the rod appears (`ld c, 10;
/// call DelayFrames`, player_animations.asm:379-380).
pub const FISHING_CAST_DELAY_FRAMES: u16 = 10;
/// Frames the rod stays out waiting for a bite (`ld c, 100; call
/// DelayFrames`, player_animations.asm:398-399).
pub const FISHING_ROD_OUT_FRAMES: u16 = 100;
/// Shake iterations on a bite (10 × `.ShakePlayerSprite` + `Delay3`,
/// player_animations.asm:411-419).
pub const FISHING_SHAKE_ITERATIONS: u16 = 10;
/// Frames per shake iteration (`Delay3` — 3 frames).
pub const FISHING_SHAKE_STEP_FRAMES: u16 = 3;
/// Frames the "!" emotion bubble stays up (`EmotionBubble`'s
/// `ld c, 60; call DelayFrames`, emotion_bubbles.asm:57-58).
pub const FISHING_BUBBLE_FRAMES: u16 = 60;

/// Total frames of the whole animation: 10 + 100 + 30 + 60.
pub const FISHING_ANIM_FRAMES: u16 = FISHING_CAST_DELAY_FRAMES
    + FISHING_ROD_OUT_FRAMES
    + FISHING_SHAKE_ITERATIONS * FISHING_SHAKE_STEP_FRAMES
    + FISHING_BUBBLE_FRAMES;

/// Phase of the player-side fishing rod animation (`FishingAnim`,
/// player_animations.asm:378-469).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishingAnimPhase {
    /// `DelayFrames(10)` — nothing drawn yet (the rod OAM and the fishing
    /// pose tiles are set up only after this delay).
    CastDelay,
    /// `DelayFrames(100)` — the rod is out and the player holds the fishing
    /// pose; `wRodResponse` decides the outcome at the end of this phase.
    RodOut,
    /// Bite only: 10 × `Delay3` toggling the player sprite's and rod's Y by
    /// ±1 px (`.ShakePlayerSprite` — `xor $1`).
    Shake,
    /// Bite only: the "!" `EmotionBubble` over the player (60 frames). The
    /// rod is hidden during this phase when the player faces up (so it does
    /// not overlap the bubble), then unhidden.
    Bubble,
    /// Finished; the caller shows the result text (`PrintText`).
    Done,
}

/// Frame-driven state of the rod animation, mirroring the original's
/// blocking `FishingAnim` routine. Constructed when a rod use passes the
/// `FishingInit` gates; ticked once per frame by `update_frame` (which
/// freezes gameplay while it runs, matching the original's `DelayFrames`
/// loops); when [`Self::is_done`] the result text is queued.
///
/// Gen-1 references:
/// - engine/overworld/player_animations.asm — `FishingAnim` (:378-469),
///   `FishingRodOAM` (:471-476), `RedFishingTiles` (:485-489)
/// - engine/overworld/emotion_bubbles.asm — `EmotionBubble` (60-frame
///   `DelayFrames`, `EXCLAMATION_BUBBLE` for the rod flow)
#[derive(Debug, Clone, Copy)]
pub struct FishingAnimState {
    /// Elapsed ticks (0 before the first `tick`).
    frame: u16,
    /// Player facing when the anim started (`wSpritePlayerStateData1ImageIndex`
    /// selects the `FishingRodOAM` entry).
    facing: Direction,
    /// `wRodResponse != 0`/`!= 2` — a bite plays the shake + bubble.
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

    /// Facing captured at construction (the `FishingRodOAM` entry and the
    /// fishing pose sprite).
    pub fn facing(&self) -> Direction {
        self.facing
    }

    /// Whether the player is holding the fishing pose (pose + rod shown).
    /// False during the initial 10-frame delay and after the anim ends —
    /// the original swaps in the `RedFishingTiles` after `DelayFrames(10)`
    /// and only restores the normal sprite via the map sprite reload after
    /// the anim (the pose is drawn while the anim is active).
    pub fn pose_active(&self) -> bool {
        matches!(
            self.phase,
            FishingAnimPhase::RodOut | FishingAnimPhase::Shake | FishingAnimPhase::Bubble
        )
    }

    /// Whether the rod OAM piece is drawn this frame. Hidden during the
    /// bubble for the up-facing player (`wShadowOAMSprite39YCoord` moved to
    /// `SCREEN_HEIGHT_PX + OAM_Y_OFS`, player_animations.asm:421-428), and
    /// not yet present during the cast delay.
    pub fn rod_visible(&self) -> bool {
        self.pose_active()
            && !(self.phase == FishingAnimPhase::Bubble && self.facing == Direction::Up)
    }

    /// Whether the "!" emotion bubble is displayed above the player.
    pub fn bubble_active(&self) -> bool {
        self.phase == FishingAnimPhase::Bubble
    }

    /// Vertical offset (±1 px) of the player sprite and rod during the bite
    /// shake (`.ShakePlayerSprite` toggles the Y coordinate with `xor $1`
    /// every `Delay3`).
    pub fn player_shake_offset(&self) -> i32 {
        if self.phase != FishingAnimPhase::Shake {
            return 0;
        }
        let shake_start = FISHING_CAST_DELAY_FRAMES + FISHING_ROD_OUT_FRAMES;
        let iteration = (self.frame - shake_start) / FISHING_SHAKE_STEP_FRAMES;
        if iteration % 2 == 0 { 1 } else { 0 }
    }

    /// The rod's OAM piece for `facing` — `FishingRodOAM`
    /// (player_animations.asm:471-476), converted from OAM to screen coords
    /// (OAM Y includes the +16 `OAM_Y_OFS`) and expressed as an OFFSET from
    /// the player sprite's top-left. The original's OAM values are absolute
    /// screen coords authored for its bottom-anchored player (top-left at
    /// screen (128,128), feet at the 144px bottom edge — see
    /// `wSpritePlayerStateData1YPixels`/`lda_coord 8, 9`); this port centers
    /// the player at screen (72,64), so the offsets preserve the original's
    /// rod-vs-player relationship. Returns `(dx, dy, tile index into the
    /// 8×24 `fishing_rod` sheet, x_flip)`; the sheet's tiles are $fd (0,
    /// DOWN/UP) and $fe (1, LEFT/RIGHT — X-flipped for RIGHT). In the
    /// original the DOWN/RIGHT pieces landed off-screen (below/right of the
    /// 160×144 screen); with the port's centered player they are visible.
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

/// Number of animation steps of the boulder-dust puff — `AnimateBoulderDust`'s
/// `ld c, 8` loop (engine/overworld/dust_smoke.asm:12).
pub const BOULDER_DUST_STEPS: u8 = 8;
/// Frames each dust step lasts — the original's `Delay3` (3 frames).
pub const BOULDER_DUST_STEP_FRAMES: u8 = 3;

/// The 2×2 smoke-puff block kicked up when a STRENGTH boulder slides one
/// tile (engine/overworld/dust_smoke.asm — `AnimateBoulderDust`, called
/// from `RunMapScript`'s `DoBoulderDustAnimation` while BIT_BOULDER_DUST
/// is set).
///
/// The original writes a 2×2 OAM block of smoke tiles (`gfx/overworld/
/// smoke.2bpp`) once, positioned from the player's sprite position plus
/// the per-facing `BoulderDustAnimationOffsets` (cut.asm:170-176 — the
/// puff appears at the boulder's base, "2 blocks away from the player"),
/// then runs 8 steps of `Delay3` (3 frames) each. Every step the block
/// drifts 1px against the boulder's slide direction
/// (`MoveBoulderDustFunctionPointerTable`) and the sprite palette toggles
/// (rOBP1 XOR %01100100), flashing the two gray shades.
///
/// The offsets are in screen pixels from the player sprite's top-left and
/// are used directly (the original's OAM Y +16 `OAM_Y_OFS` cancels out of
/// the visible position — same conversion as [`FishingAnimState::rod_piece`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoulderDustState {
    /// The push direction — the player's facing when the boulder moved
    /// (also the dust's base offset and drift direction).
    facing: Direction,
    /// The player's map tile when the push started: the dust block is
    /// anchored to that world spot, because the original writes its OAM
    /// block once from the player's then-current sprite position (the
    /// animation outlives the push lockout, so the anchor must not track
    /// the player afterward).
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
    /// player sprite's top-left (BoulderDustAnimationOffsets, cut.asm:170-176).
    pub fn base_offset(&self) -> (i32, i32) {
        match self.facing {
            Direction::Down => (8, 52),
            Direction::Up => (8, -12),
            Direction::Left => (-24, 20),
            Direction::Right => (40, 20),
        }
    }

    /// Per-step pixel drift of the dust block — opposite to the boulder's
    /// slide direction (MoveBoulderDustFunctionPointerTable, dust_smoke.asm:
    /// 59-63: down → Y−1, up → Y+1, left → X+1, right → X−1). The puff
    /// lingers as the boulder slides away from it.
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
    /// lower-left and lower-right tiles — the original's OAM-adjust loop
    /// starts at the upper-right sprite (`wShadowOAMSprite36` + 1,
    /// dust_smoke.asm:32-52), leaving the upper-left tile in place.
    pub fn tile_drifts(&self) -> [(i32, i32); 4] {
        let (dx, dy) = self.drift_px();
        match self.facing {
            Direction::Left | Direction::Right => [(0, 0), (dx, 0), (dx, 0), (dx, 0)],
            _ => [(dx, dy); 4],
        }
    }

    /// True on odd steps: the original toggles the dust's OBP1 palette
    /// (XOR %01100100) once per step, flashing the two gray shades of the
    /// smoke sprite.
    pub fn palette_flipped(&self) -> bool {
        self.step % 2 == 1
    }
}

// ── FLASH white-out ───────────────────────────────────────────────

/// Frames of the all-palettes-white flash when FLASH lights a dark cave
/// (`GBPalWhiteOutWithDelay3` — white-out plus a 3-frame wait).
pub const FLASH_WHITE_FRAMES: u8 = 3;

// ── S.S. Anne departure (VermilionDockSSAnneLeavesScript) ─────────

/// Initial pause of the departure cutscene — `ld c, 120; call DelayFrames`
/// (scripts/VermilionDock.asm:44-47, after MUSIC_SURFING starts).
pub const SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES: u16 = 120;
/// The water-fill commit — `hAutoBGTransferEnabled = 1; call Delay3`
/// (VermilionDock.asm:50-56) pushing the screen tile buffer's water fill
/// to VRAM. The port's ship occupies only the two bottom map rows, so no
/// separate visible upper-hull wipe exists; the phase is kept for frame
/// fidelity (the erase phase below is the visible ship removal).
pub const SHIP_DEPARTURE_WATER_FILL_FRAMES: u16 = 3;
/// View-scroll iterations — `ld e, $8` (.shift_columns_up,
/// VermilionDock.asm:67).
pub const SHIP_DEPARTURE_SCROLL_ITERATIONS: u16 = 8;
/// Frames per iteration: 16 smoke-drift substeps (`ld b, $10`) × an
/// 8-frame delay each (`ld c, $8; .delay_between_drifts`).
pub const SHIP_DEPARTURE_ITERATION_FRAMES: u16 = 16 * 8;
/// Final pause inside `VermilionDock_EraseSSAnne` (`ld c, 120;
/// call DelayFrames`, VermilionDock.asm:222-223).
pub const SHIP_DEPARTURE_ERASE_FRAMES: u16 = 120;
/// Tiles the view scrolls east per iteration (`add hl, $2` on
/// wMapViewVRAMPointer, VermilionDock.asm:69-74) — the ship slides left.
pub const SHIP_DEPARTURE_SCROLL_PX_PER_ITERATION: i32 = 2 * 8;
/// Smoke-puff spawn spacing: the original emits a new 2×2 puff block above
/// the smokestack every iteration at screen X = 88 − 16i (wSSAnneSmokeX
/// starts at 88 and `sub 16` per emission).
pub const SHIP_DEPARTURE_PUFF_SPACING_PX: i32 = 16;
/// Smoke-puff drift: every 8-frame substep all live puffs drift +2px right
/// (VermilionDock_AnimSmokePuffDriftRight, `inc [hl]` twice per sprite).
pub const SHIP_DEPARTURE_PUFF_DRIFT_PX_PER_SUBSTEP: i32 = 2;
/// Substeps per iteration (the drift loop's `ld b, $10`).
pub const SHIP_DEPARTURE_SUBSTEPS_PER_ITERATION: u16 = 16;
/// Frames per drift substep (`ld c, $8; .delay_between_drifts`).
pub const SHIP_DEPARTURE_SUBSTEP_FRAMES: u16 = 8;
/// The smokestack's map position (tile units) — the puffs' world anchor.
/// The original emits at screen (11, 10.5) tiles of the 20-wide dock; the
/// port's 28-wide map keeps the same relative column (11/20 ≈ 16/28) and
/// the same row (10.5, the top of the ship hull rows 10-11).
pub const SHIP_DEPARTURE_SMOKESTACK_TILE_X: u16 = 16;
pub const SHIP_DEPARTURE_SMOKESTACK_TILE_Y: f32 = 10.5;
/// The first puff's screen X at departure start (wSSAnneSmokeX = 88).
pub const SHIP_DEPARTURE_PUFF_START_SCREEN_X: i32 = 88;

/// Phase of the S.S. Anne departure cutscene
/// (`VermilionDockSSAnneLeavesScript` + `VermilionDock_EraseSSAnne`,
/// scripts/VermilionDock.asm:33-123, 182-224).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipDeparturePhase {
    /// `DelayFrames(120)` after the Surfing music starts — the ship sits
    /// at the dock while the music plays.
    InitialPause,
    /// `hAutoBGTransferEnabled` + `Delay3` — the original commits the
    /// screen-buffer water fill (screen rows 10-15) to VRAM here. The
    /// port's two-row hull has no separate upper portion, so the visible
    /// erase happens in [`Self::Erase`] (which mirrors the original's
    /// `VermilionDock_EraseSSAnne` full-area fill).
    WaterFill,
    /// `.shift_columns_up`: 8 iterations of a 2-tile east view scroll
    /// (128 frames each) with a smoke puff emitted per iteration and all
    /// live puffs drifting right.
    Scroll,
    /// `VermilionDock_EraseSSAnne`: the ship's map blocks become water,
    /// the dock→ship warp is removed (wNumberOfWarps--), the horn plays
    /// again, and `DelayFrames(120)` closes the cutscene.
    Erase,
    /// Finished; the caller proceeds to the forced walk-out.
    Done,
}

/// Total frames of the whole cutscene: 120 + 3 + 8×128 + 120.
pub const SHIP_DEPARTURE_TOTAL_FRAMES: u16 = SHIP_DEPARTURE_INITIAL_PAUSE_FRAMES
    + SHIP_DEPARTURE_WATER_FILL_FRAMES
    + SHIP_DEPARTURE_SCROLL_ITERATIONS * SHIP_DEPARTURE_ITERATION_FRAMES
    + SHIP_DEPARTURE_ERASE_FRAMES;

/// Frame-driven state of the S.S. Anne departure cutscene. Constructed
/// when the VermilionDock scene's `playShipDeparture()` effect fires;
/// ticked once per frame by `update_frame` (freezing gameplay, matching
/// the original's blocking DelayFrames loops). The first horn
/// (SFX_SS_ANNE_HORN, played with `PlaySoundWaitForCurrent` before the
/// scroll loop) is emitted when the scroll begins; the second
/// (non-blocking `PlaySound`) when the erase phase begins. The renderer
/// reads the scroll offset and puff positions; `update.rs` applies the
/// ship erase + warp removal at the erase transition.
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

    /// Advance one frame. Returns the SFX to play this frame, if any:
    /// SFX_SS_ANNE_HORN on the first scroll frame (the asm's
    /// `PlaySoundWaitForCurrent` before `.shift_columns_up`) and on the
    /// first erase frame (`VermilionDock_EraseSSAnne`'s `PlaySound`).
    pub fn tick(&mut self) -> Option<&'static str> {
        if self.phase == ShipDeparturePhase::Done {
            return None;
        }
        let mut sfx = None;
        let next = self.frame + 1;
        let next_phase = Self::phase_for(next);
        if next_phase != self.phase {
            match next_phase {
                ShipDeparturePhase::Scroll => {
                    sfx = Some("SFX_SS_ANNE_HORN");
                }
                ShipDeparturePhase::Erase => {
                    sfx = Some("SFX_SS_ANNE_HORN");
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
    /// begun (the block mutation in `update.rs` lands at the same time;
    /// this flag covers the same frame for renderers that draw before the
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

    /// Horizontal scroll of the map view in pixels (0..=128). The original
    /// advances wMapViewVRAMPointer 2 tiles per iteration (16px) and, via
    /// the LY-split `VermilionDock_SyncScrollWithLY`, ramps rSCX by one
    /// more pixel per 8-frame substep — net movement 16i + (substep+1).
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
    /// departure start: spawns at X = 88 − 16i (wSSAnneSmokeX) and drifts
    /// +2px per substep from its spawn substep (the spawn iteration's own
    /// drift loop moves it immediately — `VermilionDock_EmitSmokePuff`
    /// runs before the 16-drift loop of the same iteration). Renderers
    /// rebase onto their own view by adding
    /// `(smokestack_screen_x - SHIP_DEPARTURE_PUFF_START_SCREEN_X)`.
    pub fn puff_x_offset(&self, i: usize) -> i32 {
        let i = i as i32;
        let spawn = SHIP_DEPARTURE_PUFF_START_SCREEN_X - SHIP_DEPARTURE_PUFF_SPACING_PX * i;
        let s = self.scroll_substep() as i32;
        let substeps_live = (s - SHIP_DEPARTURE_SUBSTEPS_PER_ITERATION as i32 * i).max(0);
        spawn + SHIP_DEPARTURE_PUFF_DRIFT_PX_PER_SUBSTEP * (substeps_live + 1)
    }

    /// Screen y (px) of every puff at departure start — the original's
    /// OAM Y=100 minus the OAM_Y_OFS, i.e. the smokestack row (10.5 tiles).
    pub fn puff_screen_y(&self) -> i32 {
        (SHIP_DEPARTURE_SMOKESTACK_TILE_Y * 8.0) as i32
    }
}

impl Default for ShipDepartureState {
    fn default() -> Self {
        Self::new()
    }
}
