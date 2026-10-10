//! Poké Mart shop logic — thin pokered shell over
//! [`dotzuki_engine::items::mart`]. The generic Buy/Sell/Quit interaction
//! state machine lives in the engine; this module binds pokered's data and
//! rules: [`ItemId`], the Gen-1 pricing (sell = list/2, key items
//! unsellable — engine/events/pokemart.asm:70-77), the Gen-1 bag
//! ([`Inventory`]) with its overflow-spill semantics, and pokered's
//! `MenuInput { up, down, a, b }` (converted to the engine's
//! `MenuInput { up, down, confirm, cancel }` at the boundary).

use crate::alloc_prelude::*;
use crate::items::inventory::{BAG_ITEM_CAPACITY, Inventory, InventoryError};
use crate::main_menu::MenuInput;
use dotzuki_engine::items::mart::{MartBackend, MartStock};
use dotzuki_engine::menu::MenuInput as EngineMenuInput;
use pokered_data::item_data::get_item_data;
use pokered_data::items::ItemId;

#[path = "shop_field_text.rs"]
mod field_text;
use field_text::{AfterText, FieldFlow, MartText, TextUpdate};

// Re-export the engine's mart types so existing `items::shop::*` paths
// keep working. `SoundId` is pokered's historical name for the engine's
// `MartSound` cue enum.
pub use dotzuki_engine::items::mart::{
    BuyMenuState, BuyResult, ConfirmChoice, MartPhase, MartSound as SoundId, MartTopChoice,
    MartUpdate, SellMenuState, SellResult,
};

/// Items the shop stocks (engine `MartStock` over pokered's [`ItemId`]).
pub type ShopInventory = MartStock<ItemId>;

/// Build a shop stock from script-facing item names.
///
/// `MartStock::from_strings` parses via strum's `EnumString` (exact
/// PascalCase variant names only), which silently rejects the asm-style
/// SCREAMING_SNAKE const names (`"POKE_BALL"`) that `openShop` calls in
/// `maps/*/script.scene` use — the frontend then logs "unknown item id" and
/// never opens the shop. Resolve through [`ItemId::from_const_name`]
/// instead (case/underscore/whitespace-insensitive).
pub fn shop_stock_from_script_names<S: AsRef<str>>(items: &[S]) -> Result<ShopInventory, String> {
    let mut parsed = Vec::with_capacity(items.len());
    for s in items {
        match ItemId::from_const_name(s.as_ref()) {
            Some(id) => parsed.push(id),
            None => return Err(s.as_ref().to_string()),
        }
    }
    Ok(ShopInventory::new(parsed))
}

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
pub struct MartState(dotzuki_engine::items::mart::MartState<ItemId>, Option<FieldFlow>);

impl MartState {
    /// Begin a mart session with the given shop inventory.
    pub fn new(inventory: ShopInventory) -> Self {
        Self(dotzuki_engine::items::mart::MartState::new(inventory), None)
    }

    /// English mart scripts use the original DONE/PROMPT/CONT ownership.
    /// The translated presentation and generic engine clients retain their flow.
    pub fn configure_field_text(&mut self, delay_frames: u16) {
        self.1 = Some(FieldFlow::new(delay_frames));
    }

    pub fn display_phase(&self) -> &MartPhase {
        self.1.as_ref().and_then(|flow| flow.text.as_ref())
            .map_or(&self.0.phase, |text| &text.underlay)
    }

    pub fn field_message_lines(&self) -> Vec<String> {
        self.1.as_ref().map_or_else(Vec::new, |flow| flow.text.as_ref()
            .map_or_else(|| flow.retained_lines.clone(), MartText::visible_lines))
    }

    pub fn field_message_active(&self) -> bool {
        self.1.as_ref().is_some_and(|flow| flow.text.is_some())
    }

    pub fn field_message_waiting(&self) -> bool {
        self.1.as_ref().and_then(|flow| flow.text.as_ref()).is_some_and(MartText::waiting)
    }

    pub fn field_prompt_arrow_visible(&self) -> bool {
        self.1.as_ref().and_then(|flow| flow.text.as_ref())
            .is_some_and(MartText::prompt_arrow_visible)
    }

    /// Drain the PressAB cue from HandleMenuInput or manual text advance.
    /// Quantity selection and the outer exit acknowledgement are silent.
    pub fn take_button_sound(&mut self) -> bool {
        self.1.as_mut().is_some_and(|flow| core::mem::take(&mut flow.text_advance))
    }

    pub fn take_text_advance(&mut self) -> bool {
        self.take_button_sound()
    }

    /// Advance the mart state machine by one frame of input.
    ///
    /// `player.money` and `player.bag` are mutated in-place when a
    /// transaction is committed.
    pub fn update_frame(&mut self, input: MenuInput, player: &mut PlayerData) -> MartUpdate {
        self.update_frame_with_text_input(input, player, input.a, input.b, false)
    }

    pub fn update_frame_with_text_input(&mut self, mut input: MenuInput, player: &mut PlayerData,
        held_a: bool, held_b: bool, sound_playing: bool) -> MartUpdate {
        if let Some(flow) = self.1.as_mut() {
            if flow.exit_wait_a {
                return if held_a { MartUpdate::Continue } else { MartUpdate::Exit };
            }
            if let Some(text) = flow.text.as_mut() {
                match text.tick(input, held_a || held_b, sound_playing, flow.delay) {
                    TextUpdate::Printing => return MartUpdate::Continue,
                    TextUpdate::Scroll => { flow.text_advance = true; return MartUpdate::Continue; }
                    TextUpdate::Finished { acknowledged } => {
                        let text = flow.text.take().unwrap();
                        flow.text_advance |= acknowledged && text.after != AfterText::Exit;
                        flow.retained_lines = text.page().to_vec();
                        match text.after {
                            AfterText::Menu => {},
                            AfterText::BuyList => {
                                self.0.phase = MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 });
                                flow.retained_lines = vec!["Take your time.".into()];
                            },
                            AfterText::AnythingElse => {
                                FieldFlow::reset_main(&mut self.0.phase);
                                flow.anything_else(self.0.phase.clone());
                            },
                            AfterText::Exit => {
                                self.0.phase = text.underlay;
                                flow.exit_wait_a = held_a;
                                return if held_a { MartUpdate::Continue } else { MartUpdate::Exit };
                            },
                        }
                        return MartUpdate::Continue;
                    }
                }
            }
        }
        if let Some(flow) = self.1.as_mut() {
            if (input.a || input.b) && matches!(self.0.phase,
                MartPhase::MainMenu { .. }
                    | MartPhase::Buy(BuyMenuState::SelectItem { .. } | BuyMenuState::Confirm { .. })
                    | MartPhase::Sell(SellMenuState::SelectItem { .. } | SellMenuState::Confirm { .. })) {
                flow.text_advance = true;
            }
        }
        // HandleMenuInput clamps ordinary mart menus (wMenuWrappingEnabled=0).
        // The quantity chooser has its own wrapping policy, handled below.
        if self.1.is_some() {
            match &mut self.0.phase {
                MartPhase::MainMenu { cursor } => {
                    let index: usize = match cursor {
                        MartTopChoice::Buy => 0, MartTopChoice::Sell => 1, MartTopChoice::Quit => 2,
                    };
                    let index = if input.up { index.saturating_sub(1) }
                        else if input.down { (index + 1).min(2) } else { index };
                    *cursor = match index { 0 => MartTopChoice::Buy, 1 => MartTopChoice::Sell, _ => MartTopChoice::Quit };
                    input.up = false;
                    input.down = false;
                },
                MartPhase::Buy(BuyMenuState::Confirm { selected, .. })
                    | MartPhase::Sell(SellMenuState::Confirm { selected, .. }) => {
                    if input.up { *selected = ConfirmChoice::Yes; }
                    else if input.down { *selected = ConfirmChoice::No; }
                    input.up = false;
                    input.down = false;
                },
                _ => {},
            }
        }
        // DisplayListMenuID includes CANCEL after the last item, even when a
        // sale removed the last stack. Expose that row to the engine adapter.
        if self.1.is_some() {
            let selection = match self.0.phase {
                MartPhase::Buy(BuyMenuState::SelectItem { cursor }) => Some((false, cursor, self.0.inventory.len())),
                MartPhase::Sell(SellMenuState::SelectItem { cursor }) => Some((true, cursor, player.bag.count())),
                _ => None,
            };
            if let Some((sell, cursor, count)) = selection {
                let cursor = cursor.min(count);
                let cursor = if input.up { cursor.saturating_sub(1) }
                    else if input.down { (cursor + 1).min(count) } else { cursor };
                if input.b || (input.a && cursor == count) {
                    FieldFlow::reset_main(&mut self.0.phase);
                    self.1.as_mut().unwrap().anything_else(self.0.phase.clone());
                    return MartUpdate::Continue;
                }
                self.0.phase = if sell { MartPhase::Sell(SellMenuState::SelectItem { cursor }) }
                    else { MartPhase::Buy(BuyMenuState::SelectItem { cursor }) };
                input.up = false;
                input.down = false;
            }
        }
        let previous = self.0.phase.clone();
        if self.1.is_some() && input.b {
            // Original NO/B confirmation returns through the saved list,
            // resetting wCurrentMenuItem, rather than reopening quantity.
            let list = match &previous {
                MartPhase::Buy(BuyMenuState::Confirm { .. }) => Some(MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 })),
                MartPhase::Sell(SellMenuState::Confirm { .. }) => Some(MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 })),
                _ => None,
            };
            if let Some(list) = list {
                let flow = self.1.as_mut().unwrap();
                flow.retained_lines = match list {
                    MartPhase::Buy(_) => vec!["Take your time.".into()],
                    _ => vec!["What would you".into(), "like to sell?".into()],
                };
                self.0.phase = list;
                return MartUpdate::Continue;
            }
        }
        // Gen I's shared quantity chooser tests A before B and directions.
        // Keep the generic mart's other menu policies at their existing boundary.
        let quantity_menu = matches!(self.0.phase,
            MartPhase::Buy(BuyMenuState::Quantity { .. })
                | MartPhase::Sell(SellMenuState::Quantity { .. }));
        let direction_allowed = !quantity_menu || (!input.a && !input.b);
        let engine_input = EngineMenuInput {
            up: input.up && direction_allowed,
            down: input.down && direction_allowed,
            confirm: input.a,
            cancel: input.b && (!quantity_menu || !input.a),
        };
        let result = self.0.update_frame(engine_input, player);
        let Some(flow) = self.1.as_mut() else { return result; };
        if result == MartUpdate::Exit {
            flow.print(&["Thank you!"], previous, true, AfterText::Exit);
            return MartUpdate::Continue;
        }
        match (&previous, &self.0.phase) {
            (MartPhase::MainMenu { .. }, MartPhase::Buy(BuyMenuState::SelectItem { .. })) => {
                flow.print(&["Take your time."], previous.clone(), false, AfterText::Menu);
            },
            (MartPhase::MainMenu { .. }, MartPhase::Sell(SellMenuState::SelectItem { .. })) => {
                flow.print(&["What would you", "like to sell?"], previous.clone(), false, AfterText::Menu);
            },
            (MartPhase::MainMenu { .. }, MartPhase::MainMenu { cursor: MartTopChoice::Sell })
                if input.a && !input.b && player.bag.count() == 0 => {
                flow.print(&["You don't have", "anything to sell."], self.0.phase.clone(), true, AfterText::AnythingElse);
            },
            (MartPhase::Buy(BuyMenuState::SelectItem { .. }) | MartPhase::Sell(SellMenuState::SelectItem { .. }), MartPhase::MainMenu { .. }) => {
                FieldFlow::reset_main(&mut self.0.phase);
                flow.anything_else(self.0.phase.clone());
            },
            (MartPhase::Sell(SellMenuState::SelectItem { .. }), MartPhase::Sell(SellMenuState::Quantity { item_index, .. }))
                if player.bag.get(*item_index).is_some_and(|(item, _)| !can_sell(item)) => {
                self.0.phase = MartPhase::Sell(SellMenuState::Result { dialogue: SellResult::Unsellable, return_to_list: false });
                flow.print(&["I can't put a", "price on that."], previous.clone(), true, AfterText::AnythingElse);
            },
            (MartPhase::Buy(BuyMenuState::Quantity { .. }), MartPhase::Buy(BuyMenuState::Confirm { item_index, quantity, .. })) => {
                if let Some(data) = self.0.inventory.get(*item_index).and_then(get_item_data) {
                    flow.text = Some(MartText::new(vec![format!("{}?", data.name), "That will be".into(),
                        format!("¥{}. OK?", u32::from(data.price) * u32::from(*quantity))], previous.clone(), false, AfterText::Menu));
                    flow.retained_lines.clear();
                }
            },
            (MartPhase::Sell(SellMenuState::Quantity { .. }), MartPhase::Sell(SellMenuState::Confirm { item_index, quantity, .. })) => {
                if let Some((item, _)) = player.bag.get(*item_index) {
                    let price = sell_price(item, *quantity).unwrap_or(0);
                    flow.text = Some(MartText::new(vec!["I can pay you".into(), format!("¥{price} for that.")], previous.clone(), false, AfterText::Menu));
                    flow.retained_lines.clear();
                }
            },
            (_, MartPhase::Buy(BuyMenuState::Result { dialogue, .. })) => {
                let (lines, after): (&[&str], _) = match dialogue {
                    BuyResult::Success { .. } => (&["Here you are!", "Thank you!"], AfterText::BuyList),
                    BuyResult::NotEnoughMoney => (&["You don't have", "enough money."], AfterText::AnythingElse),
                    BuyResult::BagFull => (&["You can't carry", "any more items."], AfterText::AnythingElse),
                    BuyResult::InvalidItem => (&["That item doesn't", "exist!"], AfterText::AnythingElse),
                };
                flow.print(lines, previous.clone(), true, after);
                if matches!(dialogue, BuyResult::Success { .. }) { flow.text.as_mut().unwrap().wait_for_purchase_sound(); }
            },
            (_, MartPhase::Sell(SellMenuState::Result { dialogue: SellResult::Success { .. }, .. })) => {
                // AddAmountSoldToMoney plays the cash-register SFX, without
                // a success textbox. Keep the saved selling greeting.
                self.0.phase = MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 });
                flow.retained_lines = vec!["What would you".into(), "like to sell?".into()];
                return MartUpdate::PlaySound(SoundId::Purchase);
            },
            (MartPhase::Buy(BuyMenuState::Confirm { .. }), MartPhase::Buy(BuyMenuState::SelectItem { .. })) => {
                self.0.phase = MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 });
                flow.retained_lines = vec!["Take your time.".into()];
            },
            (MartPhase::Sell(SellMenuState::Confirm { .. }), MartPhase::Sell(SellMenuState::SelectItem { .. })) => {
                self.0.phase = MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 });
                flow.retained_lines = vec!["What would you".into(), "like to sell?".into()];
            },
            (MartPhase::Buy(BuyMenuState::Quantity { .. }), MartPhase::Buy(BuyMenuState::SelectItem { .. })) => {
                self.0.phase = MartPhase::Buy(BuyMenuState::SelectItem { cursor: 0 });
                flow.retained_lines = vec!["Take your time.".into()];
            },
            (MartPhase::Sell(SellMenuState::Quantity { .. }), MartPhase::Sell(SellMenuState::SelectItem { .. })) => {
                self.0.phase = MartPhase::Sell(SellMenuState::SelectItem { cursor: 0 });
                flow.retained_lines = vec!["What would you".into(), "like to sell?".into()];
            },
            _ => {},
        }
        result
    }
}

impl core::ops::Deref for MartState {
    type Target = dotzuki_engine::items::mart::MartState<ItemId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for MartState {
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
    *money = money.saturating_add(value).min(999_999);
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
