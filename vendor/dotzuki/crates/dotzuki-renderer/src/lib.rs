// dotzuki-renderer: General-purpose JRPG rendering library built from
// Game Boy tile rendering principles.
//
// This is NOT a Game Boy hardware emulator. It provides a higher-level
// rendering API that draws into a 160×144 pixel framebuffer and displays
// it via a scaled window using the `pixels` crate.

// no_std port (GBA / thumbv4t): the `framebuffer` feature build (no gpu,
// no resource) is bare-metal clean — the window/pixels path, PNG asset
// loading, and disk layout loading stay hosted-only behind their features
// or `target_os` gates.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", feature(prelude_import))]
#![cfg_attr(target_os = "none", allow(internal_features))]

extern crate alloc;

#[allow(unused_imports)]
mod alloc_prelude {
    pub use core::prelude::v1::*;
    pub use core::convert::{TryFrom, TryInto};
    pub use alloc::borrow::ToOwned;
    pub use core::iter::FromIterator;
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
    pub use core::{assert_eq, assert_ne, matches, todo, unimplemented, write, writeln};
    pub use core::debug_assert;
}

#[cfg_attr(target_os = "none", prelude_import)]
#[allow(unused_imports)]
use alloc_prelude::*;

pub mod asset_provider;
pub mod battle_anim;
pub mod battle_scene;
pub mod battle_transition;
pub mod charmap;
pub mod embedded_font;
pub mod icon;
pub mod indexed_framebuffer;
pub mod input;
pub mod layer_renderer;
pub mod layout;
pub mod layout_engine;
pub mod menu;
#[cfg(feature = "gpu")]
pub mod mon_icon;
pub mod palette;
#[cfg(any(feature = "gpu", target_os = "none"))]
pub mod party_hp_bar;
#[cfg(feature = "resource")]
pub mod resource;
pub mod sprite;
pub mod text_renderer;
pub mod textbox;
pub mod tile;
pub mod tilemap;
pub mod title;
pub mod transition;
pub mod walk_sprite;
#[cfg(all(feature = "gpu", not(target_arch = "wasm32")))]
pub mod window;
pub mod window_layer;

pub use dotzuki_engine::render::{DirtyRegion, FrameBuffer, Rgba, BYTES_PER_PIXEL, TILE_SIZE};
pub use dotzuki_engine::render_config::RenderConfig;
pub use indexed_framebuffer::{
    index_bits, packed_len, quantize, DefaultPalette, FbSurface, IndexedFrameBuffer,
    RgbaIndexedFrameBuffer, SCREEN_HEIGHT, SCREEN_WIDTH,
};

#[cfg(test)]
mod tests;
