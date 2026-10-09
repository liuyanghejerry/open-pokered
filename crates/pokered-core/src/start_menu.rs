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

use crate::alloc_prelude::*;
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
    /// PrintSafariZoneSteps (player_state.asm:219-255, called from
    /// home/start_menu.asm:12): inside the Safari Zone the START menu opens
    /// with the "NNN/500" steps + "BALL×× NN" info box in the top-left.
    /// None = not in the Safari Zone.
    pub safari_info: Option<SafariZoneInfo>,
    field_initialization: Option<(u8, StartMenuInput)>,
}

/// The Safari Zone START-menu info box contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafariZoneInfo {
    /// Remaining steps (of 500).
    pub steps: u16,
    /// Remaining Safari Balls (of 30).
    pub balls: u8,
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
            safari_info: None,
            field_initialization: None,
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
        self.field_initialization = None;
        self.has_pokedex = has_pokedex;
        self.is_link_connected = is_link_connected;
        self.items = Self::build_items(has_pokedex, has_pokemon, is_link_connected);
        self.cursor = self.saved_cursor.min(self.items.len().saturating_sub(1));
    }

    /// DisplayTextIDInit's transfers precede DrawStartMenu and its first
    /// Joypad poll. Only field entry owns this wait; submenu returns do not.
    pub fn begin_field_initialization(&mut self, previous: StartMenuInput) {
        self.field_initialization = Some((23, previous));
    }

    pub fn field_initialization_active(&self) -> bool {
        self.field_initialization.is_some()
    }

    /// START sound occurs at DrawStartMenu, three frames before its Joypad.
    pub fn field_initialization_sound_due(&self) -> bool {
        self.field_initialization.is_some_and(|(remaining, _)| remaining == 4)
    }

    /// Ignored pulses are discarded. A button held across initialization
    /// is compared with the preceding FIELD Joypad sample, not UI frames.
    pub fn sample_field_initialization(&mut self, held: StartMenuInput) -> Option<StartMenuInput> {
        let (remaining, previous) = self.field_initialization?;
        if remaining > 1 {
            self.field_initialization = Some((remaining - 1, previous));
            return None;
        }
        self.field_initialization = None;
        Some(StartMenuInput {
            up: held.up && !previous.up,
            down: held.down && !previous.down,
            a: held.a && !previous.a,
            b: held.b && !previous.b,
            start: held.start && !previous.start,
        })
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

#[cfg(test)]
mod safari_info_tests {
    use super::*;

    /// The info box contents mirror wSafariSteps / wNumSafariBalls at menu
    /// open (PrintSafariZoneSteps, player_state.asm:219-255).
    #[test]
    fn safari_info_defaults_to_none() {
        let m = StartMenuState::new(true, true, false);
        assert!(m.safari_info.is_none(), "no Safari run → no info box");
    }

    #[test]
    fn safari_info_carries_steps_and_balls() {
        let mut m = StartMenuState::new(true, true, false);
        m.safari_info = Some(SafariZoneInfo { steps: 427, balls: 17 });
        let info = m.safari_info.unwrap();
        assert_eq!(info.steps, 427);
        assert_eq!(info.balls, 17);
    }
}

#[cfg(test)]
mod field_initialization_tests {
    use super::*;
    #[test]
    fn initialization_discards_short_pulses_but_reads_new_held_keys_at_first_joypad() {
        let opening=StartMenuInput {start:true,..StartMenuInput::none()};
        for held_down in [false,true] {
            let mut menu=StartMenuState::new(false,true,false);
            menu.begin_field_initialization(opening);
            for frame in 1..=22 {
                assert_eq!(menu.field_initialization_sound_due(),frame==20);
                let input=StartMenuInput {start:true,down:frame>=11 && (held_down || frame==11),..StartMenuInput::none()};
                assert!(menu.sample_field_initialization(input).is_none());
                assert_eq!(menu.current_item(),StartMenuItem::Pokemon);
            }
            let first=menu.sample_field_initialization(StartMenuInput {start:true,down:held_down,..StartMenuInput::none()}).unwrap();
            assert!(!first.start,"opening START is still held, not a new close");
            menu.update_frame(first);
            assert_eq!(menu.current_item(),if held_down {StartMenuItem::Item} else {StartMenuItem::Pokemon});
        }
    }
    #[test]
    fn opening_held_direction_is_not_replayed_and_submenu_return_has_no_field_wait() {
        let opening=StartMenuInput {start:true,down:true,..StartMenuInput::none()};
        let mut menu=StartMenuState::new(false,true,false);
        menu.begin_field_initialization(opening);
        for _ in 0..22 {assert!(menu.sample_field_initialization(opening).is_none());}
        assert_eq!(menu.sample_field_initialization(opening),Some(StartMenuInput::none()));
        menu.begin_field_initialization(opening);
        menu.open(false,true,false);
        assert!(!menu.field_initialization_active());
    }
}
