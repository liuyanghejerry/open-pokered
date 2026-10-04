//! Pokémon Red/Blue - App Library
//!
//! This crate provides the core game logic shared between native and web
//! builds. Dual-target: hosted builds keep full std; bare-metal GBA builds
//! (`target_os = "none"`) compile against core + alloc only — the CLI, link
//! play, hot-reload, tooling and device audio are hosted-only there.

// `game::debug_state_snapshot` builds one large `serde_json::json!` block whose
// expansion depth tracks its top-level field count; it sits at the default
// limit of 128, so adding a single field to it fails to compile. Large nested
// values (`evaluation`, `battle_live`) are hoisted out of that macro, and the
// ceiling is raised so the next field added does not break the build.
#![recursion_limit = "256"]
#![no_std]

#[cfg(not(target_os = "none"))]
#[macro_use]
extern crate std;
extern crate alloc;

pub mod battle_config;
pub mod game;
#[cfg(feature = "ewram-audit")]
pub mod mem_audit;

#[cfg(feature = "ewram-audit")]
#[global_allocator]
static EWRAM_AUDIT_ALLOCATOR: mem_audit::AuditAllocator = mem_audit::AuditAllocator;

pub mod render;

#[cfg(not(target_os = "none"))]
pub mod save_editor;

// The pokered-agent observation/navigation layer is hosted-only (it pulls
// the DSL semantics stack); bare-metal (GBA) builds exclude it.
#[cfg(not(target_os = "none"))]
pub mod agent_nav;
#[cfg(not(target_os = "none"))]
pub mod agent_state;
#[cfg(not(target_os = "none"))]
pub mod agent_travel;
pub mod audio;

// Link play works on hosted targets only: the session/router and codec are
// pure mpsc/serde (std channels), the transports are TCP (native) or
// BroadcastChannel (wasm). Bare metal has no link layer.
#[cfg(not(target_os = "none"))]
pub mod link;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
pub mod direct_battle;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
pub mod tools;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32"), not(target_os = "none")))]
pub mod cli;

#[cfg(all(feature = "desktop", debug_assertions, not(target_arch = "wasm32"), not(target_os = "none")))]
pub mod hot_reload;

// Vec/String/vec!/format! prelude shim (see pokered-data's twin).
#[allow(unused_imports)]
pub(crate) mod alloc_prelude {
    pub use alloc::borrow::{Cow, ToOwned};
    pub use alloc::boxed::Box;
    pub use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}

pub use game::PokemonGame;
pub use render::BattleVisualEffects;

