//! Semantic observation layer for AI agents (milestone M1).
//!
//! Distills the live game state into a typed, wire-stable snapshot:
//! the high-level [`AgentMode`] classifier (overworld / dialogue / menu /
//! battle / transition), a party/bag/badge summary, dialogue and battle
//! detail, and the [`NearbyEntity`] aggregation (NPCs, trainers, item
//! balls, signs, warps, hidden items) around the player.
//!
//! Everything here is pure: functions take references into `pokered-core`
//! runtime state plus static `pokered-data` tables and never mutate
//! anything. The debug-server command surface (`get_agent_state` /
//! `get_nearby`) lives in `pokered-app`; only the profile/level
//! vocabulary crosses the wire from `pokered-debug-server`.
//!
//! Coordinates everywhere are **step units** (1 step = 2 GB tiles = half
//! a block); distances are Manhattan in step units.

pub mod mode;
pub mod nav;
pub mod nearby;
pub mod profile;
pub mod semantics;
pub mod snapshot;
pub mod world;

pub use mode::{classify_mode, AgentMode, ModeInput, OverworldObs};
pub use nav::{
    direction_between, find_approach, find_path, InteractOutcome, InteractResult, NavGrid,
    NavigationOutcome, NavigationResult,
};
pub use semantics::{
    extract_map_semantics, generate_event_graph_json, generate_world_semantics, BattleKind,
    CoverageReport, EdgeKind, EventEdge, EventGraph, MapSemantics, ScriptSemantics, StateEffect,
    StatePredicate, WorldSemantics,
};
pub use world::{
    find_tile_route, RouteLeg, RouteLegKind, TileCross, TileLeg, TravelOutcome, TravelResult,
    WorldEdge, WorldGraph,
};
pub use nearby::{
    hidden_item_spots, nearby_entities, EntityKind, HiddenItemSpot, NearbyEntity, NpcObs,
    Position, DEFAULT_NEARBY_RADIUS, INTERACT_REACH,
};
pub use profile::{ObservationLevel, ObservationProfile};
pub use snapshot::{
    build_agent_snapshot, AgentSnapshot, BadgeSummary, BagItemSummary, BattleMonSummary,
    BattleSummary, ChoiceSummary, DialogueSummary, MapRef, ObservationSource, PartyMonSummary,
};
