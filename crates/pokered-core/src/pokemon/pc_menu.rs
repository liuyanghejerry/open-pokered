use crate::main_menu::MenuInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcMainMenuTarget {
    BillsPc,
    PlayersPc,
    OaksPc,
    PkmnLeague,
    LogOff,
}

#[derive(Debug, Clone)]
pub struct PcMainMenuState {
    items: Vec<PcMainMenuTarget>,
    cursor: usize,
    met_bill: bool,
}

impl PcMainMenuState {
    pub fn new(has_pokedex: bool, has_beaten_league: bool, met_bill: bool) -> Self {
        let mut items = Vec::new();
        items.push(PcMainMenuTarget::BillsPc);
        items.push(PcMainMenuTarget::PlayersPc);
        if has_pokedex {
            items.push(PcMainMenuTarget::OaksPc);
        }
        if has_beaten_league {
            items.push(PcMainMenuTarget::PkmnLeague);
        }
        items.push(PcMainMenuTarget::LogOff);

        Self {
            items,
            cursor: 0,
            met_bill,
        }
    }

    pub fn update_frame(&mut self, input: MenuInput) -> Option<PcMainMenuTarget> {
        if input.b {
            return Some(PcMainMenuTarget::LogOff);
        }
        if input.up {
            self.cursor_up();
        } else if input.down {
            self.cursor_down();
        }
        if input.a {
            return Some(self.items[self.cursor]);
        }
        None
    }

    fn cursor_up(&mut self) {
        if self.cursor == 0 {
            self.cursor = self.items.len().saturating_sub(1);
        } else {
            self.cursor -= 1;
        }
    }

    fn cursor_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.cursor += 1;
        if self.cursor >= self.items.len() {
            self.cursor = 0;
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn items(&self) -> &[PcMainMenuTarget] {
        &self.items
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn met_bill(&self) -> bool {
        self.met_bill
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillsPcAction {
    Withdraw,
    Deposit,
    Release,
    ChangeBox,
    Exit,
}

#[derive(Debug, Clone)]
pub struct BillsPcMenuState {
    cursor: usize,
    saved_cursor: usize,
    current_box: usize,
}

impl BillsPcMenuState {
    const ITEMS: [BillsPcAction; 5] = [
        BillsPcAction::Withdraw,
        BillsPcAction::Deposit,
        BillsPcAction::Release,
        BillsPcAction::ChangeBox,
        BillsPcAction::Exit,
    ];

    pub fn new(current_box: usize) -> Self {
        Self {
            cursor: 0,
            saved_cursor: 0,
            current_box,
        }
    }

    pub fn update_frame(&mut self, input: MenuInput) -> Option<BillsPcAction> {
        if input.b {
            return Some(BillsPcAction::Exit);
        }
        if input.up {
            self.cursor_up();
        } else if input.down {
            self.cursor_down();
        }
        if input.a {
            self.saved_cursor = self.cursor;
            return Some(Self::ITEMS[self.cursor]);
        }
        None
    }

    pub fn restore_saved_cursor(&mut self) {
        self.cursor = self.saved_cursor;
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

    pub fn current_box(&self) -> usize {
        self.current_box
    }

    pub fn set_current_box(&mut self, box_num: usize) {
        self.current_box = box_num;
    }

    pub fn item_count(&self) -> usize {
        Self::ITEMS.len()
    }

    pub fn current_action(&self) -> BillsPcAction {
        Self::ITEMS[self.cursor]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayersPcAction {
    WithdrawItem,
    DepositItem,
    TossItem,
    LogOff,
}

#[derive(Debug, Clone)]
pub struct PlayersPcMenuState {
    cursor: usize,
    saved_cursor: usize,
}

impl PlayersPcMenuState {
    const ITEMS: [PlayersPcAction; 4] = [
        PlayersPcAction::WithdrawItem,
        PlayersPcAction::DepositItem,
        PlayersPcAction::TossItem,
        PlayersPcAction::LogOff,
    ];

    pub fn new() -> Self {
        Self {
            cursor: 0,
            saved_cursor: 0,
        }
    }

    pub fn update_frame(&mut self, input: MenuInput) -> Option<PlayersPcAction> {
        if input.b {
            return Some(PlayersPcAction::LogOff);
        }
        if input.up {
            self.cursor_up();
        } else if input.down {
            self.cursor_down();
        }
        if input.a {
            self.saved_cursor = self.cursor;
            return Some(Self::ITEMS[self.cursor]);
        }
        None
    }

    pub fn restore_saved_cursor(&mut self) {
        self.cursor = self.saved_cursor;
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

    pub fn item_count(&self) -> usize {
        Self::ITEMS.len()
    }

    pub fn current_action(&self) -> PlayersPcAction {
        Self::ITEMS[self.cursor]
    }
}

impl Default for PlayersPcMenuState {
    fn default() -> Self {
        Self::new()
    }
}
