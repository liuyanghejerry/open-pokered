//! # Item kind classification
//!
//! [`ItemKind`] classifies items into gameplay categories that determine
//! default behaviours for selling, discarding, stacking, and consumption.
//!
//! | Variant | Default sellable | Default discardable | Default stackable | Default consumed on use |
//! |---------|-----------------|-------------------|------------------|------------------------|
//! | Consumable | true | true | true | true |
//! | Equipment | true | true | false | false |
//! | KeyItem | false | false | false | false |
//! | Evolution | false | true | true | true |
//! | StatBoost | true | true | true | true |
//! | Currency | true | true | true | true |
//! | TeachMove | false | true | false | true |
//! | Custom(Id) | true | true | true | true |

use core::fmt::Debug;
use core::hash::Hash;

/// Broad classification of an item's purpose, influencing default shop, bag,
/// and usage behaviour.
///
/// The generic parameter `Id` allows a game-specific crate to supply
/// additional custom kinds (e.g. `ItemKind::Custom(GameItemKind::TM)`).
///
/// # Default methods
///
/// Each variant has baked-in defaults for four orthogonal behaviours.
/// A game's [`ItemProvider`](crate::items::ItemProvider) implementation may
/// override any of these per-item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind<Id: Copy + Eq + Hash + Debug> {
    /// Single-use recovery / utility items (potions, antidotes, repels).
    Consumable,
    /// Equippable gear (weapons, armour, accessories).
    Equipment,
    /// Plot-critical items that cannot be sold, discarded, or stacked.
    KeyItem,
    /// Items that trigger a monster evolution.
    Evolution,
    /// Permanent stat-enhancing items (vitamins, feathers).
    StatBoost,
    /// In-game currency (dollars, coins, shards).
    Currency,
    /// Items that teach a new move (TMs, HMs, skill discs).
    TeachMove,
    /// Game-specific kind not covered by the standard variants.
    Custom(Id),
}

impl<Id: Copy + Eq + Hash + Debug> ItemKind<Id> {
    /// Whether shops will buy this kind of item by default.
    pub fn default_sellable(&self) -> bool {
        match self {
            ItemKind::KeyItem | ItemKind::Evolution | ItemKind::TeachMove => false,
            _ => true,
        }
    }

    /// Whether this kind of item can be discarded from the bag by default.
    pub fn default_discardable(&self) -> bool {
        match self {
            ItemKind::KeyItem => false,
            _ => true,
        }
    }

    /// Whether multiple copies of this kind of item stack in a single
    /// inventory slot by default.
    pub fn default_stackable(&self) -> bool {
        match self {
            ItemKind::Equipment | ItemKind::KeyItem | ItemKind::TeachMove => false,
            _ => true,
        }
    }

    /// Whether this kind of item is consumed (removed from inventory) on
    /// successful use by default.
    pub fn default_consumed_on_use(&self) -> bool {
        match self {
            ItemKind::Equipment | ItemKind::KeyItem => false,
            _ => true,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Dummy id type for testing the Custom variant.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum TestKind {
        Tm,
    }

    /// Shorthand: a fully-qualified [`ItemKind`] with the test id type.
    type Kind = ItemKind<TestKind>;

    // ── default_sellable ──────────────────────────────────────────────────

    #[test]
    fn sellable_consumable() {
        assert!(Kind::Consumable.default_sellable());
    }

    #[test]
    fn sellable_equipment() {
        assert!(Kind::Equipment.default_sellable());
    }

    #[test]
    fn sellable_key_item() {
        assert!(!Kind::KeyItem.default_sellable());
    }

    #[test]
    fn sellable_evolution() {
        assert!(!Kind::Evolution.default_sellable());
    }

    #[test]
    fn sellable_stat_boost() {
        assert!(Kind::StatBoost.default_sellable());
    }

    #[test]
    fn sellable_currency() {
        assert!(Kind::Currency.default_sellable());
    }

    #[test]
    fn sellable_teach_move() {
        assert!(!Kind::TeachMove.default_sellable());
    }

    #[test]
    fn sellable_custom() {
        assert!(Kind::Custom(TestKind::Tm).default_sellable());
    }

    // ── default_discardable ───────────────────────────────────────────────

    #[test]
    fn discardable_consumable() {
        assert!(Kind::Consumable.default_discardable());
    }

    #[test]
    fn discardable_equipment() {
        assert!(Kind::Equipment.default_discardable());
    }

    #[test]
    fn discardable_key_item() {
        assert!(!Kind::KeyItem.default_discardable());
    }

    #[test]
    fn discardable_evolution() {
        assert!(Kind::Evolution.default_discardable());
    }

    #[test]
    fn discardable_stat_boost() {
        assert!(Kind::StatBoost.default_discardable());
    }

    #[test]
    fn discardable_currency() {
        assert!(Kind::Currency.default_discardable());
    }

    #[test]
    fn discardable_teach_move() {
        assert!(Kind::TeachMove.default_discardable());
    }

    #[test]
    fn discardable_custom() {
        assert!(Kind::Custom(TestKind::Tm).default_discardable());
    }

    // ── default_stackable ─────────────────────────────────────────────────

    #[test]
    fn stackable_consumable() {
        assert!(Kind::Consumable.default_stackable());
    }

    #[test]
    fn stackable_equipment() {
        assert!(!Kind::Equipment.default_stackable());
    }

    #[test]
    fn stackable_key_item() {
        assert!(!Kind::KeyItem.default_stackable());
    }

    #[test]
    fn stackable_evolution() {
        assert!(Kind::Evolution.default_stackable());
    }

    #[test]
    fn stackable_stat_boost() {
        assert!(Kind::StatBoost.default_stackable());
    }

    #[test]
    fn stackable_currency() {
        assert!(Kind::Currency.default_stackable());
    }

    #[test]
    fn stackable_teach_move() {
        assert!(!Kind::TeachMove.default_stackable());
    }

    #[test]
    fn stackable_custom() {
        assert!(Kind::Custom(TestKind::Tm).default_stackable());
    }

    // ── default_consumed_on_use ────────────────────────────────────────────

    #[test]
    fn consumed_consumable() {
        assert!(Kind::Consumable.default_consumed_on_use());
    }

    #[test]
    fn consumed_equipment() {
        assert!(!Kind::Equipment.default_consumed_on_use());
    }

    #[test]
    fn consumed_key_item() {
        assert!(!Kind::KeyItem.default_consumed_on_use());
    }

    #[test]
    fn consumed_evolution() {
        assert!(Kind::Evolution.default_consumed_on_use());
    }

    #[test]
    fn consumed_stat_boost() {
        assert!(Kind::StatBoost.default_consumed_on_use());
    }

    #[test]
    fn consumed_currency() {
        assert!(Kind::Currency.default_consumed_on_use());
    }

    #[test]
    fn consumed_teach_move() {
        assert!(Kind::TeachMove.default_consumed_on_use());
    }

    #[test]
    fn consumed_custom() {
        assert!(Kind::Custom(TestKind::Tm).default_consumed_on_use());
    }
}
