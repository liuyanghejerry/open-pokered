// pokered-renderer: Graphics rendering layer for Pokémon Red/Blue Rust rewrite
//
// This is NOT a Game Boy hardware emulator. It provides a higher-level
// rendering API that draws into a 160×144 pixel framebuffer and displays
// it via a scaled window using the `pixels` crate.
//
// Most modules are re-exported from the generic `jrpg-renderer` crate.
// Pokemon-specific modules are defined here.

// Re-export everything from jrpg-renderer (all generic rendering modules)
pub use jrpg_renderer::*;

// Pokemon-specific modules (not in jrpg-renderer)
pub mod embedded;
#[cfg(feature = "gpu")]
pub mod resource;

#[cfg(test)]
mod tests;
