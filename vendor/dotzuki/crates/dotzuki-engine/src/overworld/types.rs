//! Core overworld type definitions for a JRPG engine.
//!
//! These types define the foundational data structures for the overworld
//! map system, player movement, NPCs, and map data. All types are generic
//! over their game-specific identifiers using the [`MapTrait`] and
//! [`TilesetTrait`] trait bounds.

use crate::map::MapTrait;
use crate::tileset::TilesetTrait;

use serde::{Deserialize, Serialize};
use core::fmt::Debug;
use core::hash::Hash;

// ── Direction ──────────────────────────────────────────────────────

/// Cardinal direction for movement and connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Down,
    Up,
    Left,
    Right,
}

/// Transport mode for player movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportMode {
    Walking,
    Biking,
    Surfing,
}

/// Player movement state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MovementState {
    Idle,
    Walking,
    Jumping,
}

// ── Map Connection ─────────────────────────────────────────────────

/// A single map connection (e.g., north exit leads to Route 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MapConnection<M: MapTrait> {
    pub direction: Direction,
    pub target_map: M,
    /// Offset in blocks for alignment when crossing the boundary.
    pub offset: i8,
}

impl<M: MapTrait> MapConnection<M> {
    /// Create a new map connection.
    pub fn new(direction: Direction, target_map: M, offset: i8) -> Self {
        Self {
            direction,
            target_map,
            offset,
        }
    }
}

/// All connections for a map (up to one per cardinal direction).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MapConnections<M: MapTrait> {
    pub north: Option<MapConnection<M>>,
    pub south: Option<MapConnection<M>>,
    pub west: Option<MapConnection<M>>,
    pub east: Option<MapConnection<M>>,
}

impl<M: MapTrait> Default for MapConnections<M> {
    fn default() -> Self {
        Self {
            north: None,
            south: None,
            west: None,
            east: None,
        }
    }
}

impl<M: MapTrait> MapConnections<M> {
    /// Number of active connections.
    pub fn count(&self) -> usize {
        self.north.is_some() as usize
            + self.south.is_some() as usize
            + self.west.is_some() as usize
            + self.east.is_some() as usize
    }

    /// Get connection for a direction, if any.
    pub fn get(&self, dir: Direction) -> Option<&MapConnection<M>> {
        match dir {
            Direction::Up => self.north.as_ref(),
            Direction::Down => self.south.as_ref(),
            Direction::Left => self.west.as_ref(),
            Direction::Right => self.east.as_ref(),
        }
    }
}

// ── Warp Point ─────────────────────────────────────────────────────

/// A warp point within a map (door, staircase, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WarpPoint<M: MapTrait> {
    /// Position in the map (block coordinates).
    pub x: u8,
    pub y: u8,
    /// Target map to warp to.
    pub target_map: M,
    /// Index of the target warp in the destination map.
    pub target_warp_id: u8,
    /// Whether this warp sends the player back to the last-visited map.
    pub is_last_map: bool,
}

impl<M: MapTrait> WarpPoint<M> {
    /// Create a new warp point.
    pub fn new(x: u8, y: u8, target_map: M, target_warp_id: u8) -> Self {
        Self {
            x,
            y,
            target_map,
            target_warp_id,
            is_last_map: false,
        }
    }
}

// ── Sign ───────────────────────────────────────────────────────────

/// A sign in the map that displays text when interacted with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Sign {
    pub x: u8,
    pub y: u8,
    /// Index into the map's text table.
    pub text_id: u8,
}

impl Sign {
    /// Create a new sign.
    pub fn new(x: u8, y: u8, text_id: u8) -> Self {
        Self { x, y, text_id }
    }
}

// ── NPC Definition ─────────────────────────────────────────────────

/// NPC movement pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcMovementType {
    /// NPC stays in place and faces a fixed direction.
    Stationary,
    /// NPC walks randomly within their range.
    Wander,
    /// NPC walks a fixed path.
    FixedPath,
    /// NPC turns to face the player when spoken to.
    FacePlayer,
}

/// Axis restriction for [`NpcMovementType::Wander`] — the classic GB
/// "movement byte 2" of a random-walk NPC ($00 any / $01 up-down /
/// $02 left-right). Gen-1 random walkers have NO radial leash: they walk
/// the allowed axis until blocked, forever.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcWanderAxis {
    /// Any of the four directions (movement byte 2 = $00).
    #[default]
    Any,
    /// Only vertically — up/down (movement byte 2 = $01, UP_DOWN).
    Vertical,
    /// Only horizontally — left/right (movement byte 2 = $02, LEFT_RIGHT).
    Horizontal,
}

/// Definition of an NPC placed on the map (static data from map objects).
///
/// This is a generic NPC definition with no game-specific fields.
/// Game-specific data (e.g., trainer flags, item drops) should be
/// stored in a separate struct alongside this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NpcDefinition {
    /// Sprite ID (index into sprite table).
    pub sprite_id: u8,
    /// Starting position.
    pub x: u8,
    pub y: u8,
    /// Movement type.
    pub movement: NpcMovementType,
    /// Axis restriction for Wander NPCs (classic movement byte 2). Ignored
    /// for other movement types.
    pub wander_axis: NpcWanderAxis,
    /// Facing direction.
    pub facing: Direction,
    /// Range of movement (0 = stationary).
    pub range: u8,
    /// Text ID triggered on interaction.
    pub text_id: u8,
}

impl NpcDefinition {
    /// Create a new NPC definition.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sprite_id: u8,
        x: u8,
        y: u8,
        movement: NpcMovementType,
        facing: Direction,
        range: u8,
        text_id: u8,
    ) -> Self {
        Self {
            sprite_id,
            x,
            y,
            movement,
            wander_axis: NpcWanderAxis::Any,
            facing,
            range,
            text_id,
        }
    }

    /// Create a new NPC definition with an explicit Wander axis (the classic
    /// GB "movement byte 2"). Only meaningful for [`NpcMovementType::Wander`].
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_axis(
        sprite_id: u8,
        x: u8,
        y: u8,
        movement: NpcMovementType,
        wander_axis: NpcWanderAxis,
        facing: Direction,
        range: u8,
        text_id: u8,
    ) -> Self {
        Self {
            sprite_id,
            x,
            y,
            movement,
            wander_axis,
            facing,
            range,
            text_id,
        }
    }
}

// ── Map Data ───────────────────────────────────────────────────────

/// Complete runtime data for a loaded map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MapData<M: MapTrait, T: TilesetTrait, Mus> {
    pub id: M,
    pub width: u8,
    pub height: u8,
    pub tileset: T,
    pub music: Mus,
    /// Block data — the actual tile layout. Each byte is a block index
    /// into the tileset's block definitions. Size = width * height.
    pub blocks: Vec<u8>,
    pub warps: Vec<WarpPoint<M>>,
    pub npcs: Vec<NpcDefinition>,
    pub signs: Vec<Sign>,
    pub connections: MapConnections<M>,
}

impl<M: MapTrait, T: TilesetTrait, Mus> MapData<M, T, Mus> {
    /// Create a new MapData with all fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: M,
        width: u8,
        height: u8,
        tileset: T,
        music: Mus,
        blocks: Vec<u8>,
        warps: Vec<WarpPoint<M>>,
        npcs: Vec<NpcDefinition>,
        signs: Vec<Sign>,
        connections: MapConnections<M>,
    ) -> Self {
        Self {
            id,
            width,
            height,
            tileset,
            music,
            blocks,
            warps,
            npcs,
            signs,
            connections,
        }
    }

    /// Replace the block at BLOCK coords (`block_x`, `block_y`) with `block_id`.
    ///
    /// Returns `false` (no-op) if the coordinates fall outside the map. The
    /// collision and rendering systems read `blocks` live every frame, so the
    /// change takes effect immediately with no cache to invalidate. The change
    /// is transient: a map reload rebuilds `blocks`, so callers that need a
    /// persistent change must re-apply it on map entry.
    pub fn set_block(&mut self, block_x: u8, block_y: u8, block_id: u8) -> bool {
        let (w, h) = (self.width as usize, self.height as usize);
        let (bx, by) = (block_x as usize, block_y as usize);
        if bx >= w || by >= h {
            return false;
        }
        let idx = by * w + bx;
        if idx < self.blocks.len() {
            self.blocks[idx] = block_id;
            true
        } else {
            false
        }
    }
}

// ── Player State ───────────────────────────────────────────────────

/// Runtime player state in the overworld.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PlayerState {
    pub x: u16,
    pub y: u16,
    pub facing: Direction,
    pub movement_state: MovementState,
    pub transport: TransportMode,
    /// Whether biking advances at double speed (the classic bike speedup).
    /// Defaults to on; a game can switch it off for specific rules (e.g.
    /// a steep slope cancels the speedup while the player presses
    /// UP/LEFT/RIGHT).
    #[serde(default = "default_bike_speedup")]
    pub bike_speedup_active: bool,
}

fn default_bike_speedup() -> bool {
    true
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            facing: Direction::Down,
            movement_state: MovementState::Idle,
            transport: TransportMode::Walking,
            bike_speedup_active: true,
        }
    }
}

// ── Overworld State ────────────────────────────────────────────────

/// Top-level overworld state, holding the current map and player.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OverworldState<M: MapTrait> {
    pub current_map: M,
    pub player: PlayerState,
    /// Walk animation counter (0-15).
    pub walk_counter: u8,
    /// Steps until next wild encounter check resets.
    pub encounter_cooldown: u8,
    /// Remaining repel steps (0 = inactive).
    pub repel_steps: u16,
    /// Whether the player is currently standing on a warp coordinate.
    pub standing_on_warp: bool,
    /// Whether the player just warped onto a door tile and needs to auto-step off.
    pub standing_on_door: bool,
    /// Whether the player is currently performing the auto-step out of a door.
    pub exiting_door: bool,
}

impl<M: MapTrait> OverworldState<M> {
    /// Create a new overworld state starting at the given map.
    pub fn new(start_map: M) -> Self {
        Self {
            current_map: start_map,
            player: PlayerState::default(),
            walk_counter: 0,
            encounter_cooldown: 0,
            repel_steps: 0,
            standing_on_warp: false,
            standing_on_door: false,
            exiting_door: false,
        }
    }
}

// ── Overworld Input ────────────────────────────────────────────────

/// Overworld input state for a single frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OverworldInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
    pub select: bool,
}

impl OverworldInput {
    /// Create a new overworld input state with the given button states.
    pub fn new(
        up: bool,
        down: bool,
        left: bool,
        right: bool,
        a: bool,
        b: bool,
        start: bool,
        select: bool,
    ) -> Self {
        Self {
            up,
            down,
            left,
            right,
            a,
            b,
            start,
            select,
        }
    }

    /// Create an input state with no keys pressed.
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            a: false,
            b: false,
            start: false,
            select: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    struct TestMap;
    impl MapTrait for TestMap {}

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    struct TestTileset;
    impl TilesetTrait for TestTileset {
        fn id(&self) -> u8 {
            0
        }
        fn name(&self) -> &'static str {
            "test"
        }
    }

    /// Build a 3×2 (w×h) map whose blocks are [0,1,2, 3,4,5].
    fn make_map() -> MapData<TestMap, TestTileset, u8> {
        MapData::new(
            TestMap,
            3,
            2,
            TestTileset,
            0u8,
            vec![0, 1, 2, 3, 4, 5],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            MapConnections::default(),
        )
    }

    #[test]
    fn set_block_in_bounds_mutates_and_returns_true() {
        let mut map = make_map();
        // (block_x=2, block_y=1) -> idx = 1*3 + 2 = 5
        assert!(map.set_block(2, 1, 99));
        assert_eq!(map.blocks[5], 99);
        // Other blocks untouched.
        assert_eq!(map.blocks, vec![0, 1, 2, 3, 4, 99]);

        // Top-left corner -> idx 0.
        assert!(map.set_block(0, 0, 42));
        assert_eq!(map.blocks[0], 42);
    }

    #[test]
    fn set_block_out_of_bounds_returns_false_and_no_change() {
        let mut map = make_map();
        let before = map.blocks.clone();

        // x out of range (width is 3, so valid x is 0..=2).
        assert!(!map.set_block(3, 0, 99));
        // y out of range (height is 2, so valid y is 0..=1).
        assert!(!map.set_block(0, 2, 99));
        // Both out of range.
        assert!(!map.set_block(10, 10, 99));

        assert_eq!(map.blocks, before, "out-of-bounds writes must not mutate");
    }
}
