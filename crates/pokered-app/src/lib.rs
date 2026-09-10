//! Pokémon Red/Blue - App Library
//!
//! This crate provides the core game logic shared between native and web
//! builds. Dual-target: hosted builds keep full std; bare-metal GBA builds
//! (`target_os = "none"`) compile against core + alloc only — the CLI, link
//! play, hot-reload, tooling and device audio are hosted-only there.

#![no_std]

#[cfg(not(target_os = "none"))]
#[macro_use]
extern crate std;
extern crate alloc;

pub mod battle_config;
pub mod game;
pub mod render;

#[cfg(not(target_os = "none"))]
pub mod save_editor;

pub mod audio;

// Link play works on hosted targets only: the session/router and codec are
// pure mpsc/serde (std channels), the transports are TCP (native) or
// BroadcastChannel (wasm). Bare metal has no link layer.
#[cfg(not(target_os = "none"))]
pub mod link;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
pub mod direct_battle;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
pub mod tools;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))]
pub mod cli;

#[cfg(all(debug_assertions, not(target_arch = "wasm32"), not(target_os = "none")))]
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


