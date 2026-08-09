pub const NUM_BOXES: usize = 12;
pub const HOF_TEAM_CAPACITY: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveScreenInfo {
    pub player_name: String,
    pub num_badges: u8,
    pub pokedex_owned: u16,
    pub play_time_hours: u16,
    pub play_time_minutes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YesNoInput {
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
}

impl YesNoInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            a: false,
            b: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YesNoChoice {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveSfxEvent {
    None,
    Save,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePhase {
    AskSave,
    ConfirmOverwrite,
    Saving {
        frames_remaining: u32,
    },
    /// Shows "{PLAYER} saved the game!" text and plays SFX_SAVE.
    /// Waits for external signal that SFX has finished.
    SaveComplete,
    /// Final 30-frame wait after SFX finishes (matches original ASM).
    WaitAfterSave {
        frames_remaining: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveMenuResult {
    Active,
    Saved,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct SaveMenuState {
    pub phase: SavePhase,
    pub cursor: YesNoChoice,
    pub info: SaveScreenInfo,
    pub has_previous_save: bool,
    pub is_different_player: bool,
    /// Set during tick when SFX should be triggered. Read by game.rs, then cleared next tick.
    pub sfx_event: SaveSfxEvent,
}

impl SaveMenuState {
    pub fn new(info: SaveScreenInfo, has_previous_save: bool, is_different_player: bool) -> Self {
        Self {
            phase: SavePhase::AskSave,
            cursor: YesNoChoice::Yes,
            info,
            has_previous_save,
            is_different_player,
            sfx_event: SaveSfxEvent::None,
        }
    }

    pub fn tick(&mut self, input: YesNoInput) -> SaveMenuResult {
        self.sfx_event = SaveSfxEvent::None;
        match &mut self.phase {
            SavePhase::AskSave => self.handle_yes_no(input, true),
            SavePhase::ConfirmOverwrite => self.handle_yes_no(input, false),
            SavePhase::Saving { frames_remaining } => {
                if *frames_remaining > 0 {
                    *frames_remaining -= 1;
                }
                if *frames_remaining == 0 {
                    self.phase = SavePhase::SaveComplete;
                    self.sfx_event = SaveSfxEvent::Save;
                }
                SaveMenuResult::Active
            }
            SavePhase::SaveComplete => SaveMenuResult::Active,
            SavePhase::WaitAfterSave { frames_remaining } => {
                if *frames_remaining > 0 {
                    *frames_remaining -= 1;
                }
                if *frames_remaining == 0 {
                    return SaveMenuResult::Saved;
                }
                SaveMenuResult::Active
            }
        }
    }

    pub fn notify_sfx_done(&mut self) {
        if self.phase == SavePhase::SaveComplete {
            self.phase = SavePhase::WaitAfterSave {
                frames_remaining: 30,
            };
        }
    }

    fn handle_yes_no(&mut self, input: YesNoInput, is_first_prompt: bool) -> SaveMenuResult {
        if input.up {
            self.cursor = YesNoChoice::Yes;
        } else if input.down {
            self.cursor = YesNoChoice::No;
        }

        if input.b {
            return SaveMenuResult::Cancelled;
        }

        if input.a {
            match self.cursor {
                YesNoChoice::No => SaveMenuResult::Cancelled,
                YesNoChoice::Yes => {
                    if is_first_prompt && self.has_previous_save && self.is_different_player {
                        self.phase = SavePhase::ConfirmOverwrite;
                        self.cursor = YesNoChoice::Yes;
                        SaveMenuResult::Active
                    } else {
                        self.phase = SavePhase::Saving {
                            frames_remaining: 120,
                        };
                        SaveMenuResult::Active
                    }
                }
            }
        } else {
            SaveMenuResult::Active
        }
    }
}

pub const BOX_NAMES: [&str; NUM_BOXES] = [
    "BOX 1", "BOX 2", "BOX 3", "BOX 4", "BOX 5", "BOX 6", "BOX 7", "BOX 8", "BOX 9", "BOX10",
    "BOX11", "BOX12",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeBoxResult {
    Active,
    Selected(usize),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct ChangeBoxMenuState {
    pub confirmed: bool,
    pub confirm_cursor: YesNoChoice,
    pub cursor: usize,
    pub current_box: usize,
    pub box_counts: [u8; NUM_BOXES],
    pub first_change: bool,
}

impl ChangeBoxMenuState {
    pub fn new(current_box: usize, box_counts: [u8; NUM_BOXES], first_change: bool) -> Self {
        Self {
            confirmed: false,
            confirm_cursor: YesNoChoice::Yes,
            cursor: current_box.min(NUM_BOXES - 1),
            current_box: current_box.min(NUM_BOXES - 1),
            box_counts,
            first_change,
        }
    }

    pub fn tick(&mut self, input: YesNoInput) -> ChangeBoxResult {
        if !self.confirmed {
            return self.handle_confirm(input);
        }
        self.handle_box_selection(input)
    }

    fn handle_confirm(&mut self, input: YesNoInput) -> ChangeBoxResult {
        if input.up {
            self.confirm_cursor = YesNoChoice::Yes;
        } else if input.down {
            self.confirm_cursor = YesNoChoice::No;
        }

        if input.b {
            return ChangeBoxResult::Cancelled;
        }

        if input.a {
            match self.confirm_cursor {
                YesNoChoice::No => ChangeBoxResult::Cancelled,
                YesNoChoice::Yes => {
                    self.confirmed = true;
                    ChangeBoxResult::Active
                }
            }
        } else {
            ChangeBoxResult::Active
        }
    }

    fn handle_box_selection(&mut self, input: YesNoInput) -> ChangeBoxResult {
        if input.down {
            if self.cursor < NUM_BOXES - 1 {
                self.cursor += 1;
            } else {
                self.cursor = 0;
            }
        } else if input.up {
            if self.cursor > 0 {
                self.cursor -= 1;
            } else {
                self.cursor = NUM_BOXES - 1;
            }
        }

        if input.b {
            return ChangeBoxResult::Cancelled;
        }

        if input.a {
            return ChangeBoxResult::Selected(self.cursor);
        }

        ChangeBoxResult::Active
    }

    pub fn box_has_pokemon(&self, box_index: usize) -> bool {
        box_index < NUM_BOXES && self.box_counts[box_index] > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClearSaveResult {
    Active,
    Cleared,
    Cancelled,
}

/// Defaults cursor to No (ASM uses NO_YES_MENU where No is the first option)
#[derive(Debug, Clone)]
pub struct ClearSaveState {
    pub cursor: YesNoChoice,
}

impl ClearSaveState {
    pub fn new() -> Self {
        Self {
            cursor: YesNoChoice::No,
        }
    }

    pub fn tick(&mut self, input: YesNoInput) -> ClearSaveResult {
        if input.up {
            self.cursor = YesNoChoice::No;
        } else if input.down {
            self.cursor = YesNoChoice::Yes;
        }

        if input.b {
            return ClearSaveResult::Cancelled;
        }

        if input.a {
            return match self.cursor {
                YesNoChoice::No => ClearSaveResult::Cancelled,
                YesNoChoice::Yes => ClearSaveResult::Cleared,
            };
        }

        ClearSaveResult::Active
    }
}

impl Default for ClearSaveState {
    fn default() -> Self {
        Self::new()
    }
}

/// Matches CalcCheckSum from save.asm: sum all bytes, then one's complement (`cpl`).
pub fn calc_checksum(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    !sum
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFileStatus {
    None,
    BadChecksum,
    Good,
}

impl SaveFileStatus {
    pub fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::BadChecksum,
            2 => Self::Good,
            _ => Self::None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::BadChecksum => 1,
            Self::Good => 2,
        }
    }

    pub fn has_save(self) -> bool {
        self == Self::Good
    }
}
