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
//!
//! # Pinyin mode (Chinese only, toggled with Select)
//!
//! The cursor covers one unified space: letter rows 0..=2 (A-Z only —
//! the specials/ED rows and the case row are alphabet-mode only) followed
//! by up to two candidate-strip rows. A on a letter appends it to the
//! pinyin buffer and refreshes the candidates; d-pad into the strip and
//! A commits the highlighted candidate to the name. After a commit the
//! cursor returns to the last typed letter, ready for the next syllable.
//!
//! # Name length is a width budget, not a character count
//!
//! `max_length()` (7/10) counts width units: an ASCII character costs 1
//! (5px glyph in an 8px slot), a CJK character costs 2 (10px full-width
//! glyph — two slots). A player name therefore holds up to 7 ASCII or 3
//! CJK characters, a nickname 10 ASCII or 5 CJK — mirroring how the
//! original's byte-sized NAME_LENGTH behaves under a DBCS encoding.

use pokered_data::charmap::naming_tiles;
use pokered_data::pinyin_dict;

/// Input mode for the naming screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Alphabet,
    Pinyin,
}

/// Max name length for player/rival (excluding terminator), in width
/// units — see the module docs. 7 units = 7 ASCII or 3 CJK characters.
pub const PLAYER_NAME_MAX: usize = 7;

/// Max name length for Pokémon nickname (excluding terminator), in width
/// units — see the module docs. 10 units = 10 ASCII or 5 CJK characters.
pub const MON_NAME_MAX: usize = 10;

/// Width units a CJK character costs against the name budget.
pub const CJK_UNITS: usize = 2;

/// Width units `ch` costs against the name budget: 2 for CJK-block
/// characters (radicals U+2E80 and up — ideographs, kana, Hangul — which
/// render 10px full-width), 1 for everything else (ASCII and the Latin-1
/// punctuation the alphabet grid offers, which render 5px half-width).
pub fn char_units(ch: char) -> usize {
    if (ch as u32) >= 0x2E80 {
        CJK_UNITS
    } else {
        1
    }
}

/// Total width units of `name`.
pub fn name_units(name: &str) -> usize {
    name.chars().map(char_units).sum()
}

/// Alphabet grid dimensions.
pub const GRID_ROWS: usize = 5;
pub const GRID_COLS: usize = 9;

/// Total cursor rows: 5 grid rows + 1 case toggle row = 6.
pub const TOTAL_ROWS: usize = 6;

/// Letter rows used in pinyin mode: rows 0..=2 of the alphabet grid (A-Z).
/// The specials/ED rows and the case row are alphabet-mode only.
pub const PINYIN_GRID_ROWS: usize = 3;

/// Candidate slots per candidate-strip line (matches the 6×3-tile slots the
/// naming screen renderer draws).
pub const CANDIDATES_PER_LINE: usize = 6;

/// Pinyin buffer cap: the longest syllables ("zhuang"/"chuang") are 6 letters.
const PINYIN_BUF_MAX: usize = 6;

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
    /// Read by the renderers to draw the pinyin buffer/candidate strip.
    pub input_mode: InputMode,
    /// Read by the renderers to draw the pinyin buffer/candidate strip.
    pub pinyin_buf: String,
    /// Read by the renderers to draw the pinyin buffer/candidate strip.
    pub pinyin_candidates: Vec<char>,
    /// Grid position of the last letter typed in pinyin mode; the cursor
    /// returns here when leaving the candidate strip.
    pinyin_last_typed: (usize, usize),
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
            pinyin_last_typed: (0, 0),
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

    /// Width units the name currently occupies (ASCII 1, CJK 2) — the
    /// renderer fills this many underscore slots.
    pub fn used_units(&self) -> usize {
        name_units(&self.name)
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
            let entering_pinyin = self.input_mode == InputMode::Alphabet;
            self.input_mode = match self.input_mode {
                InputMode::Alphabet => InputMode::Pinyin,
                InputMode::Pinyin => InputMode::Alphabet,
            };
            self.pinyin_buf.clear();
            self.pinyin_candidates.clear();
            if self.cursor_row >= PINYIN_GRID_ROWS {
                if entering_pinyin {
                    // Alphabet rows 3-5 (specials/ED/case) don't exist in
                    // pinyin mode.
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                } else {
                    // Leaving the candidate strip: return to the letter the
                    // user last typed.
                    (self.cursor_row, self.cursor_col) = self.pinyin_last_typed;
                }
            }
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

    /// One pinyin-mode frame. The cursor space is the 3 letter rows followed
    /// by up to 2 candidate-strip lines; see the module docs.
    fn update_pinyin(&mut self, input: NamingInput) -> NamingScreenResult {
        if input.b {
            if !self.pinyin_buf.is_empty() {
                self.pinyin_buf.pop();
                self.refresh_pinyin_candidates();
            } else {
                self.name.pop();
            }
            return NamingScreenResult::Editing;
        }
        if input.a {
            return self.pinyin_a_press();
        }
        if input.down {
            let old_row = self.cursor_row;
            self.cursor_row += 1;
            if self.cursor_row >= self.pinyin_cursor_rows() {
                self.cursor_row = 0;
            }
            if old_row < PINYIN_GRID_ROWS && self.cursor_row >= PINYIN_GRID_ROWS {
                // Entering the strip from the letters: start at its first
                // slot, not wherever the letter column happened to be.
                self.cursor_col = 0;
            }
        } else if input.up {
            let old_row = self.cursor_row;
            if self.cursor_row == 0 {
                self.cursor_row = self.pinyin_cursor_rows() - 1;
            } else {
                self.cursor_row -= 1;
            }
            if old_row < PINYIN_GRID_ROWS && self.cursor_row >= PINYIN_GRID_ROWS {
                self.cursor_col = 0;
            }
        } else if input.right {
            self.cursor_col = (self.cursor_col + 1) % self.pinyin_row_cols();
        } else if input.left {
            let cols = self.pinyin_row_cols();
            self.cursor_col = if self.cursor_col == 0 { cols - 1 } else { self.cursor_col - 1 };
        }
        self.clamp_pinyin_col();
        NamingScreenResult::Editing
    }

    /// A press in pinyin mode: commit a candidate, or append the selected
    /// letter to the pinyin buffer.
    fn pinyin_a_press(&mut self) -> NamingScreenResult {
        if self.cursor_row >= PINYIN_GRID_ROWS {
            // Candidate strip: commit the highlighted character. A CJK
            // candidate costs 2 width units; it is only appended when the
            // budget still covers it.
            let idx = (self.cursor_row - PINYIN_GRID_ROWS) * CANDIDATES_PER_LINE + self.cursor_col;
            let Some(&ch) = self.pinyin_candidates.get(idx) else {
                return NamingScreenResult::Editing;
            };
            if self.used_units() + char_units(ch) <= self.max_length() {
                self.name.push(ch);
            }
            self.pinyin_buf.clear();
            self.pinyin_candidates.clear();
            if self.max_length().saturating_sub(self.used_units()) < CJK_UNITS {
                // No room left for another CJK character — the same
                // contract as the alphabet path filling every slot: leave
                // pinyin mode with the cursor on the ED tile.
                self.input_mode = InputMode::Alphabet;
                self.cursor_row = 4;
                self.cursor_col = 8;
            } else {
                (self.cursor_row, self.cursor_col) = self.pinyin_last_typed;
            }
            return NamingScreenResult::Editing;
        }

        // Letter row: append the tile's letter to the pinyin buffer.
        let tile_id = self.current_alphabet()[self.cursor_row][self.cursor_col];
        if let Some(c) = pokered_data::charmap::decode_char(tile_id) {
            let ch = c.chars().next().unwrap_or(' ');
            if ch.is_ascii_alphabetic() && self.pinyin_buf.len() < PINYIN_BUF_MAX {
                self.pinyin_buf.push(ch.to_ascii_lowercase());
                self.pinyin_last_typed = (self.cursor_row, self.cursor_col);
                self.refresh_pinyin_candidates();
            }
        }
        NamingScreenResult::Editing
    }

    /// Re-run the dictionary lookup after the buffer changed, keeping the
    /// cursor on a valid slot.
    fn refresh_pinyin_candidates(&mut self) {
        self.pinyin_candidates = pinyin_dict::lookup_pinyin(&self.pinyin_buf);
        if self.cursor_row >= PINYIN_GRID_ROWS {
            let line = self.cursor_row - PINYIN_GRID_ROWS;
            if line >= self.candidate_lines() {
                // The strip vanished or shrank: return to the last typed
                // letter.
                (self.cursor_row, self.cursor_col) = self.pinyin_last_typed;
            } else {
                self.clamp_pinyin_col();
            }
        }
    }

    /// Total cursor rows in pinyin mode: letter rows + candidate lines.
    fn pinyin_cursor_rows(&self) -> usize {
        PINYIN_GRID_ROWS + self.candidate_lines()
    }

    fn candidate_lines(&self) -> usize {
        self.pinyin_candidates.len().div_ceil(CANDIDATES_PER_LINE)
    }

    /// Valid columns on the current cursor row (9 for letter rows, the
    /// populated slot count for candidate lines). Never returns 0 — the
    /// value is used as a modulo divisor.
    fn pinyin_row_cols(&self) -> usize {
        if self.cursor_row < PINYIN_GRID_ROWS {
            GRID_COLS
        } else {
            let line = self.cursor_row - PINYIN_GRID_ROWS;
            let len = self.pinyin_candidates.len();
            if line == 0 {
                len.min(CANDIDATES_PER_LINE).max(1)
            } else {
                len.saturating_sub(CANDIDATES_PER_LINE).max(1)
            }
        }
    }

    /// Keep the column valid after a row change.
    fn clamp_pinyin_col(&mut self) {
        let cols = self.pinyin_row_cols();
        if cols > 0 && self.cursor_col >= cols {
            self.cursor_col = cols - 1;
        }
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

        // Normal character — add to name if space remains (grid characters
        // cost 1 width unit each).
        let tile_id = self.current_alphabet()[self.cursor_row][self.cursor_col];

        // Convert tile ID to char and add to name
        if let Some(c) = pokered_data::charmap::decode_char(tile_id) {
            let ch = c.chars().next().unwrap_or(' ');
            if self.used_units() < self.max_length() {
                self.name.push(ch);

                // Per ASM: when all spaces filled, force cursor to ED tile
                if self.used_units() >= self.max_length() {
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
