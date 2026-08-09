//! Battle menu state machines.
//!
//! Replicates `engine/battle/core.asm` DisplayBattleMenu + MoveSelectionMenu.
//!
//! **Main battle menu** — 2×2 grid navigated with D-pad.
//! Display layout (English version, text_boxes.asm):
//!   FIGHT   | POKeMON
//!   ITEM    | RUN
//!
//! Grid positions map to internal IDs with swap (core.asm 2140-2151):
//!   (0,0)=FIGHT  (0,1)=POKeMON
//!   (1,0)=ITEM   (1,1)=RUN
//!
//! **Safari battle menu** — 2×2 grid:
//!   BALL | BAIT
//!   ROCK | RUN
//!
//! **Move selection menu** — up to 4 moves, vertical list:
//!   Each move has PP; disabled moves and 0-PP moves are rejected.
//!
//! **Bag/Item menu** — vertical list of items from player's bag.
//!   Replicates `engine/battle/core.asm` BagWasSelected → DisplayBagMenu flow.

use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;

use crate::main_menu::MenuInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleMenuAction {
    Fight,
    Bag,
    Pokemon,
    Run,
}

/// 2×2 grid position: row (0=top, 1=bottom) × col (0=left, 1=right).
///
/// Layout used by renderer/menu UI:
///   (0,0)=FIGHT   (0,1)=POKeMON
///   (1,0)=BAG     (1,1)=RUN
#[derive(Debug, Clone)]
pub struct BattleMenuState {
    row: usize,
    col: usize,
    /// `wBattleAndStartSavedMenuItem` — remembers position across re-displays.
    saved_position: usize,
}

impl BattleMenuState {
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            saved_position: 0,
        }
    }

    pub fn restore_saved_position(&mut self) {
        let (r, c) = Self::index_to_grid(self.saved_position);
        self.row = r;
        self.col = c;
    }

    fn index_to_grid(index: usize) -> (usize, usize) {
        match index {
            0 => (0, 0),
            1 => (1, 0),
            2 => (0, 1),
            3 => (1, 1),
            _ => (0, 0),
        }
    }

    fn grid_to_index(row: usize, col: usize) -> usize {
        row * 2 + col
    }

    fn grid_to_action(row: usize, col: usize) -> BattleMenuAction {
        match (row, col) {
            (0, 0) => BattleMenuAction::Fight,
            (0, 1) => BattleMenuAction::Pokemon,
            (1, 0) => BattleMenuAction::Bag,
            (1, 1) => BattleMenuAction::Run,
            _ => BattleMenuAction::Fight,
        }
    }

    pub fn update_frame(&mut self, input: BattleMenuInput) -> Option<BattleMenuAction> {
        if input.up && self.row > 0 {
            self.row -= 1;
        } else if input.down && self.row < 1 {
            self.row += 1;
        }
        if input.left && self.col > 0 {
            self.col -= 1;
        } else if input.right && self.col < 1 {
            self.col += 1;
        }

        if input.a {
            self.saved_position = Self::grid_to_index(self.row, self.col);
            return Some(Self::grid_to_action(self.row, self.col));
        }

        None
    }

    pub fn row(&self) -> usize {
        self.row
    }

    pub fn col(&self) -> usize {
        self.col
    }

    pub fn current_action(&self) -> BattleMenuAction {
        Self::grid_to_action(self.row, self.col)
    }
}

impl Default for BattleMenuState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleMenuInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
}

impl BattleMenuInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            a: false,
            b: false,
        }
    }

    pub fn from_menu_input(input: MenuInput, left: bool, right: bool) -> Self {
        Self {
            up: input.up,
            down: input.down,
            left,
            right,
            a: input.a,
            b: input.b,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafariMenuAction {
    Ball,
    Bait,
    Rock,
    Run,
}

#[derive(Debug, Clone)]
pub struct SafariBattleMenuState {
    row: usize,
    col: usize,
    pub safari_balls_remaining: u8,
}

impl SafariBattleMenuState {
    pub fn new(safari_balls: u8) -> Self {
        Self {
            row: 0,
            col: 0,
            safari_balls_remaining: safari_balls,
        }
    }

    fn grid_to_action(row: usize, col: usize) -> SafariMenuAction {
        match (row, col) {
            (0, 0) => SafariMenuAction::Ball,
            (0, 1) => SafariMenuAction::Bait,
            (1, 0) => SafariMenuAction::Rock,
            (1, 1) => SafariMenuAction::Run,
            _ => SafariMenuAction::Ball,
        }
    }

    pub fn update_frame(&mut self, input: BattleMenuInput) -> Option<SafariMenuAction> {
        if input.up && self.row > 0 {
            self.row -= 1;
        } else if input.down && self.row < 1 {
            self.row += 1;
        }
        if input.left && self.col > 0 {
            self.col -= 1;
        } else if input.right && self.col < 1 {
            self.col += 1;
        }

        if input.a {
            return Some(Self::grid_to_action(self.row, self.col));
        }

        None
    }

    pub fn row(&self) -> usize {
        self.row
    }

    pub fn col(&self) -> usize {
        self.col
    }

    pub fn current_action(&self) -> SafariMenuAction {
        Self::grid_to_action(self.row, self.col)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMenuResult {
    Selected(usize),
    Cancelled,
    NoPP(usize),
    Disabled(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveSlot {
    pub move_id: MoveId,
    pub current_pp: u8,
    pub max_pp: u8,
    pub is_disabled: bool,
}

#[derive(Debug, Clone)]
pub struct MoveMenuState {
    moves: Vec<MoveSlot>,
    cursor: usize,
    pub can_cancel: bool,
}

impl MoveMenuState {
    pub fn new(moves: Vec<MoveSlot>) -> Self {
        let cursor = 0;
        Self {
            moves,
            cursor,
            can_cancel: true,
        }
    }

    pub fn update_frame(&mut self, input: MenuInput) -> Option<MoveMenuResult> {
        if input.b && self.can_cancel {
            return Some(MoveMenuResult::Cancelled);
        }

        if input.up {
            self.cursor_up();
        } else if input.down {
            self.cursor_down();
        }

        if input.a {
            return Some(self.try_select());
        }

        None
    }

    fn cursor_up(&mut self) {
        if self.cursor == 0 {
            self.cursor = self.moves.len().saturating_sub(1);
        } else {
            self.cursor -= 1;
        }
    }

    fn cursor_down(&mut self) {
        if self.moves.is_empty() {
            return;
        }
        self.cursor += 1;
        if self.cursor >= self.moves.len() {
            self.cursor = 0;
        }
    }

    fn try_select(&self) -> MoveMenuResult {
        let slot = &self.moves[self.cursor];
        if slot.is_disabled {
            return MoveMenuResult::Disabled(self.cursor);
        }
        if slot.current_pp == 0 {
            return MoveMenuResult::NoPP(self.cursor);
        }
        MoveMenuResult::Selected(self.cursor)
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn moves(&self) -> &[MoveSlot] {
        &self.moves
    }

    pub fn move_count(&self) -> usize {
        self.moves.len()
    }

    pub fn current_move(&self) -> Option<&MoveSlot> {
        self.moves.get(self.cursor)
    }

    pub fn has_any_usable_move(&self) -> bool {
        self.moves
            .iter()
            .any(|m| m.current_pp > 0 && !m.is_disabled)
    }
}

/// Actions available in the party submenu after selecting a Pokemon.
/// Matches SWITCH/STATS/CANCEL menu from original game (text_boxes.asm).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartySubMenuAction {
    Switch,
    Stats,
    Cancel,
}

/// Party submenu state for SWITCH/STATS/CANCEL selection.
#[derive(Debug, Clone)]
pub struct PartySubMenuState {
    cursor: usize,
}

impl PartySubMenuState {
    pub fn new() -> Self {
        Self { cursor: 0 }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn current_action(&self) -> PartySubMenuAction {
        match self.cursor {
            0 => PartySubMenuAction::Switch,
            1 => PartySubMenuAction::Stats,
            _ => PartySubMenuAction::Cancel,
        }
    }

    pub fn update_frame(&mut self, input: BattleMenuInput) -> Option<PartySubMenuAction> {
        if input.up && self.cursor > 0 {
            self.cursor -= 1;
        } else if input.down && self.cursor < 2 {
            self.cursor += 1;
        }

        if input.a {
            return Some(self.current_action());
        }

        if input.b {
            return Some(PartySubMenuAction::Cancel);
        }

        None
    }
}

impl Default for PartySubMenuState {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of selecting an item from the bag menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagMenuResult {
    /// Player selected an item to use.
    Selected(ItemId),
    /// Player pressed B to cancel.
    Cancelled,
}

/// Bag/Item menu state for selecting items during battle.
/// Vertical list of items, navigated with up/down.
#[derive(Clone)]
pub struct BagMenuState {
    /// Items available in the bag (from player's inventory).
    items: Vec<(ItemId, u8)>,
    /// Current cursor position (0-based index).
    cursor: usize,
    /// Saved cursor position for returning to menu.
    saved_cursor: usize,
}

impl BagMenuState {
    pub fn new(items: Vec<(ItemId, u8)>) -> Self {
        Self {
            items,
            cursor: 0,
            saved_cursor: 0,
        }
    }

    pub fn with_saved_cursor(items: Vec<(ItemId, u8)>, saved: usize) -> Self {
        let cursor = saved.min(items.len().saturating_sub(1));
        Self {
            items,
            cursor,
            saved_cursor: saved,
        }
    }

    pub fn update_frame(&mut self, input: BattleMenuInput) -> Option<BagMenuResult> {
        if input.b {
            return Some(BagMenuResult::Cancelled);
        }

        let len = self.items.len();
        if len == 0 {
            if input.a || input.b {
                return Some(BagMenuResult::Cancelled);
            }
            return None;
        }

        if input.up {
            self.cursor = if self.cursor == 0 {
                len - 1
            } else {
                self.cursor - 1
            };
        } else if input.down {
            self.cursor = (self.cursor + 1) % len;
        }

        if input.a {
            self.saved_cursor = self.cursor;
            let (item_id, _) = self.items[self.cursor];
            return Some(BagMenuResult::Selected(item_id));
        }

        None
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn saved_cursor(&self) -> usize {
        self.saved_cursor
    }

    pub fn items(&self) -> &[(ItemId, u8)] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current_item(&self) -> Option<(ItemId, u8)> {
        self.items.get(self.cursor).copied()
    }
}

/// Categories of items for battle filtering.
/// Used to determine which items are usable during battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemCategory {
    /// Poké Balls (can be used in wild battles only).
    Ball,
    /// Healing items (Potions, Fresh Water, etc.).
    Healing,
    /// Status cure items (Antidote, Burn Heal, etc.).
    StatusCure,
    /// Revive items (Revive, Max Revive).
    Revive,
    /// Battle stat boost items (X Attack, X Defend, etc.).
    BattleStat,
    /// Items usable both in and out of battle.
    UsableInBattle,
    /// Items only usable out of battle (Key items, etc.).
    NotUsableInBattle,
}

impl ItemCategory {
    pub fn from_item(item: ItemId) -> Self {
        match item {
            ItemId::MasterBall
            | ItemId::UltraBall
            | ItemId::GreatBall
            | ItemId::PokeBall
            | ItemId::SafariBall => ItemCategory::Ball,

            ItemId::Potion
            | ItemId::SuperPotion
            | ItemId::HyperPotion
            | ItemId::MaxPotion
            | ItemId::FullRestore
            | ItemId::FreshWater
            | ItemId::SodaPop
            | ItemId::Lemonade => ItemCategory::Healing,

            ItemId::Antidote
            | ItemId::BurnHeal
            | ItemId::IceHeal
            | ItemId::Awakening
            | ItemId::ParlyzHeal
            | ItemId::FullHeal => ItemCategory::StatusCure,

            ItemId::Revive | ItemId::MaxRevive => ItemCategory::Revive,

            ItemId::XAttack
            | ItemId::XDefend
            | ItemId::XSpeed
            | ItemId::XSpecial
            | ItemId::XAccuracy
            | ItemId::DireHit
            | ItemId::GuardSpec => ItemCategory::BattleStat,

            ItemId::PokeDoll | ItemId::PokeFlute => ItemCategory::UsableInBattle,

            _ => ItemCategory::NotUsableInBattle,
        }
    }

    pub fn is_usable_in_battle(self) -> bool {
        matches!(
            self,
            ItemCategory::Ball
                | ItemCategory::Healing
                | ItemCategory::StatusCure
                | ItemCategory::Revive
                | ItemCategory::BattleStat
                | ItemCategory::UsableInBattle
        )
    }

    pub fn is_usable_in_trainer_battle(self) -> bool {
        matches!(
            self,
            ItemCategory::Healing
                | ItemCategory::StatusCure
                | ItemCategory::Revive
                | ItemCategory::BattleStat
                | ItemCategory::UsableInBattle
        )
    }
}
