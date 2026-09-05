// pokered-renderer: Graphics rendering layer for Pokémon Red/Blue Rust rewrite
//
// This is NOT a Game Boy hardware emulator. It provides a higher-level
// rendering API that draws into a 160×144 pixel framebuffer and displays
// it via a scaled window using the `pixels` crate.
//
// Most modules are re-exported from the generic `dotzuki-renderer` crate.
// Pokemon-specific modules are defined here.

// Re-export everything from dotzuki-renderer (all generic rendering modules)
pub use dotzuki_renderer::*;

use dotzuki_renderer::palette::GbColor;

// Pokemon-specific framebuffer: the indexed 4-shade buffer with an RGBA
// facade. All pokered draw code keeps calling set_pixel(Rgba)/fill_rect/
// clear on `FrameBuffer`; writes quantize through the grayscale base
// palette (exact for the pokered render chain), and the 160×144 storage
// is 5,760 bytes of packed 2bpp instead of 92,160 bytes of RGBA.
// The engine's RGBA `dotzuki_renderer::FrameBuffer` stays available for the
// true-color paths (e.g. the `--demo` tileset demo).
pub type FrameBuffer = dotzuki_renderer::RgbaIndexedFrameBuffer<GbColor>;

// Pokemon-specific modules (not in dotzuki-renderer)
pub mod embedded;
// Pokered asset-name override of the engine's `mon_icon` loader — shadows the
// glob re-export above so `pokered_renderer::mon_icon::load_mon_icon_tiles`
// resolves the party icons against the real pret/pokered gfx files.
pub mod mon_icon;
#[cfg(feature = "gpu")]
pub mod resource;

#[cfg(test)]
mod tests;
