//! M4 static `.scene` semantic analysis: walk every map's compiled scene
//! AST and extract machine-readable state semantics per storyline —
//! which flags it reads, which state it changes (flags, items, Pokémon,
//! badges, battles, warps, objects, tiles) — with explicit `Unknown`
//! markers for anything unrecognized. Incompleteness is acceptable;
//! invented semantics is not.
//!
//! Modules: [`types`] (schema), [`extract`] (AST walker), [`graph`]
//! (event graph), [`gen`] (deterministic artifact generation).

pub mod extract;
pub mod gen;
pub mod graph;
pub mod types;

pub use extract::{extract_map_semantics, extract_storyline};
pub use gen::{generate_event_graph_json, generate_world_semantics, WorldSemantics};
pub use graph::{EdgeKind, EventEdge, EventGraph};
pub use types::{
    BattleKind, CoverageReport, MapSemantics, ScriptSemantics, StateEffect, StatePredicate,
    UnknownConstruct,
};
