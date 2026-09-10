//! Battle animation system — subanimations, frame blocks, and special effects.
//!
//! Faithful port of the original pokered battle animation engine:
//!   - the main driver — playing animations and subanimations, drawing
//!     frame blocks, the special-effect routines and shared move animations
//!   - per-move animation command sequences
//!   - 86 subanimation definitions
//!   - 122 frame block OAM tile layouts (stored as pixel offsets)
//!   - 177 (Y,X) base coordinate pairs
//!   - per-animation-id frame hooks (the special-effect dispatch table),
//!     evaluated after every drawn frame block
//!   - `effects.rs` — the framebuffer special-effect (SE) state machines
//!     shared by the pokered frontends (substitute doll, minimize blob,
//!     squish, HUD shake, spiral/shoot balls, petals/leaves/droplets, …)
//!
//! This is NOT a hardware OAM emulator. We model the animation data at a
//! higher level: each "OAM tile" becomes a positioned sprite tile in the
//! framebuffer, and special effects are translated to palette/scroll/sprite
//! operations on our `FrameBuffer` / `SpriteLayer` abstractions.

mod data;
mod effects;
mod player;
mod types;

pub use data::*;
pub use effects::*;
pub use player::*;
pub use types::*;
