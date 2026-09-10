use core::fmt::Debug;
use core::hash::Hash;

/// Describes how a tile interacts with entities on the overworld map.
///
/// This is used by the movement and collision systems to determine
/// whether the player or NPCs can walk on a tile, trigger special
/// behaviour (ledges, grass encounters, doors), or are blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionType {
    /// The tile can be walked on freely.
    Passable,

    /// The tile blocks all movement.
    Impassable,

    /// A ledge tile that allows jumping down.
    ///
    /// The `direction` field indicates which direction the player
    /// faces when jumping (e.g., 0 = down).
    Ledge {
        /// Direction the player faces when jumping the ledge.
        direction: u8,
    },

    /// A counter tile that the player can interact with from behind.
    Counter,

    /// Tall grass where wild monster encounters can occur.
    ///
    /// When `Some(id)`, specifies a special grass tile ID for
    /// encounter calculations. When `None`, uses the default
    /// encounter rate.
    Grass(Option<u8>),

    /// Water tile that requires Surf to traverse.
    Water,

    /// A warp tile that triggers a map transition.
    Warp,

    /// A door tile that can be entered.
    Door,
}

/// Marker trait for tile metadata identifiers.
///
/// Implementations are typically lightweight enums or numeric IDs
/// representing a specific set of tile collision data (e.g.,
/// a tileset's collision table).
pub trait TileMetaTrait: Copy + Eq + Hash + Debug + 'static {}

/// Provides collision and terrain metadata for tiles in a given tileset.
///
/// The engine queries this trait whenever an entity attempts to move
/// onto a tile, to determine whether the movement is valid and what
/// special behaviour (if any) should trigger.
pub trait TileMetadata<T: TileMetaTrait> {
    /// Returns `true` if a tile with the given ID in the given tileset
    /// can be walked on freely.
    fn is_passable(&self, tileset: T, tile_id: u8) -> bool;

    /// Returns the full `CollisionType` for a tile.
    fn collision_type(&self, tileset: T, tile_id: u8) -> CollisionType;

    /// Returns `true` if the tile is a ledge.
    fn is_ledge(&self, tileset: T, tile_id: u8) -> bool;

    /// Returns `true` if the tile is a counter.
    fn is_counter(&self, tileset: T, tile_id: u8) -> bool;

    /// Returns `true` if the tile is tall grass.
    fn is_grass(&self, tileset: T, tile_id: u8) -> bool;

    /// Returns the special grass tile ID for encounter calculations,
    /// or `None` if the default encounter rate should be used.
    fn get_grass_tile(&self, tileset: T) -> Option<u8>;
}
