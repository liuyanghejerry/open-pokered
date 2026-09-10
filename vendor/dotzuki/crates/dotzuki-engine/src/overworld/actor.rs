//! Render- and map-model-agnostic overworld walking actor.
//!
//! The GB-shaped overworld ([`super::player_movement`] over `MapTrait`+`TilesetTrait`
//! with `u8` coords and 4×4 blocks) doesn't fit full-color games whose maps are
//! source-direct image grids with `i32` coords. This is the **generic** actor every
//! game can drive: it owns tile position, smooth pixel interpolation, facing, and the
//! walk-cycle animation state, asking the map only [`OverworldCollision::is_blocked`].
//!
//! It returns *indices*, never pixels — the consumer's renderer is free (wuxia/minimon
//! blit a full-color [`crate`]-external `WalkSprite`; pokered keeps its GB-OAM painter).
//! NPC occupancy and warps stay caller-side: fold NPCs into `is_blocked`, and check
//! warps against the tile [`OverworldActor::update`] reports a step completed on.

use super::types::Direction;

/// Pixels the actor advances per frame while walking (a `tile`-px step over `tile/SPEED`
/// frames — 16px over 8 frames at the default, matching the classic GB cadence).
const WALK_SPEED: f32 = 2.0;
/// Walk-cycle: swap the two step frames every this many moving-frames.
const ANIM_PHASE: u32 = 4;
/// Frames stationary before the actor is considered idle (bridges the 1-frame gap
/// between consecutive tile steps so a continuous walk doesn't flicker to neutral).
const IDLE_GRACE: u32 = 2;
/// Pixels per frame while *running* (a 16px step over 4 frames — twice walk speed).
const RUN_SPEED: f32 = 4.0;
/// Walk-cycle while running: swap the two step frames twice as fast as walking.
const RUN_ANIM_PHASE: u32 = 2;

/// The minimum a map must answer for the actor to move: is walking onto `(x, y)`
/// blocked? Out-of-bounds should return `true` (enclosed world). Callers fold in
/// dynamic collision (NPC tiles, locked doors) by composing it into this impl.
pub trait OverworldCollision {
    fn is_blocked(&self, x: i32, y: i32) -> bool;

    /// Elevation-aware form of [`is_blocked`](Self::is_blocked): is walking onto
    /// `(x, y)` blocked *at elevation `level`*? The default ignores the level
    /// (single-level maps behave exactly as before); multi-level maps override
    /// it to answer from the per-level collision grid.
    fn is_blocked_at(&self, level: u8, x: i32, y: i32) -> bool {
        let _ = level;
        self.is_blocked(x, y)
    }
}

/// Locomotion state the renderer maps to sprite columns: standing, walking, or
/// running. `Run` only looks distinct when the sheet has dedicated run frames (the
/// canonical overworld layout puts them at cols 3/4 — see [`frame_col`]); otherwise
/// the consumer reuses the walk frames at the faster running cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locomotion {
    Idle,
    Walk,
    Run,
}

/// A walking overworld actor (player or NPC): tile target + interpolated pixel
/// position + facing + walk animation. Tile size is configurable (`tile`).
#[derive(Debug, Clone)]
pub struct OverworldActor {
    /// Target tile (where the foot tile is / is heading).
    tile_x: i32,
    tile_y: i32,
    /// Foot-tile top-left in world pixels (smoothly interpolated while walking).
    px: f32,
    py: f32,
    facing: Direction,
    moving: bool,
    /// Walk-cycle phase (advances while moving) and idle-frame counter.
    anim: u32,
    idle: u32,
    /// Whether the consumer is holding "run" this frame (faster speed + animation).
    running: bool,
    tile: i32,
    /// Elevation level the actor walks on (0 = ground). Collision is queried
    /// per level; stair tiles move the actor between levels (caller-side).
    elevation: u8,
}

impl OverworldActor {
    /// Spawn at tile `(tile_x, tile_y)` facing down, with `tile`-px tiles.
    pub fn new(tile_x: i32, tile_y: i32, tile: i32) -> Self {
        Self {
            tile_x,
            tile_y,
            px: (tile_x * tile) as f32,
            py: (tile_y * tile) as f32,
            facing: Direction::Down,
            moving: false,
            anim: 0,
            idle: IDLE_GRACE,
            running: false,
            tile,
            elevation: 0,
        }
    }

    pub fn facing(&self) -> Direction {
        self.facing
    }
    pub fn set_facing(&mut self, dir: Direction) {
        self.facing = dir;
    }
    pub fn is_moving(&self) -> bool {
        self.moving
    }
    /// Set whether the actor is running this frame — the consumer calls this each
    /// frame (e.g. from a held run button) before [`update`](Self::update). Running
    /// uses a faster step speed and animation; the renderer reads
    /// [`locomotion`](Self::locomotion) + [`step_phase`](Self::step_phase) to pick frames.
    pub fn set_running(&mut self, running: bool) {
        self.running = running;
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    /// Target tile coordinates.
    pub fn tile(&self) -> (i32, i32) {
        (self.tile_x, self.tile_y)
    }
    /// Interpolated foot-tile top-left, in world pixels (for camera + drawing).
    pub fn px(&self) -> f32 {
        self.px
    }
    pub fn py(&self) -> f32 {
        self.py
    }
    /// Elevation level the actor walks on (0 = ground).
    pub fn elevation(&self) -> u8 {
        self.elevation
    }
    /// Set the elevation level (stair transitions, spawns, save restore).
    /// The caller clamps to the map's level count.
    pub fn set_elevation(&mut self, level: u8) {
        self.elevation = level;
    }

    /// Snap to tile `(x, y)`, facing `dir`, stationary (warps, spawns, debug teleport).
    pub fn place(&mut self, x: i32, y: i32, dir: Direction) {
        self.tile_x = x;
        self.tile_y = y;
        self.px = (x * self.tile) as f32;
        self.py = (y * self.tile) as f32;
        self.facing = dir;
        self.moving = false;
        self.idle = IDLE_GRACE;
    }

    /// Advance one frame. When idle and `held` is set, the actor turns to face it and
    /// — if the next tile isn't [`OverworldCollision::is_blocked_at`] at the
    /// actor's [`elevation`](Self::elevation) — begins a step.
    /// The in-progress step's pixel interpolation is advanced. Returns `Some((x, y))`
    /// on the frame a step *completes* (the tile just arrived on) so the caller can
    /// check warps; `None` otherwise.
    pub fn update(
        &mut self,
        held: Option<Direction>,
        map: &impl OverworldCollision,
    ) -> Option<(i32, i32)> {
        if !self.moving {
            if let Some(dir) = held {
                self.facing = dir;
                let (dx, dy) = direction_delta(dir);
                let (nx, ny) = (self.tile_x + dx, self.tile_y + dy);
                if !map.is_blocked_at(self.elevation, nx, ny) {
                    self.tile_x = nx;
                    self.tile_y = ny;
                    self.moving = true;
                }
            }
        }

        let mut arrived = None;
        if self.moving {
            let (tx, ty) = (
                (self.tile_x * self.tile) as f32,
                (self.tile_y * self.tile) as f32,
            );
            let speed = if self.running { RUN_SPEED } else { WALK_SPEED };
            self.px = step_toward(self.px, tx, speed);
            self.py = step_toward(self.py, ty, speed);
            if (self.px - tx).abs() < 0.001 && (self.py - ty).abs() < 0.001 {
                self.px = tx;
                self.py = ty;
                self.moving = false;
                arrived = Some((self.tile_x, self.tile_y));
            }
        }

        if self.moving {
            self.anim = self.anim.wrapping_add(1);
            self.idle = 0;
        } else {
            self.idle = self.idle.saturating_add(1);
        }
        arrived
    }

    /// Sprite-sheet row for the current facing (the convention `WalkSprite` and the
    /// `character-sprite-gen` skill author: down=0, up=1, left=2, right=3).
    pub fn facing_row(&self) -> u32 {
        match self.facing {
            Direction::Down => 0,
            Direction::Up => 1,
            Direction::Left => 2,
            Direction::Right => 3,
        }
    }

    /// Generic 3-frame walk index: `0` = neutral/idle, `1`/`2` = the two step frames
    /// (alternating while walking). The consumer maps this to its sheet's columns
    /// (a clean sheet uses it directly; a sheet whose neutral pose is elsewhere remaps).
    pub fn walk_frame(&self) -> u32 {
        if self.idle >= IDLE_GRACE {
            0
        } else {
            1 + (self.anim / ANIM_PHASE) % 2
        }
    }

    /// Locomotion state for picking sprite columns: `Idle` when stationary, else
    /// `Run` if the run flag is set ([`set_running`](Self::set_running)) else `Walk`.
    pub fn locomotion(&self) -> Locomotion {
        if self.idle >= IDLE_GRACE {
            Locomotion::Idle
        } else if self.running {
            Locomotion::Run
        } else {
            Locomotion::Walk
        }
    }

    /// Which of the two step frames to show (`0` or `1`), alternating faster while
    /// running. Pair with [`frame_col`] to resolve the actual sheet column.
    pub fn step_phase(&self) -> u32 {
        let phase = if self.running {
            RUN_ANIM_PHASE
        } else {
            ANIM_PHASE
        };
        (self.anim / phase) % 2
    }
}

/// Resolve the sprite-sheet column for a locomotion state + step phase, given the
/// sheet's column count. Canonical overworld layout: col 0 = stand, cols 1/2 = walk,
/// cols 3/4 = run. Sheets without run frames (`cols < 5`) reuse the walk columns while
/// running (the faster cadence still reads as a run). `phase` is `0`/`1` from
/// [`OverworldActor::step_phase`].
pub fn frame_col(loc: Locomotion, phase: u32, cols: u32) -> u32 {
    let walk = (1 + phase).min(cols.saturating_sub(1));
    match loc {
        Locomotion::Idle => 0,
        Locomotion::Walk => walk,
        Locomotion::Run => {
            if cols >= 5 {
                3 + phase
            } else {
                walk
            }
        }
    }
}

/// Unit step delta for a cardinal direction.
fn direction_delta(dir: Direction) -> (i32, i32) {
    match dir {
        Direction::Down => (0, 1),
        Direction::Up => (0, -1),
        Direction::Left => (-1, 0),
        Direction::Right => (1, 0),
    }
}

/// Move `cur` toward `tgt` by at most `speed` pixels.
fn step_toward(cur: f32, tgt: f32, speed: f32) -> f32 {
    cur + (tgt - cur).clamp(-speed, speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map where a set of tiles is solid; everything else (incl. OOB by convention
    /// of the caller) walkable.
    struct Walls(&'static [(i32, i32)]);
    impl OverworldCollision for Walls {
        fn is_blocked(&self, x: i32, y: i32) -> bool {
            self.0.contains(&(x, y))
        }
    }

    /// A two-level map: per-level solid sets (index = level).
    struct Floors(&'static [&'static [(i32, i32)]]);
    impl OverworldCollision for Floors {
        fn is_blocked(&self, x: i32, y: i32) -> bool {
            self.is_blocked_at(0, x, y)
        }
        fn is_blocked_at(&self, level: u8, x: i32, y: i32) -> bool {
            self.0
                .get(level as usize)
                .map(|walls| walls.contains(&(x, y)))
                .unwrap_or(true)
        }
    }

    /// Elevation defaults to ground level and is settable (stair transitions).
    #[test]
    fn elevation_defaults_and_sets() {
        let mut a = OverworldActor::new(5, 5, 16);
        assert_eq!(a.elevation(), 0);
        a.set_elevation(2);
        assert_eq!(a.elevation(), 2);
    }

    /// Movement collision is queried at the actor's elevation: a tile solid
    /// only on level 0 blocks the ground actor but not the level-1 actor.
    #[test]
    fn movement_blocked_per_elevation() {
        // (6, 5) is solid on level 0 only.
        let map = Floors(&[&[(6, 5)], &[]]);

        let mut ground = OverworldActor::new(5, 5, 16);
        assert_eq!(ground.update(Some(Direction::Right), &map), None);
        assert!(
            !ground.is_moving(),
            "solid at level 0 blocks the ground actor"
        );
        assert_eq!(ground.tile(), (5, 5));

        let mut upper = OverworldActor::new(5, 5, 16);
        upper.set_elevation(1);
        assert!(matches!(upper.update(Some(Direction::Right), &map), None));
        assert!(
            upper.is_moving(),
            "passable at level 1 lets the upper actor move"
        );
        assert_eq!(upper.tile(), (6, 5));
    }

    /// A held direction onto a free tile completes a 16px step over 8 frames and
    /// reports the arrival tile exactly once.
    #[test]
    fn walks_one_tile_in_eight_frames() {
        let map = Walls(&[]);
        let mut a = OverworldActor::new(5, 5, 16);
        for f in 0..8 {
            let got = a.update(Some(Direction::Right), &map);
            if f < 7 {
                assert!(a.is_moving(), "still mid-step at frame {f}");
                assert_eq!(got, None);
            } else {
                assert_eq!(got, Some((6, 5)), "arrives on frame 8");
            }
        }
        assert_eq!(a.tile(), (6, 5));
        assert!(!a.is_moving());
        assert_eq!(a.facing(), Direction::Right);
    }

    /// Facing a wall turns the actor but does not move it.
    #[test]
    fn blocked_turns_without_moving() {
        let map = Walls(&[(5, 4)]);
        let mut a = OverworldActor::new(5, 5, 16);
        assert_eq!(a.update(Some(Direction::Up), &map), None);
        assert!(!a.is_moving());
        assert_eq!(a.tile(), (5, 5));
        assert_eq!(a.facing(), Direction::Up);
    }

    /// Idle shows the neutral frame; walking alternates the two step frames.
    #[test]
    fn walk_frame_neutral_then_alternates() {
        let map = Walls(&[]);
        let mut a = OverworldActor::new(0, 0, 16);
        assert_eq!(a.walk_frame(), 0, "starts idle/neutral");
        let mut seen = crate::hash::HashSet::default();
        for _ in 0..16 {
            a.update(Some(Direction::Down), &map);
            if a.is_moving() {
                seen.insert(a.walk_frame());
            }
        }
        assert!(
            seen.contains(&1) && seen.contains(&2),
            "both step frames appear"
        );
        assert!(!seen.contains(&0), "never neutral while walking");
    }

    /// Running covers a 16px tile in 4 frames (twice walk speed) and reports Run.
    #[test]
    fn running_steps_twice_as_fast() {
        let map = Walls(&[]);
        let mut a = OverworldActor::new(5, 5, 16);
        a.set_running(true);
        for f in 0..4 {
            let got = a.update(Some(Direction::Right), &map);
            if f < 3 {
                assert!(a.is_moving(), "still mid-step at frame {f}");
                assert_eq!(a.locomotion(), Locomotion::Run);
            } else {
                assert_eq!(got, Some((6, 5)), "arrives on frame 4 when running");
            }
        }
        assert_eq!(a.tile(), (6, 5));
    }

    /// Locomotion reports Idle when stationary, Walk/Run by the run flag while moving.
    #[test]
    fn locomotion_reflects_run_flag() {
        let map = Walls(&[]);
        let mut a = OverworldActor::new(0, 0, 16);
        assert_eq!(a.locomotion(), Locomotion::Idle);
        a.update(Some(Direction::Down), &map);
        assert_eq!(a.locomotion(), Locomotion::Walk, "moving, not running");
        a.set_running(true);
        a.update(Some(Direction::Down), &map);
        assert_eq!(a.locomotion(), Locomotion::Run, "moving + run flag");
    }

    /// Canonical column layout: stand=0, walk=1/2, run=3/4; run falls back to walk
    /// on sheets without run frames.
    #[test]
    fn frame_col_canonical_and_fallback() {
        // 5-col sheet (has run frames)
        assert_eq!(frame_col(Locomotion::Idle, 0, 5), 0);
        assert_eq!(frame_col(Locomotion::Walk, 0, 5), 1);
        assert_eq!(frame_col(Locomotion::Walk, 1, 5), 2);
        assert_eq!(frame_col(Locomotion::Run, 0, 5), 3);
        assert_eq!(frame_col(Locomotion::Run, 1, 5), 4);
        // 3-col sheet (no run frames) — run reuses the walk columns
        assert_eq!(frame_col(Locomotion::Run, 0, 3), 1);
        assert_eq!(frame_col(Locomotion::Run, 1, 3), 2);
        assert_eq!(frame_col(Locomotion::Walk, 1, 3), 2);
    }

    #[test]
    fn facing_row_matches_sheet_convention() {
        let mut a = OverworldActor::new(0, 0, 16);
        for (dir, row) in [
            (Direction::Down, 0),
            (Direction::Up, 1),
            (Direction::Left, 2),
            (Direction::Right, 3),
        ] {
            a.set_facing(dir);
            assert_eq!(a.facing_row(), row);
        }
    }
}
