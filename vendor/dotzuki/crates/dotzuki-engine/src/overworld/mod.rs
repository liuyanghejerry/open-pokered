//! Overworld systems, in **two tiers** — pick the one that matches the game:
//!
//! - **[`actor`] — simple, render- & map-agnostic.** `OverworldActor` + the tiny
//!   `OverworldCollision` trait (`is_blocked(i32, i32)`): held-direction → tile step
//!   → walk animation, and nothing else. For lightweight, non-Game-Boy games whose
//!   maps aren't block/tileset based — e.g. the wuxia game and the minimon example,
//!   which plug in their own render backend (GB tiles vs a full-colour sprite sheet).
//!   The actor returns frame *indices*, never pixels.
//!
//! - **[`player_movement`] + [`collision`] + [`map_transitions`] + [`sprites`] —
//!   rich, Game-Boy-faithful.** `try_move`/`advance_step` over `MapTrait` +
//!   `TilesetTrait` with `u8` / 4×4-block coordinates, modelling the *full* classic JRPG
//!   overworld: ledge jumps, warps, map-edge connections, tileset-aware collision,
//!   OAM sprites, NPC movement scripts, wild encounters. Used by pokered (the
//!   flagship), which drives it directly (`pokered-core::overworld` just re-exports).
//!   [`presentation`] complements this tier with pure frame-counted animation
//!   state machines (teleport spins, elevator shake, water/flower tile
//!   animation, fishing rod, boulder dust, ship departure).
//!
//! Both tiers share [`types::Direction`]. They are intentionally **separate**: the
//! rich tier is a strict superset of behaviour, so collapsing the flagship onto the
//! simple actor would *lose* fidelity for no gain. New simple games use `actor`;
//! JRPG-grade games use the rich tier.

pub mod actor;
pub mod collision;
pub mod encounter;
pub mod event_flags;
pub mod map_transitions;
pub mod npc_interaction;
pub mod npc_movement;
pub mod player_movement;
pub mod presentation;
pub mod sprites;
pub mod types;

pub use collision::{
    check_movement_collision, check_sprite_collision, check_warp_at_position,
    direction_to_pad_input, direction_to_sprite_facing, get_block_at, get_target_coords,
    is_facing_map_edge, CollisionProvider, CollisionResult, SpritePosition, PAD_DOWN, PAD_LEFT,
    PAD_RIGHT, PAD_UP, SPRITE_FACING_DOWN, SPRITE_FACING_LEFT, SPRITE_FACING_RIGHT,
    SPRITE_FACING_UP,
};
pub use encounter::{EncounterEngine, EncounterMode, EncounterProvider, EncounterStep};
pub use npc_interaction::{
    check_line_of_sight, check_sign_interaction, mark_defeated, try_interact, InteractionResult,
    LineOfSightResult,
};
pub use npc_movement::{
    direction_toward, get_npc_positions, is_scripted_move_done, npc_at_position,
    npc_at_position_mut, npc_in_front_of_player, start_scripted_move, update_npc_movement,
    NpcRuntimeState, NPC_MAX_DELAY, NPC_WALK_FRAMES,
};
pub use player_movement::{
    advance_step, direction_delta, frames_per_step, get_tile_at_position, opposite_direction,
    process_frame, try_move, InputState, MoveResult, WALK_COUNTER_INIT,
};
pub use types::{
    Direction, MapConnection, MapConnections, MapData, MovementState, NpcDefinition,
    NpcMovementType, NpcWanderAxis, OverworldInput, OverworldState, PlayerState, Sign,
    TransportMode, WarpPoint,
};
