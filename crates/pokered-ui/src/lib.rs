// Dual-target crate: hosted builds keep full std via the extern crate below;
// bare-metal GBA builds compile against core + alloc only.
#![no_std]

#[cfg(not(target_os = "none"))]
#[macro_use]
extern crate std;
extern crate alloc;

pub mod backends;
pub mod custom_elements;
mod engine;
pub mod menus;
pub mod v2;

pub use engine::{
    BracketSides, DamageRect, Frame, InkColor, LabelValue, Painter, Rgba, TilePos, TileRect, Ui,
};

pub use pokered_data::{SCREEN_HEIGHT_PX, SCREEN_WIDTH_PX, TILE_SIZE_PX};

// Internal std::sync shims (see pokered-data's twin).
pub(crate) mod sync_compat;

// With `#![no_std]` the Vec/String/Box/vec!/format! family leaves the prelude
// on BOTH targets. Modules glob-import this to keep using them unqualified.
#[allow(unused_imports)]
pub(crate) mod alloc_prelude {
    pub use alloc::borrow::{Cow, ToOwned};
    pub use alloc::boxed::Box;
    pub use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
    pub use alloc::format;
    pub use alloc::rc::Rc;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}
