use core::fmt::Debug;
use core::hash::Hash;

/// Trait representing a tileset identifier in a JRPG engine.
///
/// A tileset defines a collection of graphical tiles used to render
/// the overworld or dungeon maps. Implementations should be lightweight
/// (typically an enum or numeric ID) since they are copied frequently.
pub trait TilesetTrait: Copy + Eq + Hash + Debug + 'static {
    /// Returns the numeric identifier for this tileset.
    fn id(&self) -> u8;

    /// Returns the human-readable name of this tileset.
    fn name(&self) -> &'static str;
}

/// Provider trait that supplies tileset data to the engine.
///
/// Implementations of this trait are responsible for loading and
/// serving tileset metadata, block data, and tile dimensions.
/// The engine queries this provider when it needs to render a
/// map or resolve tile properties.
pub trait TilesetProvider<T: TilesetTrait> {
    /// Returns the total number of tilesets managed by this provider.
    fn tileset_count(&self) -> usize;

    /// Looks up a tileset by its numeric identifier.
    fn tileset_by_id(&self, id: u8) -> Option<T>;

    /// Looks up a tileset by its human-readable name.
    fn tileset_by_name(&self, name: &str) -> Option<T>;

    /// Returns the raw block data for the given tileset.
    ///
    /// Each byte in the returned slice represents a cell in the
    /// tileset's block definition table.
    fn blockset_for(&self, tileset: T) -> &[u8];

    /// Returns the size (in bytes) of a single block in this tileset.
    fn block_size(&self) -> usize;

    /// Returns the number of tiles per block as `(width, height)`.
    fn tiles_per_block(&self) -> (usize, usize);
}
