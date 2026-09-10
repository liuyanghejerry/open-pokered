//! # Equipment system
//!
//! Generic equipment types for equipping and unequipping items in
//! slot-based inventories.
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`EquipSlot<Id>`] | Slot identifier with standard + custom variants |
//! | [`EquipmentSlots<I, S>`] | Slot → item mapping with mutation methods |
//! | [`EquipError`] | Error conditions for equipment operations |

use core::fmt::Debug;
use core::hash::Hash;

// ── EquipSlot ──────────────────────────────────────────────────────────────

/// Represents a slot where an item can be equipped.
///
/// Provides a set of standard equipment slots ([`Weapon`](EquipSlot::Weapon),
/// [`Head`](EquipSlot::Head), [`Body`](EquipSlot::Body),
/// [`Accessory1`](EquipSlot::Accessory1), [`Accessory2`](EquipSlot::Accessory2),
/// [`HeldItem`](EquipSlot::HeldItem)) plus a [`Custom`](EquipSlot::Custom) variant
/// for game-specific slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipSlot<Id: Copy + Eq + Hash + Debug> {
    Weapon,
    Head,
    Body,
    Accessory1,
    Accessory2,
    HeldItem,
    Custom(Id),
}

impl<Id: Copy + Eq + Hash + Debug> EquipSlot<Id> {
    /// Returns a static slice of the six standard equipment slots (all variants
    /// except [`Custom`](EquipSlot::Custom)).
    pub fn standard() -> &'static [Self] {
        use EquipSlot::*;
        &[Weapon, Head, Body, Accessory1, Accessory2, HeldItem]
    }

    /// Returns a human-readable label for this slot.
    pub fn label(&self) -> &str {
        match self {
            EquipSlot::Weapon => "Weapon",
            EquipSlot::Head => "Head",
            EquipSlot::Body => "Body",
            EquipSlot::Accessory1 => "Accessory 1",
            EquipSlot::Accessory2 => "Accessory 2",
            EquipSlot::HeldItem => "Held Item",
            EquipSlot::Custom(_) => "Custom",
        }
    }
}

// ── EquipError ─────────────────────────────────────────────────────────────

/// Error that can occur when equipping an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipError {
    /// The target slot already contains an item (unequip it first).
    SlotFull,
    /// The target slot is not valid for this equipment set.
    InvalidSlot,
}

// ── EquipmentSlots ─────────────────────────────────────────────────────────

/// A collection of equipment slots that tracks which item (if any) is equipped
/// in each slot.
///
/// # Type parameters
///
/// * `I` — Item identifier type.
/// * `S` — Slot identifier type (typically [`EquipSlot<Id>`] or a game-specific
///   enum).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipmentSlots<I: Copy + Eq + Hash + Debug, S: Copy + Eq + Hash + Debug> {
    slots: Vec<(S, Option<I>)>,
}

// ── Constructors ───────────────────────────────────────────────────────────

impl<I: Copy + Eq + Hash + Debug, S: Copy + Eq + Hash + Debug> EquipmentSlots<I, S> {
    /// Create an empty equipment set from the given slots.
    ///
    /// All slots start with no item equipped.
    pub fn new(slots: &[S]) -> Self
    where
        S: Clone,
    {
        Self {
            slots: slots.iter().map(|s| (s.clone(), None)).collect(),
        }
    }

    /// Create an equipment set from a list of slot-item pairs.
    ///
    /// If a slot appears multiple times the last associated item wins. Any
    /// slot not present in `pairs` will not exist in the resulting set.
    pub fn from_pairs(pairs: Vec<(S, I)>) -> Self {
        let mut slots: Vec<(S, Option<I>)> = Vec::with_capacity(pairs.len());
        for (slot, item) in pairs {
            if let Some(existing) = slots.iter_mut().find(|(s, _)| *s == slot) {
                existing.1 = Some(item);
            } else {
                slots.push((slot, Some(item)));
            }
        }
        Self { slots }
    }
}

// ── Queries ────────────────────────────────────────────────────────────────

impl<I: Copy + Eq + Hash + Debug, S: Copy + Eq + Hash + Debug> EquipmentSlots<I, S> {
    /// Returns a reference to the item equipped in `slot`, or `None` if the
    /// slot is empty or does not exist.
    pub fn equipped_in(&self, slot: &S) -> Option<&I> {
        self.slots
            .iter()
            .find(|(s, _)| s == slot)
            .and_then(|(_, item)| item.as_ref())
    }

    /// Returns all occupied slot-item pairs.
    pub fn all_equipped(&self) -> Vec<(S, I)>
    where
        S: Clone,
        I: Clone,
    {
        self.slots
            .iter()
            .filter_map(|(s, item)| item.as_ref().map(|i| (s.clone(), i.clone())))
            .collect()
    }

    /// Returns `true` if `item` is equipped in any slot.
    pub fn is_equipped(&self, item: &I) -> bool {
        self.slots
            .iter()
            .any(|(_, slot_item)| matches!(slot_item, Some(i) if i == item))
    }

    /// Returns the total number of slots (both occupied and empty).
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Returns an iterator over all slot entries.
    pub fn iter(&self) -> impl Iterator<Item = &(S, Option<I>)> {
        self.slots.iter()
    }
}

// ── Mutations ──────────────────────────────────────────────────────────────

impl<I: Copy + Eq + Hash + Debug, S: Copy + Eq + Hash + Debug> EquipmentSlots<I, S> {
    /// Equip `item` into `slot`.
    ///
    /// # Errors
    ///
    /// * [`EquipError::SlotFull`] — the slot already has an item.
    /// * [`EquipError::InvalidSlot`] — `slot` does not exist in this set.
    pub fn equip(&mut self, slot: S, item: I) -> Result<(), EquipError> {
        for (s, current) in self.slots.iter_mut() {
            if *s == slot {
                if current.is_some() {
                    return Err(EquipError::SlotFull);
                }
                *current = Some(item);
                return Ok(());
            }
        }
        Err(EquipError::InvalidSlot)
    }

    /// Remove and return the item equipped in `slot`, or `None` if the slot is
    /// empty or does not exist.
    pub fn unequip(&mut self, slot: &S) -> Option<I> {
        self.slots
            .iter_mut()
            .find(|(s, _)| s == slot)
            .and_then(|(_, item)| item.take())
    }

    /// Swap the items in two slots (including empty ↔ occupied).
    ///
    /// # Errors
    ///
    /// * [`EquipError::InvalidSlot`] — either `a` or `b` does not exist.
    pub fn swap(&mut self, a: &S, b: &S) -> Result<(), EquipError> {
        let a_pos = self.slots.iter().position(|(s, _)| s == a);
        let b_pos = self.slots.iter().position(|(s, _)| s == b);

        match (a_pos, b_pos) {
            (Some(ai), Some(bi)) => {
                let a_item = self.slots[ai].1.take();
                let b_item = self.slots[bi].1.take();
                self.slots[ai].1 = b_item;
                self.slots[bi].1 = a_item;
                Ok(())
            }
            (None, _) | (_, None) => Err(EquipError::InvalidSlot),
        }
    }

    /// Unequip all slots and return the previously-equipped items.
    ///
    /// After this call every slot is empty.
    pub fn clear(&mut self) -> Vec<I> {
        let mut items = Vec::new();
        for (_, slot_item) in self.slots.iter_mut() {
            if let Some(item) = slot_item.take() {
                items.push(item);
            }
        }
        items
    }
}

// ── EquipProvider ──────────────────────────────────────────────────────────

/// Optional provider trait for games with an equipment system.
///
/// Split from [`ItemProvider`](super::ItemProvider) so games without
/// equipment (e.g. early JRPGs) never declare slot or stat placeholder
/// types. Games with equipment implement this *in addition to*
/// [`ItemProvider`](super::ItemProvider); the engine's equipment flows bound
/// on `EquipProvider` only where they actually need slot/bonus data.
///
/// The `Stat` associated type fixes the stat key at impl time, so
/// [`stat_bonuses`](EquipProvider::stat_bonuses) can return real per-item
/// data (a generic `<Stat>` method parameter would let the *caller* pick the
/// type, which no impl could satisfy with anything but an empty slice).
pub trait EquipProvider: super::ItemProvider {
    /// Game-specific equipment slot identifier. Use an uninhabited enum if
    /// the standard [`EquipSlot`] variants suffice.
    type CustomSlot: Copy + Eq + Hash + Debug;
    /// The game's stat identifier for equipment bonuses (typically the same
    /// type as the game's `MonsterProvider::Stat`).
    type Stat: Copy;

    /// Which slots this item can be equipped into. Empty means the item is
    /// not equipment.
    fn equip_slots(&self, item: &Self::Item) -> Vec<EquipSlot<Self::CustomSlot>>;

    /// Additive stat bonuses granted while this item is equipped. The game
    /// applies these to its own monster stats; the engine never computes
    /// totals itself. Defaults to no bonuses.
    fn stat_bonuses(&self, item: &Self::Item) -> &[(Self::Stat, i16)] {
        let _ = item;
        &[]
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal item id for equipment tests.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum TestItem {
        IronSword,
        SteelHelm,
        LeatherArmor,
        RingOfPower,
        Potion,
    }

    type Slot = EquipSlot<&'static str>;

    // -- EquipSlot ---------------------------------------------------------

    #[test]
    fn standard_returns_six_slots() {
        let slots = EquipSlot::<&str>::standard();
        assert_eq!(slots.len(), 6);
        assert!(slots.contains(&EquipSlot::<&str>::Weapon));
        assert!(slots.contains(&EquipSlot::<&str>::Head));
        assert!(slots.contains(&EquipSlot::<&str>::Body));
        assert!(slots.contains(&EquipSlot::<&str>::Accessory1));
        assert!(slots.contains(&EquipSlot::<&str>::Accessory2));
        assert!(slots.contains(&EquipSlot::<&str>::HeldItem));
    }

    #[test]
    fn standard_excludes_custom() {
        let slots = EquipSlot::<&str>::standard();
        assert!(!slots.contains(&EquipSlot::<&str>::Custom("ring")));
    }

    #[test]
    fn label_returns_human_readable_name() {
        assert_eq!(EquipSlot::<&str>::Weapon.label(), "Weapon");
        assert_eq!(EquipSlot::<&str>::Head.label(), "Head");
        assert_eq!(EquipSlot::<&str>::Body.label(), "Body");
        assert_eq!(EquipSlot::<&str>::Accessory1.label(), "Accessory 1");
        assert_eq!(EquipSlot::<&str>::Accessory2.label(), "Accessory 2");
        assert_eq!(EquipSlot::<&str>::HeldItem.label(), "Held Item");
        assert_eq!(EquipSlot::<&str>::Custom("ring").label(), "Custom");
    }

    // -- EquipmentSlots: construction ---------------------------------------

    #[test]
    fn new_creates_empty_slots() {
        let slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert_eq!(slots.slot_count(), 6);
        for slot in EquipSlot::<&str>::standard() {
            assert!(slots.equipped_in(slot).is_none());
        }
    }

    #[test]
    fn new_empty_slice_creates_no_slots() {
        let slots: EquipmentSlots<TestItem, Slot> = EquipmentSlots::new(&[]);
        assert_eq!(slots.slot_count(), 0);
    }

    #[test]
    fn from_pairs_initializes_with_items() {
        let slots: EquipmentSlots<TestItem, Slot> = EquipmentSlots::from_pairs(vec![
            (EquipSlot::Weapon, TestItem::IronSword),
            (EquipSlot::Head, TestItem::SteelHelm),
        ]);
        assert_eq!(slots.slot_count(), 2);
        assert_eq!(
            slots.equipped_in(&EquipSlot::Weapon),
            Some(&TestItem::IronSword)
        );
        assert_eq!(
            slots.equipped_in(&EquipSlot::Head),
            Some(&TestItem::SteelHelm)
        );
    }

    #[test]
    fn from_pairs_deduplicates_slots() {
        let slots: EquipmentSlots<TestItem, Slot> = EquipmentSlots::from_pairs(vec![
            (EquipSlot::Weapon, TestItem::IronSword),
            (EquipSlot::Weapon, TestItem::Potion), // overwrites
        ]);
        assert_eq!(slots.slot_count(), 1);
        assert_eq!(
            slots.equipped_in(&EquipSlot::Weapon),
            Some(&TestItem::Potion)
        );
    }

    // -- EquipmentSlots: equip ----------------------------------------------

    #[test]
    fn equip_succeeds_on_empty_slot() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert_eq!(slots.equip(EquipSlot::Weapon, TestItem::IronSword), Ok(()));
        assert_eq!(
            slots.equipped_in(&EquipSlot::Weapon),
            Some(&TestItem::IronSword)
        );
    }

    #[test]
    fn equip_into_occupied_slot_fails_slot_full() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        let result = slots.equip(EquipSlot::Weapon, TestItem::SteelHelm);
        assert_eq!(result, Err(EquipError::SlotFull));
        // Original item should remain
        assert_eq!(
            slots.equipped_in(&EquipSlot::Weapon),
            Some(&TestItem::IronSword)
        );
    }

    #[test]
    fn equip_invalid_slot_fails_invalid_slot() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        let custom_slot = EquipSlot::Custom("ring");
        let result = slots.equip(custom_slot, TestItem::RingOfPower);
        assert_eq!(result, Err(EquipError::InvalidSlot));
    }

    // -- EquipmentSlots: unequip --------------------------------------------

    #[test]
    fn unequip_returns_item() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        let item = slots.unequip(&EquipSlot::Weapon);
        assert_eq!(item, Some(TestItem::IronSword));
        assert!(slots.equipped_in(&EquipSlot::Weapon).is_none());
    }

    #[test]
    fn unequip_empty_slot_returns_none() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert!(slots.unequip(&EquipSlot::Weapon).is_none());
    }

    #[test]
    fn unequip_invalid_slot_returns_none() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert!(slots.unequip(&EquipSlot::Custom("ring")).is_none());
    }

    // -- EquipmentSlots: is_equipped ----------------------------------------

    #[test]
    fn is_equipped_returns_true_when_item_is_equipped() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        assert!(slots.is_equipped(&TestItem::IronSword));
    }

    #[test]
    fn is_equipped_returns_false_when_item_not_equipped() {
        let slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert!(!slots.is_equipped(&TestItem::IronSword));
    }

    #[test]
    fn is_equipped_returns_false_after_unequip() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        slots.unequip(&EquipSlot::Weapon);
        assert!(!slots.is_equipped(&TestItem::IronSword));
    }

    // -- EquipmentSlots: swap -----------------------------------------------

    #[test]
    fn swap_two_occupied_slots() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        slots.equip(EquipSlot::Head, TestItem::SteelHelm).unwrap();
        slots.swap(&EquipSlot::Weapon, &EquipSlot::Head).unwrap();
        assert_eq!(
            slots.equipped_in(&EquipSlot::Weapon),
            Some(&TestItem::SteelHelm)
        );
        assert_eq!(
            slots.equipped_in(&EquipSlot::Head),
            Some(&TestItem::IronSword)
        );
    }

    #[test]
    fn swap_occupied_with_empty() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        slots.swap(&EquipSlot::Weapon, &EquipSlot::Head).unwrap();
        assert!(slots.equipped_in(&EquipSlot::Weapon).is_none());
        assert_eq!(
            slots.equipped_in(&EquipSlot::Head),
            Some(&TestItem::IronSword)
        );
    }

    #[test]
    fn swap_invalid_slot_fails() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        let result = slots.swap(&EquipSlot::Weapon, &EquipSlot::Custom("ring"));
        assert_eq!(result, Err(EquipError::InvalidSlot));
    }

    // -- EquipmentSlots: clear ----------------------------------------------

    #[test]
    fn clear_returns_all_items() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        slots.equip(EquipSlot::Head, TestItem::SteelHelm).unwrap();
        slots
            .equip(EquipSlot::Body, TestItem::LeatherArmor)
            .unwrap();

        let mut items = slots.clear();
        items.sort_by_key(|i| format!("{:?}", i));
        assert_eq!(items.len(), 3);
        assert!(items.contains(&TestItem::IronSword));
        assert!(items.contains(&TestItem::SteelHelm));
        assert!(items.contains(&TestItem::LeatherArmor));

        // All slots should be empty now
        assert!(slots.equipped_in(&EquipSlot::Weapon).is_none());
        assert!(slots.equipped_in(&EquipSlot::Head).is_none());
        assert!(slots.equipped_in(&EquipSlot::Body).is_none());
    }

    #[test]
    fn clear_on_empty_slots_returns_empty_vec() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        let items = slots.clear();
        assert!(items.is_empty());
        assert_eq!(slots.slot_count(), 6);
    }

    // -- EquipmentSlots: all_equipped ---------------------------------------

    #[test]
    fn all_equipped_returns_only_occupied_slots() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        slots.equip(EquipSlot::Head, TestItem::SteelHelm).unwrap();
        // Accessory1, Accessory2, Body, HeldItem remain empty

        let equipped = slots.all_equipped();
        assert_eq!(equipped.len(), 2);
        assert!(equipped.contains(&(EquipSlot::Weapon, TestItem::IronSword)));
        assert!(equipped.contains(&(EquipSlot::Head, TestItem::SteelHelm)));
    }

    #[test]
    fn all_equipped_returns_empty_when_nothing_equipped() {
        let slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert!(slots.all_equipped().is_empty());
    }

    // -- EquipmentSlots: slot_count -----------------------------------------

    #[test]
    fn slot_count_returns_total_slots() {
        let slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert_eq!(slots.slot_count(), 6);
    }

    #[test]
    fn slot_count_unchanged_by_equip_or_unequip() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        assert_eq!(slots.slot_count(), 6);
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        assert_eq!(slots.slot_count(), 6);
        slots.unequip(&EquipSlot::Weapon);
        assert_eq!(slots.slot_count(), 6);
    }

    // -- EquipmentSlots: iter -----------------------------------------------

    #[test]
    fn iter_yields_all_slots() {
        let mut slots: EquipmentSlots<TestItem, Slot> =
            EquipmentSlots::new(EquipSlot::<&str>::standard());
        slots.equip(EquipSlot::Weapon, TestItem::IronSword).unwrap();
        let entries: Vec<_> = slots.iter().collect();
        assert_eq!(entries.len(), 6);
        // Weapon slot should have the item
        let weapon_entry = entries
            .iter()
            .find(|(s, _)| *s == EquipSlot::Weapon)
            .unwrap();
        assert_eq!(weapon_entry.1, Some(TestItem::IronSword));
    }

    // -- EquipProvider -------------------------------------------------------

    use crate::items::{ItemKind, ItemProvider, ItemResult};

    /// Stat key for the EquipProvider test game.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestStat {
        Attack,
        Defense,
    }

    struct TestGame;

    impl ItemProvider for TestGame {
        type Item = TestItem;
        type Effect = ();
        type Monster = ();
        type CustomKind = ();

        fn item_name(&self, _item: &TestItem) -> &str {
            "X"
        }
        fn item_description(&self, _item: &TestItem) -> &str {
            "X"
        }
        fn item_effect(&self, _item: &TestItem) {}
        fn item_price(&self, _item: &TestItem) -> u32 {
            0
        }
        fn can_use_outside_battle(&self, _item: &TestItem) -> bool {
            false
        }
        fn can_use_in_battle(&self, _item: &TestItem) -> bool {
            false
        }
        fn use_on_monster(&self, _item: &TestItem, _m: &mut ()) -> ItemResult {
            ItemResult::NoEffect
        }
        fn consume(&self, _item: &TestItem) -> bool {
            false
        }
        fn item_kind(&self, item: &TestItem) -> ItemKind<()> {
            match item {
                TestItem::Potion => ItemKind::Consumable,
                _ => ItemKind::Equipment,
            }
        }
    }

    impl EquipProvider for TestGame {
        type CustomSlot = &'static str;
        type Stat = TestStat;

        fn equip_slots(&self, item: &TestItem) -> Vec<Slot> {
            match item {
                TestItem::IronSword => vec![EquipSlot::Weapon],
                TestItem::SteelHelm => vec![EquipSlot::Head],
                TestItem::RingOfPower => vec![EquipSlot::Accessory1, EquipSlot::Accessory2],
                _ => Vec::new(),
            }
        }

        fn stat_bonuses(&self, item: &TestItem) -> &[(TestStat, i16)] {
            match item {
                TestItem::IronSword => &[(TestStat::Attack, 5)],
                TestItem::SteelHelm => &[(TestStat::Defense, 3)],
                TestItem::RingOfPower => &[(TestStat::Attack, 2), (TestStat::Defense, 2)],
                _ => &[],
            }
        }
    }

    #[test]
    fn equip_provider_returns_real_stat_bonuses() {
        let game = TestGame;
        assert_eq!(
            game.stat_bonuses(&TestItem::IronSword),
            &[(TestStat::Attack, 5)]
        );
        assert_eq!(
            game.stat_bonuses(&TestItem::RingOfPower),
            &[(TestStat::Attack, 2), (TestStat::Defense, 2)]
        );
        assert!(game.stat_bonuses(&TestItem::Potion).is_empty());
    }

    #[test]
    fn equip_provider_slots_gate_equippability() {
        let game = TestGame;
        assert_eq!(game.equip_slots(&TestItem::IronSword), vec![Slot::Weapon]);
        assert_eq!(
            game.equip_slots(&TestItem::RingOfPower),
            vec![Slot::Accessory1, Slot::Accessory2]
        );
        assert!(game.equip_slots(&TestItem::Potion).is_empty());
    }
}
