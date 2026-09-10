/// Icon kind used by party / status menus.
///
/// Categorizes the visual shape of the monster/character icons displayed
/// in UI elements such as the party screen. A presentation-layer default
/// taxonomy: games map their species to one of these shapes when rendering
/// party icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IconKind {
    /// Generic biped monster (default)
    Mon = 0,
    /// Ball-shaped icon
    Ball = 1,
    /// Spiral / shell-fossil icon
    Helix = 2,
    /// Round fairy-type icon
    Fairy = 3,
    /// Bird icon
    Bird = 4,
    /// Aquatic icon
    Water = 5,
    /// Bug icon
    Bug = 6,
    /// Plant / grass icon
    Grass = 7,
    /// Snake icon
    Snake = 8,
    /// Quadruped icon
    Quadruped = 9,
}
