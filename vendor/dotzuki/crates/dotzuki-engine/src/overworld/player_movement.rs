//! Generic player movement system for the overworld.
//!
//! Implements player movement: the movement loop with input handling,
//! walking/sprite-advance logic, and player state transitions.

use crate::map::MapTrait;
use crate::tileset::TilesetTrait;

use super::collision::{
    check_movement_collision, check_warp_at_position, is_facing_map_edge, CollisionProvider,
    CollisionResult, SpritePosition,
};
use super::types::{
    Direction, MapData, MovementState, OverworldState as GenericOverworldState, TransportMode,
};

/// Walk counter initial value (8 frames per tile).
pub const WALK_COUNTER_INIT: u8 = 8;

/// Input state from the player's controller.
#[derive(Debug, Clone, Copy, Default)]
pub struct InputState {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a_button: bool,
    pub b_button: bool,
    pub start: bool,
    pub select: bool,
}

impl InputState {
    /// Get the direction being pressed, if any.
    /// Priority matches the original game: Down > Up > Left > Right.
    pub fn direction_pressed(&self) -> Option<Direction> {
        if self.down {
            Some(Direction::Down)
        } else if self.up {
            Some(Direction::Up)
        } else if self.left {
            Some(Direction::Left)
        } else if self.right {
            Some(Direction::Right)
        } else {
            None
        }
    }

    /// Convert to the raw d-pad bitmask used in the original game.
    pub fn to_pad_bits(&self) -> u8 {
        let mut bits = 0u8;
        if self.down {
            bits |= super::collision::PAD_DOWN;
        }
        if self.up {
            bits |= super::collision::PAD_UP;
        }
        if self.left {
            bits |= super::collision::PAD_LEFT;
        }
        if self.right {
            bits |= super::collision::PAD_RIGHT;
        }
        bits
    }
}

/// Result of processing a movement attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveResult {
    /// Player started walking to the target tile.
    Walking,
    /// Player started a ledge jump.
    LedgeJump,
    /// Player only turned to face a new direction (no movement).
    TurnedOnly,
    /// Movement was blocked (wall, NPC, etc.).
    Blocked(CollisionResult),
    /// Player reached the edge of the map (connection should be checked).
    ReachedMapEdge,
    /// Player stepped onto a warp tile.
    Warped { warp_index: usize },
    /// Player is still mid-step from previous movement.
    StillMoving,
    /// No input was pressed.
    NoInput,
}

/// Try to move the player in a direction.
///
/// Returns a `MoveResult` indicating what happened.
pub fn try_move<M: MapTrait, T: TilesetTrait>(
    state: &mut GenericOverworldState<M>,
    direction: Direction,
    tileset: T,
    map_width_blocks: u8,
    map_height_blocks: u8,
    standing_tile: u8,
    target_tile: u8,
    npc_positions: &[SpritePosition],
    held_input: u8,
    provider: &impl CollisionProvider<T>,
) -> MoveResult {
    if state.player.movement_state != MovementState::Idle {
        return MoveResult::StillMoving;
    }

    let was_facing = state.player.facing;
    state.player.facing = direction;

    let result = check_movement_collision(
        state.player.x,
        state.player.y,
        direction,
        tileset,
        map_width_blocks,
        map_height_blocks,
        standing_tile,
        target_tile,
        state.player.transport,
        npc_positions,
        held_input,
        provider,
    );

    match result {
        CollisionResult::Passable | CollisionResult::StopSurfing => {
            // StopSurfing: the surfer stepped onto a passable land tile and
            // returns to walking (CollisionCheckOnWater .stopSurfing).
            if result == CollisionResult::StopSurfing {
                state.player.transport = TransportMode::Walking;
            }
            state.standing_on_warp = false;
            state.player.movement_state = MovementState::Walking;
            state.walk_counter = WALK_COUNTER_INIT;
            MoveResult::Walking
        }
        CollisionResult::LedgeJump => {
            state.standing_on_warp = false;
            state.player.movement_state = MovementState::Jumping;
            state.walk_counter = WALK_COUNTER_INIT * 2;
            MoveResult::LedgeJump
        }
        CollisionResult::MapEdge => MoveResult::ReachedMapEdge,
        _ => {
            if was_facing != direction {
                MoveResult::TurnedOnly
            } else {
                MoveResult::Blocked(result)
            }
        }
    }
}

/// Advance the player's position by one pixel-step during movement.
///
/// Returns true when the step is complete (walk counter reached 0).
pub fn advance_step<M: MapTrait>(state: &mut GenericOverworldState<M>) -> bool {
    if state.walk_counter == 0 {
        return true;
    }

    let decrement =
        if state.player.transport == TransportMode::Biking && state.player.bike_speedup_active {
            2
        } else {
            1
        };

    state.walk_counter = state.walk_counter.saturating_sub(decrement);

    if state.walk_counter == 0 {
        let (dx, dy) = direction_delta(state.player.facing);

        if state.player.movement_state == MovementState::Jumping {
            let new_x = (state.player.x as i32 + dx as i32 * 2) as u16;
            let new_y = (state.player.y as i32 + dy as i32 * 2) as u16;
            state.player.x = new_x;
            state.player.y = new_y;
        } else {
            let new_x = (state.player.x as i32 + dx as i32) as u16;
            let new_y = (state.player.y as i32 + dy as i32) as u16;
            state.player.x = new_x;
            state.player.y = new_y;
        }

        state.player.movement_state = MovementState::Idle;

        if state.encounter_cooldown > 0 {
            state.encounter_cooldown -= 1;
        }

        // NOTE: REPEL is intentionally NOT decremented here. In the classic
        // model the counter ticks inside the wild-encounter check itself
        // (pokered: wild_encounters.asm:19-25), which only runs when the
        // step may actually roll an encounter (not while warping, ledge
        // jumping, cooldown-active, or during scripted movement). Games
        // that want a plain per-step tick may call `tick_repel_step`.

        return true;
    }

    false
}

/// Decrement the REPEL counter by one (saturating at 0). Games call this from
/// their own encounter-roll gating, mirroring the classic TryDoWildEncounter
/// placement; see the note on [`advance_step`].
pub fn tick_repel_step<M: MapTrait>(state: &mut GenericOverworldState<M>) {
    if state.repel_steps > 0 {
        state.repel_steps -= 1;
    }
}

/// Get the x/y delta for a direction.
pub fn direction_delta(dir: Direction) -> (i8, i8) {
    match dir {
        Direction::Down => (0, 1),
        Direction::Up => (0, -1),
        Direction::Left => (-1, 0),
        Direction::Right => (1, 0),
    }
}

/// Get the opposite direction.
pub fn opposite_direction(dir: Direction) -> Direction {
    match dir {
        Direction::Down => Direction::Up,
        Direction::Up => Direction::Down,
        Direction::Left => Direction::Right,
        Direction::Right => Direction::Left,
    }
}

/// Calculate the number of frames for a step based on transport mode.
pub fn frames_per_step(transport: TransportMode) -> u8 {
    match transport {
        TransportMode::Walking => WALK_COUNTER_INIT,
        TransportMode::Biking => WALK_COUNTER_INIT / 2,
        TransportMode::Surfing => WALK_COUNTER_INIT,
    }
}

/// Convert Direction to the facing index (0=Down, 1=Up, 2=Left, 3=Right).
pub fn direction_to_facing_index(dir: Direction) -> u8 {
    match dir {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::Left => 2,
        Direction::Right => 3,
    }
}

/// Get the tile ID at a specific position in the map.
pub fn get_tile_at_position<M: MapTrait, T: TilesetTrait, Mus>(
    map: &MapData<M, T, Mus>,
    x: u16,
    y: u16,
    provider: &impl CollisionProvider<T>,
) -> u8 {
    provider.get_tile_at_position(map.tileset, &map.blocks, map.width, x, y)
}

pub fn get_target_tile_for_direction<M: MapTrait, T: TilesetTrait, Mus>(
    map: &MapData<M, T, Mus>,
    x: u16,
    y: u16,
    dir: Direction,
    provider: &impl CollisionProvider<T>,
) -> u8 {
    let (dx, dy) = direction_delta(dir);
    let target_x = ((x as i32) + dx as i32).max(0) as u16;
    let target_y = ((y as i32) + dy as i32).max(0) as u16;
    provider.get_tile_at_position(map.tileset, &map.blocks, map.width, target_x, target_y)
}

/// Extra-warp check (classic behavior).
pub fn extra_warp_check<M: MapTrait, T: TilesetTrait, Mus>(
    map: &MapData<M, T, Mus>,
    player_x: u16,
    player_y: u16,
    facing: Direction,
    provider: &impl CollisionProvider<T>,
) -> bool {
    // Check for game-specific special cases (e.g. SS_ANNE_BOW tile 0x15).
    let tile_in_front = get_target_tile_for_direction(map, player_x, player_y, facing, provider);
    if let Some(result) = provider.check_extra_warp_special(map.tileset, tile_in_front) {
        return result;
    }

    if provider.uses_warp_tile_in_front_check(map.tileset) {
        let facing_idx = direction_to_facing_index(facing);
        provider.is_warp_carpet_tile_in_front(map.tileset, facing_idx, tile_in_front)
    } else {
        is_facing_map_edge(player_x, player_y, facing, map.width, map.height)
    }
}

/// Two-phase warp check after a step completes onto a warp position.
pub fn check_warps_no_collision<M: MapTrait, T: TilesetTrait, Mus>(
    state: &mut GenericOverworldState<M>,
    map: &MapData<M, T, Mus>,
    standing_tile: u8,
    direction_held: bool,
    provider: &impl CollisionProvider<T>,
) -> Option<usize> {
    let warp_idx = check_warp_at_position(state.player.x, state.player.y, map)?;

    state.standing_on_warp = true;

    if provider.is_door_tile(map.tileset, standing_tile) {
        return Some(warp_idx);
    }

    if provider.is_warp_tile(map.tileset, standing_tile) {
        state.standing_on_warp = false;
        return Some(warp_idx);
    }

    if extra_warp_check(
        map,
        state.player.x,
        state.player.y,
        state.player.facing,
        provider,
    ) {
        if direction_held {
            return Some(warp_idx);
        }
    }

    None
}

/// CheckWarpsCollision path: when collision occurs while standing_on_warp is set.
pub fn check_collision_warp<M: MapTrait, T: TilesetTrait, Mus>(
    state: &mut GenericOverworldState<M>,
    map: &MapData<M, T, Mus>,
    move_result: MoveResult,
    provider: &impl CollisionProvider<T>,
) -> MoveResult {
    match move_result {
        MoveResult::Blocked(_) | MoveResult::ReachedMapEdge => {
            if state.standing_on_warp {
                if extra_warp_check(
                    map,
                    state.player.x,
                    state.player.y,
                    state.player.facing,
                    provider,
                ) {
                    if let Some(warp_idx) =
                        check_warp_at_position(state.player.x, state.player.y, map)
                    {
                        return MoveResult::Warped {
                            warp_index: warp_idx,
                        };
                    }
                }
            }
            move_result
        }
        _ => move_result,
    }
}

/// Process one frame of overworld movement.
///
/// This is the high-level frame-by-frame update, combining input
/// processing and step advancement.
pub fn process_frame<M: MapTrait, T: TilesetTrait, Mus>(
    state: &mut GenericOverworldState<M>,
    input: &InputState,
    map: &MapData<M, T, Mus>,
    standing_tile: u8,
    target_tile: u8,
    npc_positions: &[SpritePosition],
    provider: &impl CollisionProvider<T>,
) -> MoveResult {
    // If currently moving, advance the step
    if state.player.movement_state != MovementState::Idle {
        let step_done = advance_step(state);
        if step_done {
            let new_standing_tile =
                get_tile_at_position(map, state.player.x, state.player.y, provider);
            let direction_held = input.direction_pressed().is_some();

            if let Some(warp_idx) =
                check_warps_no_collision(state, map, new_standing_tile, direction_held, provider)
            {
                return MoveResult::Warped {
                    warp_index: warp_idx,
                };
            }

            if let Some(direction) = input.direction_pressed() {
                let held_input = input.to_pad_bits();

                let new_target_tile = get_target_tile_for_direction(
                    map,
                    state.player.x,
                    state.player.y,
                    direction,
                    provider,
                );

                let move_result = try_move(
                    state,
                    direction,
                    map.tileset,
                    map.width,
                    map.height,
                    new_standing_tile,
                    new_target_tile,
                    npc_positions,
                    held_input,
                    provider,
                );

                return check_collision_warp(state, map, move_result, provider);
            }
        }
        return MoveResult::StillMoving;
    }

    // Not moving — check for new input
    let direction = match input.direction_pressed() {
        Some(dir) => dir,
        None => return MoveResult::NoInput,
    };

    let held_input = input.to_pad_bits();

    let move_result = try_move(
        state,
        direction,
        map.tileset,
        map.width,
        map.height,
        standing_tile,
        target_tile,
        npc_positions,
        held_input,
        provider,
    );

    check_collision_warp(state, map, move_result, provider)
}
