//! Main menu state machine.
//!
//! Replicates the main menu from `engine/menus/main_menu.asm`:
//! - If a save file exists: CONTINUE / NEW GAME / OPTION (3 items)
//! - If no save file:       NEW GAME / OPTION             (2 items)
//! - B button returns to the title screen
//! - Selecting CONTINUE shows save info, then waits for A (load) or B (back)
//!
//! OPTION transitions to the options screen; `last_choice` records
//! `MainMenuChoice::Option` so the frontend knows to return here (not to the
//! Start menu) when the options screen is closed.

use crate::game_state::{GameScreen, MainMenuChoice, SaveFileSummary, ScreenAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuInput {
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
}

impl MenuInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            a: false,
            b: false,
        }
    }
}

/// CONTINUE / NEW GAME / OPTION.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuItem {
    Continue,
    NewGame,
    Option,
}

/// Sub-phase after selecting CONTINUE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinueInfoPhase {
    WaitingForInput,
}

#[derive(Debug, Clone)]
pub struct MainMenuState {
    pub items: Vec<MainMenuItem>,
    pub cursor: usize,
    pub has_save: bool,
    pub save_summary: Option<SaveFileSummary>,
    pub continue_info_phase: Option<ContinueInfoPhase>,
    /// Frames the save-info panel has been shown (the original delays input
    /// by 30 frames — `DisplayContinueGameInfo`'s `ld c, 30 / call
    /// DelayFrames`, engine/menus/main_menu.asm:378-379).
    pub continue_info_frames: u32,
    pub last_choice: Option<MainMenuChoice>,
    pub init_delay_frames: u32,
    pub init_delay_done: bool,
}

/// Frames before accepting input (original: `ld c, 20 / call DelayFrames`).
pub const MENU_INIT_DELAY_FRAMES: u32 = 20;

pub const MENU_SELECT_DELAY_FRAMES: u32 = 20;

/// Frames the CONTINUE save-info panel shows before it accepts input
/// (engine/menus/main_menu.asm:378-379).
pub const CONTINUE_INFO_DELAY_FRAMES: u32 = 30;

impl MainMenuState {
    pub fn new(save_summary: Option<SaveFileSummary>) -> Self {
        let has_save = save_summary.is_some();
        let items = if has_save {
            vec![
                MainMenuItem::Continue,
                MainMenuItem::NewGame,
                MainMenuItem::Option,
            ]
        } else {
            vec![MainMenuItem::NewGame, MainMenuItem::Option]
        };

        Self {
            items,
            cursor: 0,
            has_save,
            save_summary,
            continue_info_phase: None,
            continue_info_frames: 0,
            last_choice: None,
            init_delay_frames: 0,
            init_delay_done: false,
        }
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn current_item(&self) -> MainMenuItem {
        self.items[self.cursor]
    }

    pub fn cursor_up(&mut self) {
        if self.cursor == 0 {
            self.cursor = self.items.len() - 1;
        } else {
            self.cursor -= 1;
        }
    }

    pub fn cursor_down(&mut self) {
        self.cursor += 1;
        if self.cursor >= self.items.len() {
            self.cursor = 0;
        }
    }

    pub fn update_frame(&mut self, input: MenuInput) -> ScreenAction {
        if !self.init_delay_done {
            self.init_delay_frames += 1;
            if self.init_delay_frames >= MENU_INIT_DELAY_FRAMES {
                self.init_delay_done = true;
            }
            return ScreenAction::Continue;
        }

        if let Some(ContinueInfoPhase::WaitingForInput) = self.continue_info_phase {
            return self.update_continue_info(input);
        }

        if input.b {
            self.last_choice = Some(MainMenuChoice::BackToTitle);
            return ScreenAction::Transition(GameScreen::TitleScreen);
        }

        if input.up {
            self.cursor_up();
        } else if input.down {
            self.cursor_down();
        }

        if input.a {
            return self.select_current_item();
        }

        ScreenAction::Continue
    }

    fn select_current_item(&mut self) -> ScreenAction {
        match self.items[self.cursor] {
            MainMenuItem::Continue => {
                // The original shows the save-info panel first
                // (`DisplayContinueGameInfo`, engine/menus/main_menu.asm:357);
                // A on the panel loads the game, B returns to the menu.
                self.continue_info_phase = Some(ContinueInfoPhase::WaitingForInput);
                self.continue_info_frames = 0;
                ScreenAction::Continue
            }
            MainMenuItem::NewGame => {
                self.last_choice = Some(MainMenuChoice::NewGame);
                ScreenAction::Transition(GameScreen::OakSpeech)
            }
            MainMenuItem::Option => {
                self.last_choice = Some(MainMenuChoice::Option);
                ScreenAction::Transition(GameScreen::OptionsMenu)
            }
        }
    }

    fn update_continue_info(&mut self, input: MenuInput) -> ScreenAction {
        // 30-frame display delay before the panel accepts input
        // (main_menu.asm:378-379).
        if self.continue_info_frames < CONTINUE_INFO_DELAY_FRAMES {
            self.continue_info_frames += 1;
            return ScreenAction::Continue;
        }
        if input.a {
            self.continue_info_phase = None;
            self.last_choice = Some(MainMenuChoice::Continue);
            return ScreenAction::Transition(GameScreen::Overworld);
        }
        if input.b {
            self.continue_info_phase = None;
            return ScreenAction::Continue;
        }
        ScreenAction::Continue
    }

    pub fn return_from_options(&mut self) {
        self.last_choice = None;
    }

    pub fn is_showing_continue_info(&self) -> bool {
        self.continue_info_phase.is_some()
    }

    pub fn item_labels(&self) -> Vec<&'static str> {
        self.items
            .iter()
            .map(|item| match item {
                MainMenuItem::Continue => "CONTINUE",
                MainMenuItem::NewGame => "NEW GAME",
                MainMenuItem::Option => "OPTION",
            })
            .collect()
    }

    /// Returns (label, value) pairs matching `SaveScreenInfoText` in
    /// engine/menus/main_menu.asm (`DisplayContinueGameInfo`): PLAYER /
    /// BADGES / #DEX / TIME.
    pub fn continue_info_lines(&self) -> Vec<(&'static str, String)> {
        match &self.save_summary {
            Some(summary) => {
                vec![
                    (
                        "PLAYER",
                        pokered_data::charmap::decode_string(&summary.player_name),
                    ),
                    ("BADGES", format!("{}", summary.badge_count())),
                    ("#DEX", format!("{}", summary.pokedex_owned)),
                    (
                        "TIME",
                        format!(
                            "{}:{:02}",
                            summary.play_time_hours, summary.play_time_minutes
                        ),
                    ),
                ]
            }
            None => vec![],
        }
    }
}
