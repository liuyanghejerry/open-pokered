//! Game Boy Advance-style tilemap with 16-bit metadata per tile.
//!
//! This module provides [`TilemapEntry`] and [`Tilemap`] as an upgrade
//! over the classic GB format (1 byte per tile, index only). Each entry
//! carries flip flags, palette bank selection, layer priority, and
//! optional collision/animation data — sufficient to drive modern tile
//! renderers without external lookup tables.

use crate::tile_meta::CollisionType;

/// A single entry in a tilemap, supporting 16-bit-style metadata.
///
/// Inspired by GBA's screen entry format but not bound to GBA hardware.
/// Each entry describes which tile to draw, how it should be transformed,
/// and optional gameplay metadata (collision, animation frame group).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilemapEntry {
    /// Tile index — supports >256 tiles (unlike the 1-byte GB limit).
    pub tile_id: u16,
    /// Flip the tile horizontally before rendering.
    pub flip_h: bool,
    /// Flip the tile vertically before rendering.
    pub flip_v: bool,
    /// Palette bank selection (which sub-palette to use).
    pub palette_group: u8,
    /// Layer priority (0 = lowest, rendered first / behind).
    pub priority: u8,
    /// Per-tile collision override. When `Some`, this overrides the
    /// collision data that would normally come from tile-metadata tables.
    pub collision_override: Option<CollisionType>,
    /// Animation frame group ID. When `Some`, the tile belongs to an
    /// animated sequence (e.g. water, flowers). The renderer uses this
    /// group index together with a global frame counter to select the
    /// correct tile.
    pub animation_group: Option<u8>,
}

impl Default for TilemapEntry {
    fn default() -> Self {
        Self {
            tile_id: 0,
            flip_h: false,
            flip_v: false,
            palette_group: 0,
            priority: 0,
            collision_override: None,
            animation_group: None,
        }
    }
}

/// A two-dimensional tilemap using [`TilemapEntry`] instead of raw byte
/// indices.
///
/// Unlike the classic GB `TileMap` (a flat `Vec<u8>` of tile indices),
/// this tilemap carries per-tile metadata and supports arbitrary widths
/// and heights.
#[derive(Debug, Clone)]
pub struct Tilemap {
    /// Number of tiles horizontally.
    pub width: u16,
    /// Number of tiles vertically.
    pub height: u16,
    /// Row-major tile entries. `entries[y * width as usize + x]`.
    pub entries: Vec<TilemapEntry>,
}

impl Tilemap {
    /// Creates a new tilemap filled with default entries (tile_id = 0,
    /// no flips, no collision/animation overrides).
    ///
    /// # Panics
    ///
    /// Panics if `width` or `height` is 0.
    pub fn new(width: u16, height: u16) -> Self {
        assert!(width > 0, "Tilemap width must be > 0");
        assert!(height > 0, "Tilemap height must be > 0");

        let len = width as usize * height as usize;
        Self {
            width,
            height,
            entries: vec![TilemapEntry::default(); len],
        }
    }

    /// Converts a classic Game Boy tilemap (flat `&[u8]` of tile indices)
    /// into a [`Tilemap`] of [`TilemapEntry`]s.
    ///
    /// Each byte in `data` becomes a `TilemapEntry` with that byte as
    /// `tile_id` and all other fields set to their defaults.
    ///
    /// # Panics
    ///
    /// Panics if `data.len()` is less than `width * height`.
    pub fn from_gb_tilemap(data: &[u8], width: u16, height: u16) -> Self {
        assert!(width > 0, "Tilemap width must be > 0");
        assert!(height > 0, "Tilemap height must be > 0");

        let expected = width as usize * height as usize;
        assert!(
            data.len() >= expected,
            "GB tilemap data too short: expected at least {} bytes, got {}",
            expected,
            data.len(),
        );

        let entries: Vec<TilemapEntry> = data[..expected]
            .iter()
            .map(|&b| TilemapEntry {
                tile_id: b as u16,
                ..Default::default()
            })
            .collect();

        Self {
            width,
            height,
            entries,
        }
    }

    #[inline]
    fn index(&self, x: u16, y: u16) -> usize {
        y as usize * self.width as usize + x as usize
    }

    /// Returns `true` if (x, y) is within bounds.
    #[inline]
    pub fn in_bounds(&self, x: u16, y: u16) -> bool {
        x < self.width && y < self.height
    }

    /// Returns a reference to the tile entry at (x, y), or `None` if
    /// out of bounds.
    #[inline]
    pub fn get(&self, x: u16, y: u16) -> Option<&TilemapEntry> {
        if self.in_bounds(x, y) {
            Some(&self.entries[self.index(x, y)])
        } else {
            None
        }
    }

    /// Returns a mutable reference to the tile entry at (x, y), or
    /// `None` if out of bounds.
    #[inline]
    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut TilemapEntry> {
        if self.in_bounds(x, y) {
            let idx = self.index(x, y);
            Some(&mut self.entries[idx])
        } else {
            None
        }
    }

    /// Sets the tile entry at (x, y).  Silently does nothing if out of
    /// bounds.
    #[inline]
    pub fn set(&mut self, x: u16, y: u16, entry: TilemapEntry) {
        if self.in_bounds(x, y) {
            let idx = self.index(x, y);
            self.entries[idx] = entry;
        }
    }

    /// Fills a rectangular region with copies of `entry`.
    ///
    /// Coordinates are clamped to the tilemap bounds; no-op if the
    /// region lies entirely outside.
    pub fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16, entry: TilemapEntry) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        for row in y..y_end {
            for col in x..x_end {
                self.set(col, row, entry);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // TilemapEntry
    // ----------------------------------------------------------------

    #[test]
    fn entry_default_is_zero_tile() {
        let e = TilemapEntry::default();
        assert_eq!(e.tile_id, 0);
        assert!(!e.flip_h);
        assert!(!e.flip_v);
        assert_eq!(e.palette_group, 0);
        assert_eq!(e.priority, 0);
        assert_eq!(e.collision_override, None);
        assert_eq!(e.animation_group, None);
    }

    #[test]
    fn entry_with_collision_override() {
        let e = TilemapEntry {
            collision_override: Some(CollisionType::Water),
            ..Default::default()
        };
        assert_eq!(e.collision_override, Some(CollisionType::Water));
    }

    #[test]
    fn entry_with_animation() {
        let e = TilemapEntry {
            animation_group: Some(3),
            ..Default::default()
        };
        assert_eq!(e.animation_group, Some(3));
    }

    // ----------------------------------------------------------------
    // Tilemap::new
    // ----------------------------------------------------------------

    #[test]
    fn new_creates_correct_dimensions() {
        let tm = Tilemap::new(32, 32);
        assert_eq!(tm.width, 32);
        assert_eq!(tm.height, 32);
        assert_eq!(tm.entries.len(), 1024);
    }

    #[test]
    fn new_fills_with_default_entries() {
        let tm = Tilemap::new(4, 3);
        for entry in &tm.entries {
            assert_eq!(*entry, TilemapEntry::default());
        }
    }

    #[test]
    #[should_panic]
    fn new_panics_on_zero_width() {
        Tilemap::new(0, 10);
    }

    #[test]
    #[should_panic]
    fn new_panics_on_zero_height() {
        Tilemap::new(10, 0);
    }

    // ----------------------------------------------------------------
    // Tilemap::from_gb_tilemap
    // ----------------------------------------------------------------

    #[test]
    fn from_gb_tilemap_32x32() {
        let data = vec![42u8; 1024];
        let tm = Tilemap::from_gb_tilemap(&data, 32, 32);
        assert_eq!(tm.width, 32);
        assert_eq!(tm.height, 32);
        for e in &tm.entries {
            assert_eq!(e.tile_id, 42);
            assert!(!e.flip_h);
            assert!(!e.flip_v);
        }
    }

    #[test]
    fn from_gb_tilemap_preserves_values() {
        let data: Vec<u8> = (0..9).collect();
        let tm = Tilemap::from_gb_tilemap(&data, 3, 3);
        for (i, e) in tm.entries.iter().enumerate() {
            assert_eq!(e.tile_id, i as u16);
        }
    }

    #[test]
    fn from_gb_tilemap_ignores_extra_bytes() {
        let mut data = vec![7u8; 9];
        data.push(99);
        let tm = Tilemap::from_gb_tilemap(&data, 3, 3);
        assert_eq!(tm.entries.len(), 9);
        for e in &tm.entries {
            assert_eq!(e.tile_id, 7);
        }
    }

    #[test]
    #[should_panic]
    fn from_gb_tilemap_panics_on_short_data() {
        Tilemap::from_gb_tilemap(&[1, 2, 3], 4, 4);
    }

    // ----------------------------------------------------------------
    // in_bounds / get / get_mut / set
    // ----------------------------------------------------------------

    #[test]
    fn in_bounds() {
        let tm = Tilemap::new(10, 8);
        assert!(tm.in_bounds(0, 0));
        assert!(tm.in_bounds(9, 7));
        assert!(!tm.in_bounds(10, 0));
        assert!(!tm.in_bounds(0, 8));
    }

    #[test]
    fn get_returns_entry() {
        let mut tm = Tilemap::new(5, 5);
        let e = TilemapEntry {
            tile_id: 99,
            ..Default::default()
        };
        tm.set(2, 3, e);
        assert_eq!(tm.get(2, 3), Some(&e));
    }

    #[test]
    fn get_returns_none_oob() {
        let tm = Tilemap::new(5, 5);
        assert_eq!(tm.get(5, 0), None);
        assert_eq!(tm.get(0, 5), None);
    }

    #[test]
    fn get_mut_modifies_entry() {
        let mut tm = Tilemap::new(4, 4);
        {
            let e = tm.get_mut(1, 2).unwrap();
            e.flip_h = true;
        }
        assert!(tm.get(1, 2).unwrap().flip_h);
    }

    #[test]
    fn get_mut_returns_none_oob() {
        let mut tm = Tilemap::new(3, 3);
        assert!(tm.get_mut(3, 0).is_none());
    }

    #[test]
    fn set_and_get_roundtrip() {
        let mut tm = Tilemap::new(16, 16);
        let e = TilemapEntry {
            tile_id: 255,
            flip_h: true,
            flip_v: true,
            palette_group: 3,
            priority: 1,
            collision_override: Some(CollisionType::Impassable),
            animation_group: Some(2),
        };
        tm.set(10, 5, e);
        assert_eq!(tm.get(10, 5), Some(&e));
    }

    #[test]
    fn set_oob_is_noop() {
        let mut tm = Tilemap::new(4, 4);
        let e = TilemapEntry {
            tile_id: 99,
            ..Default::default()
        };
        tm.set(4, 0, e);

        for i in 0..4 {
            for j in 0..4 {
                assert_eq!(tm.get(i, j), Some(&TilemapEntry::default()));
            }
        }
    }

    // ----------------------------------------------------------------
    // fill_rect
    // ----------------------------------------------------------------

    #[test]
    fn fill_rect_partial() {
        let mut tm = Tilemap::new(8, 8);
        let e = TilemapEntry {
            tile_id: 42,
            ..Default::default()
        };
        tm.fill_rect(2, 3, 4, 2, e);
        for y in 3..5 {
            for x in 2..6 {
                assert_eq!(tm.get(x, y).unwrap().tile_id, 42);
            }
        }

        assert_eq!(tm.get(0, 0).unwrap().tile_id, 0);
        assert_eq!(tm.get(7, 7).unwrap().tile_id, 0);
    }

    #[test]
    fn fill_rect_clamped_to_bounds() {
        let mut tm = Tilemap::new(4, 4);
        let e = TilemapEntry {
            tile_id: 99,
            ..Default::default()
        };
        tm.fill_rect(2, 2, 10, 10, e);

        for y in 0..4 {
            for x in 0..4 {
                if x >= 2 && y >= 2 {
                    assert_eq!(tm.get(x, y).unwrap().tile_id, 99);
                } else {
                    assert_eq!(tm.get(x, y).unwrap().tile_id, 0);
                }
            }
        }
    }

    // ----------------------------------------------------------------
    // Large tilemap (supports > 256 tiles)
    // ----------------------------------------------------------------

    #[test]
    fn supports_tile_ids_above_255() {
        let e = TilemapEntry {
            tile_id: 1023,
            ..Default::default()
        };
        let mut tm = Tilemap::new(1, 1);
        tm.set(0, 0, e);
        assert_eq!(tm.get(0, 0).unwrap().tile_id, 1023);
    }

    #[test]
    fn non_square_tilemap() {
        let tm = Tilemap::new(64, 16);
        assert_eq!(tm.entries.len(), 1024);
        assert!(tm.get(63, 15).is_some());
        assert!(tm.get(64, 0).is_none());
    }
}
