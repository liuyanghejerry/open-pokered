// Dual-target crate: hosted builds (macOS/…) keep full std via the extern
// crate below; bare-metal GBA builds (thumbv4t-none-eabi, target_os = "none")
// compile against core + alloc only.
#![no_std]

// `#[macro_use]` so hosted builds also get std's macro surface (thread_local!,
// eprintln!, …) — with plain `extern crate std` those macro names don't resolve.
#[cfg(not(target_os = "none"))]
#[macro_use]
extern crate std;
extern crate alloc;

pub use pokered_data as data;

pub mod battle;
pub mod credits;
pub mod debug_log;
pub mod events;
pub mod evolution_screen;
pub mod game_state;
pub mod gamefreak_splash;
pub mod hof_ceremony;
pub mod intro_scene;
pub mod items;
pub mod link;
pub mod main_menu;
pub mod naming_screen;
pub mod oak_speech;
pub mod pc_screen;
pub mod options_menu;
pub mod overworld;
pub mod bag_screen;
pub mod party_screen;
pub mod party_select;
pub mod pokedex_screen;
pub mod rng;
pub mod hash_compat;
pub mod stats_screen;
pub mod pokemon;
pub mod save;
pub mod save_menu;
pub mod slots;
pub mod slots_screen;
pub mod elevator_screen;
pub mod start_menu;
pub mod text;
pub mod title_screen;
pub mod town_map_screen;
pub mod trade;
pub mod trainer_card_screen;

// Internal std::sync shims (see pokered-data's twin): hosted keeps std's
// OnceLock/LazyLock/Mutex, bare metal swaps in spin-backed equivalents.
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

#[cfg(test)]
mod main_menu_tests;

#[cfg(test)]
mod naming_screen_tests;

#[cfg(test)]
mod options_menu_tests;

#[cfg(test)]
mod save_menu_tests;

#[cfg(test)]
mod start_menu_tests;

#[cfg(test)]
mod title_screen_tests;
