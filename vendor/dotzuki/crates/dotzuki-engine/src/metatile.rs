//! # metatile
//!
//! Defines `MetatileDef` and `MetatileRegistry` — the JRPG engine generalization
//! of the Game Boy "block" concept.  A metatile is a logical unit comprised of
//! N×N tiles (2×2, 3×3, 4×4, …) that carries collision, trigger, animation and
//! z-offset metadata.  The module also provides a converter from the original
//! Game Boy `.bst` block format (16-byte blocks, 4×4 tiles).

use crate::tile_meta::CollisionType;

// ---------------------------------------------------------------------------
// MetatileCell — a single tile placed inside a metatile
// ---------------------------------------------------------------------------

/// One tile cell within a metatile.  Beyond the basic tile index, this
/// carries optional flip flags and a palette bank selector that are commonly
/// needed by tilemap renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetatileCell {
    /// Index of the tile in the tileset's tile table.
    pub tile_id: u8,
    /// Flip horizontally.
    pub flip_x: bool,
    /// Flip vertically.
    pub flip_y: bool,
    /// Palette bank index (0–7 for GBC-style, 0 for DMG).
    pub palette_bank: u8,
}

impl Default for MetatileCell {
    fn default() -> Self {
        Self {
            tile_id: 0,
            flip_x: false,
            flip_y: false,
            palette_bank: 0,
        }
    }
}

impl MetatileCell {
    /// Create an entry that references only a tile ID with no flips.
    pub fn from_tile_id(tile_id: u8) -> Self {
        Self {
            tile_id,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// CollisionCell
// ---------------------------------------------------------------------------

/// Per-cell collision descriptor.  May differ from the metatile's visual
/// bounds when, for example, only the bottom row of the metatile is solid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionCell {
    pub collision: CollisionType,
}

impl CollisionCell {
    /// Convenience constructor.
    pub fn new(collision: CollisionType) -> Self {
        Self { collision }
    }

    /// Returns a passable cell.
    pub fn passable() -> Self {
        Self {
            collision: CollisionType::Passable,
        }
    }

    /// Returns an impassable cell.
    pub fn impassable() -> Self {
        Self {
            collision: CollisionType::Impassable,
        }
    }
}

// ---------------------------------------------------------------------------
// Trigger
// ---------------------------------------------------------------------------

/// When a trigger associated with a metatile fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// Fires every frame the player (or NPC) stands on the tile.
    OnStep,
    /// Fires once when the entity enters the metatile's area.
    OnEnter,
    /// Fires when the player presses the A button facing the metatile.
    OnInteract,
}

/// A trigger definition bound to a metatile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerDef {
    /// Name of the script (or event handler) to execute.
    pub script_name: String,
    /// When the trigger fires.
    pub trigger_type: TriggerType,
}

// ---------------------------------------------------------------------------
// MetatileDef
// ---------------------------------------------------------------------------

/// A metatile — a logical unit comprised of N×N tiles.
///
/// This is the JRPG engine generalization of Game Boy's "block" concept
/// (4×4 tiles = 16 bytes).  Variable-size support enables use with other
/// tile-based games (2×2, 3×3, …).
#[derive(Debug, Clone)]
pub struct MetatileDef {
    /// Width and height in tiles, e.g. `(4, 4)` for GB blocks.
    pub size: (u8, u8),

    /// The tiles that make up this metatile, stored in **row-major** order.
    /// Must contain exactly `size.0 * size.1` entries.
    pub tiles: Vec<MetatileCell>,

    /// Per-cell collision, same dimensions as `size`.  If `collision.len()`
    /// differs from `tile_count()`, the engine should treat the whole
    /// metatile uniformly (e.g. all cells inherit the first collision entry).
    pub collision: Vec<CollisionCell>,

    /// Optional trigger script that fires when the player interacts with
    /// or steps on this metatile.
    pub trigger: Option<TriggerDef>,

    /// Optional animation group index — tiles inside the metatile animate
    /// in lockstep if they share the same group.
    pub animation_group: Option<u8>,

    /// Vertical render offset (pixels).  Positive values render the
    /// metatile lower on screen, useful for layered maps or floating
    /// platforms.  Default is `0`.
    pub z_offset: i8,
}

impl MetatileDef {
    /// Creates an empty metatile of the given size.
    ///
    /// All tiles default to tile ID 0 with no flips; all collision cells
    /// are `Passable`.  `trigger`, `animation_group` and `z_offset` are
    /// initialised to their defaults.
    ///
    /// # Panics
    ///
    /// Panics if either dimension is 0.
    pub fn new(size: (u8, u8)) -> Self {
        assert!(size.0 > 0, "metatile width must be > 0");
        assert!(size.1 > 0, "metatile height must be > 0");

        let count = (size.0 as usize) * (size.1 as usize);
        MetatileDef {
            size,
            tiles: vec![MetatileCell::default(); count],
            collision: vec![CollisionCell::passable(); count],
            trigger: None,
            animation_group: None,
            z_offset: 0,
        }
    }

    /// Returns the total number of tiles in this metatile (`size.0 * size.1`).
    pub fn tile_count(&self) -> usize {
        (self.size.0 as usize) * (self.size.1 as usize)
    }

    /// Converts a 16-byte Game Boy `.bst` block into a 4×4 `MetatileDef`.
    ///
    /// Each byte in `block_data` is a raw tile ID.  Entries are placed in
    /// **row-major** order (top-left to bottom-right).  All collision cells
    /// default to `Passable`.
    ///
    /// # Panics
    ///
    /// Panics if `block_data` does not contain exactly 16 bytes.
    pub fn from_gb_block(block_data: &[u8]) -> Self {
        assert_eq!(
            block_data.len(),
            16,
            "GB block data must be exactly 16 bytes, got {}",
            block_data.len()
        );

        let tiles: Vec<MetatileCell> = block_data
            .iter()
            .map(|&id| MetatileCell::from_tile_id(id))
            .collect();

        MetatileDef {
            size: (4, 4),
            tiles,
            collision: vec![CollisionCell::passable(); 16],
            trigger: None,
            animation_group: None,
            z_offset: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// MetatileRegistry
// ---------------------------------------------------------------------------

/// Registry of all metatile definitions for a tileset.
///
/// A registry holds every `MetatileDef` used by a map or tileset.  The map's
/// block data references metatiles by their index in this registry (the
/// index is returned by [`add_def`](MetatileRegistry::add_def)).
#[derive(Debug, Clone)]
pub struct MetatileRegistry {
    pub defs: Vec<MetatileDef>,
}

impl MetatileRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        MetatileRegistry { defs: Vec::new() }
    }

    /// Adds a metatile definition and returns its index in the registry.
    pub fn add_def(&mut self, def: MetatileDef) -> usize {
        let index = self.defs.len();
        self.defs.push(def);
        index
    }

    /// Looks up a metatile definition by index.  Returns `None` if the
    /// index is out of bounds.
    pub fn get(&self, index: usize) -> Option<&MetatileDef> {
        self.defs.get(index)
    }

    /// Parses a complete `.bst` byte slice into a `MetatileRegistry`.
    ///
    /// The slice is expected to be a multiple of **16 bytes** (the standard
    /// Game Boy block size).  Each 16-byte chunk is fed to
    /// [`MetatileDef::from_gb_block`].
    ///
    /// # Panics
    ///
    /// Panics if `bst_data.len()` is not a multiple of 16.
    pub fn from_gb_blocksets(bst_data: &[u8]) -> Self {
        assert!(
            bst_data.len() % 16 == 0,
            "GB blockset data length must be a multiple of 16, got {}",
            bst_data.len()
        );

        let mut registry = MetatileRegistry::new();
        for chunk in bst_data.chunks_exact(16) {
            registry.add_def(MetatileDef::from_gb_block(chunk));
        }
        registry
    }
}

impl Default for MetatileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // MetatileDef::new
    // ------------------------------------------------------------------

    #[test]
    fn test_new_creates_empty_metatile() {
        let m = MetatileDef::new((2, 3));
        assert_eq!(m.size, (2, 3));
        assert_eq!(m.tile_count(), 6);
        assert_eq!(m.tiles.len(), 6);
        assert_eq!(m.collision.len(), 6);
        assert!(m.trigger.is_none());
        assert!(m.animation_group.is_none());
        assert_eq!(m.z_offset, 0);

        // All tiles should be zero with no flips.
        for tile in &m.tiles {
            assert_eq!(tile.tile_id, 0);
            assert!(!tile.flip_x);
            assert!(!tile.flip_y);
            assert_eq!(tile.palette_bank, 0);
        }

        // All collision cells should be passable.
        for cell in &m.collision {
            assert_eq!(cell.collision, CollisionType::Passable);
        }
    }

    #[test]
    fn test_new_2x2() {
        let m = MetatileDef::new((2, 2));
        assert_eq!(m.size, (2, 2));
        assert_eq!(m.tile_count(), 4);
    }

    #[test]
    fn test_new_4x4() {
        let m = MetatileDef::new((4, 4));
        assert_eq!(m.size, (4, 4));
        assert_eq!(m.tile_count(), 16);
    }

    #[test]
    #[should_panic(expected = "metatile width must be > 0")]
    fn test_new_zero_width_panics() {
        MetatileDef::new((0, 4));
    }

    #[test]
    #[should_panic(expected = "metatile height must be > 0")]
    fn test_new_zero_height_panics() {
        MetatileDef::new((4, 0));
    }

    // ------------------------------------------------------------------
    // MetatileDef::from_gb_block
    // ------------------------------------------------------------------

    #[test]
    fn test_from_gb_block_full_16_bytes() {
        let data: [u8; 16] = [
            0x00, 0x01, 0x02, 0x03, // row 0
            0x10, 0x11, 0x12, 0x13, // row 1
            0x20, 0x21, 0x22, 0x23, // row 2
            0x30, 0x31, 0x32, 0x33, // row 3
        ];

        let m = MetatileDef::from_gb_block(&data);

        assert_eq!(m.size, (4, 4));
        assert_eq!(m.tile_count(), 16);
        assert_eq!(m.z_offset, 0);
        assert!(m.trigger.is_none());

        // Verify row-major ordering.
        assert_eq!(m.tiles[0].tile_id, 0x00);
        assert_eq!(m.tiles[1].tile_id, 0x01);
        assert_eq!(m.tiles[2].tile_id, 0x02);
        assert_eq!(m.tiles[3].tile_id, 0x03);
        assert_eq!(m.tiles[4].tile_id, 0x10);
        assert_eq!(m.tiles[15].tile_id, 0x33);

        // All entries should have default flip/palette.
        for tile in &m.tiles {
            assert!(!tile.flip_x);
            assert!(!tile.flip_y);
            assert_eq!(tile.palette_bank, 0);
        }
    }

    #[test]
    fn test_from_gb_block_all_zeros() {
        let data = [0u8; 16];
        let m = MetatileDef::from_gb_block(&data);

        assert_eq!(m.size, (4, 4));
        for tile in &m.tiles {
            assert_eq!(tile.tile_id, 0);
        }
    }

    #[test]
    fn test_from_gb_block_with_high_tile_ids() {
        let mut data = [0u8; 16];
        // Fill with tiles 0xA0 .. 0xAF
        for (i, b) in data.iter_mut().enumerate() {
            *b = 0xA0 + i as u8;
        }

        let m = MetatileDef::from_gb_block(&data);
        assert_eq!(m.tiles[0].tile_id, 0xA0);
        assert_eq!(m.tiles[15].tile_id, 0xAF);
    }

    #[test]
    #[should_panic(expected = "GB block data must be exactly 16 bytes")]
    fn test_from_gb_block_too_short_panics() {
        MetatileDef::from_gb_block(&[0u8; 8]);
    }

    #[test]
    #[should_panic(expected = "GB block data must be exactly 16 bytes")]
    fn test_from_gb_block_too_long_panics() {
        MetatileDef::from_gb_block(&[0u8; 32]);
    }

    #[test]
    #[should_panic(expected = "GB block data must be exactly 16 bytes")]
    fn test_from_gb_block_empty_panics() {
        MetatileDef::from_gb_block(&[]);
    }

    // ------------------------------------------------------------------
    // MetatileDef::tile_count
    // ------------------------------------------------------------------

    #[test]
    fn test_tile_count_various_sizes() {
        assert_eq!(MetatileDef::new((1, 1)).tile_count(), 1);
        assert_eq!(MetatileDef::new((2, 2)).tile_count(), 4);
        assert_eq!(MetatileDef::new((3, 3)).tile_count(), 9);
        assert_eq!(MetatileDef::new((4, 4)).tile_count(), 16);
        assert_eq!(MetatileDef::new((2, 5)).tile_count(), 10);
    }

    // ------------------------------------------------------------------
    // MetatileDef — Trigger / animation / z_offset
    // ------------------------------------------------------------------

    #[test]
    fn test_metatile_with_trigger() {
        let mut m = MetatileDef::new((2, 2));
        m.trigger = Some(TriggerDef {
            script_name: "heal_party".into(),
            trigger_type: TriggerType::OnStep,
        });

        let t = m.trigger.as_ref().unwrap();
        assert_eq!(t.script_name, "heal_party");
        assert_eq!(t.trigger_type, TriggerType::OnStep);
    }

    #[test]
    fn test_metatile_with_animation_group() {
        let mut m = MetatileDef::new((2, 2));
        m.animation_group = Some(7);
        assert_eq!(m.animation_group, Some(7));
    }

    #[test]
    fn test_metatile_z_offset() {
        let mut m = MetatileDef::new((2, 2));
        m.z_offset = -4;
        assert_eq!(m.z_offset, -4);

        m.z_offset = 8;
        assert_eq!(m.z_offset, 8);
    }

    // ------------------------------------------------------------------
    // MetatileRegistry
    // ------------------------------------------------------------------

    #[test]
    fn test_registry_new_is_empty() {
        let r = MetatileRegistry::new();
        assert!(r.defs.is_empty());
        assert!(r.get(0).is_none());
    }

    #[test]
    fn test_registry_add_and_get() {
        let mut r = MetatileRegistry::new();

        let m1 = MetatileDef::new((2, 2));
        let m2 = MetatileDef::new((3, 3));

        let idx1 = r.add_def(m1);
        let idx2 = r.add_def(m2);

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);

        assert!(r.get(0).is_some());
        assert!(r.get(1).is_some());
        assert!(r.get(2).is_none());

        assert_eq!(r.get(0).unwrap().size, (2, 2));
        assert_eq!(r.get(1).unwrap().size, (3, 3));
    }

    #[test]
    fn test_registry_get_out_of_bounds() {
        let r = MetatileRegistry::new();
        assert!(r.get(0).is_none());
        assert!(r.get(100).is_none());
    }

    #[test]
    fn test_registry_default() {
        let r = MetatileRegistry::default();
        assert!(r.defs.is_empty());
    }

    // ------------------------------------------------------------------
    // MetatileRegistry::from_gb_blocksets
    // ------------------------------------------------------------------

    #[test]
    fn test_from_gb_blocksets_single_block() {
        let data: Vec<u8> = (0..16).collect(); // 0x00..0x0F
        let registry = MetatileRegistry::from_gb_blocksets(&data);

        assert_eq!(registry.defs.len(), 1);
        let m = registry.get(0).unwrap();
        assert_eq!(m.size, (4, 4));
        assert_eq!(m.tiles[0].tile_id, 0x00);
        assert_eq!(m.tiles[15].tile_id, 0x0F);
    }

    #[test]
    fn test_from_gb_blocksets_multiple_blocks() {
        // Two blocks: first is 0x00–0x0F, second is 0x10–0x1F
        let data: Vec<u8> = (0..32).collect();
        let registry = MetatileRegistry::from_gb_blocksets(&data);

        assert_eq!(registry.defs.len(), 2);

        let m0 = registry.get(0).unwrap();
        assert_eq!(m0.tiles[0].tile_id, 0x00);
        assert_eq!(m0.tiles[15].tile_id, 0x0F);

        let m1 = registry.get(1).unwrap();
        assert_eq!(m1.tiles[0].tile_id, 0x10);
        assert_eq!(m1.tiles[15].tile_id, 0x1F);
    }

    #[test]
    fn test_from_gb_blocksets_many_blocks() {
        // 256 blocks (4096 bytes) — a real tileset.
        let data = vec![0u8; 256 * 16];
        let registry = MetatileRegistry::from_gb_blocksets(&data);
        assert_eq!(registry.defs.len(), 256);

        for def in &registry.defs {
            assert_eq!(def.size, (4, 4));
            assert_eq!(def.tile_count(), 16);
        }
    }

    #[test]
    fn test_from_gb_blocksets_empty() {
        let registry = MetatileRegistry::from_gb_blocksets(&[]);
        assert!(registry.defs.is_empty());
    }

    #[test]
    #[should_panic(expected = "GB blockset data length must be a multiple of 16")]
    fn test_from_gb_blocksets_unaligned_panics() {
        MetatileRegistry::from_gb_blocksets(&[0u8; 10]);
    }

    #[test]
    #[should_panic(expected = "GB blockset data length must be a multiple of 16")]
    fn test_from_gb_blocksets_odd_length_panics() {
        MetatileRegistry::from_gb_blocksets(&[0u8; 17]);
    }

    // ------------------------------------------------------------------
    // CollisionCell
    // ------------------------------------------------------------------

    #[test]
    fn test_collision_cell_passable() {
        let c = CollisionCell::passable();
        assert_eq!(c.collision, CollisionType::Passable);
    }

    #[test]
    fn test_collision_cell_impassable() {
        let c = CollisionCell::impassable();
        assert_eq!(c.collision, CollisionType::Impassable);
    }

    #[test]
    fn test_collision_cell_custom() {
        let c = CollisionCell::new(CollisionType::Water);
        assert_eq!(c.collision, CollisionType::Water);
    }

    // ------------------------------------------------------------------
    // MetatileCell
    // ------------------------------------------------------------------

    #[test]
    fn test_tilemap_entry_default() {
        let t = MetatileCell::default();
        assert_eq!(t.tile_id, 0);
        assert!(!t.flip_x);
        assert!(!t.flip_y);
        assert_eq!(t.palette_bank, 0);
    }

    #[test]
    fn test_tilemap_entry_from_tile_id() {
        let t = MetatileCell::from_tile_id(42);
        assert_eq!(t.tile_id, 42);
        assert!(!t.flip_x);
        assert!(!t.flip_y);
    }

    #[test]
    fn test_tilemap_entry_with_flips() {
        let t = MetatileCell {
            tile_id: 0x80,
            flip_x: true,
            flip_y: false,
            palette_bank: 3,
        };
        assert!(t.flip_x);
        assert!(!t.flip_y);
        assert_eq!(t.palette_bank, 3);
    }

    // ------------------------------------------------------------------
    // TriggerDef / TriggerType
    // ------------------------------------------------------------------

    #[test]
    fn test_trigger_type_equality() {
        assert_eq!(TriggerType::OnStep, TriggerType::OnStep);
        assert_ne!(TriggerType::OnStep, TriggerType::OnInteract);
    }

    #[test]
    fn test_trigger_def() {
        let td = TriggerDef {
            script_name: "warp_to_start_town".into(),
            trigger_type: TriggerType::OnEnter,
        };
        assert_eq!(td.script_name, "warp_to_start_town");
        assert_eq!(td.trigger_type, TriggerType::OnEnter);
    }
}
