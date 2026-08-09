//! Centralized map and tileset constants.
//!
//! These constants were historically scattered across both `pokered-data` and
//! `pokered-renderer`. They are defined here once and imported by all crates
//! to eliminate duplication.
//!
//! Source: `constants/map_constants.asm` in the original disassembly.

/// Number of city/town maps (PALLET_TOWN..SAFFRON_CITY = 0x00..0x0A).
pub const NUM_CITY_MAPS: u8 = 0x0B;

/// First indoor map ID (REDS_HOUSE_1F = 0x25).
pub const FIRST_INDOOR_MAP: u8 = 0x25;

/// Tileset ID for Pokemon Tower / Agatha's room.
pub const TILESET_CEMETERY: u8 = 15;

/// Tileset ID for caves (Rock Tunnel, Victory Road, etc).
pub const TILESET_CAVERN: u8 = 17;
