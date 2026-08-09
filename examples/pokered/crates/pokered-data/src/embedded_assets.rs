//! Compile-time embedded asset data for `.tmx` and `.js` files.
//!
//! In **release** builds the content is baked into the binary via `include_str!`
//! so no filesystem I/O is required at runtime.
//!
//! In **debug** builds the tables are empty; the caller should load from the
//! filesystem instead, which enables hot-reload during development.

// ── Include generated data (release only) ───────────────────────────────────

#[cfg(not(debug_assertions))]
include!(concat!(env!("OUT_DIR"), "/tmx_assets_gen.rs"));

#[cfg(not(debug_assertions))]
include!(concat!(env!("OUT_DIR"), "/script_assets_gen.rs"));

// ── Debug stubs ─────────────────────────────────────────────────────────────

/// Empty stubs used in debug builds so the rest of the crate compiles even
/// when `tmx_assets_gen.rs` / `script_assets_gen.rs` are not included.
#[cfg(debug_assertions)]
mod debug_stubs {
    pub static ALL_TMX_FILES: &[(&str, &str)] = &[];
    pub static SCRIPT_FILES: &[(&str, &str)] = &[];
    pub const TMX_COUNT: usize = 0;
    pub const SCRIPT_COUNT: usize = 0;
}

#[cfg(debug_assertions)]
use debug_stubs::*;

// ── Public accessors ────────────────────────────────────────────────────────

/// Return the compile-time embedded content of a `.tmx` file identified by its
/// file stem (e.g. `"demo"` for `assets/maps/demo.tmx`).
///
/// In debug builds this always returns `None` — the caller should load from
/// the filesystem to support hot-reload.
pub fn get_tmx_content(name: &str) -> Option<&'static str> {
    ALL_TMX_FILES
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, content)| *content)
}

/// Return the compile-time embedded content of a script file identified by its
/// key (e.g. `"PalletTown/script"` for `assets/scripts/PalletTown/script.js`).
///
/// In debug builds this always returns `None` — the caller should load from
/// the filesystem to support hot-reload.
pub fn get_script_content(key: &str) -> Option<&'static str> {
    SCRIPT_FILES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, content)| *content)
}

/// Number of embedded `.tmx` files (0 in debug builds).
pub fn tmx_count() -> usize {
    TMX_COUNT
}

/// Number of embedded script files (0 in debug builds).
pub fn script_count() -> usize {
    SCRIPT_COUNT
}
