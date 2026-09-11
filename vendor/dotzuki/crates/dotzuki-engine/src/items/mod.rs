//! # Items module
//!
//! Core traits and types for item management in a JRPG engine.
//!
//! Provides [`ItemProvider`] for querying item metadata and applying effects,
//! and [`ShopProvider`] for shop inventories.  Both traits use associated types
//! so that implementing crates supply their own concrete item, effect, monster,
//! and shop-identifier types — no game-specific data lives in this module.
//!
//! ## Supporting types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`Inventory<I>`] | Generic item inventory with `add` / `remove` / `contains` |
//! | [`ItemResult`] | Outcome of attempting to use an item |
//! | [`BagCategory`] | Broad classification of item types for UI organisation |

use core::cmp::Ordering;
use core::fmt::Debug;
use core::hash::Hash;

pub mod equip;
pub mod kind;
pub mod mart;
pub mod use_driver;
pub use equip::{EquipProvider, EquipSlot};
pub use kind::ItemKind;
pub use mart::{MartBackend, MartDriver, MartState, MartStock};
pub use use_driver::{buy, sell, use_item, ItemUseResult, ShopError, ShopReceipt, UsageContext};

// ── Supporting types ──────────────────────────────────────────────────────

/// Outcome of attempting to apply an item to a monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemResult {
    /// Item was consumed and its effect applied successfully.
    Used,
    /// The item cannot be used in the current context (e.g., battle-only item
    /// used outside battle).
    NotUsable,
    /// The player's bag does not contain this item.
    NotOwned,
    /// Item was applicable but produced no effect (e.g., healing a fully-healed
    /// monster).
    NoEffect,
}

/// Broad classification of item types, used by UI code to organise bag menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BagCategory {
    /// General consumables and utility items.
    Items,
    /// HP / PP / status recovery items.
    Medicine,
    /// Capture devices (balls / traps / etc.).
    Balls,
    /// Items usable only in battle (X items, Guard Spec., etc.).
    Battle,
    /// Key / plot items that cannot be sold or discarded.
    Key,
    /// Catch-all for categories not covered above.
    Other,
}

/// Error returned when adding an item to an inventory fails due to capacity
/// limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddError {
    /// The inventory has reached its maximum number of distinct item slots.
    InventoryFull,
    /// A single slot has reached its per-slot quantity cap.
    PerSlotCapReached(u32),
}

/// Generic item inventory that stores `(item, quantity)` pairs.
///
/// The type parameter `I` is the item identifier type, which must satisfy
/// `Copy + Eq + Hash + Debug`.  Items with the same identity are stacked
/// into a single slot — no duplicate entries.
///
/// The const parameter `N` is the **fixed slot capacity**: the inventory is
/// stored inline in an array of `N` slots (zero heap allocation) and can hold
/// at most `N` distinct items.  `max_per_slot` is an optional quantity cap;
/// when `None` (the default, via [`new`](Inventory::new)) quantities are
/// effectively unlimited.
///
/// Occupied slots are kept contiguous at the front of the backing array, so
/// removal shifts subsequent slots left — identical ordering semantics to the
/// former `Vec`-backed implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory<I: Copy + Eq + Hash + Debug, const N: usize> {
    /// Fixed-capacity slot storage; occupied slots live at `items[..len]`.
    items: [Option<(I, u32)>; N],
    /// Number of occupied slots (contiguous prefix of `items`).
    len: usize,
    /// Maximum quantity per slot (`None` = unlimited).
    max_per_slot: Option<u32>,
}

/// Convenience alias for a large-capacity inventory.
///
/// `N = 256` is effectively unbounded for any real DOTZUKI item table.  This
/// alias exists for forward-compatibility: if a second generic parameter is
/// ever added to `Inventory` for tag/kind filtering, `SimpleInventory` will
/// expand to `Inventory<I, 256, ()>` so that existing usage keeps compiling.
pub type SimpleInventory<I> = Inventory<I, 256>;

impl<I: Copy + Eq + Hash + Debug, const N: usize> Inventory<I, N> {
    /// Create an empty inventory with `N` slots and no per-slot quantity cap.
    pub fn new() -> Self {
        Self {
            items: [None; N],
            len: 0,
            max_per_slot: None,
        }
    }

    /// Create an empty inventory with `N` slots and the given per-slot
    /// quantity cap.
    pub fn with_capacity(max_per_slot: u32) -> Self {
        Self {
            items: [None; N],
            len: 0,
            max_per_slot: Some(max_per_slot),
        }
    }

    /// Number of distinct item slots (not total item count).
    pub fn count(&self) -> usize {
        self.len
    }

    /// Returns `true` if the inventory holds no items.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Maximum number of distinct item slots (`N`).
    pub fn capacity(&self) -> usize {
        N
    }

    /// Returns `true` if the inventory holds at least `quantity` of `item`.
    pub fn contains(&self, item: &I, quantity: u32) -> bool {
        self.iter().any(|(i, q)| i == item && *q >= quantity)
    }

    /// Add `quantity` copies of `item`.
    ///
    /// If the item already exists in a slot the quantities are merged;
    /// otherwise a new slot is appended.
    ///
    /// # Errors
    ///
    /// Returns [`AddError::InventoryFull`] if the inventory has reached its
    /// slot limit and the item would require a new slot.
    ///
    /// Returns [`AddError::PerSlotCapReached`] if the combined quantity would
    /// exceed the per-slot cap.
    pub fn add(&mut self, item: I, quantity: u32) -> Result<(), AddError> {
        if quantity == 0 {
            return Ok(());
        }
        // Reject if adding to an existing slot would exceed per-slot cap.
        if self.would_exceed_per_slot_cap(&item, quantity) {
            return Err(AddError::PerSlotCapReached(self.max_per_slot.unwrap()));
        }
        // If the item does not already exist, check the slot cap.
        let exists = self.iter().any(|(i, _)| *i == item);
        if !exists && self.is_full() {
            return Err(AddError::InventoryFull);
        }
        for slot in &mut self.items[..self.len] {
            if let Some((existing, qty)) = slot {
                if *existing == item {
                    *qty = qty.saturating_add(quantity);
                    return Ok(());
                }
            }
        }
        self.items[self.len] = Some((item, quantity));
        self.len += 1;
        Ok(())
    }

    /// Remove up to `quantity` copies of `item` from the first matching slot.
    /// Returns `true` if the removal succeeded (item found and quantity
    /// sufficient).
    pub fn remove(&mut self, item: &I, quantity: u32) -> bool {
        for i in 0..self.len {
            if let Some((existing, qty)) = &mut self.items[i] {
                if existing == item {
                    if *qty < quantity {
                        return false;
                    }
                    if *qty == quantity {
                        self.remove_at(i);
                    } else {
                        *qty -= quantity;
                    }
                    return true;
                }
            }
        }
        false
    }

    // ── New methods ───────────────────────────────────────────────────────

    /// Quantity of `item` in the inventory (0 if not owned).
    pub fn quantity(&self, item: &I) -> u32 {
        self.iter()
            .find(|(i, _)| i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }

    /// Returns `true` if the inventory has reached its slot limit.
    pub fn is_full(&self) -> bool {
        self.len >= N
    }

    /// Returns `true` if adding `add_quantity` of `item` would exceed the
    /// per-slot quantity cap.
    pub fn would_exceed_per_slot_cap(&self, item: &I, add_quantity: u32) -> bool {
        let Some(cap) = self.max_per_slot else {
            return false;
        };
        let current = self.quantity(item);
        current.saturating_add(add_quantity) > cap
    }

    /// Return all slot entries matching a predicate.
    pub fn filter<F>(&self, pred: F) -> Vec<&(I, u32)>
    where
        F: Fn(&I) -> bool,
    {
        self.iter().filter(|(i, _)| pred(i)).collect()
    }

    /// Sort slots by a custom comparator.
    pub fn sort_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&(I, u32), &(I, u32)) -> Ordering,
    {
        self.items[..self.len].sort_by(|a, b| cmp(a.as_ref().unwrap(), b.as_ref().unwrap()));
    }

    /// Sort slots by item name, using the provided `name_fn` to extract a
    /// string name from each item.
    pub fn sort_by_name<F>(&mut self, name_fn: F)
    where
        F: Fn(&I) -> &str,
    {
        self.items[..self.len]
            .sort_by(|a, b| name_fn(&a.as_ref().unwrap().0).cmp(name_fn(&b.as_ref().unwrap().0)));
    }

    /// Consume the inventory and return its occupied slots as a `Vec`.
    pub fn into_inner(self) -> Vec<(I, u32)> {
        self.items.iter().flatten().copied().collect()
    }

    /// Iterate over all `(item, quantity)` entries.
    pub fn iter(&self) -> impl Iterator<Item = &(I, u32)> {
        self.items[..self.len].iter().filter_map(Option::as_ref)
    }

    /// Mutably iterate over all `(item, quantity)` entries.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (I, u32)> {
        self.items[..self.len].iter_mut().filter_map(Option::as_mut)
    }

    /// The `(item, quantity)` entry at `index`, if occupied.
    pub fn get(&self, index: usize) -> Option<&(I, u32)> {
        self.items.get(index).and_then(Option::as_ref)
    }

    /// Mutable access to the `(item, quantity)` entry at `index`.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut (I, u32)> {
        self.items.get_mut(index).and_then(Option::as_mut)
    }

    /// Append a new slot at the end (caller must ensure `!is_full()`).
    /// Per-slot quantities are not merged or capped here.
    pub fn push_slot(&mut self, item: I, quantity: u32) -> Result<(), AddError> {
        if self.is_full() {
            return Err(AddError::InventoryFull);
        }
        self.items[self.len] = Some((item, quantity));
        self.len += 1;
        Ok(())
    }

    /// Remove the slot at `index`, shifting subsequent slots left.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds (not an occupied slot).
    pub fn remove_at(&mut self, index: usize) {
        assert!(index < self.len, "inventory slot index out of bounds");
        self.items.copy_within(index + 1..self.len, index);
        self.items[self.len - 1] = None;
        self.len -= 1;
    }

    /// Swap the two slots at `a` and `b` (both must be occupied).
    ///
    /// # Panics
    ///
    /// Panics if either index is out of bounds.
    pub fn swap(&mut self, a: usize, b: usize) {
        self.items.swap(a, b);
    }

    /// Remove all items.
    pub fn clear(&mut self) {
        self.items.fill(None);
        self.len = 0;
    }
}

impl<I: Copy + Eq + Hash + Debug, const N: usize> Default for Inventory<I, N> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Traits ────────────────────────────────────────────────────────────────

/// Provider trait for item metadata, usage rules, and effects.
///
/// Game-specific crates implement this trait to supply all item data the
/// engine needs: names, descriptions, prices, usage eligibility, and the
/// logic for applying an item to a monster.
///
/// # Associated types
///
/// * `Item` — The item identifier (typically an enum).
/// * `Effect` — The effect descriptor (heal amount, status cure, etc.).
/// * `Monster` — The monster / character type that items can target.
pub trait ItemProvider {
    /// Concrete item identifier type.
    type Item: Copy + Eq + Hash + Debug;
    /// Describes what the item does when used.
    type Effect;
    /// The monster / party-member type that items may be applied to.
    type Monster;

    /// Game-specific item kind discriminant (e.g. an enum of custom sub-categories).
    type CustomKind: Copy + Eq + Hash + Debug;

    /// Human-readable name of the item (e.g., `"Potion"`).
    fn item_name(&self, item: &Self::Item) -> &str;

    /// In-game flavour / description text.
    fn item_description(&self, item: &Self::Item) -> &str;

    /// The effect this item produces.
    fn item_effect(&self, item: &Self::Item) -> Self::Effect;

    /// Base purchase / sale price in the in-game currency.
    fn item_price(&self, item: &Self::Item) -> u32;

    /// Whether the item may be used outside of battle (e.g. from the bag
    /// menu on the overworld).
    fn can_use_outside_battle(&self, item: &Self::Item) -> bool;

    /// Whether the item may be used during battle.
    fn can_use_in_battle(&self, item: &Self::Item) -> bool;

    /// Attempt to apply the item's effect to `monster`.
    ///
    /// Returns [`ItemResult::Used`] on success, or an appropriate error
    /// variant if the item could not be applied.
    fn use_on_monster(&self, item: &Self::Item, monster: &mut Self::Monster) -> ItemResult;

    /// Returns `true` if the item is consumed (removed from inventory) after
    /// a successful use.  Permanent items (key items, reusable tools, etc.)
    /// return `false`.
    fn consume(&self, item: &Self::Item) -> bool;

    /// Classify the item into a gameplay category (`Consumable`, `Equipment`,
    /// `KeyItem`, `Custom(...)`, etc.). Used by the engine for default
    /// shop/bag behaviours.
    ///
    /// Equipment metadata (slots, stat bonuses) lives on the optional
    /// [`EquipProvider`](crate::items::equip::EquipProvider) trait so that
    /// games without an equipment system never declare slot or stat types.
    fn item_kind(&self, item: &Self::Item) -> ItemKind<Self::CustomKind>;

    /// Called when this item is used to teach a move to `target`. Returns
    /// `None` (the default) if this item is not a move-teaching item.
    fn on_teach_move<M: crate::party::MonsterProvider>(
        &self,
        item: Self::Item,
        target: &mut crate::party::MonsterInstance<M>,
    ) -> Option<ItemUseResult<Self::Item>> {
        let _ = (item, target);
        None
    }

    /// Called when this item is used in the field (overworld). Returns
    /// `None` (the default) if the item has no field effect.
    fn on_use_field(&self, item: Self::Item) -> Option<ItemUseResult<Self::Item>> {
        let _ = item;
        None
    }

    // ── P0e: opaque item-effect dispatch (additive, defaulted) ──────────────

    /// Where / whether this item may be used (field, battle, both, or none).
    ///
    /// Defaults to [`UsageContext::FieldAndBattle`]. The
    /// [`use_item`](crate::items::use_item) driver uses this to gate usage by
    /// the active context before dispatching the effect.
    fn usable_in(&self, item: &Self::Item) -> UsageContext {
        let _ = item;
        UsageContext::FieldAndBattle
    }

    /// Apply the item's effect to an optional target monster, in a context.
    ///
    /// This is an **opaque** dispatch: the engine routes the call and the game
    /// owns ALL numbers — heal amounts, status cures, vitamins, level-up candy,
    /// repel steps, capture rolls, and any game-specific item bugs. The engine
    /// only reports the [`ItemUseResult`] back to
    /// [`use_item`](crate::items::use_item), which consumes from the bag on
    /// success.
    ///
    /// `provider` is the game's [`MonsterProvider`](crate::party::MonsterProvider)
    /// instance, passed through so the effect implementation can query species
    /// data, stat formulas, etc.
    ///
    /// Generic over the [`MonsterProvider`](crate::party::MonsterProvider) so
    /// the engine never couples to a concrete monster model. Defaults to
    /// [`ItemUseResult::NoEffect`] so games (and tests) that only need bag/shop
    /// bookkeeping compile unchanged.
    fn apply_effect<M: crate::party::MonsterProvider>(
        &self,
        provider: &M,
        item: Self::Item,
        ctx: UsageContext,
        target: Option<&mut crate::party::MonsterInstance<M>>,
        rng: &mut dyn crate::battle::rng::BattleRng,
    ) -> ItemUseResult<Self::Item> {
        let _ = (provider, item, ctx, target, rng);
        ItemUseResult::NoEffect
    }
}

/// Provider trait for shop / mart data.
///
/// Shops in a JRPG may sell items at prices that differ from the item's
/// base price.  This trait lets the engine query a shop's inventory and
/// its display name.
///
/// # Associated types
///
/// * `Item` — The item identifier (must match the [`ItemProvider::Item`] type).
/// * `ShopId` — The shop identifier (typically an enum of shop locations).
pub trait ShopProvider {
    /// Concrete item identifier type.
    type Item: Copy + Eq + Hash + Debug;
    /// Shop location / identity type.
    type ShopId: Copy + Eq + Hash + Debug;

    /// Returns the shop's inventory as `(item, price)` pairs.
    ///
    /// The price in each pair is the price **this specific shop** charges,
    /// which may differ from the base [`ItemProvider::item_price`].
    fn shop_inventory(&self, shop_id: &Self::ShopId) -> Vec<(Self::Item, u32)>;

    /// Human-readable name of the shop (e.g., `"City Mart"`).
    fn shop_name(&self, shop_id: &Self::ShopId) -> &str;

    // ── P0e: buy/sell pricing (additive, defaulted) ─────────────────────────

    /// Price the player pays to buy one unit of `item`.
    ///
    /// Defaults to `0`; games override with their list price. Used by the
    /// [`buy`](crate::items::buy) driver.
    fn buy_price(&self, item: &Self::Item) -> u32 {
        let _ = item;
        0
    }

    /// Price the player receives for selling one unit of `item`.
    ///
    /// Defaults to half the [`buy_price`](ShopProvider::buy_price), matching
    /// the Gen-1 mart sell rate. Used by the [`sell`](crate::items::sell)
    /// driver.
    fn sell_price(&self, item: &Self::Item) -> u32 {
        self.buy_price(item) / 2
    }

    /// Whether the shop will buy `item` from the player.
    ///
    /// Defaults to `true`; games return `false` for key items and anything
    /// else that cannot be sold. Used by the [`sell`](crate::items::sell)
    /// driver.
    fn can_sell(&self, item: &Self::Item) -> bool {
        let _ = item;
        true
    }

    // ── P0e: discount & stock limit features (additive, defaulted) ────────

    /// Discount multiplier applied when buying from this shop.
    ///
    /// `1.0` = full price, `0.8` = 20 % off, `1.2` = premium surcharge.
    /// Used by the [`buy`](crate::items::buy) driver.
    fn discount_rate(&self, _shop_id: &Self::ShopId) -> f32 {
        1.0
    }

    /// Per-shop sell-back multiplier applied **on top of**
    /// [`sell_price`](ShopProvider::sell_price).
    ///
    /// Defaults to `1.0` (pass-through). The Gen-1 half-price rule is already
    /// encoded in the `sell_price` default (`buy_price / 2`), so a non-unit
    /// default here would compose to quarter price. Override per shop for
    /// special vendors that pay more or less than the game-wide sell price.
    fn sell_rate(&self, _shop_id: &Self::ShopId) -> f32 {
        1.0
    }

    /// Whether `item` has limited stock in this shop.
    ///
    /// Defaults to `false` — unlimited stock by default.
    fn has_limited_stock(&self, _item: &Self::Item) -> bool {
        false
    }

    /// Maximum stock count for `item` when [`has_limited_stock`] is `true`.
    ///
    /// Defaults to `0`; meaningful only when `has_limited_stock` returns
    /// `true`.
    fn max_stock(&self, _item: &Self::Item) -> u32 {
        0
    }

    /// Whether this shop periodically restocks sold‑out items.
    ///
    /// Defaults to `false`.
    fn restocks(&self, _shop_id: &Self::ShopId) -> bool {
        false
    }

    /// How many game‑ticks / steps / frames between restock cycles.
    ///
    /// Defaults to `0`; meaningful only when [`restocks`] returns `true`.
    fn restock_interval(&self, _shop_id: &Self::ShopId) -> u32 {
        0
    }
}

// ── Mock tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- Mock types ---------------------------------------------------------

    /// Minimal item type for testing the provider traits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct MockItem {
        name: &'static str,
        price: u32,
        heal_amount: u32,
    }

    /// Effect descriptor for mock items.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockEffect {
        Heal(u32),
        None,
    }

    /// Minimal monster for testing item application.
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct MockMonster {
        name: &'static str,
        max_hp: u32,
        current_hp: u32,
    }

    // -- Mock providers -----------------------------------------------------

    struct MockItemProvider;

    impl ItemProvider for MockItemProvider {
        type Item = MockItem;
        type Effect = MockEffect;
        type Monster = MockMonster;
        type CustomKind = ();

        fn item_name(&self, item: &Self::Item) -> &str {
            item.name
        }

        fn item_description(&self, item: &Self::Item) -> &str {
            if item.heal_amount > 0 {
                "Restores HP."
            } else {
                "Has no effect in battle."
            }
        }

        fn item_effect(&self, item: &Self::Item) -> Self::Effect {
            if item.heal_amount > 0 {
                MockEffect::Heal(item.heal_amount)
            } else {
                MockEffect::None
            }
        }

        fn item_price(&self, item: &Self::Item) -> u32 {
            item.price
        }

        fn can_use_outside_battle(&self, _item: &Self::Item) -> bool {
            true
        }

        fn can_use_in_battle(&self, _item: &Self::Item) -> bool {
            true
        }

        fn use_on_monster(&self, item: &Self::Item, monster: &mut Self::Monster) -> ItemResult {
            match self.item_effect(item) {
                MockEffect::Heal(amount) => {
                    if monster.current_hp >= monster.max_hp {
                        return ItemResult::NoEffect;
                    }
                    monster.current_hp = (monster.current_hp + amount).min(monster.max_hp);
                    ItemResult::Used
                }
                MockEffect::None => ItemResult::NoEffect,
            }
        }

        fn consume(&self, _item: &Self::Item) -> bool {
            true
        }

        fn item_kind(&self, item: &Self::Item) -> ItemKind<()> {
            if item.heal_amount > 0 {
                ItemKind::Consumable
            } else {
                ItemKind::Consumable
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum MockShopId {
        CityMart,
    }

    struct MockShopProvider;

    impl ShopProvider for MockShopProvider {
        type Item = MockItem;
        type ShopId = MockShopId;

        fn shop_inventory(&self, shop_id: &Self::ShopId) -> Vec<(Self::Item, u32)> {
            match shop_id {
                MockShopId::CityMart => vec![
                    (
                        MockItem {
                            name: "Potion",
                            price: 300,
                            heal_amount: 20,
                        },
                        300,
                    ),
                    (
                        MockItem {
                            name: "Elixir",
                            price: 500,
                            heal_amount: 0,
                        },
                        500,
                    ),
                ],
            }
        }

        fn shop_name(&self, shop_id: &Self::ShopId) -> &str {
            match shop_id {
                MockShopId::CityMart => "City Mart",
            }
        }
    }

    // -- Tests: ItemProvider ------------------------------------------------

    #[test]
    fn potion_heals_monster() {
        let provider = MockItemProvider;
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        let mut monster = MockMonster {
            name: "Sprout",
            max_hp: 100,
            current_hp: 50,
        };

        let result = provider.use_on_monster(&potion, &mut monster);
        assert_eq!(result, ItemResult::Used);
        assert_eq!(monster.current_hp, 70);
        assert!(provider.consume(&potion));
    }

    #[test]
    fn potion_no_effect_on_full_hp() {
        let provider = MockItemProvider;
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        let mut monster = MockMonster {
            name: "Sprout",
            max_hp: 100,
            current_hp: 100,
        };

        let result = provider.use_on_monster(&potion, &mut monster);
        assert_eq!(result, ItemResult::NoEffect);
        assert_eq!(monster.current_hp, 100);
    }

    #[test]
    fn elixir_has_no_heal_effect() {
        let provider = MockItemProvider;
        let elixir = MockItem {
            name: "Elixir",
            price: 500,
            heal_amount: 0,
        };
        let mut monster = MockMonster {
            name: "Sprout",
            max_hp: 100,
            current_hp: 50,
        };

        let result = provider.use_on_monster(&elixir, &mut monster);
        assert_eq!(result, ItemResult::NoEffect);
        assert_eq!(monster.current_hp, 50); // unchanged
    }

    // -- Tests: ShopProvider ------------------------------------------------

    #[test]
    fn shop_inventory_has_two_items() {
        let provider = MockShopProvider;
        let inventory = provider.shop_inventory(&MockShopId::CityMart);

        assert_eq!(inventory.len(), 2);
        assert_eq!(inventory[0].0.name, "Potion");
        assert_eq!(inventory[0].1, 300);
        assert_eq!(inventory[1].0.name, "Elixir");
        assert_eq!(inventory[1].1, 500);
    }

    #[test]
    fn shop_name_is_correct() {
        let provider = MockShopProvider;
        assert_eq!(provider.shop_name(&MockShopId::CityMart), "City Mart");
    }

    // -- Tests: Inventory ---------------------------------------------------

    #[test]
    fn inventory_add_and_remove() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };

        inv.add(potion, 3).unwrap();
        assert_eq!(inv.count(), 1);
        assert!(inv.contains(&potion, 2));
        assert!(!inv.contains(&potion, 4));

        assert!(inv.remove(&potion, 2));
        assert_eq!(inv.count(), 1);
        assert!(inv.contains(&potion, 1));

        assert!(inv.remove(&potion, 1));
        assert_eq!(inv.count(), 0);
        assert!(!inv.contains(&potion, 1));
    }

    #[test]
    fn inventory_stacks_same_item() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };

        inv.add(potion, 3).unwrap();
        inv.add(potion, 5).unwrap();
        assert_eq!(inv.count(), 1); // merged, not a new slot
        assert!(inv.contains(&potion, 8));
    }

    #[test]
    fn inventory_remove_insufficient_quantity() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };

        inv.add(potion, 2).unwrap();
        assert!(!inv.remove(&potion, 5));
        assert_eq!(inv.count(), 1);
        assert!(inv.contains(&potion, 2)); // unchanged
    }

    #[test]
    fn inventory_remove_nonexistent_item() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };

        assert!(!inv.remove(&potion, 1));
    }

    // ── New inventory tests ────────────────────────────────────────────────

    #[test]
    fn inventory_new_is_unlimited() {
        let inv: Inventory<MockItem, 8> = Inventory::new();
        assert!(!inv.is_full());
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        assert!(!inv.would_exceed_per_slot_cap(&potion, u32::MAX));
    }

    #[test]
    fn inventory_with_capacity_rejects_overfill() {
        let mut inv = Inventory::<MockItem, 2>::with_capacity(10);
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        let elixir = MockItem {
            name: "Elixir",
            price: 500,
            heal_amount: 0,
        };
        let antidote = MockItem {
            name: "Antidote",
            price: 200,
            heal_amount: 0,
        };

        assert!(inv.add(potion, 1).is_ok());
        assert!(inv.add(elixir, 1).is_ok());
        assert_eq!(inv.add(antidote, 1), Err(AddError::InventoryFull));
    }

    #[test]
    fn inventory_with_capacity_rejects_per_slot_overflow() {
        let mut inv = Inventory::<MockItem, 10>::with_capacity(5);
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };

        assert!(inv.add(potion, 3).is_ok());
        assert!(inv.add(potion, 2).is_ok()); // total = 5, at cap
        assert_eq!(inv.add(potion, 1), Err(AddError::PerSlotCapReached(5)));
    }

    #[test]
    fn inventory_quantity() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };

        assert_eq!(inv.quantity(&potion), 0);
        inv.add(potion, 3).unwrap();
        assert_eq!(inv.quantity(&potion), 3);
    }

    #[test]
    fn inventory_add_zero_is_ok() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        assert!(inv.add(potion, 0).is_ok());
        assert_eq!(inv.count(), 0);
    }

    #[test]
    fn inventory_filter() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        inv.add(
            MockItem {
                name: "Potion",
                price: 300,
                heal_amount: 20,
            },
            1,
        )
        .unwrap();
        inv.add(
            MockItem {
                name: "Elixir",
                price: 500,
                heal_amount: 0,
            },
            1,
        )
        .unwrap();
        inv.add(
            MockItem {
                name: "Antidote",
                price: 200,
                heal_amount: 0,
            },
            1,
        )
        .unwrap();

        let cheap = inv.filter(|i| i.price < 350);
        assert_eq!(cheap.len(), 2); // Potion + Antidote
    }

    #[test]
    fn inventory_sort_by_name() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let antidote = MockItem {
            name: "Antidote",
            price: 200,
            heal_amount: 0,
        };
        let elixir = MockItem {
            name: "Elixir",
            price: 500,
            heal_amount: 0,
        };
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };

        inv.add(elixir, 1).unwrap();
        inv.add(antidote, 1).unwrap();
        inv.add(potion, 1).unwrap();

        inv.sort_by_name(|i| i.name);
        assert_eq!(inv.get(0).unwrap().0.name, "Antidote");
        assert_eq!(inv.get(1).unwrap().0.name, "Elixir");
        assert_eq!(inv.get(2).unwrap().0.name, "Potion");
    }

    #[test]
    fn inventory_sort_by_price() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        inv.add(
            MockItem {
                name: "Potion",
                price: 300,
                heal_amount: 20,
            },
            1,
        )
        .unwrap();
        inv.add(
            MockItem {
                name: "Antidote",
                price: 200,
                heal_amount: 0,
            },
            1,
        )
        .unwrap();
        inv.add(
            MockItem {
                name: "Elixir",
                price: 500,
                heal_amount: 0,
            },
            1,
        )
        .unwrap();

        inv.sort_by(|a, b| a.0.price.cmp(&b.0.price));
        assert_eq!(inv.get(0).unwrap().0.name, "Antidote");
        assert_eq!(inv.get(1).unwrap().0.name, "Potion");
        assert_eq!(inv.get(2).unwrap().0.name, "Elixir");
    }

    #[test]
    fn inventory_into_inner() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        inv.add(potion, 3).unwrap();

        let inner = inv.into_inner();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].0.name, "Potion");
        assert_eq!(inner[0].1, 3);
    }

    #[test]
    fn inventory_iter() {
        let mut inv: Inventory<MockItem, 8> = Inventory::new();
        inv.add(
            MockItem {
                name: "Potion",
                price: 300,
                heal_amount: 20,
            },
            1,
        )
        .unwrap();
        inv.add(
            MockItem {
                name: "Elixir",
                price: 500,
                heal_amount: 0,
            },
            1,
        )
        .unwrap();

        let names: Vec<&str> = inv.iter().map(|(i, _)| i.name).collect();
        assert_eq!(names, vec!["Potion", "Elixir"]);
    }

    #[test]
    fn inventory_simple_inventory_alias() {
        let mut inv: SimpleInventory<MockItem> = SimpleInventory::new();
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        inv.add(potion, 1).unwrap();
        assert_eq!(inv.count(), 1);
    }

    #[test]
    fn inventory_is_full_false_when_under_cap() {
        let mut inv = Inventory::<MockItem, 3>::with_capacity(99);
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        assert!(!inv.is_full());
        inv.add(potion, 1).unwrap();
        assert!(!inv.is_full());
    }

    #[test]
    fn inventory_is_full_true_at_cap() {
        let mut inv = Inventory::<MockItem, 2>::with_capacity(99);
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        let elixir = MockItem {
            name: "Elixir",
            price: 500,
            heal_amount: 0,
        };
        inv.add(potion, 1).unwrap();
        inv.add(elixir, 1).unwrap();
        assert!(inv.is_full());
    }

    #[test]
    fn inventory_would_exceed_per_slot_cap() {
        let mut inv = Inventory::<MockItem, 10>::with_capacity(5);
        let potion = MockItem {
            name: "Potion",
            price: 300,
            heal_amount: 20,
        };
        assert!(!inv.would_exceed_per_slot_cap(&potion, 5)); // OK, at cap
        assert!(inv.would_exceed_per_slot_cap(&potion, 6)); // exceeds
        inv.add(potion, 3).unwrap();
        assert!(!inv.would_exceed_per_slot_cap(&potion, 2)); // 3+2=5, at cap
        assert!(inv.would_exceed_per_slot_cap(&potion, 3)); // 3+3=6, exceeds
    }
}
