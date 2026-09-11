//! Mart / shop interaction state machine — the UI-level counterpart of the
//! [`buy`](super::buy) / [`sell`](super::sell) money-and-inventory drivers.
//!
//! [`MartState`] owns only the *interaction flow* of a shop: the
//! Buy/Sell/Quit top menu, item-list cursors, quantity selection, the
//! Yes/No confirmation sub-phase, and result display. It knows nothing
//! about prices, money, or bag capacity — those are supplied by the game
//! through the [`MartBackend`] callback trait, so every game keeps its own
//! pricing rules, currency semantics, and inventory quirks (per-slot caps,
//! overflow spill, unsellable key items, …).
//!
//! [`MartDriver`] is a ready-made backend that routes transactions through
//! the engine's [`buy`](super::buy) / [`sell`](super::sell) drivers and a
//! [`ShopProvider`](super::ShopProvider); games with special bag semantics
//! implement [`MartBackend`] directly instead.
//!
//! Input uses the engine-wide [`MenuInput`]; sound cues are reported as
//! [`MartSound`] values the game maps to its own audio ids.

use core::fmt::Debug;
use core::hash::Hash;
use core::str::FromStr;

use super::use_driver::{buy, sell, ShopError};
use super::{Inventory, ShopProvider};
use crate::menu::MenuInput;

// ── Small enums ───────────────────────────────────────────────────

/// Sound cues the mart layer can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MartSound {
    /// A purchase was committed successfully.
    Purchase,
}

/// Yes/No choice used inside the confirmation sub-phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    Yes,
    No,
}

impl ConfirmChoice {
    fn toggle(self) -> Self {
        match self {
            ConfirmChoice::Yes => ConfirmChoice::No,
            ConfirmChoice::No => ConfirmChoice::Yes,
        }
    }
}

/// Top-menu cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MartTopChoice {
    Buy,
    Sell,
    Quit,
}

impl MartTopChoice {
    const ORDER: [MartTopChoice; 3] = [MartTopChoice::Buy, MartTopChoice::Sell, MartTopChoice::Quit];

    pub fn position(self) -> usize {
        Self::ORDER.iter().position(|&c| c == self).expect("valid choice")
    }

    fn next(self) -> Self {
        let pos = self.position();
        Self::ORDER[(pos + 1) % 3]
    }

    fn prev(self) -> Self {
        let pos = self.position();
        Self::ORDER[(pos + 2) % 3] // equivalent to (pos - 1 + 3) % 3
    }
}

/// Returned by [`MartState::update_frame`] to signal the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MartUpdate {
    /// No special action — keep rendering.
    Continue,
    /// Play the given sound cue (e.g. after a successful purchase).
    PlaySound(MartSound),
    /// The mart interaction is over — return to the previous screen.
    Exit,
}

// ── Transaction results ───────────────────────────────────────────

/// Outcome of a buy attempt, reported by [`MartBackend::commit_buy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuyResult {
    Success { total_cost: u32 },
    NotEnoughMoney,
    BagFull,
    InvalidItem,
}

/// Outcome of a sell attempt, reported by [`MartBackend::commit_sell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellResult {
    Success { total_value: u32 },
    Unsellable,
    NotInBag,
    InvalidItem,
}

// ── Sub-state enums ───────────────────────────────────────────────

/// Phases inside the Buy flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuyMenuState {
    /// Cursor over the shop inventory list.
    SelectItem { cursor: usize },
    /// Choosing quantity (1‥99).
    Quantity { item_index: usize, quantity: u8 },
    /// Yes/No confirmation before committing money.
    Confirm {
        item_index: usize,
        quantity: u8,
        selected: ConfirmChoice,
    },
    /// Result of the transaction attempt.
    Result {
        dialogue: BuyResult,
        /// true → go back to the item list; false → go back to the top menu.
        return_to_list: bool,
    },
}

/// Phases inside the Sell flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SellMenuState {
    /// Cursor over the player's bag (saleable items).
    SelectItem { cursor: usize },
    /// Choosing quantity (1‥max_quantity).
    Quantity {
        item_index: usize,
        quantity: u8,
        max_quantity: u8,
    },
    /// Yes/No confirmation before committing.
    Confirm {
        item_index: usize,
        quantity: u8,
        max_quantity: u8,
        selected: ConfirmChoice,
    },
    /// Result of the sell attempt.
    Result {
        dialogue: SellResult,
        /// true → go back to the sell list; false → go back to the top menu.
        return_to_list: bool,
    },
}

/// Actual state machine phase (read via [`MartState::phase`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MartPhase {
    MainMenu { cursor: MartTopChoice },
    Buy(BuyMenuState),
    Sell(SellMenuState),
    Exiting,
}

// ── Shop stock list ───────────────────────────────────────────────

/// The list of items a shop stocks, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MartStock<I: Copy + Eq + Hash + Debug> {
    items: Vec<I>,
}

impl<I: Copy + Eq + Hash + Debug> MartStock<I> {
    pub fn new(items: Vec<I>) -> Self {
        Self { items }
    }

    pub fn items(&self) -> &[I] {
        &self.items
    }

    pub fn get(&self, index: usize) -> Option<I> {
        self.items.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<I: Copy + Eq + Hash + Debug + FromStr> MartStock<I> {
    /// Build a [`MartStock`] from a list of item name strings.
    ///
    /// Each string is parsed into an `I` via [`FromStr`]. Returns `Err`
    /// with the offending string if parsing fails.
    pub fn from_strings<S: AsRef<str>>(items: &[S]) -> Result<Self, String> {
        let mut parsed = Vec::with_capacity(items.len());
        for s in items {
            match I::from_str(s.as_ref()) {
                Ok(id) => parsed.push(id),
                Err(_) => return Err(s.as_ref().to_string()),
            }
        }
        Ok(Self::new(parsed))
    }
}

// ── Backend callbacks ─────────────────────────────────────────────

/// Price / capacity / transaction callbacks the game supplies to
/// [`MartState`]. The state machine owns navigation; the backend owns all
/// money and bag semantics.
///
/// Implementations must not mutate anything when a commit fails.
pub trait MartBackend {
    /// Concrete item identifier type.
    type Item: Copy + Eq + Hash + Debug;

    /// Number of occupied bag slots (drives the sell-list cursor).
    fn bag_len(&self) -> usize;

    /// `(item, owned_quantity)` at bag slot `index`, if occupied.
    fn bag_entry(&self, index: usize) -> Option<(Self::Item, u8)>;

    /// Whether `item` may be purchased at all (gates entry into the
    /// quantity-select phase). Defaults to `true`.
    fn can_buy(&self, item: &Self::Item) -> bool {
        let _ = item;
        true
    }

    /// Commit a purchase of `quantity` × `item`: add to the bag and deduct
    /// the money. On failure nothing may be mutated.
    fn commit_buy(&mut self, item: Self::Item, quantity: u8) -> BuyResult;

    /// Commit a sale of `quantity` units of the item at `bag_index`:
    /// remove from the bag and credit the money. On failure nothing may be
    /// mutated.
    fn commit_sell(&mut self, bag_index: usize, quantity: u8) -> SellResult;
}

/// Ready-made [`MartBackend`] that routes transactions through the engine's
/// [`buy`] / [`sell`] drivers and a [`ShopProvider`].
///
/// Suitable for games whose bag is a plain engine [`Inventory`]; games with
/// custom bag semantics (overflow spill, key-item rules beyond
/// `can_sell`, …) implement [`MartBackend`] directly.
pub struct MartDriver<'a, S: ShopProvider, const N: usize> {
    pub provider: &'a S,
    pub shop_id: S::ShopId,
    pub money: &'a mut u32,
    pub bag: &'a mut Inventory<S::Item, N>,
}

impl<S: ShopProvider, const N: usize> MartBackend for MartDriver<'_, S, N> {
    type Item = S::Item;

    fn bag_len(&self) -> usize {
        self.bag.count()
    }

    fn bag_entry(&self, index: usize) -> Option<(S::Item, u8)> {
        self.bag
            .get(index)
            .map(|&(item, qty)| (item, qty.min(u8::MAX as u32) as u8))
    }

    fn commit_buy(&mut self, item: S::Item, quantity: u8) -> BuyResult {
        match buy(
            self.provider,
            &self.shop_id,
            self.bag,
            self.money,
            item,
            quantity as u32,
        ) {
            Ok(receipt) => BuyResult::Success {
                total_cost: receipt.total,
            },
            Err(ShopError::NotEnoughMoney) => BuyResult::NotEnoughMoney,
            Err(ShopError::InventoryFull) => BuyResult::BagFull,
            Err(ShopError::CannotSell | ShopError::InvalidQuantity) => BuyResult::InvalidItem,
        }
    }

    fn commit_sell(&mut self, bag_index: usize, quantity: u8) -> SellResult {
        let Some(&(item, _)) = self.bag.get(bag_index) else {
            return SellResult::NotInBag;
        };
        match sell(
            self.provider,
            &self.shop_id,
            self.bag,
            self.money,
            item,
            quantity as u32,
        ) {
            Ok(receipt) => SellResult::Success {
                total_value: receipt.total,
            },
            // The machine caps the sell quantity at the owned amount, so a
            // CannotSell here means the shop refuses the item.
            Err(ShopError::CannotSell) => SellResult::Unsellable,
            Err(_) => SellResult::InvalidItem,
        }
    }
}

// ── Top-level mart state ──────────────────────────────────────────

/// Complete mart interaction state machine.
///
/// Construct with [`MartState::new`], drive with
/// [`MartState::update_frame`] once per input frame, passing the game's
/// [`MartBackend`] for all price/capacity/transaction decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MartState<I: Copy + Eq + Hash + Debug> {
    /// Items the shop stocks.
    pub inventory: MartStock<I>,
    pub phase: MartPhase,
}

impl<I: Copy + Eq + Hash + Debug> MartState<I> {
    // ── constructor ──────────────────────────

    /// Begin a mart session with the given shop stock.
    pub fn new(inventory: MartStock<I>) -> Self {
        Self {
            inventory,
            phase: MartPhase::MainMenu {
                cursor: MartTopChoice::Buy,
            },
        }
    }

    // ── per-frame update ─────────────────────

    /// Advance the mart state machine by one frame of input.
    ///
    /// `backend` is consulted for bag contents and is mutated when a
    /// transaction is committed.
    pub fn update_frame<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &mut B,
    ) -> MartUpdate {
        match &self.phase {
            MartPhase::MainMenu { cursor } => self.update_main_menu(input, backend, *cursor),
            MartPhase::Buy(bs) => self.update_buy(input, backend, bs.clone()),
            MartPhase::Sell(ss) => self.update_sell(input, backend, ss.clone()),
            MartPhase::Exiting => MartUpdate::Exit,
        }
    }

    // ── helpers: top menu ────────────────────

    fn update_main_menu<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &B,
        cursor: MartTopChoice,
    ) -> MartUpdate {
        if input.cancel {
            self.phase = MartPhase::Exiting;
            return MartUpdate::Exit;
        }
        let new_cursor = if input.up {
            cursor.prev()
        } else if input.down {
            cursor.next()
        } else {
            cursor
        };
        if new_cursor != cursor {
            self.phase = MartPhase::MainMenu { cursor: new_cursor };
        }
        if input.confirm {
            match new_cursor {
                MartTopChoice::Buy => {
                    self.phase = MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 });
                }
                MartTopChoice::Sell => {
                    // If the bag is empty, stay on the main menu.
                    if backend.bag_len() == 0 {
                        return MartUpdate::Continue;
                    }
                    self.phase = MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 });
                }
                MartTopChoice::Quit => {
                    self.phase = MartPhase::Exiting;
                    return MartUpdate::Exit;
                }
            }
        }
        MartUpdate::Continue
    }

    // ── helpers: buy flow ────────────────────

    fn update_buy<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &mut B,
        bs: BuyMenuState,
    ) -> MartUpdate {
        match bs {
            BuyMenuState::SelectItem { cursor } => self.update_buy_select(input, backend, cursor),
            BuyMenuState::Quantity {
                item_index,
                quantity,
            } => self.update_buy_quantity(input, item_index, quantity),
            BuyMenuState::Confirm {
                item_index,
                quantity,
                selected,
            } => self.update_buy_confirm(input, backend, item_index, quantity, selected),
            BuyMenuState::Result {
                return_to_list, ..
            } => {
                // Auto-dismiss result on next frame.
                if return_to_list {
                    self.phase = MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 });
                } else {
                    self.phase = MartPhase::MainMenu {
                        cursor: MartTopChoice::Buy,
                    };
                }
                MartUpdate::Continue
            }
        }
    }

    fn update_buy_select<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &B,
        cursor: usize,
    ) -> MartUpdate {
        if input.cancel {
            self.phase = MartPhase::MainMenu {
                cursor: MartTopChoice::Buy,
            };
            return MartUpdate::Continue;
        }
        let len = self.inventory.items().len();
        let new_cursor = if len == 0 {
            0
        } else if input.up {
            if cursor == 0 {
                len.saturating_sub(1)
            } else {
                cursor - 1
            }
        } else if input.down {
            if cursor + 1 >= len {
                0
            } else {
                cursor + 1
            }
        } else {
            cursor
        };
        if new_cursor != cursor {
            self.phase = MartPhase::Buy(BuyMenuState::SelectItem { cursor: new_cursor });
        }
        if input.confirm {
            if let Some(item) = self.inventory.get(new_cursor) {
                if backend.can_buy(&item) {
                    self.phase = MartPhase::Buy(BuyMenuState::Quantity {
                        item_index: new_cursor,
                        quantity: 1,
                    });
                }
            }
        }
        MartUpdate::Continue
    }

    fn update_buy_quantity(
        &mut self,
        input: MenuInput,
        item_index: usize,
        mut quantity: u8,
    ) -> MartUpdate {
        if input.cancel {
            // Back to item select, cursor preserved.
            self.phase = MartPhase::Buy(BuyMenuState::SelectItem {
                cursor: item_index,
            });
            return MartUpdate::Continue;
        }
        if input.up {
            quantity = if quantity >= 99 { 1 } else { quantity + 1 };
        } else if input.down {
            quantity = if quantity <= 1 { 99 } else { quantity - 1 };
        }
        if input.confirm {
            self.phase = MartPhase::Buy(BuyMenuState::Confirm {
                item_index,
                quantity,
                selected: ConfirmChoice::Yes,
            });
        } else {
            self.phase = MartPhase::Buy(BuyMenuState::Quantity {
                item_index,
                quantity,
            });
        }
        MartUpdate::Continue
    }

    fn update_buy_confirm<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &mut B,
        item_index: usize,
        quantity: u8,
        selected: ConfirmChoice,
    ) -> MartUpdate {
        if input.cancel {
            // Back to the quantity phase.
            self.phase = MartPhase::Buy(BuyMenuState::Quantity {
                item_index,
                quantity,
            });
            return MartUpdate::Continue;
        }
        let new_selected = if input.up || input.down {
            selected.toggle()
        } else {
            selected
        };
        if new_selected != selected {
            self.phase = MartPhase::Buy(BuyMenuState::Confirm {
                item_index,
                quantity,
                selected: new_selected,
            });
        }
        if input.confirm {
            match new_selected {
                ConfirmChoice::Yes => {
                    let item = match self.inventory.get(item_index) {
                        Some(it) => it,
                        None => {
                            self.phase = MartPhase::Buy(BuyMenuState::Result {
                                dialogue: BuyResult::InvalidItem,
                                return_to_list: false,
                            });
                            return MartUpdate::Continue;
                        }
                    };
                    let result = backend.commit_buy(item, quantity);
                    let play_sfx = matches!(result, BuyResult::Success { .. });
                    self.phase = MartPhase::Buy(BuyMenuState::Result {
                        return_to_list: matches!(result, BuyResult::Success { .. }),
                        dialogue: result,
                    });
                    if play_sfx {
                        return MartUpdate::PlaySound(MartSound::Purchase);
                    }
                }
                ConfirmChoice::No => {
                    // Back to item select, cursor preserved.
                    self.phase = MartPhase::Buy(BuyMenuState::SelectItem {
                        cursor: item_index,
                    });
                }
            }
        }
        MartUpdate::Continue
    }

    // ── helpers: sell flow ───────────────────

    fn update_sell<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &mut B,
        ss: SellMenuState,
    ) -> MartUpdate {
        match ss {
            SellMenuState::SelectItem { cursor } => {
                self.update_sell_select(input, backend, cursor)
            }
            SellMenuState::Quantity {
                item_index,
                quantity,
                max_quantity,
            } => self.update_sell_quantity(input, item_index, quantity, max_quantity),
            SellMenuState::Confirm {
                item_index,
                quantity,
                max_quantity,
                selected,
            } => {
                self.update_sell_confirm(input, backend, item_index, quantity, max_quantity, selected)
            }
            SellMenuState::Result {
                return_to_list, ..
            } => {
                // Auto-dismiss result on next frame.
                if return_to_list {
                    self.phase = MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 });
                } else {
                    self.phase = MartPhase::MainMenu {
                        cursor: MartTopChoice::Sell,
                    };
                }
                MartUpdate::Continue
            }
        }
    }

    fn update_sell_select<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &B,
        cursor: usize,
    ) -> MartUpdate {
        if input.cancel {
            self.phase = MartPhase::MainMenu {
                cursor: MartTopChoice::Sell,
            };
            return MartUpdate::Continue;
        }
        let len = backend.bag_len();
        let new_cursor = if len == 0 {
            0
        } else if input.up {
            if cursor == 0 {
                len.saturating_sub(1)
            } else {
                cursor - 1
            }
        } else if input.down {
            if cursor + 1 >= len {
                0
            } else {
                cursor + 1
            }
        } else {
            cursor
        };
        if new_cursor != cursor {
            self.phase = MartPhase::Sell(SellMenuState::SelectItem { cursor: new_cursor });
        }
        if input.confirm {
            if let Some((_item, owned)) = backend.bag_entry(new_cursor) {
                self.phase = MartPhase::Sell(SellMenuState::Quantity {
                    item_index: new_cursor,
                    quantity: 1,
                    max_quantity: owned,
                });
            }
        }
        MartUpdate::Continue
    }

    fn update_sell_quantity(
        &mut self,
        input: MenuInput,
        item_index: usize,
        mut quantity: u8,
        max_quantity: u8,
    ) -> MartUpdate {
        if input.cancel {
            // Back to sell item select, cursor preserved.
            self.phase = MartPhase::Sell(SellMenuState::SelectItem {
                cursor: item_index,
            });
            return MartUpdate::Continue;
        }
        if input.up {
            quantity = if quantity >= max_quantity {
                1
            } else {
                quantity + 1
            };
        } else if input.down {
            quantity = if quantity <= 1 {
                max_quantity
            } else {
                quantity - 1
            };
        }
        if input.confirm {
            self.phase = MartPhase::Sell(SellMenuState::Confirm {
                item_index,
                quantity,
                max_quantity,
                selected: ConfirmChoice::Yes,
            });
        } else {
            self.phase = MartPhase::Sell(SellMenuState::Quantity {
                item_index,
                quantity,
                max_quantity,
            });
        }
        MartUpdate::Continue
    }

    fn update_sell_confirm<B: MartBackend<Item = I>>(
        &mut self,
        input: MenuInput,
        backend: &mut B,
        item_index: usize,
        quantity: u8,
        max_quantity: u8,
        selected: ConfirmChoice,
    ) -> MartUpdate {
        if input.cancel {
            // Back to the quantity phase.
            self.phase = MartPhase::Sell(SellMenuState::Quantity {
                item_index,
                quantity,
                max_quantity,
            });
            return MartUpdate::Continue;
        }
        let new_selected = if input.up || input.down {
            selected.toggle()
        } else {
            selected
        };
        if new_selected != selected {
            self.phase = MartPhase::Sell(SellMenuState::Confirm {
                item_index,
                quantity,
                max_quantity,
                selected: new_selected,
            });
        }
        if input.confirm {
            match new_selected {
                ConfirmChoice::Yes => {
                    let result = backend.commit_sell(item_index, quantity);
                    let return_to_list = matches!(result, SellResult::Success { .. });
                    self.phase = MartPhase::Sell(SellMenuState::Result {
                        dialogue: result,
                        return_to_list,
                    });
                }
                ConfirmChoice::No => {
                    // Back to sell item select.
                    self.phase = MartPhase::Sell(SellMenuState::SelectItem {
                        cursor: item_index,
                    });
                }
            }
        }
        MartUpdate::Continue
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemKind, ItemProvider, ItemResult};

    fn menu_up() -> MenuInput {
        MenuInput {
            up: true,
            ..MenuInput::default()
        }
    }

    fn menu_down() -> MenuInput {
        MenuInput {
            down: true,
            ..MenuInput::default()
        }
    }

    fn menu_confirm() -> MenuInput {
        MenuInput {
            confirm: true,
            ..MenuInput::default()
        }
    }

    fn menu_cancel() -> MenuInput {
        MenuInput {
            cancel: true,
            ..MenuInput::default()
        }
    }

    // -- Mock backend -------------------------------------------------

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Item {
        Ball,
        Potion,
        Antidote,
        KeyRelic,
    }

    /// Gen-1-style mock shop rules: list prices, half-price sell-back, key
    /// items unsellable, fixed slot capacity.
    struct MockMart {
        money: u32,
        bag: Vec<(Item, u8)>,
        slot_capacity: usize,
    }

    impl MockMart {
        fn new(money: u32) -> Self {
            Self {
                money,
                bag: Vec::new(),
                slot_capacity: 20,
            }
        }

        fn price(item: Item) -> Option<u32> {
            match item {
                Item::Ball => Some(200),
                Item::Potion => Some(300),
                Item::Antidote => Some(100),
                Item::KeyRelic => None,
            }
        }

        fn owned(&self, item: Item) -> u32 {
            self.bag
                .iter()
                .filter(|&&(i, _)| i == item)
                .map(|&(_, q)| q as u32)
                .sum()
        }
    }

    impl MartBackend for MockMart {
        type Item = Item;

        fn bag_len(&self) -> usize {
            self.bag.len()
        }

        fn bag_entry(&self, index: usize) -> Option<(Item, u8)> {
            self.bag.get(index).copied()
        }

        fn can_buy(&self, item: &Item) -> bool {
            Self::price(*item).is_some()
        }

        fn commit_buy(&mut self, item: Item, quantity: u8) -> BuyResult {
            let cost = match Self::price(item) {
                Some(p) => p * quantity as u32,
                None => return BuyResult::InvalidItem,
            };
            if self.money < cost {
                return BuyResult::NotEnoughMoney;
            }
            if !self.bag.iter().any(|&(i, _)| i == item) && self.bag.len() >= self.slot_capacity {
                return BuyResult::BagFull;
            }
            match self.bag.iter_mut().find(|(i, _)| *i == item) {
                Some(slot) => slot.1 += quantity,
                None => self.bag.push((item, quantity)),
            }
            self.money -= cost;
            BuyResult::Success { total_cost: cost }
        }

        fn commit_sell(&mut self, bag_index: usize, quantity: u8) -> SellResult {
            let Some(&(item, owned)) = self.bag.get(bag_index) else {
                return SellResult::NotInBag;
            };
            if item == Item::KeyRelic {
                return SellResult::Unsellable;
            }
            if quantity > owned {
                return SellResult::NotInBag;
            }
            let value = Self::price(item).map(|p| p / 2 * quantity as u32).unwrap_or(0);
            if quantity == owned {
                self.bag.remove(bag_index);
            } else {
                self.bag[bag_index].1 -= quantity;
            }
            self.money += value;
            SellResult::Success { total_value: value }
        }
    }

    fn stock(items: &[Item]) -> MartStock<Item> {
        MartStock::new(items.to_vec())
    }

    // -- MartStock ----------------------------------------------------

    #[test]
    fn mart_stock_basic() {
        let shop = stock(&[Item::Ball, Item::Potion, Item::Antidote]);
        assert_eq!(shop.len(), 3);
        assert!(!shop.is_empty());
        assert_eq!(shop.get(0), Some(Item::Ball));
        assert_eq!(shop.get(1), Some(Item::Potion));
        assert_eq!(shop.get(3), None);
    }

    #[test]
    fn mart_stock_empty() {
        let shop: MartStock<Item> = stock(&[]);
        assert!(shop.is_empty());
        assert_eq!(shop.len(), 0);
    }

    #[test]
    fn mart_stock_from_strings() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum Named {
            Potion,
            Antidote,
        }
        impl FromStr for Named {
            type Err = ();
            fn from_str(s: &str) -> Result<Self, ()> {
                match s {
                    "Potion" => Ok(Named::Potion),
                    "Antidote" => Ok(Named::Antidote),
                    _ => Err(()),
                }
            }
        }
        let items = vec!["Potion".to_string(), "Antidote".to_string()];
        let shop: MartStock<Named> = MartStock::from_strings(&items).unwrap();
        assert_eq!(shop.items(), &[Named::Potion, Named::Antidote]);

        let bad = vec!["Potion".to_string(), "NotAnItem".to_string()];
        let err = MartStock::<Named>::from_strings(&bad).unwrap_err();
        assert_eq!(err, "NotAnItem");

        let empty: Vec<String> = vec![];
        assert!(MartStock::<Named>::from_strings(&empty).unwrap().is_empty());
    }

    // -- MartState: buy flow -------------------------------------------

    #[test]
    fn mart_buy_happy_path() {
        let mut mart = MartState::new(stock(&[Item::Ball, Item::Potion, Item::Antidote]));
        let mut p = MockMart::new(1000);

        // MainMenu: cursor starts at Buy.
        assert!(matches!(
            mart.phase,
            MartPhase::MainMenu {
                cursor: MartTopChoice::Buy
            }
        ));

        // Confirm → enter Buy.
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
        ));

        // Down → Potion (index 1). Confirm → Quantity.
        mart.update_frame(menu_down(), &mut p);
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Quantity {
                item_index: 1,
                quantity: 1,
            })
        ));

        // Up ×2 → quantity=3. Confirm → Confirm phase.
        mart.update_frame(menu_up(), &mut p);
        mart.update_frame(menu_up(), &mut p);
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Confirm {
                item_index: 1,
                quantity: 3,
                selected: ConfirmChoice::Yes,
            })
        ));

        // Confirm on Yes → commits purchase.
        assert_eq!(
            mart.update_frame(menu_confirm(), &mut p),
            MartUpdate::PlaySound(MartSound::Purchase)
        );
        assert_eq!(p.money, 100); // 1000 - 900
        assert_eq!(p.owned(Item::Potion), 3);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Result {
                dialogue: BuyResult::Success { total_cost: 900 },
                return_to_list: true,
            })
        ));

        // Next frame → auto-dismiss Result, back to SelectItem.
        assert_eq!(
            mart.update_frame(MenuInput::default(), &mut p),
            MartUpdate::Continue
        );
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
        ));
    }

    #[test]
    fn mart_buy_not_enough_money() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(100);

        // Enter Buy → SelectItem 0 → Quantity 1 → Confirm Yes → Confirm.
        mart.update_frame(menu_confirm(), &mut p);
        mart.update_frame(menu_confirm(), &mut p);
        mart.update_frame(menu_confirm(), &mut p);
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Result {
                dialogue: BuyResult::NotEnoughMoney,
                return_to_list: false,
            })
        ));
        assert_eq!(p.money, 100); // unchanged

        // Auto-dismiss → back to MainMenu.
        assert_eq!(
            mart.update_frame(MenuInput::default(), &mut p),
            MartUpdate::Continue
        );
        assert!(matches!(mart.phase, MartPhase::MainMenu { .. }));
    }

    #[test]
    fn mart_buy_bag_full() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(999999);
        p.slot_capacity = 1;
        p.bag.push((Item::Ball, 1));

        mart.update_frame(menu_confirm(), &mut p);
        mart.update_frame(menu_confirm(), &mut p);
        mart.update_frame(menu_confirm(), &mut p);
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Result {
                dialogue: BuyResult::BagFull,
                return_to_list: false,
            })
        ));
        assert_eq!(p.money, 999999);

        // Auto-dismiss → MainMenu.
        assert_eq!(
            mart.update_frame(MenuInput::default(), &mut p),
            MartUpdate::Continue
        );
        assert!(matches!(mart.phase, MartPhase::MainMenu { .. }));
    }

    #[test]
    fn mart_buy_cancel_backout_from_quantity() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(1000);

        mart.update_frame(menu_confirm(), &mut p); // into SelectItem
        mart.update_frame(menu_confirm(), &mut p); // into Quantity
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Quantity {
                item_index: 0,
                quantity: 1,
            })
        ));

        // Cancel → back to SelectItem.
        assert_eq!(mart.update_frame(menu_cancel(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
        ));
    }

    #[test]
    fn mart_buy_cancel_backout_from_confirm() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(1000);

        mart.update_frame(menu_confirm(), &mut p); // SelectItem
        mart.update_frame(menu_confirm(), &mut p); // Quantity
        mart.update_frame(menu_confirm(), &mut p); // Confirm
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Confirm {
                item_index: 0,
                quantity: 1,
                selected: ConfirmChoice::Yes,
            })
        ));

        // Cancel → back to Quantity.
        assert_eq!(mart.update_frame(menu_cancel(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Quantity {
                item_index: 0,
                quantity: 1,
            })
        ));
    }

    #[test]
    fn mart_buy_quantity_wrap() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(1000);

        mart.update_frame(menu_confirm(), &mut p); // SelectItem
        mart.update_frame(menu_confirm(), &mut p); // Quantity { quantity: 1 }

        // Down at 1 → wraps to 99.
        mart.update_frame(menu_down(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Quantity {
                item_index: 0,
                quantity: 99,
            })
        ));

        // Up at 99 → wraps to 1.
        mart.update_frame(menu_up(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Quantity {
                item_index: 0,
                quantity: 1,
            })
        ));

        // Up ×2 → 3.
        mart.update_frame(menu_up(), &mut p);
        mart.update_frame(menu_up(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Quantity {
                item_index: 0,
                quantity: 3,
            })
        ));
    }

    #[test]
    fn mart_buy_unbuyable_item_stays_on_select() {
        // can_buy = false (e.g. an unknown / priceless item) refuses to enter
        // the quantity phase.
        let mut mart = MartState::new(stock(&[Item::KeyRelic]));
        let mut p = MockMart::new(1000);
        mart.update_frame(menu_confirm(), &mut p); // into SelectItem
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
        ));
    }

    // -- MartState: sell flow ------------------------------------------

    #[test]
    fn mart_sell_happy_path() {
        let mut mart = MartState::new(stock(&[Item::Ball]));
        let mut p = MockMart::new(0);
        p.bag.push((Item::Potion, 5));

        // MainMenu → down to Sell → Confirm.
        mart.update_frame(menu_down(), &mut p);
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })
        ));

        // Confirm on Potion → Quantity.
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::Quantity {
                item_index: 0,
                quantity: 1,
                max_quantity: 5,
            })
        ));

        // Up → quantity=2. Confirm → Confirm phase.
        mart.update_frame(menu_up(), &mut p);
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::Confirm {
                item_index: 0,
                quantity: 2,
                max_quantity: 5,
                selected: ConfirmChoice::Yes,
            })
        ));

        // Confirm → commit sell.
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert_eq!(p.money, 300); // price 300, sell half = 150 × 2
        assert_eq!(p.owned(Item::Potion), 3);
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::Result {
                dialogue: SellResult::Success { total_value: 300 },
                return_to_list: true,
            })
        ));

        // Auto-dismiss → back to SelectItem.
        assert_eq!(
            mart.update_frame(MenuInput::default(), &mut p),
            MartUpdate::Continue
        );
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })
        ));
    }

    #[test]
    fn mart_sell_empty_bag_stays_on_main_menu() {
        let mut mart = MartState::new(stock(&[Item::Ball]));
        let mut p = MockMart::new(0);
        mart.update_frame(menu_down(), &mut p); // cursor → Sell
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::MainMenu {
                cursor: MartTopChoice::Sell
            }
        ));
    }

    #[test]
    fn mart_sell_unsellable_item() {
        let mut mart = MartState::new(stock(&[Item::Ball]));
        let mut p = MockMart::new(0);
        p.bag.push((Item::KeyRelic, 1));

        mart.update_frame(menu_down(), &mut p);
        mart.update_frame(menu_confirm(), &mut p); // into SelectItem
        mart.update_frame(menu_confirm(), &mut p); // into Quantity
        mart.update_frame(menu_confirm(), &mut p); // into Confirm
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::Result {
                dialogue: SellResult::Unsellable,
                return_to_list: false,
            })
        ));
        assert_eq!(p.money, 0);
        assert_eq!(p.owned(Item::KeyRelic), 1);
    }

    #[test]
    fn mart_sell_quantity_wrap() {
        let mut mart = MartState::new(stock(&[Item::Ball]));
        let mut p = MockMart::new(0);
        p.bag.push((Item::Potion, 3));

        mart.update_frame(menu_down(), &mut p); // cursor → Sell
        mart.update_frame(menu_confirm(), &mut p); // into SelectItem
        mart.update_frame(menu_confirm(), &mut p); // into Quantity { quantity: 1, max: 3 }

        // Down at 1 → wraps to 3.
        mart.update_frame(menu_down(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::Quantity {
                quantity: 3,
                max_quantity: 3,
                ..
            })
        ));

        // Up at 3 → wraps to 1.
        mart.update_frame(menu_up(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::Sell(SellMenuState::Quantity {
                quantity: 1,
                max_quantity: 3,
                ..
            })
        ));
    }

    // -- MartState: top menu -------------------------------------------

    #[test]
    fn mart_top_menu_quit_returns_exit() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(1000);

        // Navigate to Quit (Down×2).
        mart.update_frame(menu_down(), &mut p);
        mart.update_frame(menu_down(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::MainMenu {
                cursor: MartTopChoice::Quit,
            }
        ));

        // Confirm on Quit → Exit.
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Exit);
        assert!(matches!(mart.phase, MartPhase::Exiting));
    }

    #[test]
    fn mart_cancel_at_main_menu_exits() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(1000);

        assert_eq!(mart.update_frame(menu_cancel(), &mut p), MartUpdate::Exit);
        assert!(matches!(mart.phase, MartPhase::Exiting));
        // Exiting is sticky.
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Exit);
    }

    #[test]
    fn mart_main_menu_navigation_wraps() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(1000);

        // Up from Buy → Quit.
        mart.update_frame(menu_up(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::MainMenu {
                cursor: MartTopChoice::Quit,
            }
        ));

        // Down from Quit → Buy.
        mart.update_frame(menu_down(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::MainMenu {
                cursor: MartTopChoice::Buy,
            }
        ));
    }

    #[test]
    fn mart_confirm_no_returns_to_select_item() {
        let mut mart = MartState::new(stock(&[Item::Potion]));
        let mut p = MockMart::new(1000);

        mart.update_frame(menu_confirm(), &mut p); // SelectItem
        mart.update_frame(menu_confirm(), &mut p); // Quantity
        mart.update_frame(menu_confirm(), &mut p); // Confirm { selected: Yes }

        // Toggle to No.
        mart.update_frame(menu_up(), &mut p);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::Confirm {
                selected: ConfirmChoice::No,
                ..
            })
        ));

        // Confirm on No → back to SelectItem, money unchanged.
        let old_money = p.money;
        assert_eq!(mart.update_frame(menu_confirm(), &mut p), MartUpdate::Continue);
        assert_eq!(p.money, old_money);
        assert!(matches!(
            mart.phase,
            MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })
        ));
    }

    // -- MartDriver (backend over the buy/sell drivers) ----------------

    struct DriverGame;

    impl ItemProvider for DriverGame {
        type Item = Item;
        type Effect = ();
        type Monster = ();
        type CustomKind = ();
        fn item_name(&self, _i: &Item) -> &str {
            "X"
        }
        fn item_description(&self, _i: &Item) -> &str {
            "X"
        }
        fn item_effect(&self, _i: &Item) {}
        fn item_price(&self, item: &Item) -> u32 {
            MockMart::price(*item).unwrap_or(0)
        }
        fn can_use_outside_battle(&self, _i: &Item) -> bool {
            true
        }
        fn can_use_in_battle(&self, _i: &Item) -> bool {
            true
        }
        fn use_on_monster(&self, _i: &Item, _m: &mut ()) -> ItemResult {
            ItemResult::NoEffect
        }
        fn consume(&self, _i: &Item) -> bool {
            true
        }
        fn item_kind(&self, _i: &Item) -> ItemKind<()> {
            ItemKind::Consumable
        }
    }

    impl ShopProvider for DriverGame {
        type Item = Item;
        type ShopId = u8;
        fn shop_inventory(&self, _shop_id: &u8) -> Vec<(Item, u32)> {
            vec![(Item::Potion, 300)]
        }
        fn shop_name(&self, _shop_id: &u8) -> &str {
            "Mart"
        }
        fn buy_price(&self, item: &Item) -> u32 {
            MockMart::price(*item).unwrap_or(0)
        }
        // sell_price uses the default (buy_price / 2).
        fn can_sell(&self, item: &Item) -> bool {
            *item != Item::KeyRelic
        }
    }

    #[test]
    fn mart_driver_commit_buy_routes_through_buy_driver() {
        let game = DriverGame;
        let mut bag = Inventory::<Item, 20>::new();
        let mut money = 1000u32;
        let mut backend = MartDriver {
            provider: &game,
            shop_id: 0u8,
            money: &mut money,
            bag: &mut bag,
        };
        assert_eq!(
            backend.commit_buy(Item::Potion, 2),
            BuyResult::Success { total_cost: 600 }
        );
        assert_eq!(*backend.money, 400);
        assert!(backend.bag.contains(&Item::Potion, 2));

        // Not enough money → nothing changes.
        assert_eq!(
            backend.commit_buy(Item::Potion, 99),
            BuyResult::NotEnoughMoney
        );
        assert_eq!(*backend.money, 400);
    }

    #[test]
    fn mart_driver_commit_sell_routes_through_sell_driver() {
        let game = DriverGame;
        let mut bag = Inventory::<Item, 20>::new();
        bag.add(Item::Potion, 5).unwrap();
        bag.add(Item::KeyRelic, 1).unwrap();
        let mut money = 0u32;
        let mut backend = MartDriver {
            provider: &game,
            shop_id: 0u8,
            money: &mut money,
            bag: &mut bag,
        };
        // Sell 2 Potions at half price (150 each).
        assert_eq!(
            backend.commit_sell(0, 2),
            SellResult::Success { total_value: 300 }
        );
        assert_eq!(*backend.money, 300);
        assert!(backend.bag.contains(&Item::Potion, 3));

        // The key item (slot 1) is refused.
        assert_eq!(backend.commit_sell(1, 1), SellResult::Unsellable);
        assert!(backend.bag.contains(&Item::KeyRelic, 1));

        // Out-of-range slot.
        assert_eq!(backend.commit_sell(9, 1), SellResult::NotInBag);
    }

    #[test]
    fn mart_state_full_flow_via_mart_driver() {
        // The state machine works unchanged on the engine-driver backend.
        let game = DriverGame;
        let mut bag = Inventory::<Item, 20>::new();
        let mut money = 1000u32;
        let mut backend = MartDriver {
            provider: &game,
            shop_id: 0u8,
            money: &mut money,
            bag: &mut bag,
        };
        let mut mart = MartState::new(stock(&[Item::Potion]));

        mart.update_frame(menu_confirm(), &mut backend); // into Buy/SelectItem
        mart.update_frame(menu_confirm(), &mut backend); // into Quantity
        mart.update_frame(menu_confirm(), &mut backend); // into Confirm
        assert_eq!(
            mart.update_frame(menu_confirm(), &mut backend),
            MartUpdate::PlaySound(MartSound::Purchase)
        );
        assert_eq!(*backend.money, 700);
        assert!(backend.bag.contains(&Item::Potion, 1));
    }
}
