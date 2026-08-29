//! Poké Mart shop logic — thin pokered shell over
//! [`dotzuki_engine::items::mart`]. The generic Buy/Sell/Quit interaction
//! state machine lives in the engine; this module binds pokered's data and
//! rules: [`ItemId`], the Gen-1 pricing (sell = list/2, key items
//! unsellable — engine/events/pokemart.asm:70-77), the Gen-1 bag
//! ([`Inventory`]) with its overflow-spill semantics, and pokered's
//! `MenuInput { up, down, a, b }` (converted to the engine's
//! `MenuInput { up, down, confirm, cancel }` at the boundary).

use crate::items::inventory::{BAG_ITEM_CAPACITY, Inventory, InventoryError};
use crate::main_menu::MenuInput;
use dotzuki_engine::items::mart::{MartBackend, MartStock};
use dotzuki_engine::menu::MenuInput as EngineMenuInput;
use pokered_data::item_data::get_item_data;
use pokered_data::items::ItemId;

// Re-export the engine's mart types so existing `items::shop::*` paths
// keep working. `SoundId` is pokered's historical name for the engine's
// `MartSound` cue enum.
pub use dotzuki_engine::items::mart::{
    BuyMenuState, BuyResult, ConfirmChoice, MartPhase, MartSound as SoundId, MartTopChoice,
    MartUpdate, SellMenuState, SellResult,
};

/// Items the shop stocks (engine `MartStock` over pokered's [`ItemId`]).
pub type ShopInventory = MartStock<ItemId>;

/// Bundled player data consumed by mart transactions.
#[derive(Debug, Clone)]
pub struct PlayerData {
    pub money: u32,
    pub bag: Inventory<BAG_ITEM_CAPACITY>,
}

/// Price/capacity/transaction callbacks for the engine's mart machine,
/// routed through pokered's Gen-1 rules below.
impl MartBackend for PlayerData {
    type Item = ItemId;

    fn bag_len(&self) -> usize {
        self.bag.count()
    }

    fn bag_entry(&self, index: usize) -> Option<(ItemId, u8)> {
        self.bag.get(index)
    }

    fn can_buy(&self, item: &ItemId) -> bool {
        get_item_data(*item).is_some()
    }

    fn commit_buy(&mut self, item: ItemId, quantity: u8) -> BuyResult {
        try_buy(item, quantity, &mut self.money, &mut self.bag)
    }

    fn commit_sell(&mut self, bag_index: usize, quantity: u8) -> SellResult {
        try_sell(bag_index, quantity, &mut self.money, &mut self.bag)
    }
}

/// Complete mart state machine (engine `MartState<ItemId>` driven by
/// pokered's [`MenuInput`] and [`PlayerData`]).
///
/// Deref's to the engine state for read access (`mart.phase`,
/// `mart.inventory`); [`MartState::update_frame`] keeps pokered's original
/// signature, converting the input at the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MartState(dotzuki_engine::items::mart::MartState<ItemId>);

impl MartState {
    /// Begin a mart session with the given shop inventory.
    pub fn new(inventory: ShopInventory) -> Self {
        Self(dotzuki_engine::items::mart::MartState::new(inventory))
    }

    /// Advance the mart state machine by one frame of input.
    ///
    /// `player.money` and `player.bag` are mutated in-place when a
    /// transaction is committed.
    pub fn update_frame(&mut self, input: MenuInput, player: &mut PlayerData) -> MartUpdate {
        let engine_input = EngineMenuInput {
            up: input.up,
            down: input.down,
            confirm: input.a,
            cancel: input.b,
        };
        self.0.update_frame(engine_input, player)
    }
}

impl std::ops::Deref for MartState {
    type Target = dotzuki_engine::items::mart::MartState<ItemId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for MartState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// ──────────────────────────────────────────
//  EXISTING — shop types & functions
//  DO NOT REMOVE — used by tests + other crates
// ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopMenuChoice {
    Buy,
    Sell,
    Quit,
}

#[derive(Debug, Clone)]
pub struct ShopMenuState {
    cursor: usize,
}

impl ShopMenuState {
    const ITEMS: [ShopMenuChoice; 3] = [
        ShopMenuChoice::Buy,
        ShopMenuChoice::Sell,
        ShopMenuChoice::Quit,
    ];

    pub fn new() -> Self {
        Self { cursor: 0 }
    }

    pub fn update_frame(&mut self, input: MenuInput) -> Option<ShopMenuChoice> {
        if input.b {
            return Some(ShopMenuChoice::Quit);
        }
        if input.up {
            self.cursor_up();
        } else if input.down {
            self.cursor_down();
        }
        if input.a {
            return Some(Self::ITEMS[self.cursor]);
        }
        None
    }

    fn cursor_up(&mut self) {
        if self.cursor == 0 {
            self.cursor = Self::ITEMS.len() - 1;
        } else {
            self.cursor -= 1;
        }
    }

    fn cursor_down(&mut self) {
        self.cursor += 1;
        if self.cursor >= Self::ITEMS.len() {
            self.cursor = 0;
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn current_choice(&self) -> ShopMenuChoice {
        Self::ITEMS[self.cursor]
    }
}

impl Default for ShopMenuState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn buy_price(item: ItemId, quantity: u8) -> Option<u32> {
    let data = get_item_data(item)?;
    Some(data.price as u32 * quantity as u32)
}

pub fn sell_price(item: ItemId, quantity: u8) -> Option<u32> {
    let data = get_item_data(item)?;
    Some((data.price as u32 / 2) * quantity as u32)
}

pub fn can_sell(item: ItemId) -> bool {
    // engine/events/pokemart.asm:70-77 — only key items (and HMs, which are
    // key items) cannot be sold. ¥0 items like MASTER BALL / MOON STONE / ETHER
    // and every TM are sellable in the original.
    get_item_data(item).is_some_and(|d| !d.is_key_item)
}

pub fn try_buy(item: ItemId, quantity: u8, money: &mut u32, bag: &mut Inventory<BAG_ITEM_CAPACITY>) -> BuyResult {
    let cost = match buy_price(item, quantity) {
        Some(c) => c,
        None => return BuyResult::InvalidItem,
    };
    if *money < cost {
        return BuyResult::NotEnoughMoney;
    }
    if bag.add_item(item, quantity).is_err() {
        return BuyResult::BagFull;
    }
    *money -= cost;
    BuyResult::Success { total_cost: cost }
}

pub fn try_sell(
    bag_index: usize,
    quantity: u8,
    money: &mut u32,
    bag: &mut Inventory<BAG_ITEM_CAPACITY>,
) -> SellResult {
    let (item, owned) = match bag.get(bag_index) {
        Some(entry) => entry,
        None => return SellResult::NotInBag,
    };
    if !can_sell(item) {
        return SellResult::Unsellable;
    }
    if quantity > owned {
        return SellResult::NotInBag;
    }
    let value = match sell_price(item, quantity) {
        Some(v) => v,
        None => return SellResult::InvalidItem,
    };
    match bag.remove_item_at(bag_index, quantity) {
        Ok(()) => {}
        Err(InventoryError::IndexOutOfBounds) | Err(InventoryError::NotEnoughItems) => {
            return SellResult::NotInBag;
        }
        Err(_) => return SellResult::NotInBag,
    }
    *money = money.saturating_add(value);
    SellResult::Success { total_value: value }
}

#[cfg(test)]
mod can_sell_tests {
    use super::*;
    use pokered_data::items::ItemId;

    /// engine/events/pokemart.asm:70-77 — only key items (and HMs) are blocked.
    #[test]
    fn non_key_items_are_sellable_including_zero_price_and_tms() {
        assert!(can_sell(ItemId::MasterBall), "¥0 items sellable");
        assert!(can_sell(ItemId::MoonStone));
        assert!(can_sell(ItemId::Tm01), "TMs sellable");
        assert!(!can_sell(ItemId::Hm01), "HMs blocked");
        assert!(!can_sell(ItemId::Bicycle), "key items blocked");
        assert!(!can_sell(ItemId::BoulderBadge), "badges blocked");
    }
}
