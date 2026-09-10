//! Generic NPC movement system for the overworld.
//!
//! Implements NPC movement: per-frame sprite updates for all NPCs and the
//! trainer-notice (emotion bubble) animation.
//!
//! All functions are generic over tileset and collision provider types.

use alloc::collections::VecDeque;

use crate::map::MapTrait;
use crate::tileset::TilesetTrait;

use super::collision::{CollisionProvider, SpritePosition};
use super::player_movement::direction_delta;
use super::types::{Direction, MapData, NpcMovementType, NpcWanderAxis};

// ── NPC Runtime State ──────────────────────────────────────────────

/// Runtime state for a single NPC on the current map.
///
/// Game-specific data (e.g., trainer flags, item drops) should be stored
/// in a separate parallel array and looked up by `npc_index`.
#[derive(Debug, Clone)]
pub struct NpcRuntimeState {
    pub npc_index: u8,
    pub sprite_id: u8,
    pub x: u16,
    pub y: u16,
    pub home_x: u16,
    pub home_y: u16,
    pub facing: Direction,
    pub scripted_frame: Option<u8>,
    pub movement_type: NpcMovementType,
    /// Axis restriction for Wander NPCs (classic GB movement byte 2).
    pub wander_axis: NpcWanderAxis,
    pub range: u8,
    pub walk_counter: u8,
    pub delay_counter: u8,
    pub text_id: u8,
    pub defeated: bool,
    pub visible: bool,
    pub scripted_path: VecDeque<(u16, u16)>,
}

// ── Constants ──────────────────────────────────────────────────────

/// Frames to walk one tile. The classic GB walkers take $10 frames per tile —
/// HALF the player's speed (WALKANIMATIONCOUNTER = $10, movement.asm:296-339).
pub const NPC_WALK_FRAMES: u8 = 16;
/// Maximum delay between random NPC movements: `Random & $7F` ∈ 0..=127
/// (movement.asm:352-361; a rolled 0 becomes 256 — that quirk is preserved
/// by callers as an immediate re-roll, matching the original's wrap).
pub const NPC_MAX_DELAY: u8 = 127;

// ── Helpers ────────────────────────────────────────────────────────

/// Determine direction from `from` toward `to`.
///
/// Uses the axial heuristic: if dx.abs() > dy.abs(), horizontal;
/// otherwise vertical. Returns `None` if both positions are the same.
pub fn direction_toward(from_x: u16, from_y: u16, to_x: u16, to_y: u16) -> Option<Direction> {
    let dx = to_x as i32 - from_x as i32;
    let dy = to_y as i32 - from_y as i32;
    if dx == 0 && dy == 0 {
        return None;
    }
    if dx.abs() > dy.abs() {
        Some(if dx > 0 {
            Direction::Right
        } else {
            Direction::Left
        })
    } else {
        Some(if dy > 0 {
            Direction::Down
        } else {
            Direction::Up
        })
    }
}

// ── Scripted Movement ──────────────────────────────────────────────

/// Start an NPC walking a fixed path of tile coordinates.
pub fn start_scripted_move(npc: &mut NpcRuntimeState, path: &[(u8, u8)]) {
    npc.scripted_path.clear();
    for &(x, y) in path {
        npc.scripted_path.push_back((x as u16, y as u16));
    }
}

/// Returns `true` when the NPC has finished its scripted path and is idle.
pub fn is_scripted_move_done(npc: &NpcRuntimeState) -> bool {
    npc.scripted_path.is_empty() && npc.walk_counter == 0
}

// ── Position Utilities ─────────────────────────────────────────────

/// Collect tile positions of all visible NPCs for collision checks.
pub fn get_npc_positions(npcs: &[NpcRuntimeState]) -> Vec<SpritePosition> {
    npcs.iter()
        .filter(|n| n.visible)
        .map(|n| SpritePosition { x: n.x, y: n.y })
        .collect()
}

// ── NPC Lookup ─────────────────────────────────────────────────────

/// Find a visible NPC at the given tile position.
pub fn npc_at_position(npcs: &[NpcRuntimeState], x: u16, y: u16) -> Option<&NpcRuntimeState> {
    npcs.iter().find(|n| n.visible && n.x == x && n.y == y)
}

/// Mutable version of [`npc_at_position`].
pub fn npc_at_position_mut(
    npcs: &mut [NpcRuntimeState],
    x: u16,
    y: u16,
) -> Option<&mut NpcRuntimeState> {
    npcs.iter_mut().find(|n| n.visible && n.x == x && n.y == y)
}

// ── NPC Update Loop ────────────────────────────────────────────────

/// Update all NPC movement for one frame.
///
/// This is the main entry point called every frame from the overworld loop
/// (equivalent to `DoMovementForAllSprites` in the original game).
///
/// Each NPC that is visible, not mid-step, and not on a scripted path is
/// updated based on its movement type (Stationary, Wander, FacePlayer, or FixedPath).
///
/// # Parameters
/// - `npcs` — mutable slice of NPC runtime states
/// - `player_x`, `player_y` — player's current tile position
/// - `player_dest` — player's destination if mid-step (for collision avoidance)
/// - `map_width_blocks`, `map_height_blocks` — map dimensions in blocks
/// - `rng_value` — random value for wander direction/delay
/// - `blocks` — block data for the current map
/// - `tileset` — current tileset
/// - `provider` — collision provider for tile passability
pub fn update_npc_movement<T: TilesetTrait>(
    npcs: &mut [NpcRuntimeState],
    player_x: u16,
    player_y: u16,
    player_dest: Option<(u16, u16)>,
    map_width_blocks: u8,
    map_height_blocks: u8,
    rng_value: u8,
    blocks: &[u8],
    tileset: T,
    provider: &impl CollisionProvider<T>,
) {
    let max_x = (map_width_blocks as u16) * 2;
    let max_y = (map_height_blocks as u16) * 2;

    // Build the occupied-tile set: current position of each visible NPC,
    // plus its destination if it is mid-step (to avoid inter-NPC collisions).
    let occupied: Vec<(u16, u16)> = npcs
        .iter()
        .filter(|n| n.visible)
        .flat_map(|n| {
            let cur = (n.x, n.y);
            if n.walk_counter > 0 {
                let (dx, dy) = direction_delta(n.facing);
                let dest = (
                    (n.x as i32 + dx as i32).max(0) as u16,
                    (n.y as i32 + dy as i32).max(0) as u16,
                );
                vec![cur, dest]
            } else {
                vec![cur]
            }
        })
        .collect();

    for i in 0..npcs.len() {
        let npc = &mut npcs[i];
        if !npc.visible {
            continue;
        }

        // ── Finish current step ──────────────────────────────────
        if npc.walk_counter > 0 {
            npc.walk_counter -= 1;
            if npc.walk_counter == 0 {
                let (dx, dy) = direction_delta(npc.facing);
                npc.x = (npc.x as i32 + dx as i32).max(0) as u16;
                npc.y = (npc.y as i32 + dy as i32).max(0) as u16;

                // Advance scripted path if we reached the next waypoint
                if !npc.scripted_path.is_empty() {
                    let &(tx, ty) = npc.scripted_path.front().unwrap();
                    if npc.x == tx && npc.y == ty {
                        npc.scripted_path.pop_front();
                    }
                    if let Some(&(ntx, nty)) = npc.scripted_path.front() {
                        if let Some(dir) = direction_toward(npc.x, npc.y, ntx, nty) {
                            npc.facing = dir;
                            npc.walk_counter = NPC_WALK_FRAMES;
                        }
                    }
                }
            }
            continue;
        }

        // ── Scripted path: start next step ───────────────────────
        if !npc.scripted_path.is_empty() {
            let &(tx, ty) = npc.scripted_path.front().unwrap();
            if npc.x == tx && npc.y == ty {
                npc.scripted_path.pop_front();
                if npc.scripted_path.is_empty() {
                    continue;
                }
                let &(tx, ty) = npc.scripted_path.front().unwrap();
                if let Some(dir) = direction_toward(npc.x, npc.y, tx, ty) {
                    npc.facing = dir;
                    npc.walk_counter = NPC_WALK_FRAMES;
                }
            } else if let Some(dir) = direction_toward(npc.x, npc.y, tx, ty) {
                npc.facing = dir;
                npc.walk_counter = NPC_WALK_FRAMES;
            }
            continue;
        }

        // ── Autonomous movement by type ──────────────────────────
        match npc.movement_type {
            NpcMovementType::Stationary => {}
            NpcMovementType::Wander => {
                if npc.delay_counter > 0 {
                    npc.delay_counter -= 1;
                    continue;
                }

                // Pick a random direction from the lower 2 bits of rng_value,
                // filtered by the classic axis byte (movement byte 2:
                // UP_DOWN $01 → vertical only, LEFT_RIGHT $02 → horizontal
                // only, ANY $00 → all four — movement.asm:195-251). An
                // axis-off roll re-rolls the delay instead of moving.
                let dir_bits = (rng_value.wrapping_add(i as u8)) & 0x03;
                let dir = match dir_bits {
                    0 => Direction::Down,
                    1 => Direction::Up,
                    2 => Direction::Left,
                    3 => Direction::Right,
                    _ => unreachable!(),
                };
                let axis_ok = match npc.wander_axis {
                    NpcWanderAxis::Any => true,
                    NpcWanderAxis::Vertical => {
                        dir == Direction::Up || dir == Direction::Down
                    }
                    NpcWanderAxis::Horizontal => {
                        dir == Direction::Left || dir == Direction::Right
                    }
                };
                if !axis_ok {
                    npc.delay_counter = rng_value & NPC_MAX_DELAY;
                    continue;
                }

                let (dx, dy) = direction_delta(dir);
                let tx = (npc.x as i32 + dx as i32) as u16;
                let ty = (npc.y as i32 + dy as i32) as u16;

                // Bounds check. (No radial leash: classic random walkers have
                // none — they walk until blocked.)
                if tx >= max_x || ty >= max_y {
                    npc.facing = dir;
                    npc.delay_counter = rng_value & NPC_MAX_DELAY;
                    continue;
                }

                // Check occupied tiles (other NPCs)
                let blocked = occupied
                    .iter()
                    .any(|&(ox, oy)| !(ox == npc.x && oy == npc.y) && ox == tx && oy == ty);
                let player_blocked = (tx == player_x && ty == player_y)
                    || player_dest.map_or(false, |(px, py)| tx == px && ty == py);

                if blocked || player_blocked {
                    npc.facing = dir;
                    npc.delay_counter = rng_value & NPC_MAX_DELAY;
                    continue;
                }

                // Check tile passability
                let target_tile =
                    provider.get_tile_at_position(tileset, blocks, map_width_blocks, tx, ty);
                if !provider.is_tile_passable(tileset, target_tile) {
                    npc.facing = dir;
                    npc.delay_counter = rng_value & NPC_MAX_DELAY;
                    continue;
                }

                npc.facing = dir;
                npc.walk_counter = NPC_WALK_FRAMES;
                npc.delay_counter = rng_value & NPC_MAX_DELAY;
            }
            NpcMovementType::FacePlayer => {
                let dx = player_x as i32 - npc.x as i32;
                let dy = player_y as i32 - npc.y as i32;

                if dx.abs() >= dy.abs() {
                    npc.facing = if dx > 0 {
                        Direction::Right
                    } else {
                        Direction::Left
                    };
                } else {
                    npc.facing = if dy > 0 {
                        Direction::Down
                    } else {
                        Direction::Up
                    };
                }
            }
            NpcMovementType::FixedPath => {}
        }
    }
}

// ── NPC-in-Front Check ─────────────────────────────────────────────

/// Find the NPC the player is facing, accounting for counter tiles.
///
/// In the original game, pressing A on a counter tile extends the
/// interaction range by one tile to allow talking to NPCs across
/// counters (e.g. shopkeepers in Poké Marts).
pub fn npc_in_front_of_player<'a, M: MapTrait, T: TilesetTrait, Mus>(
    npcs: &'a [NpcRuntimeState],
    player_x: u16,
    player_y: u16,
    facing: Direction,
    map: Option<&MapData<M, T, Mus>>,
    provider: &impl CollisionProvider<T>,
) -> Option<&'a NpcRuntimeState> {
    let (dx, dy) = direction_delta(facing);
    let target_x = (player_x as i32 + dx as i32) as u16;
    let target_y = (player_y as i32 + dy as i32) as u16;

    if let Some(npc) = npc_at_position(npcs, target_x, target_y) {
        return Some(npc);
    }

    // Counter tile extension: if the tile in front is a counter,
    // check one more tile in the same direction for an NPC behind it.
    if let Some(map_data) = map {
        let tile = provider.get_tile_at_position(
            map_data.tileset,
            &map_data.blocks,
            map_data.width,
            target_x,
            target_y,
        );
        if provider.is_counter_tile(map_data.tileset, tile) {
            let extended_x = (target_x as i32 + dx as i32) as u16;
            let extended_y = (target_y as i32 + dy as i32) as u16;
            return npc_at_position(npcs, extended_x, extended_y);
        }
    }

    None
}
