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
    has_pokemon: bool,
    is_link_connected: bool,
    /// `wBattleAndStartSavedMenuItem` — persists cursor position across menu opens.
    saved_cursor: usize,
    /// PrintSafariZoneSteps (player_state.asm:219-255, called from
    /// home/start_menu.asm:12): inside the Safari Zone the START menu opens
    /// with the "NNN/500" steps + "BALL×× NN" info box in the top-left.
    /// None = not in the Safari Zone.
    pub safari_info: Option<SafariZoneInfo>,
    field_initialization: Option<(u8, StartMenuInput)>,
    /// Field window/font transfers; direction delays keep existing pixels.
    field_presentation_elapsed: Option<u8>,
    first_transfer_portion: u8,
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
        let items = Self::build_items(has_pokedex, is_link_connected);
        Self {
            items,
            cursor: 0,
            has_pokedex,
            has_pokemon,
            is_link_connected,
            saved_cursor: 0,
            safari_info: None,
            field_initialization: None,
            field_presentation_elapsed: None,
            first_transfer_portion: 0,
        }
    }

    fn build_items(
        has_pokedex: bool,
        is_link_connected: bool,
    ) -> Vec<StartMenuItem> {
        let mut items = Vec::with_capacity(7);
        if has_pokedex {
            items.push(StartMenuItem::Pokedex);
        }
        // DrawStartMenu always prints POKEMON, even before a starter exists.
        items.push(StartMenuItem::Pokemon);
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
        self.field_presentation_elapsed = None;
        self.has_pokedex = has_pokedex;
        self.has_pokemon = has_pokemon;
        self.is_link_connected = is_link_connected;
        self.items = Self::build_items(has_pokedex, is_link_connected);
        self.cursor = self.saved_cursor.min(self.items.len().saturating_sub(1));
    }

    /// DisplayTextIDInit's transfers precede DrawStartMenu and its first
    /// Joypad poll. Only field entry owns this wait; submenu returns do not.
    pub fn begin_field_initialization(&mut self, previous: StartMenuInput) {
        self.field_initialization = Some((23, previous));
        self.field_presentation_elapsed = Some(0);
        self.first_transfer_portion = 0;
    }

    /// AutoBgMapTransfer retains its current third while disabled in the
    /// field/font loader. Copy order depends on UI history, not transport.
    pub fn begin_field_initialization_with_portion(&mut self, previous: StartMenuInput, portion: u8) {
        self.begin_field_initialization(previous);
        self.first_transfer_portion = portion % 3;
    }

    /// RedisplayStartMenu redraws without DisplayTextIDInit or START SFX.
    /// Its first Joypad is three hardware frames after DrawStartMenu.
    pub fn begin_redisplay_initialization(&mut self, previous: StartMenuInput) {
        self.field_initialization = Some((3, previous));
    }

    /// MenuJoypad invokes Delay3 after HandleMenuInput accepts a direction.
    /// Compare the next poll with that complete sample, ignoring short pulses
    /// inside the delay and suppressing a direction that stayed held.
    pub fn begin_direction_delay(&mut self, previous: StartMenuInput) {
        self.field_initialization = Some((3, previous));
    }

    /// CopyScreenTileBufferToVRAM reveals the window after three waits plus
    /// the LCD frame. Font loading finishes at 20; AutoBgMapTransfer then
    /// exposes six tile rows per vblank before the first Joypad at 23.
    pub fn field_presentation_stage(&self) -> u8 {
        self.field_presentation_elapsed.unwrap_or(23)
    }

    /// None retains the map; zero draws the empty border; other values are
    /// a bitmask of the three six-row portions containing transferred text.
    pub fn visible_text_portions(&self) -> Option<u8> {
        // Safari prints its extra window before DisplayTextIDInit. Preserve
        // that path until its distinct window-copy sequence is captured.
        if self.safari_info.is_some() { return Some(7); }
        let transferred = match self.field_presentation_stage() {
            0..=3 => return None,
            4..=20 => 0,
            21 => 1,
            22 => 2,
            _ => 3,
        };
        let mut mask = 0;
        for step in 0..transferred {
            mask |= 1 << ((self.first_transfer_portion + step) % 3);
        }
        Some(mask)
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
        if let Some(elapsed) = &mut self.field_presentation_elapsed {
            *elapsed = elapsed.saturating_add(1);
        }
        if remaining > 1 {
            self.field_initialization = Some((remaining - 1, previous));
            return None;
        }
        self.field_initialization = None;
        self.field_presentation_elapsed = None;
        Some(StartMenuInput {
            up: held.up && !previous.up,
            down: held.down && !previous.down,
            a: held.a && !previous.a,
            b: held.b && !previous.b,
            start: held.start && !previous.start,
        })
    }

    pub fn update_frame(&mut self, input: StartMenuInput) -> StartMenuAction {
        // HandleMenuInput moves the cursor first; DisplayStartMenu then
        // loops on a direction before testing the accompanying A/B/START.
        if input.up {
            self.cursor_up();
            return StartMenuAction::Redisplay;
        } else if input.down {
            self.cursor_down();
            return StartMenuAction::Redisplay;
        }

        if input.b || input.start {
            self.save_cursor();
            return StartMenuAction::Close;
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
            StartMenuItem::Pokemon if !self.has_pokemon => StartMenuAction::Redisplay,
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
    fn field_window_transfer_keeps_source_six_row_order_without_hiding_direction_delays() {
        // Original LCD captures: empty border at +4, first text at +21,
        // and the following two vblanks complete the other six-row portions.
        for (portion, masks) in [(0, [1, 3, 7]), (1, [2, 6, 7]), (2, [4, 5, 7])] {
            let mut menu = StartMenuState::new(false, true, false);
            menu.begin_field_initialization_with_portion(StartMenuInput::none(), portion);
            for elapsed in 0..=23 {
                let expected = match elapsed {
                    0..=3 => None,
                    4..=20 => Some(0),
                    _ => Some(masks[elapsed - 21]),
                };
                assert_eq!(menu.visible_text_portions(), expected, "portion={portion} elapsed={elapsed}");
                if elapsed < 23 { menu.sample_field_initialization(StartMenuInput::none()); }
            }
            for redisplay in [false, true] {
                if redisplay { menu.begin_redisplay_initialization(StartMenuInput::none()); }
                else { menu.begin_direction_delay(StartMenuInput::none()); }
                for _ in 0..3 {
                    assert_eq!(menu.visible_text_portions(), Some(7));
                    menu.sample_field_initialization(StartMenuInput::none());
                }
            }
        }
    }

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

#[cfg(test)]
mod redisplay_initialization_tests {
    use super::*;

    #[test]
    fn redisplay_discards_early_pulse_and_suppresses_held_return_b() {
        for held_down in [false, true] {
            let mut menu = StartMenuState::new(false, true, false);
            menu.begin_redisplay_initialization(StartMenuInput { b: true, ..StartMenuInput::none() });
            for frame in 1..=2 {
                assert!(!menu.field_initialization_sound_due());
                assert!(menu.sample_field_initialization(StartMenuInput {
                    b: true, down: held_down || frame == 1, ..StartMenuInput::none()
                }).is_none());
            }
            let first = menu.sample_field_initialization(StartMenuInput {
                b: true, down: held_down, ..StartMenuInput::none()
            }).unwrap();
            assert!(!first.b, "closing B remains held and is not a new press");
            assert_eq!(menu.update_frame(first), StartMenuAction::Redisplay);
            assert_eq!(menu.current_item(), if held_down { StartMenuItem::Item } else { StartMenuItem::Pokemon });
        }
    }
}
