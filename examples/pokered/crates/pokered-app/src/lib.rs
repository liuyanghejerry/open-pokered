//! Pokémon Red/Blue - App Library
//!
//! This crate provides the core game logic shared between native and web builds.

pub mod battle_config;
pub mod game;
pub mod render;
pub mod save_editor;

pub mod audio;

// Link play works on all targets: the session/router (session.rs), the Cable
// Club flow (cable_club.rs) and the framing codec (codec.rs) are pure
// mpsc/serde. Only the transports are platform-specific — TCP (transport.rs)
// is native-only, BroadcastChannel (broadcast_channel.rs) is wasm-only.
pub mod link;

#[cfg(not(target_arch = "wasm32"))]
pub mod direct_battle;

#[cfg(not(target_arch = "wasm32"))]
pub mod tools;

#[cfg(not(target_arch = "wasm32"))]
pub mod cli;

#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
pub mod hot_reload;

pub use game::PokemonGame;
pub use render::BattleVisualEffects;
