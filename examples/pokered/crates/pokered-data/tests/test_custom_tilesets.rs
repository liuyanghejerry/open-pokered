//! Tests for `TilesetId::Custom` runtime delegation.
//!
//! These tests exercise the accessor functions in `blockset_data`,
//! `collision`, and `tileset_data` to make sure custom tilesets registered
//! via `tileset_extras.json` correctly:
//!
//! - return their own `.bst` from `blockset_for_tileset`
//! - either return their passable-tile override or fall through to the base
//! - inherit header data (counter tiles, grass tile, animation) from the base
//! - inherit door/warp/spinner/water/dungeon classification from the base
//!
//! When `tileset_extras.json` is absent, `CUSTOM_TILESETS` is empty and these
//! per-entry assertions are skipped — only the safety-fallback for an
//! unregistered slot is exercised.

use pokered_data::blockset_data::blockset_for_tileset;
use pokered_data::collision::{get_passable_tiles, is_tile_passable};
use pokered_data::tileset_data::{
    get_grass_tile, get_tileset_header, has_water_animation, is_dungeon_tileset,
    is_outside_tileset, is_water_tileset,
};
use pokered_data::tilesets::{custom, TilesetId};

#[test]
fn registered_custom_tilesets_delegate_correctly() {
    for (slot, ct) in custom::CUSTOM_TILESETS.iter().enumerate() {
        let id = TilesetId::Custom(slot as u8);
        let base = id.base();

        // Blockset comes from the registered .bst, not the base.
        let bst = blockset_for_tileset(id);
        assert_eq!(
            bst.as_ptr(),
            ct.blockset.as_ptr(),
            "{}: blockset_for_tileset should return the registered bytes",
            ct.name,
        );

        // Passable list: override if present, else base list.
        let passable = get_passable_tiles(id);
        match ct.passable_override {
            Some(over) => assert_eq!(
                passable.as_ptr(),
                over.as_ptr(),
                "{}: passable list should be the registered override",
                ct.name,
            ),
            None => assert_eq!(
                passable.as_ptr(),
                get_passable_tiles(base).as_ptr(),
                "{}: passable list should fall through to base",
                ct.name,
            ),
        }
        // is_tile_passable should match the resolved list.
        for tile in 0..=255u8 {
            assert_eq!(
                is_tile_passable(id, tile),
                passable.contains(&tile),
                "{}: is_tile_passable disagreement at tile 0x{:02X}",
                ct.name,
                tile,
            );
        }

        // Header data inherits from the base.
        let header = get_tileset_header(id);
        let base_header = get_tileset_header(base);
        assert_eq!(header.grass_tile, base_header.grass_tile);
        assert_eq!(header.counter_tiles, base_header.counter_tiles);
        assert_eq!(get_grass_tile(id), get_grass_tile(base));
        assert_eq!(has_water_animation(id), has_water_animation(base));

        // Classification helpers inherit from the base.
        assert_eq!(is_dungeon_tileset(id), is_dungeon_tileset(base));
        assert_eq!(is_water_tileset(id), is_water_tileset(base));
        assert_eq!(is_outside_tileset(id), is_outside_tileset(base));
    }
}

#[test]
fn unregistered_custom_slot_safely_falls_back() {
    // A slot that is guaranteed to not exist (we only register through
    // build.rs from JSON; no real registry will reach this many entries).
    let id = TilesetId::Custom(u8::MAX);
    assert_eq!(id.base(), TilesetId::Overworld);
    // Accessors must not panic and must return Overworld-equivalent results.
    let _ = blockset_for_tileset(id);
    let _ = get_passable_tiles(id);
    let _ = get_tileset_header(id);
    assert!(!is_water_tileset(id) || is_water_tileset(TilesetId::Overworld));
}

#[test]
fn map_header_string_resolves_to_custom_when_registered() {
    // For every registered custom tileset, MapHeaderJson.tileset = "<name>"
    // must resolve to the right Custom slot via TilesetId::from_name.
    for (slot, ct) in custom::CUSTOM_TILESETS.iter().enumerate() {
        let id = TilesetId::from_name(ct.name).expect("from_name");
        assert_eq!(id, TilesetId::Custom(slot as u8));
    }
}
