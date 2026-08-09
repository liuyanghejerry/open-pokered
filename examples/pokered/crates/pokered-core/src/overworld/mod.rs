//! Overworld system — map loading, player movement, collision, and map connections.
//!
//! Implements M4.1 (地图加载和瓦片渲染) and M4.2 (玩家移动和碰撞检测)
//! of the Rust rewrite plan. This module provides the core data types,
//! loading functions, collision detection, and player movement for
//! the game's overworld map system.

pub mod collision;
pub mod doors_elevators;
pub mod event_flags;
pub mod field_moves;
pub mod fishing;
pub mod forced_bike;
pub mod hidden_items;
pub mod hm_effects;
pub mod map_data_loading;
pub mod map_loading;
pub mod npc_interaction;
pub mod npc_movement;
pub mod player_movement;
pub mod presentation;
pub mod script_bridge;
pub mod special_terrain;
pub mod sprites;
pub mod trainer_engine;
pub mod wild_encounters;
pub mod screen;
pub mod update;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_collision;
#[cfg(test)]
mod tests_cutscene_movement;
#[cfg(test)]
mod tests_connections;
#[cfg(test)]
mod tests_doors_elevators;
#[cfg(test)]
mod tests_field_items;
#[cfg(test)]
mod tests_forced_bike;
#[cfg(test)]
mod tests_fishing;
#[cfg(test)]
mod tests_hidden_items;
#[cfg(test)]
mod tests_field_moves;
#[cfg(test)]
mod tests_guide_features;
#[cfg(test)]
mod tests_hm_effects;
#[cfg(test)]
mod tests_link_presence;
#[cfg(test)]
mod tests_movement;
#[cfg(test)]
mod tests_npc;
#[cfg(test)]
mod tests_presentation;
#[cfg(test)]
mod tests_oak_event;
#[cfg(test)]
mod tests_oaks_lab;
#[cfg(test)]
mod tests_scripts;
#[cfg(test)]
mod tests_special_terrain;
#[cfg(test)]
mod tests_wild_encounters;

// Re-export engine types for submodules and tests.
pub use jrpg_engine::overworld::{
    Direction, MapConnection, MapConnections, MovementState, NpcDefinition,
    NpcMovementType, OverworldInput, PlayerState, Sign, TransportMode, WarpPoint,
};

// Re-export all public items from screen.rs so external
// code (`pokered-app`, `pokered-tui`) continues to work unchanged.
pub use screen::*;
