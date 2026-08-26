//! Compile-time embedded asset data for `.tmx` files.
//!
//! In **release** builds the content is baked into the binary via `include_str!`
//! so no filesystem I/O is required at runtime.
//!
//! In **debug** builds the tables are empty; the caller should load from the
//! filesystem instead, which enables hot-reload during development.

// ── Include generated data (release only) ───────────────────────────────────

#[cfg(not(debug_assertions))]
include!(concat!(env!("OUT_DIR"), "/tmx_assets_gen.rs"));

// ── Debug stubs ─────────────────────────────────────────────────────────────

/// Empty stubs used in debug builds so the rest of the crate compiles even
/// when `tmx_assets_gen.rs` is not included.
#[cfg(debug_assertions)]
mod debug_stubs {
    pub static ALL_TMX_FILES: &[(&str, &str)] = &[];
    pub const TMX_COUNT: usize = 0;
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

/// Number of embedded `.tmx` files (0 in debug builds).
pub fn tmx_count() -> usize {
    TMX_COUNT
}
