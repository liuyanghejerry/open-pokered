use core::fmt::Debug;
use core::hash::Hash;

/// Describes a connection between two maps on the overworld.
///
/// Connections allow seamless scrolling between adjacent maps
/// without a fade transition. The engine uses this to preload
/// the connected map's block data when the player approaches
/// the border.
#[derive(Debug, Clone)]
pub struct MapConnection<M: MapTrait> {
    /// The direction of the connection: "north", "south", "east", or "west".
    pub direction: String,

    /// The connected map.
    pub map: M,

    /// The offset (in tiles) along the connection border.
    ///
    /// Positive values shift the connected map to the right (horizontal)
    /// or down (vertical); negative values shift left/up.
    pub offset: i8,
}

/// Marker trait for map identifiers.
///
/// Implementations are typically lightweight enums or numeric IDs
/// that uniquely identify a map in the game world. Maps are copied
/// frequently during transitions and connection lookups, so they
/// must be cheap to clone.
pub trait MapTrait: Copy + Eq + Hash + Debug + 'static {}

/// Provider trait that supplies map data to the engine.
///
/// This is the central abstraction for loading map dimensions,
/// tile block data, border blocks, and connection information.
/// The overworld system queries this provider to render maps and
/// handle movement/collision.
pub trait MapProvider<M: MapTrait> {
    /// Returns the dimensions of the map as `(width, height)` in tiles.
    fn dimensions(&self, map: M) -> (u8, u8);

    /// Returns the tileset ID used by this map.
    fn tileset(&self, map: M) -> u8;

    /// Returns the block data for the map.
    ///
    /// Each byte in the returned slice corresponds to a block index
    /// in the tileset's block definition table. Blocks are stored
    /// in row-major order.
    fn block_data(&self, map: M) -> &[u8];

    /// Returns the border block ID used to fill the area outside the map.
    fn border_block(&self, map: M) -> u8;

    /// Returns the list of connected maps for seamless scrolling.
    fn connections(&self, map: M) -> Vec<MapConnection<M>>;
}
