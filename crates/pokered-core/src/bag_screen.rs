//! Overworld ITEM bag screen state machine.
//!
//! Reachable from the Start menu → ITEM. Lists the bag, and on a selected item
//! offers USE / TOSS / CANCEL (matching the original overworld item menu). USE
//! hands the item id back to the caller to dispatch a field effect; TOSS asks a
//! quantity and removes that many. Pure logic (no rendering) — mirrors
//! `party_screen::PartyScreenState`.

use pokered_data::items::ItemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BagScreenInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    /// SELECT button — the bag's swap-items mode (engine/menus/swap_items.asm):
    /// first press marks the row (▷), second press swaps/merges with it.
    pub select: bool,
}

impl BagScreenInput {
    pub fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagPhase {
    /// Scrolling the item list (cursor over an item, or the trailing CANCEL row).
    Browsing,
    /// USE / TOSS / CANCEL menu for the selected item. cursor: 0=USE 1=TOSS 2=CANCEL.
    ActionMenu { cursor: u8 },
    /// "Toss how many?" quantity selector for the selected item.
    TossQuantity { qty: u32 },
    /// SELECT-swap mode (swap_items.asm): the marked row waits for a second
    /// SELECT on another row to swap/merge. B cancels the mark.
    SwapFrom { row: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BagScreenAction {
    /// Still open.
    Active,
    /// Player backed out of the whole bag (return to the start menu / overworld).
    Cancelled,
    /// USE the item at `index`. The caller dispatches the field effect and is
    /// responsible for any consumption (then rebuilds the bag via `set_items`).
    UseItem { item: ItemId, index: usize },
    /// TOSS `quantity` of the item at `index`. The caller removes them and
    /// rebuilds the bag via `set_items`.
    TossItem {
        item: ItemId,
        index: usize,
        quantity: u32,
    },
}

/// One visible row: a bag item plus the trailing CANCEL row.
#[derive(Debug, Clone)]
pub struct BagScreenState {
    items: Vec<(ItemId, u32)>,
    /// 0..items.len() selects an item; items.len() is the CANCEL row.
    cursor: usize,
    /// First visible row (for scrolling long bags).
    scroll: usize,
    phase: BagPhase,
    /// How many rows are shown at once (set by the renderer's viewport).
    visible_rows: usize,
}

impl BagScreenState {
    pub fn new(items: Vec<(ItemId, u32)>) -> Self {
        Self {
            items,
            cursor: 0,
            scroll: 0,
            phase: BagPhase::Browsing,
            visible_rows: 4,
        }
    }

    /// Replace the item list (after a USE/TOSS mutates the bag), clamping the
    /// cursor. Returns to Browsing.
    pub fn set_items(&mut self, items: Vec<(ItemId, u32)>) {
        self.items = items;
        let max = self.row_count().saturating_sub(1);
        self.cursor = self.cursor.min(max);
        self.clamp_scroll();
        self.phase = BagPhase::Browsing;
    }

    pub fn items(&self) -> &[(ItemId, u32)] {
        &self.items
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn scroll(&self) -> usize {
        self.scroll
    }
    pub fn phase(&self) -> BagPhase {
        self.phase
    }
    pub fn set_visible_rows(&mut self, rows: usize) {
        self.visible_rows = rows.max(1);
        self.clamp_scroll();
    }
    /// True when the cursor is on the trailing CANCEL row.
    pub fn on_cancel_row(&self) -> bool {
        self.cursor == self.items.len()
    }
    pub fn selected_item(&self) -> Option<(ItemId, u32)> {
        self.items.get(self.cursor).copied()
    }

    /// Total selectable rows: every item + the CANCEL row.
    fn row_count(&self) -> usize {
        self.items.len() + 1
    }

    fn clamp_scroll(&mut self) {
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + self.visible_rows {
            self.scroll = self.cursor + 1 - self.visible_rows;
        }
        let max_scroll = self.row_count().saturating_sub(self.visible_rows);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }

    pub fn update_frame(&mut self, input: BagScreenInput) -> BagScreenAction {
        match self.phase {
            BagPhase::Browsing => self.update_browsing(input),
            BagPhase::ActionMenu { cursor } => self.update_action_menu(input, cursor),
            BagPhase::TossQuantity { qty } => self.update_toss_quantity(input, qty),
            BagPhase::SwapFrom { row } => self.update_swap(input, row),
        }
    }

    fn update_browsing(&mut self, input: BagScreenInput) -> BagScreenAction {
        let rows = self.row_count();
        if input.up && self.cursor > 0 {
            self.cursor -= 1;
            self.clamp_scroll();
        } else if input.down && self.cursor < rows - 1 {
            self.cursor += 1;
            self.clamp_scroll();
        }

        if input.b {
            return BagScreenAction::Cancelled;
        }
        if input.a {
            if self.on_cancel_row() {
                return BagScreenAction::Cancelled;
            }
            self.phase = BagPhase::ActionMenu { cursor: 0 };
        }
        // SELECT marks the first swap row (SwapItemsInMenu, swap_items.asm) —
        // only item rows, never CANCEL.
        if input.select && !self.on_cancel_row() && !self.items.is_empty() {
            self.phase = BagPhase::SwapFrom { row: self.cursor };
        }
        BagScreenAction::Active
    }

    /// SELECT-swap completion (swap_items.asm): the second SELECT on another
    /// row either SWAPS the two entries, or — for same-kind entries whose
    /// combined count fits one slot (≤99) — MERGES them into the first and
    /// drops the second; a merge that would overflow leaves the second filled
    /// to 99 with the remainder staying in the first. B cancels the mark.
    fn update_swap(&mut self, input: BagScreenInput, row: usize) -> BagScreenAction {
        if input.b {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Active;
        }
        if input.up && self.cursor > 0 {
            self.cursor -= 1;
            self.clamp_scroll();
        } else if input.down && self.cursor < self.row_count() - 1 {
            self.cursor += 1;
            self.clamp_scroll();
        }
        if input.select {
            let target = self.cursor;
            if target != row && target < self.items.len() && row < self.items.len() {
                let (a_item, a_qty) = self.items[row];
                let (b_item, b_qty) = self.items[target];
                if a_item == b_item {
                    // Merge: combined ≤99 all into the first; otherwise the
                    // second fills to 99, remainder stays in the first.
                    let total = a_qty + b_qty;
                    if total <= 99 {
                        self.items[row] = (a_item, total);
                        self.items.remove(target);
                    } else {
                        self.items[target] = (b_item, 99);
                        self.items[row] = (a_item, total - 99);
                    }
                } else {
                    self.items.swap(row, target);
                }
                if self.cursor >= self.row_count() {
                    self.cursor = self.row_count() - 1;
                }
                self.clamp_scroll();
            }
            self.phase = BagPhase::Browsing;
        }
        BagScreenAction::Active
    }

    fn update_action_menu(&mut self, input: BagScreenInput, mut cursor: u8) -> BagScreenAction {
        if input.up && cursor > 0 {
            cursor -= 1;
        } else if input.down && cursor < 2 {
            cursor += 1;
        }
        self.phase = BagPhase::ActionMenu { cursor };

        if input.b {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Active;
        }
        if input.a {
            let Some((item, qty)) = self.selected_item() else {
                self.phase = BagPhase::Browsing;
                return BagScreenAction::Active;
            };
            match cursor {
                0 => {
                    // USE — hand back to the caller; stay on the bag afterwards.
                    self.phase = BagPhase::Browsing;
                    return BagScreenAction::UseItem {
                        item,
                        index: self.cursor,
                    };
                }
                1 => {
                    // TOSS — pick a quantity (start at 1).
                    let _ = qty;
                    self.phase = BagPhase::TossQuantity { qty: 1 };
                }
                _ => {
                    self.phase = BagPhase::Browsing;
                }
            }
        }
        BagScreenAction::Active
    }

    fn update_toss_quantity(&mut self, input: BagScreenInput, mut qty: u32) -> BagScreenAction {
        let Some((item, have)) = self.selected_item() else {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Active;
        };
        if input.up && qty < have {
            qty += 1;
        } else if input.down && qty > 1 {
            qty -= 1;
        }
        self.phase = BagPhase::TossQuantity { qty };

        if input.b {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Active;
        }
        if input.a {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::TossItem {
                item,
                index: self.cursor,
                quantity: qty,
            };
        }
        BagScreenAction::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bag() -> Vec<(ItemId, u32)> {
        vec![(ItemId::Potion, 5), (ItemId::PokeFlute, 1), (ItemId::Bicycle, 1)]
    }

    #[test]
    fn browse_down_up_and_cancel_row() {
        let mut s = BagScreenState::new(bag());
        assert_eq!(s.cursor(), 0);
        s.update_frame(BagScreenInput { down: true, ..Default::default() });
        assert_eq!(s.cursor(), 1);
        // step to the trailing CANCEL row (3 items -> row index 3)
        s.update_frame(BagScreenInput { down: true, ..Default::default() });
        s.update_frame(BagScreenInput { down: true, ..Default::default() });
        assert!(s.on_cancel_row());
        // A on CANCEL closes
        assert_eq!(
            s.update_frame(BagScreenInput { a: true, ..Default::default() }),
            BagScreenAction::Cancelled
        );
    }

    #[test]
    fn b_closes_from_browsing() {
        let mut s = BagScreenState::new(bag());
        assert_eq!(
            s.update_frame(BagScreenInput { b: true, ..Default::default() }),
            BagScreenAction::Cancelled
        );
    }

    #[test]
    fn use_returns_item_and_index() {
        let mut s = BagScreenState::new(bag());
        s.update_frame(BagScreenInput { down: true, ..Default::default() }); // PokeFlute
        s.update_frame(BagScreenInput { a: true, ..Default::default() }); // action menu
        let act = s.update_frame(BagScreenInput { a: true, ..Default::default() }); // USE
        assert_eq!(act, BagScreenAction::UseItem { item: ItemId::PokeFlute, index: 1 });
        assert_eq!(s.phase(), BagPhase::Browsing);
    }

    #[test]
    fn toss_quantity_selects_and_returns() {
        let mut s = BagScreenState::new(bag());
        s.update_frame(BagScreenInput { a: true, ..Default::default() }); // action menu (Potion)
        s.update_frame(BagScreenInput { down: true, ..Default::default() }); // -> TOSS
        s.update_frame(BagScreenInput { a: true, ..Default::default() }); // enter quantity (qty=1)
        s.update_frame(BagScreenInput { up: true, ..Default::default() }); // qty=2
        s.update_frame(BagScreenInput { up: true, ..Default::default() }); // qty=3
        let act = s.update_frame(BagScreenInput { a: true, ..Default::default() });
        assert_eq!(
            act,
            BagScreenAction::TossItem { item: ItemId::Potion, index: 0, quantity: 3 }
        );
    }

    #[test]
    fn toss_quantity_caps_at_held() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 2)]);
        s.update_frame(BagScreenInput { a: true, ..Default::default() });
        s.update_frame(BagScreenInput { down: true, ..Default::default() });
        s.update_frame(BagScreenInput { a: true, ..Default::default() }); // qty=1
        for _ in 0..10 {
            s.update_frame(BagScreenInput { up: true, ..Default::default() });
        }
        assert_eq!(s.phase(), BagPhase::TossQuantity { qty: 2 });
    }
}

#[cfg(test)]
mod swap_tests {
    use super::*;

    fn sel() -> BagScreenInput {
        BagScreenInput { select: true, ..BagScreenInput::none() }
    }

    /// SELECT twice swaps two different items (swap_items.asm SwapItemsInMenu).
    #[test]
    fn select_swaps_two_rows() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 3), (ItemId::Antidote, 1), (ItemId::PokeBall, 5)]);
        s.update_frame(sel()); // mark row 0
        s.cursor = 2;
        s.update_frame(sel()); // swap with row 2
        let items = s.items();
        assert_eq!(items[0], (ItemId::PokeBall, 5));
        assert_eq!(items[2], (ItemId::Potion, 3));
        assert_eq!(items[1], (ItemId::Antidote, 1), "the middle row is untouched");
    }

    #[test]
    fn select_merges_same_item_rows() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 3), (ItemId::PokeBall, 5), (ItemId::Potion, 4)]);
        s.update_frame(sel()); // mark row 0
        s.cursor = 2;
        s.update_frame(sel()); // merge 3+4=7 into row 0, row 2 dropped
        let items = s.items();
        assert_eq!(items.len(), 2, "the merged row disappears");
        assert_eq!(items[0], (ItemId::Potion, 7));
    }

    #[test]
    fn select_merge_overflow_caps_second_at_99() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 60), (ItemId::PokeBall, 5), (ItemId::Potion, 80)]);
        s.update_frame(sel());
        s.cursor = 2;
        s.update_frame(sel()); // 60+80=140 > 99: second→99, first keeps 41
        let items = s.items();
        assert_eq!(items[2], (ItemId::Potion, 99));
        assert_eq!(items[0], (ItemId::Potion, 41));
    }

    #[test]
    fn select_cancel_with_b() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 3), (ItemId::Antidote, 1)]);
        s.update_frame(sel()); // mark
        s.update_frame(BagScreenInput { b: true, ..BagScreenInput::none() });
        assert_eq!(s.phase(), BagPhase::Browsing);
        assert_eq!(s.items()[0], (ItemId::Potion, 3), "nothing moved");
    }
}
