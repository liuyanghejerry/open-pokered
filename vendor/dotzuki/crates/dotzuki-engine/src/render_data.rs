use core::fmt::Debug;

/// Provides render-time string and metadata lookups for game entities.
///
/// This trait abstracts the display layer so that the UI and renderer
/// can query move names, item names, species names, and move metadata
/// without depending on concrete data types. This enables the engine
/// to remain generic across different RPG games.
pub trait RenderData {
    /// The type representing a move (ability/skill) in the game.
    type Move: Copy + Eq + Debug;

    /// The type representing an item in the game.
    type Item: Copy + Eq + Debug;

    /// The type representing a species (monster/character type) in the game.
    type Species: Copy + Eq + Debug;

    /// Returns the display name of a move.
    fn move_name(&self, m: Self::Move) -> &str;

    /// Returns the PP (power points) for a move as `(current_max, base_max)`.
    ///
    /// Some moves have PP that can be boosted beyond their base value
    /// (e.g., via PP Up items), so both values are returned.
    fn move_pp(&self, m: Self::Move) -> (u8, u8);

    /// Returns the type ID (element/attribute) of a move.
    fn move_type(&self, m: Self::Move) -> u8;

    /// Returns the display name of an item.
    fn item_name(&self, i: Self::Item) -> &str;

    /// Returns the display name of a species.
    fn species_name(&self, s: Self::Species) -> &str;
}
