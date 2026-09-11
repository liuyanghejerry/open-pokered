//! dotzuki-app: generic desktop app wrapper around dotzuki-renderer's game loop.
//!
//! Provides a reusable game loop, file-watching hot-reload, generic
//! rendering helpers, the native link-play transports + session router
//! ([`link`]), and the generic debug server ([`debug_server`]). This crate
//! has zero game-specific dependencies.

// `dotzuki_renderer::window` exists only off-wasm (it is gated on dotzuki-renderer's
// `gpu` feature, which is default-on, AND `not(target_arch = "wasm32")`). dotzuki-app
// has no features of its own, so gate this re-export on the target only — present on
// native desktop (where pokered-app uses it), absent on wasm (fixes E0432).
pub use dotzuki_renderer::input::{GbButton, InputState};
#[cfg(not(target_arch = "wasm32"))]
pub use dotzuki_renderer::window::{run, GameLoop, GameWindowConfig};
pub use dotzuki_renderer::*;

// The debug server is TCP-based (std::net), so native-only — same gating
// convention as the `window` re-export above.
#[cfg(not(target_arch = "wasm32"))]
pub mod debug_server;
pub mod hot_reload;
pub mod link;
pub mod render_helpers;
