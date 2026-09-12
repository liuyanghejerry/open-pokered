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
pub mod nearby;
pub mod profile;
pub mod snapshot;

pub use mode::{classify_mode, AgentMode, ModeInput, OverworldObs};
pub use nearby::{
    hidden_item_spots, nearby_entities, EntityKind, HiddenItemSpot, NearbyEntity, NpcObs,
    Position, DEFAULT_NEARBY_RADIUS, INTERACT_REACH,
};
pub use profile::{ObservationLevel, ObservationProfile};
pub use snapshot::{
    build_agent_snapshot, AgentSnapshot, BadgeSummary, BagItemSummary, BattleMonSummary,
    BattleSummary, ChoiceSummary, DialogueSummary, MapRef, ObservationSource, PartyMonSummary,
};
