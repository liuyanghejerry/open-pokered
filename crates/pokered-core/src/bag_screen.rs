//! Overworld ITEM bag screen state machine.
//!
//! Reachable from the Start menu → ITEM. Lists the bag, and on a selected item
//! offers USE / TOSS / CANCEL (matching the original overworld item menu). USE
//! hands the item id back to the caller to dispatch a field effect; TOSS asks a
//! quantity, prints a blocking question, confirms YES/NO, then removes that many
//! after the original protected wait and prints the result. Pure logic — mirrors
//! `party_screen::PartyScreenState`.

use crate::alloc_prelude::*;
use crate::game_state::Lang;
use crate::overworld::BedroomDialogue;
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
    TossQuestion { item: ItemId, qty: u32 },
    TossConfirm { item: ItemId, qty: u32, cursor: u8 },
    TossWait { item: ItemId, qty: u32, cursor: u8, remaining: u8, accepted: bool },
    TossResult { item: ItemId },
    TossRejected,
    /// An item which returns to ItemMenuLoop; removal waits for its PROMPT.
    FieldMessage { item: ItemId, consumed: bool },
    /// SELECT-swap mode (swap_items.asm): the marked row waits for a second
    /// SELECT on another row to swap/merge. B exits the list.
    SwapFrom { row: usize },
    /// ItemUseTMHM's first blocking text: "Booted up a TM/HM!".
    MachineBoot { item: ItemId },
    /// "It contained MOVE! Teach MOVE to a POKéMON?" plus YES/NO.
    MachineTeach { item: ItemId, cursor: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BagScreenAction {
    /// Still open.
    Active,
    /// A protected SELECT-swap completed; commit the physical item slots.
    ItemsReordered,
    /// Player backed out of the whole bag (return to the start menu / overworld).
    Cancelled,
    /// USE the item at `index`. The caller dispatches the field effect and is
    /// responsible for any consumption (then rebuilds the bag via `set_items`).
    UseItem { item: ItemId, index: usize },
    /// Final field-message acknowledgement, before rebuilding the list.
    FieldMessageClosed { item: ItemId, consumed: bool },
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
    toss_dialogue: Option<BedroomDialogue>,
    toss_frames: u32,
    press_sound: bool,
    swap_wait_frames: u8,
    pending_swap_target: Option<usize>,
}

impl BagScreenState {
    pub fn new(items: Vec<(ItemId, u32)>) -> Self {
        Self {
            items,
            cursor: 0,
            scroll: 0,
            phase: BagPhase::Browsing,
            visible_rows: 4,
            toss_dialogue: None,
            toss_frames: 0,
            press_sound: false,
            swap_wait_frames: 0,
            pending_swap_target: None,
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
        self.toss_dialogue = None;
        self.swap_wait_frames = 0;
        self.pending_swap_target = None;
    }

    /// The inventory is committed after YES's protected wait. Keep its result
    /// PROMPT until the player acknowledges it rather than rebuilding the menu.
    pub fn set_items_after_toss(&mut self, items: Vec<(ItemId, u32)>) {
        self.items = items;
        self.cursor = self.cursor.min(self.row_count().saturating_sub(1));
        self.clamp_scroll();
    }

    pub fn show_field_item_message(&mut self, item: ItemId, consumed: bool, dialogue: BedroomDialogue) {
        self.toss_dialogue = Some(dialogue);
        self.toss_frames = 0;
        self.phase = BagPhase::FieldMessage { item, consumed };
    }

    pub fn toss_dialogue(&self) -> Option<&BedroomDialogue> { self.toss_dialogue.as_ref() }

    pub fn toss_arrow_visible(&self) -> bool {
        matches!(self.phase, BagPhase::TossQuestion { .. } | BagPhase::TossResult { .. } | BagPhase::TossRejected | BagPhase::FieldMessage { .. })
            && self.toss_dialogue.as_ref().is_some_and(|d| d.waiting_for_input())
            && (self.toss_frames / 16) % 2 == 0
    }

    pub fn take_press_sound(&mut self) -> bool { core::mem::take(&mut self.press_sound) }

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
        self.update_frame_with_text(input, input.a || input.b, Lang::En, 3)
    }

    pub fn update_frame_with_text(&mut self, input: BagScreenInput, fast_held: bool, lang: Lang, delay: u16) -> BagScreenAction {
        self.press_sound = false;
        self.toss_frames = self.toss_frames.wrapping_add(1);
        // HandleItemListSwapping waits20 frames after marking and before swapping.
        // All input, including cancel, is ignored during either protected wait.
        if self.swap_wait_frames > 0 {
            self.swap_wait_frames -= 1;
            if self.swap_wait_frames == 0 {
                if let Some(target) = self.pending_swap_target.take() {
                    if let BagPhase::SwapFrom { row } = self.phase {
                        return self.complete_swap(row, target);
                    }
                }
            }
            return BagScreenAction::Active;
        }
        match self.phase {
            BagPhase::Browsing => self.update_browsing(input),
            BagPhase::ActionMenu { cursor } => self.update_action_menu(input, cursor, lang, delay),
            BagPhase::TossQuantity { qty } => self.update_toss_quantity(input, qty, lang, delay),
            BagPhase::TossQuestion { .. } | BagPhase::TossResult { .. } | BagPhase::TossRejected | BagPhase::FieldMessage { .. } => self.update_toss_text(input, fast_held, delay),
            BagPhase::TossConfirm { item, qty, cursor } => self.update_toss_confirm(input, item, qty, cursor),
            BagPhase::TossWait { item, qty, cursor, remaining, accepted } => {
                if remaining > 1 {
                    self.phase = BagPhase::TossWait { item, qty, cursor, remaining: remaining - 1, accepted };
                    BagScreenAction::Active
                } else if accepted {
                    let name = pokered_data::lang_data::item_name(item, lang == Lang::Zh);
                    let text = if lang == Lang::Zh { format!("扔掉了\n{}。", name) } else { format!("Threw away\n{}.", name) };
                    self.start_toss_text(BagPhase::TossResult { item }, &text, delay);
                    BagScreenAction::TossItem { item, index: self.cursor, quantity: qty }
                } else {
                    self.finish_toss_text();
                    BagScreenAction::Active
                }
            }
            BagPhase::SwapFrom { row } => self.update_swap(input, row),
            BagPhase::MachineBoot { item } => self.update_machine_boot(input, item),
            BagPhase::MachineTeach { item, cursor } => {
                self.update_machine_teach(input, item, cursor)
            }
        }
    }

    fn update_browsing(&mut self, input: BagScreenInput) -> BagScreenAction {
        let rows = self.row_count();
        if input.up {
            self.cursor = self.cursor.saturating_sub(1);
            self.clamp_scroll();
        } else if input.down && self.cursor < rows - 1 {
            self.cursor += 1;
            self.clamp_scroll();
        }

        // DisplayListMenuID gives A priority over B/SELECT after moving.
        if input.a {
            if self.on_cancel_row() {
                return BagScreenAction::Cancelled;
            }
            if self.selected_item().is_some_and(|(item, _)| item == ItemId::Bicycle) {
                // StartMenu_Item.choseItem skips USE/TOSS for the BICYCLE.
                self.press_sound = true;
                return BagScreenAction::UseItem { item: ItemId::Bicycle, index: self.cursor };
            }
            self.phase = BagPhase::ActionMenu { cursor: 0 };
            return BagScreenAction::Active;
        }
        if input.b { return BagScreenAction::Cancelled; }
        // SELECT marks the first swap row (SwapItemsInMenu, swap_items.asm) —
        // only item rows, never CANCEL.
        if input.select && !self.on_cancel_row() && !self.items.is_empty() {
            self.phase = BagPhase::SwapFrom { row: self.cursor };
            self.swap_wait_frames = 20;
        }
        BagScreenAction::Active
    }

    /// SELECT-swap completion (swap_items.asm): the second SELECT on another
    /// row either SWAPS the two entries, or — for same-kind entries whose
    /// combined count fits one slot (≤99) — MERGES them into the second and
    /// drops the first; a merge that would overflow leaves the second filled
    /// to 99 with the remainder staying in the first. B exits the list; A selects normally.
    fn update_swap(&mut self, input: BagScreenInput, row: usize) -> BagScreenAction {
        if input.up {
            self.cursor = self.cursor.saturating_sub(1);
            self.clamp_scroll();
        } else if input.down && self.cursor < self.row_count() - 1 {
            self.cursor += 1;
            self.clamp_scroll();
        }
        if input.a {
            self.phase = BagPhase::Browsing;
            if self.on_cancel_row() { return BagScreenAction::Cancelled; }
            self.phase = BagPhase::ActionMenu { cursor: 0 };
            return BagScreenAction::Active;
        }
        if input.b {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Cancelled;
        }
        if input.select {
            let target = self.cursor;
            // SELECT on the same item or CANCEL keeps the original mark.
            if target == row || target >= self.items.len() || row >= self.items.len() {
                return BagScreenAction::Active;
            }
            self.pending_swap_target = Some(target);
            self.swap_wait_frames = 20;
        }
        BagScreenAction::Active
    }

    fn complete_swap(&mut self, row: usize, target: usize) -> BagScreenAction {
        if row >= self.items.len() || target >= self.items.len() || row == target {
            return BagScreenAction::Active;
        }
        let (a_item, a_qty) = self.items[row];
        let (b_item, b_qty) = self.items[target];
        if a_item == b_item {
            let total = a_qty + b_qty;
            if total <= 99 {
                // Original combines into the SECOND slot, erases the FIRST,
                // then resets the current row and scroll offset to zero.
                self.items[target] = (b_item, total);
                self.items.remove(row);
                self.cursor = 0;
                self.scroll = 0;
            } else {
                self.items[target] = (b_item, 99);
                self.items[row] = (a_item, total - 99);
            }
        } else {
            self.items.swap(row, target);
        }
        self.cursor = self.cursor.min(self.row_count().saturating_sub(1));
        self.clamp_scroll();
        self.phase = BagPhase::Browsing;
        BagScreenAction::ItemsReordered
    }

    fn update_action_menu(&mut self, input: BagScreenInput, mut cursor: u8, lang: Lang, delay: u16) -> BagScreenAction {
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
                    // ItemUseTMHM prints two blocking prompts before opening
                    // the party menu. Other items dispatch immediately.
                    if crate::items::bag_use::machine_of(item).is_some() {
                        self.phase = BagPhase::MachineBoot { item };
                        return BagScreenAction::Active;
                    }
                    self.phase = BagPhase::Browsing;
                    return BagScreenAction::UseItem {
                        item,
                        index: self.cursor,
                    };
                }
                1 => {
                    // IsKeyItem/IsItemHM precede DisplayChooseQuantityMenu.
                    let _ = qty;
                    if crate::items::inventory::is_tossable(item) {
                        self.phase = BagPhase::TossQuantity { qty: 1 };
                    } else {
                        let text = if lang == Lang::Zh { "这东西太重要了，\n不能扔掉！" } else { "That's too impor-\ntant to toss!" };
                        self.start_toss_text(BagPhase::TossRejected, text, delay);
                    }
                }
                _ => {
                    self.phase = BagPhase::Browsing;
                }
            }
        }
        BagScreenAction::Active
    }

    fn update_machine_boot(&mut self, input: BagScreenInput, item: ItemId) -> BagScreenAction {
        if input.a || input.b {
            // YES is selected initially, matching DisplayTwoOptionMenu.
            self.phase = BagPhase::MachineTeach { item, cursor: 0 };
        }
        BagScreenAction::Active
    }

    fn update_machine_teach(
        &mut self,
        input: BagScreenInput,
        item: ItemId,
        mut cursor: u8,
    ) -> BagScreenAction {
        if input.up || input.down {
            cursor ^= 1;
        }
        self.phase = BagPhase::MachineTeach { item, cursor };
        if input.b || (input.a && cursor == 1) {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Active;
        }
        if input.a {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::UseItem {
                item,
                index: self.cursor,
            };
        }
        BagScreenAction::Active
    }

    fn update_toss_quantity(&mut self, input: BagScreenInput, mut qty: u32, lang: Lang, delay: u16) -> BagScreenAction {
        let Some((item, have)) = self.selected_item() else {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Active;
        };
        // Shared original quantity menu wraps and gives A priority over
        // B/directions, so a chord cannot alter the confirmed amount.
        if !input.a && !input.b {
            if input.up {
                qty = if qty >= have { 1 } else { qty + 1 };
            } else if input.down {
                qty = if qty <= 1 { have } else { qty - 1 };
            }
        }
        self.phase = BagPhase::TossQuantity { qty };

        if input.b && !input.a {
            self.phase = BagPhase::Browsing;
            return BagScreenAction::Active;
        }
        if input.a {
            let name = pokered_data::lang_data::item_name(item, lang == Lang::Zh);
            let text = if lang == Lang::Zh { format!("确定要扔掉\n{}吗？", name) } else { format!("Is it OK to toss\n{}?", name) };
            self.start_toss_text(BagPhase::TossQuestion { item, qty }, &text, delay);
        }
        BagScreenAction::Active
    }

    fn start_toss_text(&mut self, phase: BagPhase, text: &str, delay: u16) {
        let mut dialogue = BedroomDialogue::from_message(text);
        dialogue.set_text_delay_frames(delay);
        self.toss_dialogue = Some(dialogue);
        self.toss_frames = 0;
        self.phase = phase;
    }

    fn finish_toss_text(&mut self) {
        self.phase = BagPhase::Browsing;
        self.toss_dialogue = None;
    }

    fn update_toss_text(&mut self, input: BagScreenInput, fast_held: bool, delay: u16) -> BagScreenAction {
        let Some(dialogue) = &mut self.toss_dialogue else { self.finish_toss_text(); return BagScreenAction::Active; };
        dialogue.set_text_delay_frames(delay);
        if !dialogue.waiting_for_input() {
            dialogue.reveal_next_char_with_buttons(fast_held);
        } else if input.a || input.b {
            self.press_sound = true;
            if !dialogue.is_last_page() {
                dialogue.advance();
            } else if let BagPhase::TossQuestion { item, qty } = self.phase {
                // _IsItOKToTossItemText ends in PROMPT, unlike DONE questions.
                self.phase = BagPhase::TossConfirm { item, qty, cursor: 0 };
            } else if let BagPhase::FieldMessage { item, consumed } = self.phase {
                self.finish_toss_text();
                return BagScreenAction::FieldMessageClosed { item, consumed };
            } else {
                self.finish_toss_text();
            }
        }
        BagScreenAction::Active
    }

    fn update_toss_confirm(&mut self, input: BagScreenInput, item: ItemId, qty: u32, mut cursor: u8) -> BagScreenAction {
        if input.up { cursor = 0; } else if input.down { cursor = 1; }
        self.phase = BagPhase::TossConfirm { item, qty, cursor };
        if input.a || input.b {
            // DisplayTwoOptionMenu checks B first, even in an A+B chord.
            let accepted = !input.b && cursor == 0;
            self.press_sound = true;
            self.phase = BagPhase::TossWait { item, qty, cursor: if input.b { 1 } else { cursor }, remaining: 15, accepted };
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
    fn tm_hm_use_runs_boot_and_teach_confirmation_before_party() {
        for item in [ItemId::Tm01, ItemId::Hm01] {
            let mut s = BagScreenState::new(vec![(item, 1)]);
            s.update_frame(BagScreenInput { a: true, ..Default::default() });
            assert_eq!(
                s.update_frame(BagScreenInput { a: true, ..Default::default() }),
                BagScreenAction::Active
            );
            assert_eq!(s.phase(), BagPhase::MachineBoot { item });
            s.update_frame(BagScreenInput { a: true, ..Default::default() });
            assert_eq!(s.phase(), BagPhase::MachineTeach { item, cursor: 0 });
            assert_eq!(
                s.update_frame(BagScreenInput { a: true, ..Default::default() }),
                BagScreenAction::UseItem { item, index: 0 }
            );
        }
    }

    #[test]
    fn tm_hm_teach_prompt_b_selects_no_without_using_item() {
        let item = ItemId::Tm01;
        let mut s = BagScreenState::new(vec![(item, 1)]);
        s.phase = BagPhase::MachineTeach { item, cursor: 0 };
        assert_eq!(
            s.update_frame(BagScreenInput { b: true, ..Default::default() }),
            BagScreenAction::Active
        );
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
        assert_eq!(act, BagScreenAction::Active);
        assert_eq!(s.phase(), BagPhase::TossQuestion { item: ItemId::Potion, qty: 3 });
        finish_printing(&mut s);
        s.update_frame(BagScreenInput { a: true, ..Default::default() });
        s.update_frame(BagScreenInput { a: true, ..Default::default() });
        for _ in 0..14 { assert_eq!(s.update_frame(BagScreenInput::none()), BagScreenAction::Active); }
        assert_eq!(s.update_frame(BagScreenInput::none()), BagScreenAction::TossItem { item: ItemId::Potion, index: 0, quantity: 3 });
    }

    #[test]
    fn source_quantity_bag_wraps_both_boundaries() {
        for have in [1, 4, 99] {
            let mut s = BagScreenState::new(vec![(ItemId::Potion, have)]);
            s.phase = BagPhase::TossQuantity { qty: 1 };
            s.update_frame(BagScreenInput { down: true, ..Default::default() });
            assert_eq!(s.phase(), BagPhase::TossQuantity { qty: have });
            s.update_frame(BagScreenInput { up: true, ..Default::default() });
            assert_eq!(s.phase(), BagPhase::TossQuantity { qty: 1 });
            assert_eq!(s.selected_item(), Some((ItemId::Potion, have)));
        }
    }

    #[test]
    fn source_quantity_bag_confirm_precedes_cancel_and_directions() {
        for b in [false, true] {
            let mut s = BagScreenState::new(vec![(ItemId::Potion, 4)]);
            s.phase = BagPhase::TossQuantity { qty: 1 };
            let result = s.update_frame(BagScreenInput {
                a: true, b, up: true, down: true, ..Default::default()
            });
            assert_eq!(result, BagScreenAction::Active);
            assert_eq!(s.phase(), BagPhase::TossQuestion { item: ItemId::Potion, qty: 1 });
            assert_eq!(s.selected_item(), Some((ItemId::Potion, 4)));
        }
    }

    fn finish_printing(s: &mut BagScreenState) {
        for _ in 0..400 {
            if s.toss_dialogue().is_some_and(|d| d.waiting_for_input()) { return; }
            assert_eq!(s.update_frame(BagScreenInput::none()), BagScreenAction::Active);
        }
        panic!("PROMPT did not finish printing");
    }

    #[test]
    fn toss_prompt_requires_ack_then_yes_waits_fifteen_frames_and_result_requires_ack() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 4)]);
        s.phase = BagPhase::TossQuantity { qty: 2 };
        s.update_frame(BagScreenInput { a: true, ..Default::default() });
        finish_printing(&mut s);
        for _ in 0..20 { s.update_frame(BagScreenInput::none()); }
        assert!(matches!(s.phase(), BagPhase::TossQuestion { .. }), "PROMPT is not DONE");
        s.update_frame(BagScreenInput { a: true, ..Default::default() });
        assert_eq!(s.phase(), BagPhase::TossConfirm { item: ItemId::Potion, qty: 2, cursor: 0 });
        assert!(s.take_press_sound());
        s.update_frame(BagScreenInput { a: true, ..Default::default() });
        assert!(s.take_press_sound());
        for _ in 0..14 {
            // Even fresh input is ignored during DisplayTwoOptionMenu's DelayFrames.
            assert_eq!(s.update_frame(BagScreenInput { a: true, b: true, down: true, ..Default::default() }), BagScreenAction::Active);
        }
        assert_eq!(s.update_frame(BagScreenInput::none()), BagScreenAction::TossItem { item: ItemId::Potion, index: 0, quantity: 2 });
        s.set_items_after_toss(vec![(ItemId::Potion, 2)]);
        finish_printing(&mut s);
        assert_eq!(s.phase(), BagPhase::TossResult { item: ItemId::Potion });
        let (top, bottom) = s.toss_dialogue().unwrap().get_display_text().unwrap();
        assert_eq!((top.as_str(), bottom.as_str()), ("Threw away", "POTION."));
        s.update_frame(BagScreenInput { b: true, ..Default::default() });
        assert_eq!(s.phase(), BagPhase::Browsing);
        assert_eq!(s.selected_item(), Some((ItemId::Potion, 2)));
    }

    #[test]
    fn toss_no_b_and_ab_never_request_inventory_removal() {
        for case in 0..3 {
            let mut s = BagScreenState::new(vec![(ItemId::Potion, 4)]);
            s.phase = BagPhase::TossQuantity { qty: 2 };
            s.update_frame(BagScreenInput { a: true, ..Default::default() });
            finish_printing(&mut s);
            s.update_frame(BagScreenInput { a: true, ..Default::default() });
            if case == 0 { s.update_frame(BagScreenInput { down: true, ..Default::default() }); }
            assert_eq!(s.update_frame(BagScreenInput { a: case != 1, b: case != 0, ..Default::default() }), BagScreenAction::Active);
            for _ in 0..15 { assert_eq!(s.update_frame(BagScreenInput::none()), BagScreenAction::Active); }
            assert_eq!(s.phase(), BagPhase::Browsing);
            assert_eq!(s.selected_item(), Some((ItemId::Potion, 4)));
        }
    }

    #[test]
    fn important_items_skip_quantity_and_refusal_returns_to_bag() {
        for item in [ItemId::PokeFlute, ItemId::Hm01] {
            let mut s = BagScreenState::new(vec![(item, 1)]);
            s.update_frame(BagScreenInput { a: true, ..Default::default() });
            s.update_frame(BagScreenInput { down: true, ..Default::default() });
            s.update_frame(BagScreenInput { a: true, ..Default::default() });
            assert_eq!(s.phase(), BagPhase::TossRejected);
            finish_printing(&mut s);
            assert_eq!(s.update_frame(BagScreenInput { a: true, ..Default::default() }), BagScreenAction::Active);
            assert_eq!(s.phase(), BagPhase::Browsing);
            assert_eq!(s.selected_item(), Some((item, 1)));
        }
    }

    #[test]
    fn toss_quantity_wraps_held_boundary_on_repeated_presses() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 2)]);
        s.update_frame(BagScreenInput { a: true, ..Default::default() });
        s.update_frame(BagScreenInput { down: true, ..Default::default() });
        s.update_frame(BagScreenInput { a: true, ..Default::default() }); // qty=1
        for _ in 0..10 {
            s.update_frame(BagScreenInput { up: true, ..Default::default() });
        }
        assert_eq!(s.phase(), BagPhase::TossQuantity { qty: 1 });
    }
}

#[cfg(test)]
mod swap_tests {
    use super::*;

    fn finish_wait(s: &mut BagScreenState) {
        for _ in 0..20 { s.update_frame(BagScreenInput::none()); }
    }

    fn sel() -> BagScreenInput {
        BagScreenInput { select: true, ..BagScreenInput::none() }
    }

    /// SELECT twice swaps two different items (swap_items.asm SwapItemsInMenu).
    #[test]
    fn select_swaps_two_rows() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 3), (ItemId::Antidote, 1), (ItemId::PokeBall, 5)]);
        s.update_frame(sel()); finish_wait(&mut s); // mark row 0
        s.cursor = 2;
        s.update_frame(sel()); finish_wait(&mut s); // swap with row 2
        let items = s.items();
        assert_eq!(items[0], (ItemId::PokeBall, 5));
        assert_eq!(items[2], (ItemId::Potion, 3));
        assert_eq!(items[1], (ItemId::Antidote, 1), "the middle row is untouched");
    }

    #[test]
    fn select_merges_same_item_rows() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 3), (ItemId::PokeBall, 5), (ItemId::Potion, 4)]);
        s.update_frame(sel()); finish_wait(&mut s); // mark row 0
        s.cursor = 2;
        s.update_frame(sel()); finish_wait(&mut s); // second slot receives7; first is deleted; row/scroll reset0
        let items = s.items();
        assert_eq!(items.len(), 2, "the merged row disappears");
        assert_eq!(items, &[(ItemId::PokeBall, 5), (ItemId::Potion, 7)]);
        assert_eq!((s.cursor(), s.scroll()), (0, 0));
    }

    #[test]
    fn select_merge_overflow_caps_second_at_99() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 60), (ItemId::PokeBall, 5), (ItemId::Potion, 80)]);
        s.update_frame(sel()); finish_wait(&mut s);
        s.cursor = 2;
        s.update_frame(sel()); finish_wait(&mut s); // 60+80=140 > 99: second→99, first keeps 41
        let items = s.items();
        assert_eq!(items[2], (ItemId::Potion, 99));
        assert_eq!(items[0], (ItemId::Potion, 41));
    }

    #[test]
    fn select_cancel_with_b() {
        let mut s = BagScreenState::new(vec![(ItemId::Potion, 3), (ItemId::Antidote, 1)]);
        s.update_frame(sel()); finish_wait(&mut s); // mark
        assert_eq!(s.update_frame(BagScreenInput { b: true, ..BagScreenInput::none() }), BagScreenAction::Cancelled);
        assert_eq!(s.phase(), BagPhase::Browsing);
        assert_eq!(s.items()[0], (ItemId::Potion, 3), "nothing moved");
    }
}

#[cfg(test)]
mod protected_swap_wait_221 {
    use super::*;
    #[test]
    fn mark_and_swap_ignore_input_and_commit_once_after_twenty_frames() {
        let original = vec![(ItemId::Potion, 3), (ItemId::Antidote, 4)];
        let mut s = BagScreenState::new(original.clone());
        s.update_frame(BagScreenInput { select: true, ..Default::default() });
        let interference = BagScreenInput { up: true, down: true, a: true, b: true, select: true, ..Default::default() };
        for _ in 0..20 {
            assert_eq!(s.update_frame(interference), BagScreenAction::Active);
            assert_eq!(s.cursor(), 0);
            assert_eq!(s.phase(), BagPhase::SwapFrom { row: 0 });
        }
        s.update_frame(BagScreenInput { down: true, ..Default::default() });
        s.update_frame(BagScreenInput { select: true, ..Default::default() });
        for _ in 0..19 {
            assert_eq!(s.update_frame(interference), BagScreenAction::Active);
            assert_eq!(s.items(), original);
        }
        assert_eq!(s.update_frame(interference), BagScreenAction::ItemsReordered);
        assert_eq!(s.items(), &[(ItemId::Antidote, 4), (ItemId::Potion, 3)]);
        assert_eq!(s.update_frame(BagScreenInput::none()), BagScreenAction::Active);
    }
}
