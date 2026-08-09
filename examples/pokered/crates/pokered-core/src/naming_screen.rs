//! Naming screen state machine.
//!
//! Replicates `engine/menus/naming_screen.asm`:
//! - 3 screen types: Player name, Rival name, Pokémon nickname
//! - Player/Rival max 7 chars (PLAYER_NAME_LENGTH - 1)
//! - Pokémon nickname max 10 chars (NAME_LENGTH - 1)
//! - Alphabet grid: 5 rows × 9 columns, uppercase/lowercase toggle
//! - Special row 5: contains "ED" (submit) tile at col 8
//! - Row 6 (index 5 zero-based in grid): case toggle row
//! - Cursor navigates rows 0..=5 (grid rows 0-4 + case row)
//! - Columns 0..=8 in grid (mapped from wTopMenuItemX 1,3,5,...,17 → 0..8)
//!
//! The ASM uses wCurrentMenuItem (1-based rows 1-6, with 6=case row)
//! and wTopMenuItemX (odd values 1-17 for 9 columns).
//! We simplify to 0-based row/col.

use pokered_data::charmap::naming_tiles;
use pokered_data::pinyin_dict;

/// Input mode for the naming screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Alphabet,
    Pinyin,
}

/// Max name length for player/rival (excluding terminator).
pub const PLAYER_NAME_MAX: usize = 7;

/// Max name length for Pokémon nickname (excluding terminator).
pub const MON_NAME_MAX: usize = 10;

/// Alphabet grid dimensions.
pub const GRID_ROWS: usize = 5;
pub const GRID_COLS: usize = 9;

/// Total cursor rows: 5 grid rows + 1 case toggle row = 6.
pub const TOTAL_ROWS: usize = 6;

/// Frames of the white flash when the naming screen opens and after it
/// submits (`GBPalWhiteOutWithDelay3`: instant all-white palettes + Delay3,
/// naming_screen.asm:88 and :163).
pub const NAMING_FLASH_FRAMES: u8 = 3;

/// The uppercase alphabet grid (5 rows × 9 cols).
/// Matches data/text/alphabets.asm UpperCaseAlphabet.
pub const UPPER_ALPHABET: [[u8; GRID_COLS]; GRID_ROWS] = [
    // Row 0: A-I
    [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88], // A B C D E F G H I
    // Row 1: J-R
    [0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F, 0x90, 0x91], // J K L M N O P Q R
    // Row 2: S-Z, space
    [0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x7F], // S T U V W X Y Z (space)
    // Row 3: Special characters
    [0xF1, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xBA, 0xBA], // × ( ) : ; [ ] é é
    // Row 4: More special + ED tile
    [
        0xE3,
        0xE6,
        0xE7,
        0xEF,
        0xF5,
        0xF3,
        0xE8,
        0xF4,
        naming_tiles::ED_TILE,
    ], // - ? ! ♂ ♀ / . , ED
];

/// The lowercase alphabet grid.
/// Matches data/text/alphabets.asm LowerCaseAlphabet.
pub const LOWER_ALPHABET: [[u8; GRID_COLS]; GRID_ROWS] = [
    // Row 0: a-i
    [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8], // a b c d e f g h i
    // Row 1: j-r
    [0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB0, 0xB1], // j k l m n o p q r
    // Row 2: s-z, space
    [0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0x7F], // s t u v w x y z (space)
    // Row 3: Special characters (same as uppercase)
    [0xF1, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xBA, 0xBA], // × ( ) : ; [ ] é é
    // Row 4: More special + ED tile (same as uppercase)
    [
        0xE3,
        0xE6,
        0xE7,
        0xEF,
        0xF5,
        0xF3,
        0xE8,
        0xF4,
        naming_tiles::ED_TILE,
    ], // - ? ! ♂ ♀ / . , ED
];

/// The ED tile ID (row 4, col 8).
pub const ED_TILE_ID: u8 = naming_tiles::ED_TILE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamingScreenType {
    Player,
    Rival,
    Pokemon,
}

impl NamingScreenType {
    pub fn max_length(&self) -> usize {
        match self {
            Self::Player | Self::Rival => PLAYER_NAME_MAX,
            Self::Pokemon => MON_NAME_MAX,
        }
    }
}

/// Extended input for the naming screen (needs all 8 buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamingInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
    pub select: bool,
}

impl NamingInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            a: false,
            b: false,
            start: false,
            select: false,
        }
    }
}

/// Result of a naming screen frame update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamingScreenResult {
    /// Still editing — no final action yet.
    Editing,
    /// User submitted a name (Start or selected ED tile).
    Submitted(String),
    /// User cancelled (submitted empty name, per ASM '@' check).
    Cancelled,
}

/// The naming screen state machine.
#[derive(Debug, Clone)]
pub struct NamingScreenState {
    screen_type: NamingScreenType,
    name: String,
    lowercase: bool,
    cursor_row: usize,
    cursor_col: usize,
    submitted: bool,
    pub input_mode: InputMode,
    pub pinyin_buf: String,
    pub pinyin_candidates: Vec<char>,
    pub candidate_idx: usize,
}

impl NamingScreenState {
    pub fn new(screen_type: NamingScreenType) -> Self {
        Self {
            screen_type,
            name: String::new(),
            lowercase: false,
            cursor_row: 0,
            cursor_col: 0,
            submitted: false,
            input_mode: InputMode::Alphabet,
            pinyin_buf: String::new(),
            pinyin_candidates: Vec::new(),
            candidate_idx: 0,
        }
    }

    pub fn screen_type(&self) -> NamingScreenType {
        self.screen_type
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn max_length(&self) -> usize {
        self.screen_type.max_length()
    }

    pub fn is_lowercase(&self) -> bool {
        self.lowercase
    }

    pub fn cursor_row(&self) -> usize {
        self.cursor_row
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    /// Get the current alphabet grid based on case (returns tile IDs).
    pub fn current_alphabet(&self) -> &[[u8; GRID_COLS]; GRID_ROWS] {
        if self.lowercase {
            &LOWER_ALPHABET
        } else {
            &UPPER_ALPHABET
        }
    }

    /// Process one frame of input. Returns the result.
    pub fn update_frame(&mut self, input: NamingInput, is_zh: bool) -> NamingScreenResult {
        if self.submitted {
            return self.submit_result();
        }

        // Start always submits
        if input.start {
            self.submitted = true;
            return self.submit_result();
        }

        // Select toggles mode (only in Chinese)
        if input.select && is_zh {
            self.input_mode = match self.input_mode {
                InputMode::Alphabet => InputMode::Pinyin,
                InputMode::Pinyin => InputMode::Alphabet,
            };
            self.pinyin_buf.clear();
            self.pinyin_candidates.clear();
            self.candidate_idx = 0;
            return NamingScreenResult::Editing;
        }

        match self.input_mode {
            InputMode::Pinyin if is_zh => self.update_pinyin(input),
            _ => self.update_alphabet(input),
        }
    }

    fn update_alphabet(&mut self, input: NamingInput) -> NamingScreenResult {
        if input.select {
            self.lowercase = !self.lowercase;
            return NamingScreenResult::Editing;
        }
        if input.b {
            self.name.pop();
            return NamingScreenResult::Editing;
        }
        if input.a {
            return self.handle_a_press();
        }
        if input.down { self.move_down(); }
        else if input.up { self.move_up(); }
        else if input.right { self.move_right(); }
        else if input.left { self.move_left(); }
        NamingScreenResult::Editing
    }

    fn update_pinyin(&mut self, input: NamingInput) -> NamingScreenResult {
        let has_candidates = !self.pinyin_candidates.is_empty();
        if input.b {
            if has_candidates {
                self.pinyin_candidates.clear();
                self.candidate_idx = 0;
            } else if !self.pinyin_buf.is_empty() {
                self.pinyin_buf.pop();
            } else {
                self.name.pop();
            }
            return NamingScreenResult::Editing;
        }
        if input.left {
            if has_candidates && self.candidate_idx > 0 {
                self.candidate_idx -= 1;
            }
            return NamingScreenResult::Editing;
        }
        if input.right {
            if has_candidates && self.candidate_idx + 1 < self.pinyin_candidates.len() {
                self.candidate_idx += 1;
            }
            return NamingScreenResult::Editing;
        }
        if input.a {
            if has_candidates {
                let ch = self.pinyin_candidates[self.candidate_idx];
                if self.name.len() < self.max_length() {
                    self.name.push(ch);
                }
                self.pinyin_buf.clear();
                self.pinyin_candidates.clear();
                self.candidate_idx = 0;
                if self.name.len() >= self.max_length() {
                    self.input_mode = InputMode::Alphabet;
                }
                return NamingScreenResult::Editing;
            }
            return NamingScreenResult::Editing;
        }
        // Type letters into pinyin buffer via d-pad selection
        let tile_id = if self.cursor_row < GRID_ROWS {
            let alphabet = if self.lowercase { &LOWER_ALPHABET } else { &UPPER_ALPHABET };
            Some(alphabet[self.cursor_row][self.cursor_col])
        } else {
            None
        };
        if input.a && !has_candidates {
            if let Some(tile_id) = tile_id {
                if let Some(c) = pokered_data::charmap::decode_char(tile_id) {
                    let ch = c.chars().next().unwrap_or(' ');
                    if ch.is_ascii_alphabetic() {
                        self.pinyin_buf.push(ch.to_ascii_lowercase());
                        // Search candidates
                        if self.pinyin_buf.len() >= 1 {
                            self.pinyin_candidates = pinyin_dict::lookup_pinyin(&self.pinyin_buf);
                            self.candidate_idx = 0;
                        }
                        return NamingScreenResult::Editing;
                    }
                }
            }
            return NamingScreenResult::Editing;
        }
        if input.down { self.move_down(); }
        else if input.up { self.move_up(); }
        else if input.right { self.move_right(); }
        else if input.left { self.move_left(); }
        NamingScreenResult::Editing
    }

    fn handle_a_press(&mut self) -> NamingScreenResult {
        // Row 5 (case toggle row)
        if self.cursor_row == 5 {
            // In ASM, row 6 col 1 = case toggle. Other positions on row 6 also toggle.
            self.lowercase = !self.lowercase;
            return NamingScreenResult::Editing;
        }

        // Check if on the ED tile (row 4, col 8)
        if self.cursor_row == 4 && self.cursor_col == 8 {
            self.submitted = true;
            return self.submit_result();
        }

        // Normal character — add to name if space remains
        let tile_id = self.current_alphabet()[self.cursor_row][self.cursor_col];

        // Convert tile ID to char and add to name
        if let Some(c) = pokered_data::charmap::decode_char(tile_id) {
            let ch = c.chars().next().unwrap_or(' ');
            if self.name.len() < self.max_length() {
                self.name.push(ch);

                // Per ASM: when all spaces filled, force cursor to ED tile
                if self.name.len() >= self.max_length() {
                    self.cursor_row = 4;
                    self.cursor_col = 8;
                }
            }
        }

        NamingScreenResult::Editing
    }

    fn submit_result(&self) -> NamingScreenResult {
        if self.name.is_empty() {
            NamingScreenResult::Cancelled
        } else {
            NamingScreenResult::Submitted(self.name.clone())
        }
    }

    /// Move cursor down. Row wraps: 5 → 0 (per ASM .pressedDown, 6→1 in 1-based).
    fn move_down(&mut self) {
        self.cursor_row += 1;
        if self.cursor_row >= TOTAL_ROWS {
            self.cursor_row = 0;
        }
        // When entering case row (row 5), force col to 0 (per ASM behavior)
        if self.cursor_row == 5 {
            self.cursor_col = 0;
        }
    }

    /// Move cursor up. Row wraps: 0 → 5 (per ASM .pressedUp, 1→6 in 1-based).
    fn move_up(&mut self) {
        if self.cursor_row == 0 {
            self.cursor_row = 5;
            self.cursor_col = 0; // Force col on case row
        } else {
            self.cursor_row -= 1;
        }
    }

    /// Move cursor right. Wraps within row. Not available on case toggle row.
    fn move_right(&mut self) {
        if self.cursor_row == 5 {
            return; // Can't scroll on bottom row (per ASM)
        }
        self.cursor_col += 1;
        if self.cursor_col >= GRID_COLS {
            self.cursor_col = 0;
        }
    }

    /// Move cursor left. Wraps within row. Not available on case toggle row.
    fn move_left(&mut self) {
        if self.cursor_row == 5 {
            return; // Can't scroll on bottom row (per ASM)
        }
        if self.cursor_col == 0 {
            self.cursor_col = GRID_COLS - 1;
        } else {
            self.cursor_col -= 1;
        }
    }
}
