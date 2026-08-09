//! Overworld start menu state machine.
//!
//! Replicates `home/start_menu.asm` + `engine/menus/draw_start_menu.asm`:
//! - Opened by pressing START in the overworld
//! - Items: POKéDEX (conditional), POKéMON, ITEM, [player name], SAVE, OPTION, EXIT
//! - POKéDEX only appears after EVENT_GOT_POKEDEX
//! - In link mode, SAVE becomes RESET
//! - Cursor wraps at top/bottom, position is saved between opens
//! - B or START closes the menu
//! - A dispatches to the selected sub-menu

use crate::main_menu::MenuInput;

/// Start menu items matching `draw_start_menu.asm` / `home/start_menu.asm`.
///
/// The dispatch table in `home/start_menu.asm` maps index 0-6:
///   0=Pokedex, 1=Pokemon, 2=Item, 3=TrainerInfo, 4=Save, 5=Option, 6=Exit
/// Without Pokédex, index is incremented by 1 internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMenuItem {
    Pokedex,
    Pokemon,
    Item,
    /// Trainer card — shows player name, money, badges, play time.
    TrainerInfo,
    Save,
    /// Replaces SAVE when connected via link cable.
    Reset,
    Option,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMenuAction {
    Redisplay,
    Close,
    OpenPokedex,
    OpenPokemon,
    OpenItem,
    OpenTrainerInfo,
    OpenSave,
    TriggerReset,
    OpenOption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartMenuInput {
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
}

impl StartMenuInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            a: false,
            b: false,
            start: false,
        }
    }

    pub fn from_menu_input(input: MenuInput, start: bool) -> Self {
        Self {
            up: input.up,
            down: input.down,
            a: input.a,
            b: input.b,
            start,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StartMenuState {
    items: Vec<StartMenuItem>,
    cursor: usize,
    has_pokedex: bool,
    is_link_connected: bool,
    /// `wBattleAndStartSavedMenuItem` — persists cursor position across menu opens.
    saved_cursor: usize,
}

impl StartMenuState {
    pub fn new(has_pokedex: bool, has_pokemon: bool, is_link_connected: bool) -> Self {
        let items = Self::build_items(has_pokedex, has_pokemon, is_link_connected);
        Self {
            items,
            cursor: 0,
            has_pokedex,
            is_link_connected,
            saved_cursor: 0,
        }
    }

    fn build_items(
        has_pokedex: bool,
        has_pokemon: bool,
        is_link_connected: bool,
    ) -> Vec<StartMenuItem> {
        let mut items = Vec::with_capacity(7);
        if has_pokedex {
            items.push(StartMenuItem::Pokedex);
        }
        if has_pokemon {
            items.push(StartMenuItem::Pokemon);
        }
        items.push(StartMenuItem::Item);
        items.push(StartMenuItem::TrainerInfo);
        if is_link_connected {
            items.push(StartMenuItem::Reset);
        } else {
            items.push(StartMenuItem::Save);
        }
        items.push(StartMenuItem::Option);
        items.push(StartMenuItem::Exit);
        items
    }

    pub fn open(&mut self, has_pokedex: bool, has_pokemon: bool, is_link_connected: bool) {
        self.has_pokedex = has_pokedex;
        self.is_link_connected = is_link_connected;
        self.items = Self::build_items(has_pokedex, has_pokemon, is_link_connected);
        self.cursor = self.saved_cursor.min(self.items.len().saturating_sub(1));
    }

    pub fn update_frame(&mut self, input: StartMenuInput) -> StartMenuAction {
        if input.b || input.start {
            self.save_cursor();
            return StartMenuAction::Close;
        }

        if input.up {
            self.cursor_up();
        } else if input.down {
            self.cursor_down();
        }

        if input.a {
            self.save_cursor();
            return self.select_current_item();
        }

        StartMenuAction::Redisplay
    }

    fn cursor_up(&mut self) {
        if self.cursor == 0 {
            self.cursor = self.items.len() - 1;
        } else {
            self.cursor -= 1;
        }
    }

    fn cursor_down(&mut self) {
        self.cursor += 1;
        if self.cursor >= self.items.len() {
            self.cursor = 0;
        }
    }

    fn save_cursor(&mut self) {
        self.saved_cursor = self.cursor;
    }

    fn select_current_item(&self) -> StartMenuAction {
        match self.items[self.cursor] {
            StartMenuItem::Pokedex => StartMenuAction::OpenPokedex,
            StartMenuItem::Pokemon => StartMenuAction::OpenPokemon,
            StartMenuItem::Item => StartMenuAction::OpenItem,
            StartMenuItem::TrainerInfo => StartMenuAction::OpenTrainerInfo,
            StartMenuItem::Save => StartMenuAction::OpenSave,
            StartMenuItem::Reset => StartMenuAction::TriggerReset,
            StartMenuItem::Option => StartMenuAction::OpenOption,
            StartMenuItem::Exit => StartMenuAction::Close,
        }
    }

    pub fn redisplay(&mut self) {
        self.cursor = self.saved_cursor.min(self.items.len().saturating_sub(1));
    }

    pub fn items(&self) -> &[StartMenuItem] {
        &self.items
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn current_item(&self) -> StartMenuItem {
        self.items[self.cursor]
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn has_pokedex(&self) -> bool {
        self.has_pokedex
    }

    pub fn is_link_connected(&self) -> bool {
        self.is_link_connected
    }

    pub fn saved_cursor(&self) -> usize {
        self.saved_cursor
    }

    pub fn item_labels<'a>(&self, player_name: &'a str) -> Vec<ItemLabel<'a>> {
        self.items
            .iter()
            .map(|item| match item {
                StartMenuItem::Pokedex => ItemLabel::Static("POKéDEX"),
                StartMenuItem::Pokemon => ItemLabel::Static("POKéMON"),
                StartMenuItem::Item => ItemLabel::Static("ITEM"),
                StartMenuItem::TrainerInfo => ItemLabel::PlayerName(player_name),
                StartMenuItem::Save => ItemLabel::Static("SAVE"),
                StartMenuItem::Reset => ItemLabel::Static("RESET"),
                StartMenuItem::Option => ItemLabel::Static("OPTION"),
                StartMenuItem::Exit => ItemLabel::Static("EXIT"),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemLabel<'a> {
    Static(&'static str),
    PlayerName(&'a str),
}

impl<'a> ItemLabel<'a> {
    pub fn as_str(&self) -> &str {
        match self {
            ItemLabel::Static(s) => s,
            ItemLabel::PlayerName(s) => s,
        }
    }
}
