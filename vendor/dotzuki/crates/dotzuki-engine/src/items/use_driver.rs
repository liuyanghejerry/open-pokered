//! Generic item-effect application + bag/shop buy-sell flow (P0e).
//!
//! This module owns only the *control flow* of using, buying, and selling
//! items. It is 100% game-agnostic: it never decides what an item effect
//! actually does. The engine routes; the game decides.
//!
//! - [`UsageContext`] / [`ItemUseResult`] are neutral engine types.
//! - [`ItemProvider::apply_effect`](super::ItemProvider::apply_effect) is an
//!   **opaque** dispatch hook: the game implements what healing / status-cure /
//!   PP-restore / vitamins / battle items / catch / etc. actually do. The
//!   engine never inspects the effect.
//! - [`use_item`] is the shared driver for field *and* battle use: it validates
//!   ownership and the usage context
//!   ([`ItemProvider::usable_in`](super::ItemProvider::usable_in)), dispatches
//!   to `apply_effect`, then consumes one unit from the
//!   [`Inventory`](super::Inventory) iff the result says so. One place so field
//!   & battle share it.
//! - [`buy`] / [`sell`] perform pure money/inventory bookkeeping; prices and
//!   the sell rate come from [`ShopProvider`](super::ShopProvider) (Gen-1
//!   quirks stay game-side).

use core::fmt::Debug;
use core::hash::Hash;

use super::{Inventory, ItemKind, ItemProvider, ShopProvider};
use crate::battle::rng::BattleRng;
use crate::party::{MonsterInstance, MonsterProvider};

/// Where / whether an item may be used. A neutral engine enum — no game
/// specifics. Returned by [`ItemProvider::usable_in`](super::ItemProvider::usable_in)
/// and passed into [`ItemProvider::apply_effect`](super::ItemProvider::apply_effect)
/// / [`use_item`] as the active context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageContext {
    /// Usable only from the field (overworld / party menu).
    FieldOnly,
    /// Usable only inside battle.
    BattleOnly,
    /// Usable both in the field and in battle.
    FieldAndBattle,
    /// Not usable at all (e.g. a plain key item with no effect).
    None,
}

impl UsageContext {
    /// Returns `true` if an item whose eligibility is `eligibility` may be used
    /// while the *active* context is `self`.
    ///
    /// The active context is normally a concrete site ([`Self::FieldOnly`] or
    /// [`Self::BattleOnly`]); [`Self::FieldAndBattle`] as an active context is
    /// treated permissively (matches either site).
    fn allows(self, eligibility: UsageContext) -> bool {
        match eligibility {
            UsageContext::None => false,
            UsageContext::FieldAndBattle => !matches!(self, UsageContext::None),
            UsageContext::FieldOnly => {
                matches!(self, UsageContext::FieldOnly | UsageContext::FieldAndBattle)
            }
            UsageContext::BattleOnly => {
                matches!(
                    self,
                    UsageContext::BattleOnly | UsageContext::FieldAndBattle
                )
            }
        }
    }
}

/// Neutral result of an item-use attempt, returned by
/// [`ItemProvider::apply_effect`](super::ItemProvider::apply_effect) and
/// propagated by [`use_item`].
///
/// The driver consumes one unit of the item only on [`ItemUseResult::Applied`]
/// with `consume: true`, or on [`ItemUseResult::Caught`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemUseResult<I: Copy + Eq + Hash + Debug> {
    /// The effect was applied. `consume` tells the driver whether to remove one
    /// unit from the bag (consumables vs. reusable tools). `message_key` is an
    /// opaque, game-defined message identifier (the engine never reads it).
    Applied {
        /// Whether the driver should remove one unit from the inventory.
        consume: bool,
        /// Optional game-defined message id to display. Opaque to the engine.
        message_key: Option<String>,
    },
    /// The item was applicable but produced no effect (e.g. Potion at full HP
    /// -> "It won't have any effect"). Not consumed.
    NoEffect,
    /// A capture device succeeded (battle context). Consumed.
    Caught,
    /// The attempt failed (e.g. a ball that broke free, or the item could not
    /// be used here). Not consumed.
    Failed,
    /// The item triggered an evolution sequence (e.g. Thunderstone, Rare
    /// Candy, or any evolution-inducing item). The driver should NOT consume
    /// the item until the evolution is confirmed/completed.
    EvolutionTriggered {
        /// The item that triggered the evolution (may be consumed later
        /// upon evolution confirmation).
        item: I,
        /// Optional game-defined message id to display.
        message_key: Option<String>,
    },
    /// The item taught a new move to the target (e.g. TM/HM). The driver
    /// should consume the item if `consume` is true (TMs are consumed in
    /// Gen 1-4; HMs are never consumed).
    MoveLearned {
        /// Whether the driver should remove one unit from the inventory.
        consume: bool,
        /// Optional game-defined message id to display.
        message_key: Option<String>,
    },
}

impl<I: Copy + Eq + Hash + Debug> ItemUseResult<I> {
    /// Whether the driver should remove one unit from the bag for this result.
    pub fn consumes(&self) -> bool {
        match self {
            ItemUseResult::Applied { consume, .. } => *consume,
            ItemUseResult::Caught => true,
            ItemUseResult::MoveLearned { consume, .. } => *consume,
            ItemUseResult::EvolutionTriggered { .. } => false,
            ItemUseResult::NoEffect | ItemUseResult::Failed => false,
        }
    }
}

/// Engine driver for *using* an item from the bag, shared by field and battle.
///
/// Steps: validate ownership and the usage context
/// ([`ItemProvider::usable_in`](super::ItemProvider::usable_in)) → dispatch to
/// the opaque [`ItemProvider::apply_effect`](super::ItemProvider::apply_effect)
/// hook → consume one unit from `inv` iff the result says so. Effect
/// *semantics* live entirely game-side.
///
/// Returns [`ItemUseResult::Failed`] without touching `target` if the item is
/// not owned or is not usable in `ctx`.
pub fn use_item<const N: usize, I, M>(
    provider: &I,
    monster_provider: &M,
    inv: &mut Inventory<I::Item, N>,
    item: I::Item,
    ctx: UsageContext,
    target: Option<&mut MonsterInstance<M>>,
    rng: &mut dyn BattleRng,
) -> ItemUseResult<I::Item>
where
    I: ItemProvider,
    M: MonsterProvider,
{
    // Validate ownership.
    if !inv.contains(&item, 1) {
        return ItemUseResult::Failed;
    }
    // Validate the usage context.
    if !ctx.allows(provider.usable_in(&item)) {
        return ItemUseResult::Failed;
    }

    // Route by ItemKind.
    //
    // Evolution items deliberately go through `apply_effect` like any other
    // effectful item: the game returns [`ItemUseResult::EvolutionTriggered`]
    // and the driver leaves the item in the bag (the game consumes it after
    // the player confirms the evolution). Keeping evolution out of the
    // dispatch means `use_item` only requires a plain
    // [`MonsterProvider`](crate::party::MonsterProvider) — games without
    // evolution mechanics pay nothing for it.
    let kind = provider.item_kind(&item);
    let result = match kind {
        ItemKind::TeachMove => {
            if let Some(target) = target {
                provider
                    .on_teach_move(item, target)
                    .unwrap_or(ItemUseResult::NoEffect)
            } else {
                ItemUseResult::NoEffect
            }
        }
        ItemKind::KeyItem | ItemKind::Currency => provider
            .on_use_field(item)
            .unwrap_or(ItemUseResult::NoEffect),
        _ => provider.apply_effect(monster_provider, item, ctx, target, rng),
    };

    // Consume on success per the result.
    if result.consumes() {
        inv.remove(&item, 1);
    }
    result
}

/// Error from a shop transaction. Nothing is mutated when an error is returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopError {
    /// The player cannot afford the purchase.
    NotEnoughMoney,
    /// The player's inventory cannot hold the purchased quantity (slot or
    /// per-slot capacity limit reached). No money is taken.
    InventoryFull,
    /// The shop refuses to buy this item, or the player does not own enough of
    /// it to sell the requested quantity.
    CannotSell,
    /// The requested quantity is zero.
    InvalidQuantity,
}

/// Summary of a completed shop transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopReceipt {
    /// Total money that changed hands (paid for buys, received for sells).
    pub total: u32,
    /// The player's money after the transaction.
    pub money_after: u32,
}

/// Buy `quantity` units of `item` at the shop's
/// [`ShopProvider::buy_price`](super::ShopProvider::buy_price).
///
/// Bookkeeping only: checks the player can afford it and that the inventory
/// can hold the goods, then adds the item and deducts money. On any error
/// nothing is mutated (no money lost, no item added). Prices and Gen-1
/// quirks come from the [`ShopProvider`](super::ShopProvider).
pub fn buy<const N: usize, S>(
    provider: &S,
    shop_id: &S::ShopId,
    inv: &mut Inventory<S::Item, N>,
    money: &mut u32,
    item: S::Item,
    quantity: u32,
) -> Result<ShopReceipt, ShopError>
where
    S: ShopProvider,
{
    if quantity == 0 {
        return Err(ShopError::InvalidQuantity);
    }
    let unit_price = provider.buy_price(&item);
    let discount = provider.discount_rate(shop_id);
    let effective_price = (unit_price as f32 * discount) as u32;
    let total = effective_price.saturating_mul(quantity);
    if *money < total {
        return Err(ShopError::NotEnoughMoney);
    }
    // Add before charging: a capacity-limited inventory may refuse the goods,
    // and the player must not pay for items they never received.
    if inv.add(item, quantity).is_err() {
        return Err(ShopError::InventoryFull);
    }
    *money -= total;
    Ok(ShopReceipt {
        total,
        money_after: *money,
    })
}

/// Sell `quantity` units of `item` for the shop's
/// [`ShopProvider::sell_price`](super::ShopProvider::sell_price).
///
/// Bookkeeping only: verifies the shop will buy the item and the player owns
/// enough, then removes the item and credits money. On any error nothing is
/// mutated.
pub fn sell<const N: usize, S>(
    provider: &S,
    shop_id: &S::ShopId,
    inv: &mut Inventory<S::Item, N>,
    money: &mut u32,
    item: S::Item,
    quantity: u32,
) -> Result<ShopReceipt, ShopError>
where
    S: ShopProvider,
{
    if quantity == 0 {
        return Err(ShopError::InvalidQuantity);
    }
    if !provider.can_sell(&item) || !inv.contains(&item, quantity) {
        return Err(ShopError::CannotSell);
    }
    if !inv.remove(&item, quantity) {
        return Err(ShopError::CannotSell);
    }
    let base_sell = provider.sell_price(&item);
    let rate = provider.sell_rate(shop_id);
    let effective_sell = (base_sell as f32 * rate) as u32;
    let total = effective_sell.saturating_mul(quantity);
    *money = money.saturating_add(total);
    Ok(ShopReceipt {
        total,
        money_after: *money,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{BagCategory, ItemKind, ItemResult};
    use crate::party::{MonsterInstance, MonsterStatus, MoveSlot, StatSet};

    // -- Mock RNG ----------------------------------------------------------

    /// Deterministic RNG yielding a fixed sequence (cycled).
    struct SeqRng {
        seq: Vec<u8>,
        idx: usize,
    }
    impl SeqRng {
        fn new(seq: &[u8]) -> Self {
            Self {
                seq: seq.to_vec(),
                idx: 0,
            }
        }
    }
    impl BattleRng for SeqRng {
        fn next_u8(&mut self) -> u8 {
            let v = self.seq[self.idx % self.seq.len()];
            self.idx += 1;
            v
        }
    }

    // -- Mock monster provider --------------------------------------------

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Stat {
        Hp,
    }
    #[derive(Debug, Clone, Copy, Default)]
    struct MockMon;
    impl MonsterProvider for MockMon {
        type SpeciesId = u8;
        type MoveId = u8;
        type Genetics = ();
        type Training = ();
        type Stat = Stat;
        fn base_stat(&self, _s: u8, _st: Stat) -> u16 {
            50
        }
        fn calc_stat(&self, _s: u8, _st: Stat, _l: u8, _g: &(), _t: &()) -> u16 {
            50
        }
        fn stats(&self) -> &[Stat] {
            &[Stat::Hp]
        }
        fn hp_stat(&self) -> Stat {
            Stat::Hp
        }
        fn max_moves(&self) -> usize {
            4
        }
    }

    /// Build a mock instance with `current_hp`, max HP 50, given status, and one
    /// move with `pp` PP.
    fn mon(current_hp: u16, status: MonsterStatus, pp: u8) -> MonsterInstance<MockMon> {
        let provider = MockMon;
        let mut stats = StatSet::zeroed(&provider);
        stats.set(Stat::Hp, 50);
        MonsterInstance {
            species: 1,
            level: 5,
            exp: 0,
            genetics: (),
            training: (),
            stats,
            current_hp,
            status,
            moves: vec![MoveSlot {
                move_id: 0,
                pp,
                pp_up: 0,
            }],
        }
    }

    // -- Mock game (item + shop providers) --------------------------------

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Item {
        Potion,
        Antidote,
        Ball,
        XAttack,
        Bicycle,   // key item: cannot use, cannot sell
        KeyStone,  // key item: usable in field, routes via on_use_field
        FireStone, // evolution item: dispatched through apply_effect
    }

    struct Game;

    impl ItemProvider for Game {
        type Item = Item;
        type Effect = ();
        type Monster = ();
        type CustomKind = ();

        fn item_name(&self, _item: &Item) -> &str {
            "X"
        }
        fn item_description(&self, _item: &Item) -> &str {
            "X"
        }
        fn item_effect(&self, _item: &Item) {}
        fn item_price(&self, item: &Item) -> u32 {
            match item {
                Item::Potion => 300,
                Item::Antidote => 100,
                Item::Ball => 200,
                Item::XAttack => 500,
                Item::Bicycle => 0,
                Item::KeyStone => 0,
                Item::FireStone => 2100,
            }
        }
        fn can_use_outside_battle(&self, item: &Item) -> bool {
            !matches!(item, Item::Ball | Item::XAttack | Item::Bicycle)
        }
        fn can_use_in_battle(&self, item: &Item) -> bool {
            !matches!(item, Item::Bicycle | Item::KeyStone)
        }
        fn use_on_monster(&self, _item: &Item, _m: &mut ()) -> ItemResult {
            ItemResult::NoEffect
        }
        fn consume(&self, item: &Item) -> bool {
            !matches!(item, Item::Bicycle | Item::KeyStone)
        }

        fn item_kind(&self, item: &Item) -> ItemKind<()> {
            match item {
                Item::Bicycle | Item::KeyStone => ItemKind::KeyItem,
                Item::FireStone => ItemKind::Evolution,
                _ => ItemKind::Consumable,
            }
        }

        // -- P0e: usage context + opaque effect dispatch -------------------

        fn usable_in(&self, item: &Item) -> UsageContext {
            match item {
                Item::Potion | Item::Antidote => UsageContext::FieldAndBattle,
                Item::Ball | Item::XAttack => UsageContext::BattleOnly,
                Item::Bicycle => UsageContext::None,
                Item::KeyStone | Item::FireStone => UsageContext::FieldOnly,
            }
        }

        fn apply_effect<M: MonsterProvider>(
            &self,
            provider: &M,
            item: Item,
            _ctx: UsageContext,
            target: Option<&mut MonsterInstance<M>>,
            rng: &mut dyn BattleRng,
        ) -> ItemUseResult<Item> {
            let _ = provider;
            match item {
                Item::Potion => match target {
                    // Heal by 20, clamped to a fixed max of 50 (mock number;
                    // the engine never owns this — the game does).
                    Some(m) if m.current_hp < 50 => {
                        m.current_hp = (m.current_hp + 20).min(50);
                        ItemUseResult::Applied {
                            consume: true,
                            message_key: None,
                        }
                    }
                    _ => ItemUseResult::NoEffect,
                },
                Item::Antidote => match target {
                    Some(m) if m.status == MonsterStatus::Poison => {
                        m.status = MonsterStatus::Healthy;
                        ItemUseResult::Applied {
                            consume: true,
                            message_key: Some("cured".to_string()),
                        }
                    }
                    _ => ItemUseResult::NoEffect,
                },
                Item::Ball => {
                    // Catch on even rng byte, fail otherwise (battle only).
                    if rng.next_u8() % 2 == 0 {
                        ItemUseResult::Caught
                    } else {
                        ItemUseResult::Failed
                    }
                }
                Item::XAttack => ItemUseResult::Applied {
                    consume: true,
                    message_key: None,
                },
                Item::Bicycle => ItemUseResult::NoEffect,
                Item::KeyStone => ItemUseResult::Applied {
                    consume: true,
                    message_key: Some("apply_effect_called".to_string()),
                },
                // Evolution items report the trigger; the driver must NOT
                // consume — the game consumes after the player confirms.
                Item::FireStone => ItemUseResult::EvolutionTriggered {
                    item,
                    message_key: Some("evolve?".to_string()),
                },
            }
        }

        fn on_use_field(&self, item: Item) -> Option<ItemUseResult<Item>> {
            match item {
                Item::KeyStone => Some(ItemUseResult::Applied {
                    consume: false,
                    message_key: Some("field_used".to_string()),
                }),
                _ => None,
            }
        }
    }

    impl ShopProvider for Game {
        type Item = Item;
        type ShopId = u8;
        fn shop_inventory(&self, _shop_id: &u8) -> Vec<(Item, u32)> {
            vec![(Item::Potion, 300)]
        }
        fn shop_name(&self, _shop_id: &u8) -> &str {
            "Mart"
        }
        fn buy_price(&self, item: &Item) -> u32 {
            self.item_price(item)
        }
        // sell_price uses the default (buy_price / 2, Gen-1).
        fn can_sell(&self, item: &Item) -> bool {
            !matches!(item, Item::Bicycle | Item::KeyStone)
        }
    }

    fn stock(item: Item, qty: u32) -> Inventory<Item, 64> {
        let mut inv = Inventory::<Item, 64>::new();
        inv.add(item, qty).unwrap();
        inv
    }

    // -- use_item: routing + consumption -----------------------------------

    #[test]
    fn use_item_routes_to_apply_effect_and_consumes_on_applied() {
        let game = Game;
        let mut inv = stock(Item::Potion, 3);
        let mut m = mon(10, MonsterStatus::Healthy, 10);
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::Potion,
            UsageContext::FieldOnly,
            Some(&mut m),
            &mut rng,
        );
        assert!(matches!(r, ItemUseResult::Applied { consume: true, .. }));
        assert_eq!(m.current_hp, 30); // healed by 20
        assert!(inv.contains(&Item::Potion, 2)); // one consumed
        assert!(!inv.contains(&Item::Potion, 3));
    }

    #[test]
    fn use_item_no_effect_does_not_consume() {
        let game = Game;
        let mut inv = stock(Item::Potion, 3);
        let mut m = mon(50, MonsterStatus::Healthy, 10); // full HP
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::Potion,
            UsageContext::FieldOnly,
            Some(&mut m),
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::NoEffect);
        assert!(inv.contains(&Item::Potion, 3)); // not consumed
    }

    #[test]
    fn use_item_rejects_not_owned_without_touching_target() {
        let game = Game;
        let mut inv: Inventory<Item, 64> = Inventory::new(); // empty
        let mut m = mon(10, MonsterStatus::Poison, 10);
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::Antidote,
            UsageContext::FieldOnly,
            Some(&mut m),
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::Failed);
        assert_eq!(m.status, MonsterStatus::Poison); // target untouched
    }

    #[test]
    fn use_item_rejects_wrong_context() {
        let game = Game;
        let mut inv = stock(Item::XAttack, 5); // battle-only
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::XAttack,
            UsageContext::FieldOnly, // using in field, but X Attack is BattleOnly
            None,
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::Failed);
        assert!(inv.contains(&Item::XAttack, 5)); // not consumed
    }

    #[test]
    fn use_item_caught_consumes_ball() {
        let game = Game;
        let mut inv = stock(Item::Ball, 5);
        let mut rng = SeqRng::new(&[0]); // even -> Caught
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::Ball,
            UsageContext::BattleOnly,
            None,
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::Caught);
        assert!(inv.contains(&Item::Ball, 4)); // one consumed
    }

    #[test]
    fn use_item_failed_ball_not_consumed() {
        let game = Game;
        let mut inv = stock(Item::Ball, 5);
        let mut rng = SeqRng::new(&[1]); // odd -> Failed
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::Ball,
            UsageContext::BattleOnly,
            None,
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::Failed);
        assert!(inv.contains(&Item::Ball, 5)); // not consumed
    }

    #[test]
    fn use_item_status_cure_consumes() {
        let game = Game;
        let mut inv = stock(Item::Antidote, 1);
        let mut m = mon(20, MonsterStatus::Poison, 10);
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::Antidote,
            UsageContext::FieldOnly,
            Some(&mut m),
            &mut rng,
        );
        assert!(matches!(r, ItemUseResult::Applied { consume: true, .. }));
        assert_eq!(m.status, MonsterStatus::Healthy);
        assert!(!inv.contains(&Item::Antidote, 1));
    }

    #[test]
    fn use_item_default_apply_effect_is_no_effect() {
        // A provider that does not override apply_effect / usable_in gets the
        // defaults: usable everywhere, NoEffect, no consumption.
        struct Plain;
        impl ItemProvider for Plain {
            type Item = u8;
            type Effect = ();
            type Monster = ();
            type CustomKind = ();
            fn item_name(&self, _i: &u8) -> &str {
                "X"
            }
            fn item_description(&self, _i: &u8) -> &str {
                "X"
            }
            fn item_effect(&self, _i: &u8) {}
            fn item_price(&self, _i: &u8) -> u32 {
                0
            }
            fn can_use_outside_battle(&self, _i: &u8) -> bool {
                true
            }
            fn can_use_in_battle(&self, _i: &u8) -> bool {
                true
            }
            fn use_on_monster(&self, _i: &u8, _m: &mut ()) -> ItemResult {
                ItemResult::NoEffect
            }
            fn consume(&self, _i: &u8) -> bool {
                true
            }
            fn item_kind(&self, _item: &u8) -> ItemKind<()> {
                ItemKind::Consumable
            }
        }
        let game = Plain;
        let mut inv = Inventory::<u8, 64>::new();
        inv.add(7u8, 2).unwrap();
        let mut m = mon(10, MonsterStatus::Healthy, 10);
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            7u8,
            UsageContext::FieldOnly,
            Some(&mut m),
            &mut rng,
        );
        assert_eq!(r, ItemUseResult::NoEffect);
        assert!(inv.contains(&7u8, 2)); // not consumed
        assert_eq!(m.current_hp, 10); // untouched
    }

    // -- buy / sell --------------------------------------------------------

    #[test]
    fn buy_deducts_money_and_adds_item() {
        let game = Game;
        let mut inv = Inventory::<Item, 64>::new();
        let mut money = 1000u32;
        let r = buy(&game, &0u8, &mut inv, &mut money, Item::Potion, 2).unwrap();
        assert_eq!(r.total, 600); // 300 * 2
        assert_eq!(money, 400);
        assert!(inv.contains(&Item::Potion, 2));
    }

    #[test]
    fn buy_fails_when_broke_and_changes_nothing() {
        let game = Game;
        let mut inv = Inventory::<Item, 64>::new();
        let mut money = 100u32;
        let err = buy(&game, &0u8, &mut inv, &mut money, Item::Potion, 1).unwrap_err();
        assert_eq!(err, ShopError::NotEnoughMoney);
        assert_eq!(money, 100); // unchanged
        assert!(!inv.contains(&Item::Potion, 1)); // nothing added
    }

    #[test]
    fn buy_fails_when_inventory_full_and_keeps_money() {
        let game = Game;
        // One slot only, already occupied by another item.
        let mut inv = Inventory::<Item, 1>::with_capacity(99);
        inv.add(Item::Antidote, 1).unwrap();
        let mut money = 1000u32;
        let err = buy(&game, &0u8, &mut inv, &mut money, Item::Potion, 1).unwrap_err();
        assert_eq!(err, ShopError::InventoryFull);
        assert_eq!(money, 1000); // not charged
        assert!(!inv.contains(&Item::Potion, 1)); // nothing added
    }

    #[test]
    fn buy_fails_at_per_slot_cap_and_keeps_money() {
        let game = Game;
        let mut inv = Inventory::<Item, 20>::with_capacity(99);
        inv.add(Item::Potion, 99).unwrap(); // slot already at cap
        let mut money = 1000u32;
        let err = buy(&game, &0u8, &mut inv, &mut money, Item::Potion, 1).unwrap_err();
        assert_eq!(err, ShopError::InventoryFull);
        assert_eq!(money, 1000); // not charged
        assert_eq!(inv.quantity(&Item::Potion), 99); // unchanged
    }

    #[test]
    fn sell_adds_money_and_removes_item() {
        let game = Game;
        let mut inv = stock(Item::Potion, 3);
        let mut money = 0u32;
        let r = sell(&game, &0u8, &mut inv, &mut money, Item::Potion, 2).unwrap();
        // sell_price = 300/2 = 150 (Gen-1 half), sell_rate defaults to 1.0
        // (pass-through), so total = 150 * 2 = 300.
        assert_eq!(r.total, 300);
        assert_eq!(money, 300);
        assert!(inv.contains(&Item::Potion, 1));
    }

    #[test]
    fn sell_rate_override_scales_sell_price() {
        // A shop paying 80% of the game-wide sell price.
        struct Pawnshop;
        impl ShopProvider for Pawnshop {
            type Item = Item;
            type ShopId = u8;
            fn shop_inventory(&self, _shop_id: &u8) -> Vec<(Item, u32)> {
                vec![]
            }
            fn shop_name(&self, _shop_id: &u8) -> &str {
                "Pawnshop"
            }
            fn buy_price(&self, _item: &Item) -> u32 {
                300
            }
            fn sell_rate(&self, _shop_id: &u8) -> f32 {
                0.8
            }
        }
        let mut inv = stock(Item::Potion, 1);
        let mut money = 0u32;
        let r = sell(&Pawnshop, &0u8, &mut inv, &mut money, Item::Potion, 1).unwrap();
        // sell_price = 300/2 = 150, rate 0.8 → 120.
        assert_eq!(r.total, 120);
    }

    #[test]
    fn sell_rejects_key_item_and_changes_nothing() {
        let game = Game;
        let mut inv = stock(Item::Bicycle, 1);
        let mut money = 0u32;
        let err = sell(&game, &0u8, &mut inv, &mut money, Item::Bicycle, 1).unwrap_err();
        assert_eq!(err, ShopError::CannotSell);
        assert_eq!(money, 0);
        assert!(inv.contains(&Item::Bicycle, 1)); // still owned
    }

    #[test]
    fn sell_rejects_when_not_enough_owned() {
        let game = Game;
        let mut inv = stock(Item::Potion, 1);
        let mut money = 0u32;
        let err = sell(&game, &0u8, &mut inv, &mut money, Item::Potion, 5).unwrap_err();
        assert_eq!(err, ShopError::CannotSell);
        assert!(inv.contains(&Item::Potion, 1));
        assert_eq!(money, 0);
    }

    // -- ItemKind dispatch --------------------------------------------------

    #[test]
    fn use_item_key_item_routes_to_on_use_field_not_apply_effect() {
        let game = Game;
        let mut inv = stock(Item::KeyStone, 1);
        let mut m = mon(50, MonsterStatus::Healthy, 10);
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::KeyStone,
            UsageContext::FieldOnly,
            Some(&mut m),
            &mut rng,
        );
        // KeyStone is a KeyItem → routes to on_use_field, not apply_effect.
        assert_eq!(
            r,
            ItemUseResult::Applied {
                consume: false,
                message_key: Some("field_used".to_string()),
            }
        );
        // KeyItem is not consumed (on_use_field returned consume: false).
        assert!(inv.contains(&Item::KeyStone, 1));
    }

    #[test]
    fn evolution_item_dispatches_via_apply_effect_and_is_not_consumed() {
        let game = Game;
        let mut inv = stock(Item::FireStone, 1);
        let mut m = mon(50, MonsterStatus::Healthy, 10);
        let mut rng = SeqRng::new(&[0]);
        let r = use_item(
            &game,
            &MockMon,
            &mut inv,
            Item::FireStone,
            UsageContext::FieldOnly,
            Some(&mut m),
            &mut rng,
        );
        // Evolution items go through apply_effect; the game reports the
        // trigger and the driver leaves the item in the bag until the game
        // confirms the evolution.
        assert_eq!(
            r,
            ItemUseResult::EvolutionTriggered {
                item: Item::FireStone,
                message_key: Some("evolve?".to_string()),
            }
        );
        assert!(inv.contains(&Item::FireStone, 1)); // still owned
    }

    // -- BagCategory still reachable (existing API intact) ------------------

    #[test]
    fn bag_category_variants_exist() {
        let _ = BagCategory::Items;
        let _ = BagCategory::Medicine;
    }
}
