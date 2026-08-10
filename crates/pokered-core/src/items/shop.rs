use std::str::FromStr;

use crate::items::inventory::{BAG_ITEM_CAPACITY, Inventory};
use crate::main_menu::MenuInput;
use pokered_data::item_data::get_item_data;
use pokered_data::items::ItemId;

// ──────────────────────────────────────────
//  NEW — Mart state machine types
// ──────────────────────────────────────────

/// Bundled player data consumed by mart transactions.
#[derive(Debug, Clone)]
pub struct PlayerData {
    pub money: u32,
    pub bag: Inventory<BAG_ITEM_CAPACITY>,
}

/// Sound effects the mart layer can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundId {
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
    /// Play the given sound effect (e.g. after a successful purchase).
    PlaySound(SoundId),
    /// The mart interaction is over — return to the overworld.
    Exit,
}

// ── Sub-state enums ────────────────────────

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

// ── Internal phase enum ────────────────────

/// Actual state machine phase (not exposed directly — read via `MartState::phase`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MartPhase {
    MainMenu { cursor: MartTopChoice },
    Buy(BuyMenuState),
    Sell(SellMenuState),
    Exiting,
}

// ── Top-level mart state ───────────────────

/// Complete mart state machine.
///
/// Wrap-around constructor: [`MartState::new`].
/// Drive with [`MartState::update_frame`] each tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MartState {
    /// Items the shop stocks.
    pub inventory: ShopInventory,
    pub phase: MartPhase,
}

impl MartState {
    // ── constructor ──────────────────────────

    /// Begin a mart session with the given shop inventory.
    pub fn new(inventory: ShopInventory) -> Self {
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
    /// `player.money` and `player.bag` are mutated in-place when a
    /// transaction is committed.
    pub fn update_frame(&mut self, input: MenuInput, player: &mut PlayerData) -> MartUpdate {
        match &self.phase {
            MartPhase::MainMenu { cursor } => self.update_main_menu(input, player, *cursor),
            MartPhase::Buy(bs) => self.update_buy(input, player, bs.clone()),
            MartPhase::Sell(ss) => self.update_sell(input, player, ss.clone()),
            MartPhase::Exiting => MartUpdate::Exit,
        }
    }

    // ── helpers: top menu ────────────────────

    fn update_main_menu(
        &mut self,
        input: MenuInput,
        player: &PlayerData,
        cursor: MartTopChoice,
    ) -> MartUpdate {
        if input.b {
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
        if input.a {
            match new_cursor {
                MartTopChoice::Buy => {
                    self.phase = MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 });
                }
                MartTopChoice::Sell => {
                    // If bag is empty, return to main menu immediately (like asm).
                    if player.bag.is_empty() {
                        // Nothing to sell — stay on main menu.
                        // In asm this shows a "bag empty" text then returns to main.
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

    fn update_buy(
        &mut self,
        input: MenuInput,
        player: &mut PlayerData,
        bs: BuyMenuState,
    ) -> MartUpdate {
        match bs {
            BuyMenuState::SelectItem { cursor } => self.update_buy_select(input, cursor),
            BuyMenuState::Quantity {
                item_index,
                quantity,
            } => self.update_buy_quantity(input, item_index, quantity),
            BuyMenuState::Confirm {
                item_index,
                quantity,
                selected,
            } => self.update_buy_confirm(input, player, item_index, quantity, selected),
            BuyMenuState::Result {
                return_to_list,
                ..
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

    fn update_buy_select(&mut self, input: MenuInput, cursor: usize) -> MartUpdate {
        if input.b {
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
        if input.a {
            if let Some(item) = self.inventory.get(new_cursor) {
                if get_item_data(item).is_some() {
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
        if input.b {
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
        if input.a {
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

    fn update_buy_confirm(
        &mut self,
        input: MenuInput,
        player: &mut PlayerData,
        item_index: usize,
        quantity: u8,
        selected: ConfirmChoice,
    ) -> MartUpdate {
        if input.b {
            // Back to quantity phase (task spec).
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
        if input.a {
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
                    let result = try_buy(item, quantity, &mut player.money, &mut player.bag);
                    let play_sfx = matches!(result, BuyResult::Success { .. });
                    self.phase = MartPhase::Buy(BuyMenuState::Result {
                        return_to_list: matches!(result, BuyResult::Success { .. }),
                        dialogue: result,
                    });
                    if play_sfx {
                        return MartUpdate::PlaySound(SoundId::Purchase);
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

    fn update_sell(
        &mut self,
        input: MenuInput,
        player: &mut PlayerData,
        ss: SellMenuState,
    ) -> MartUpdate {
        match ss {
            SellMenuState::SelectItem { cursor } => self.update_sell_select(input, player, cursor),
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
                self.update_sell_confirm(input, player, item_index, quantity, max_quantity, selected)
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

    fn update_sell_select(
        &mut self,
        input: MenuInput,
        player: &PlayerData,
        cursor: usize,
    ) -> MartUpdate {
        if input.b {
            self.phase = MartPhase::MainMenu {
                cursor: MartTopChoice::Sell,
            };
            return MartUpdate::Continue;
        }
        let len = player.bag.count();
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
        if input.a {
            if let Some((_item, owned)) = player.bag.get(new_cursor) {
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
        if input.b {
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
        if input.a {
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

    fn update_sell_confirm(
        &mut self,
        input: MenuInput,
        player: &mut PlayerData,
        item_index: usize,
        quantity: u8,
        max_quantity: u8,
        selected: ConfirmChoice,
    ) -> MartUpdate {
        if input.b {
            // Back to quantity phase.
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
        if input.a {
            match new_selected {
                ConfirmChoice::Yes => {
                    let result = try_sell(item_index, quantity, &mut player.money, &mut player.bag);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuyResult {
    Success { total_cost: u32 },
    NotEnoughMoney,
    BagFull,
    InvalidItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellResult {
    Success { total_value: u32 },
    Unsellable,
    NotInBag,
    InvalidItem,
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
    if let Some(data) = get_item_data(item) {
        !data.is_key_item && data.price > 0
    } else {
        false
    }
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

use crate::items::inventory::InventoryError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopInventory {
    items: Vec<ItemId>,
}

impl ShopInventory {
    pub fn new(items: Vec<ItemId>) -> Self {
        Self { items }
    }

    pub fn items(&self) -> &[ItemId] {
        &self.items
    }

    pub fn get(&self, index: usize) -> Option<ItemId> {
        self.items.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Build a [`ShopInventory`] from a list of item name strings.
    ///
    /// Each string is parsed into an [`ItemId`] via [`FromStr`].
    /// Returns `Err` with the offending string if parsing fails.
    pub fn from_item_id_strings(items: &[String]) -> Result<Self, String> {
        let mut parsed = Vec::with_capacity(items.len());
        for s in items {
            let id = ItemId::from_str(s).map_err(|_| s.clone())?;
            parsed.push(id);
        }
        Ok(Self::new(parsed))
    }
}
