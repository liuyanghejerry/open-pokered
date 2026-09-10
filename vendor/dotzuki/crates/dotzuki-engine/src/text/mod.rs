//! # text
//!
//! Trait-based text engine for JRPG dialog systems.
//!
//! This module defines the [`TextProvider`] trait — the abstraction that
//! game-specific implementations must fulfill to provide character mapping,
//! rendering, and control-code handling.  It also provides [`TileBuffer`]
//! (the rendering surface), [`DialogState`], [`TextStream`], [`ControlAction`],
//! and [`DialogEngine`] — a general-purpose dialog engine that drives text
//! display one character per frame.
//!
//! ## Design
//!
//! - **Provider pattern**: All game-specific text encoding lives in the
//!   [`TextProvider`] implementation.  The engine owns no charmap data.
//! - **No game-specific semantics**: Control codes like trainer name, monster
//!   name, and item substitutions come from the game-specific provider.
//! - **Frame-by-frame typing**: [`DialogEngine::update`] processes one
//!   character per call, enabling the classic typewriter effect.

// ── TilePos ────────────────────────────────────────────────────────

/// A position on the tile grid, measured in tiles.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilePos {
    /// Horizontal position (0 = left edge).
    pub x: u16,
    /// Vertical position (0 = top edge).
    pub y: u16,
}

impl TilePos {
    /// Creates a new tile position.
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

// ── TileEntry ──────────────────────────────────────────────────────

/// A single cell in the tile buffer — which tile to draw and with what ink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileEntry {
    /// Index into the tileset's tile table.
    pub tile_id: u16,
    /// Ink / colour index (0 = transparent / background).
    pub ink: u8,
}

impl Default for TileEntry {
    fn default() -> Self {
        Self { tile_id: 0, ink: 0 }
    }
}

// ── TileBuffer ─────────────────────────────────────────────────────

/// A configurable-width tile grid representing the text rendering surface.
///
/// Each cell carries a tile ID and ink colour.  The `cursor` tracks the
/// current write position for the next character.  Dimensions are set at
/// construction time via [`TileBuffer::new`].
pub struct TileBuffer {
    /// Row-major tile entries.  `tiles[y * width_tiles + x]`.
    pub tiles: Vec<TileEntry>,
    /// Width of the buffer in tiles.
    pub width_tiles: u16,
    /// Height of the buffer in tiles.
    pub height_tiles: u16,
    /// Current text cursor position.
    pub cursor: TilePos,
}

impl TileBuffer {
    /// Creates a new, empty tile buffer with the cursor at (0, 0).
    pub fn new(width_tiles: u16, height_tiles: u16) -> Self {
        let size = (width_tiles as usize) * (height_tiles as usize);
        Self {
            tiles: vec![TileEntry::default(); size],
            width_tiles,
            height_tiles,
            cursor: TilePos::new(0, 0),
        }
    }

    /// Converts (x, y) to a linear index.  Does not bounds-check.
    #[inline]
    fn index(&self, x: u16, y: u16) -> usize {
        (y * self.width_tiles + x) as usize
    }

    /// Clears all tiles and resets the cursor to (0, 0).
    pub fn clear(&mut self) {
        self.tiles.fill(TileEntry::default());
        self.cursor = TilePos::new(0, 0);
    }

    /// Writes a tile at the given position.  Silently does nothing if
    /// the position is out of bounds.
    pub fn set_tile(&mut self, pos: TilePos, tile_id: u16, ink: u8) {
        if pos.x < self.width_tiles && pos.y < self.height_tiles {
            let idx = self.index(pos.x, pos.y);
            self.tiles[idx] = TileEntry { tile_id, ink };
        }
    }

    /// Moves the cursor to the start of the next line.
    /// Does not wrap or scroll.
    pub fn newline(&mut self) {
        self.cursor.x = 0;
        self.cursor.y += 1;
    }

    /// Scrolls the entire buffer up by one row.  The top row is discarded
    /// and the bottom row is filled with default entries.
    pub fn scroll(&mut self) {
        for y in 1..self.height_tiles {
            for x in 0..self.width_tiles {
                let src = self.index(x, y);
                let dst = self.index(x, y - 1);
                self.tiles[dst] = self.tiles[src];
            }
        }
        for x in 0..self.width_tiles {
            let idx = self.index(x, self.height_tiles - 1);
            self.tiles[idx] = TileEntry::default();
        }
    }
}

impl Default for TileBuffer {
    fn default() -> Self {
        Self::new(20, 18)
    }
}

// ── TextStream ─────────────────────────────────────────────────────

/// A decoded character stream with a cursor for sequential reading.
///
/// `C` is the character type produced by the [`TextProvider`].
pub struct TextStream<C> {
    /// All decoded characters.
    pub chars: Vec<C>,
    /// Current read position.
    pub pos: usize,
}

impl<C> TextStream<C> {
    /// Creates a new stream from a vector of decoded characters.
    pub fn new(chars: Vec<C>) -> Self {
        Self { chars, pos: 0 }
    }

    /// Returns the next character and advances the cursor, or `None`
    /// if the stream is exhausted.
    pub fn next(&mut self) -> Option<&C> {
        let c = self.chars.get(self.pos);
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// Returns the next character without advancing the cursor,
    /// or `None` if the stream is exhausted.
    pub fn peek(&self) -> Option<&C> {
        self.chars.get(self.pos)
    }

    /// Advances the cursor by `n` positions, clamped to the stream length.
    pub fn skip(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.chars.len());
    }

    /// Returns a slice of all characters from the current position onward.
    pub fn remaining(&self) -> &[C] {
        &self.chars[self.pos..]
    }

    /// Returns `true` if the cursor has reached the end of the stream.
    pub fn is_at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    /// Returns the current cursor position.
    pub fn position(&self) -> usize {
        self.pos
    }
}

// ── ControlAction ──────────────────────────────────────────────────

/// Action produced when a control code is processed by the text provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    /// No action — continue processing.
    None,
    /// Move the cursor to the next line.
    Newline,
    /// Pause until the player presses a button, then clear the text box.
    PageBreak,
    /// End the current dialog.
    Done,
    /// Change the text speed (0 = instant, higher = slower).
    SetSpeed(u8),
    /// Clear the text buffer.
    Clear,
    /// Jump to a named script handler.
    CallScript,
    /// Pause until the player presses a button.
    WaitInput,
    /// Move the cursor to a specific tile position.
    MoveCursor { x: u16, y: u16 },
    /// Scroll the buffer up by one row (discards top row).
    Scroll,
    /// Pause for a given number of frames.
    Pause(u8),
}

// ── DialogState ────────────────────────────────────────────────────

/// The current operational mode of the dialog engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogMode {
    /// Characters are being typed out, one per frame.
    Typing,
    /// The text engine is paused (e.g. a `TX_PAUSE` delay).
    Paused,
    /// Waiting for the player to press A to continue.
    WaitingForInput,
    /// Waiting for the player to press A after a scroll prompt.
    Scrolling,
    /// The dialog has ended.
    Done,
}

/// Full state of the dialog engine.
///
/// Carries mode information together with positioning bookmarks that
/// survive across page breaks and scroll events.
#[derive(Debug, Clone)]
pub struct DialogState {
    /// Current operational mode.
    pub mode: DialogMode,
    /// Which page of the dialog is currently active.
    pub page_index: usize,
    /// Index into the current page's character sequence.
    pub char_index: usize,
    /// Vertical scroll offset in pixels (for smooth scrolling).
    pub scroll_offset: u16,
}

impl Default for DialogState {
    fn default() -> Self {
        Self {
            mode: DialogMode::Typing,
            page_index: 0,
            char_index: 0,
            scroll_offset: 0,
        }
    }
}

// ── TextProvider trait ─────────────────────────────────────────────

/// The core abstraction for text encoding in a JRPG engine.
///
/// Implementations provide:
///
/// - A character type (`Char`) representing decoded text units.
/// - Single-byte decoding (`decode_byte`) from a custom charmap.
/// - Stream decoding (`decode_stream`) from raw byte sequences.
/// - Tile rendering (`render_char`) into a [`TileBuffer`].
/// - Width measurement (`string_width`) for layout.
/// - Control-code detection (`is_control_code`) and processing
///   (`process_control`).
///
/// # Associated Type
///
/// * `Char` — The decoded character type.  May be an enum with variants
///   for printable characters, control codes, and substitutions.
///
/// # Example
///
/// ```ignore
/// struct MyProvider;
///
/// impl TextProvider for MyProvider {
///     type Char = MyChar;
///     // ...
/// }
/// ```
pub trait TextProvider {
    /// The decoded character type produced by this provider.
    type Char: Clone + core::fmt::Debug;

    /// Decodes a single byte into an optional character.
    ///
    /// Returns `None` if the byte is not recognised by this charmap.
    fn decode_byte(&self, byte: u8) -> Option<Self::Char>;

    /// Decodes a byte slice into a [`TextStream`].
    ///
    /// The default implementation calls [`decode_byte`] for each byte,
    /// collecting all `Some` results.  Override for multi-byte encodings.
    fn decode_stream(&self, bytes: &[u8]) -> TextStream<Self::Char> {
        let chars: Vec<Self::Char> = bytes.iter().filter_map(|&b| self.decode_byte(b)).collect();
        TextStream::new(chars)
    }

    /// Draws a single character into the tile buffer.
    ///
    /// Implementations should write the character's tile at the buffer's
    /// current cursor position and advance the cursor by the appropriate
    /// amount (typically one tile for fixed-width fonts).
    fn render_char(&self, c: &Self::Char, buffer: &mut TileBuffer);

    /// Returns the pixel width of the given text when rendered.
    ///
    /// Used for text layout and centering calculations.
    fn string_width(&self, text: &[Self::Char]) -> u16;

    /// Returns `true` if the character is a control code (not printable).
    fn is_control_code(&self, c: &Self::Char) -> bool;

    /// Processes a control code and returns the resulting action.
    ///
    /// The `state` parameter allows the provider to inspect or modify
    /// the dialog state (e.g. to track page breaks).
    fn process_control(&self, c: &Self::Char, state: &mut DialogState) -> ControlAction;
}

// ── DialogEngine ───────────────────────────────────────────────────

/// A general-purpose dialog engine that drives text display one character
/// per frame.
///
/// `P` is the [`TextProvider`] implementation that supplies the game's
/// character encoding and rendering logic.
///
/// # Usage
///
/// ```ignore
/// let provider = MyProvider::new();
/// let mut engine = DialogEngine::new(provider);
/// let mut buffer = TileBuffer::new(20, 18);
///
/// engine.open_dialog(&[0x48, 0x45, 0x4C, 0x4C, 0x4F]); // "HELLO"
///
/// while engine.is_active() {
///     engine.update(&mut buffer); // one char per frame
/// }
///
/// engine.advance(); // close dialog
/// ```
pub struct DialogEngine<P: TextProvider> {
    /// The text provider (charmap, rendering, control codes).
    pub provider: P,
    /// Active character stream, or `None` if no dialog is open.
    pub stream: Option<TextStream<P::Char>>,
    /// Current dialog state.
    pub state: DialogState,
}

impl<P: TextProvider> DialogEngine<P> {
    /// Creates a new dialog engine with the given text provider.
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            stream: None,
            state: DialogState::default(),
        }
    }

    /// Opens a dialog with the given raw byte text.
    ///
    /// The bytes are decoded via the provider's [`TextProvider::decode_stream`]
    /// and the state is reset to the initial typing mode.
    pub fn open_dialog(&mut self, text: &[u8]) {
        self.stream = Some(self.provider.decode_stream(text));
        self.state = DialogState::default();
    }

    /// Processes one character from the current dialog stream.
    ///
    /// Call this once per frame to achieve the classic typewriter effect.
    /// Control codes are dispatched to the provider's
    /// [`TextProvider::process_control`]; printable characters are drawn
    /// via [`TextProvider::render_char`].
    ///
    /// If the engine is not in [`DialogMode::Typing`], this call is a no-op.
    /// If the stream is exhausted or a `Done` control action is returned,
    /// the dialog state transitions to [`DialogMode::Done`].
    pub fn update(&mut self, buffer: &mut TileBuffer) {
        if self.state.mode != DialogMode::Typing {
            return;
        }

        let stream = match &mut self.stream {
            Some(s) => s,
            None => return,
        };

        let c = match stream.next() {
            Some(c) => c.clone(),
            None => {
                self.state.mode = DialogMode::Done;
                return;
            }
        };

        if self.provider.is_control_code(&c) {
            let action = self.provider.process_control(&c, &mut self.state);
            match action {
                ControlAction::Newline => buffer.newline(),
                ControlAction::Done => {
                    self.state.mode = DialogMode::Done;
                }
                ControlAction::Clear => buffer.clear(),
                ControlAction::PageBreak => {
                    self.state.mode = DialogMode::WaitingForInput;
                }
                ControlAction::WaitInput => {
                    self.state.mode = DialogMode::WaitingForInput;
                }
                ControlAction::MoveCursor { x, y } => {
                    buffer.cursor = TilePos::new(x, y);
                }
                ControlAction::Scroll => buffer.scroll(),
                ControlAction::Pause(_frames) => {
                    self.state.mode = DialogMode::Paused;
                }
                _ => {}
            }
        } else {
            self.provider.render_char(&c, buffer);
        }

        // Check if we exhausted the stream in this update.
        // Don't override waiting/paused states — let advance() handle completion.
        if stream.is_at_end() && self.state.mode == DialogMode::Typing {
            self.state.mode = DialogMode::Done;
        }
    }

    /// Advances the dialog past the current page or closes it.
    ///
    /// - If paused, resumes typing.
    /// - If waiting for input or scrolling, resumes typing.
    /// - If the dialog is done, clears the stream and resets state
    ///   (so [`is_active`] returns `false`).
    ///
    /// Call this in response to the player pressing the A button.
    pub fn advance(&mut self) {
        match self.state.mode {
            DialogMode::Paused => {
                self.state.mode = DialogMode::Typing;
            }
            DialogMode::WaitingForInput | DialogMode::Scrolling => {
                self.state.mode = DialogMode::Typing;
            }
            DialogMode::Done => {
                self.stream = None;
                self.state = DialogState::default();
            }
            _ => {}
        }
    }

    /// Returns `true` if a dialog is currently active.
    ///
    /// A dialog is active when a stream is loaded and the mode is not
    /// [`DialogMode::Done`].
    pub fn is_active(&self) -> bool {
        self.stream.is_some() && self.state.mode != DialogMode::Done
    }
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Mock types ────────────────────────────────────────────────

    /// Mock character type for testing with a simple ASCII-like encoding.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockChar {
        /// Printable ASCII character (0x41-0x5A → 'A'-'Z').
        Ascii(char),
        /// Digit (0x00-0x09 → '0'-'9').
        Digit(u8),
        /// Move to next line (0xFE).
        Newline,
        /// End of dialog (0xFF).
        Done,
    }

    /// A [`TextProvider`] that uses a minimal ASCII-like charmap.
    ///
    /// Mapping:
    ///
    /// | Byte range | MockChar            |
    /// |-----------|---------------------|
    /// | 0x00-0x09 | `Digit(d)`          |
    /// | 0x41-0x5A | `Ascii(c)`          |
    /// | 0xFE      | `Newline`           |
    /// | 0xFF      | `Done`              |
    struct MockAsciiProvider;

    impl TextProvider for MockAsciiProvider {
        type Char = MockChar;

        fn decode_byte(&self, byte: u8) -> Option<Self::Char> {
            match byte {
                b @ 0x00..=0x09 => Some(MockChar::Digit(b)),
                0x41..=0x5A => Some(MockChar::Ascii(byte as char)),
                0xFE => Some(MockChar::Newline),
                0xFF => Some(MockChar::Done),
                _ => None,
            }
        }

        fn render_char(&self, c: &Self::Char, buffer: &mut TileBuffer) {
            let tile_id = match c {
                MockChar::Ascii(ch) => *ch as u16,
                MockChar::Digit(d) => (b'0' + d) as u16,
                // Control codes should not reach `render_char`.
                MockChar::Newline | MockChar::Done => return,
            };
            let pos = buffer.cursor;
            buffer.set_tile(pos, tile_id, 0);
            buffer.cursor.x += 1;
        }

        fn string_width(&self, text: &[Self::Char]) -> u16 {
            // Fixed-width: 8 pixels per character.
            text.len() as u16 * 8
        }

        fn is_control_code(&self, c: &Self::Char) -> bool {
            matches!(c, MockChar::Newline | MockChar::Done)
        }

        fn process_control(&self, c: &Self::Char, _state: &mut DialogState) -> ControlAction {
            match c {
                MockChar::Newline => ControlAction::Newline,
                MockChar::Done => ControlAction::Done,
                _ => ControlAction::None,
            }
        }
    }

    // ── Tests ─────────────────────────────────────────────────────

    #[test]
    fn test_decode_hello_world() {
        let provider = MockAsciiProvider;
        let bytes = [0x48, 0x45, 0x4C, 0x4C, 0x4F]; // HELLO
        let stream = provider.decode_stream(&bytes);
        assert_eq!(stream.chars.len(), 5);
        assert_eq!(stream.chars[0], MockChar::Ascii('H'));
        assert_eq!(stream.chars[1], MockChar::Ascii('E'));
        assert_eq!(stream.chars[2], MockChar::Ascii('L'));
        assert_eq!(stream.chars[3], MockChar::Ascii('L'));
        assert_eq!(stream.chars[4], MockChar::Ascii('O'));
    }

    #[test]
    fn test_decode_done_control() {
        let provider = MockAsciiProvider;
        assert_eq!(provider.decode_byte(0xFF), Some(MockChar::Done));
        assert!(provider.is_control_code(&MockChar::Done));
    }

    #[test]
    fn test_dialog_engine_hello() {
        let provider = MockAsciiProvider;
        let mut engine = DialogEngine::new(provider);
        let mut buffer = TileBuffer::new(20, 18);

        // "HELLO" + DONE
        engine.open_dialog(&[0x48, 0x45, 0x4C, 0x4C, 0x4F, 0xFF]);

        assert!(engine.is_active());

        // Process all 6 characters
        for _ in 0..6 {
            engine.update(&mut buffer);
        }

        // Verify H, E, L, L, O at positions (0,0)-(4,0)
        let tiles = &buffer.tiles;
        assert_eq!(tiles[0].tile_id, b'H' as u16);
        assert_eq!(tiles[1].tile_id, b'E' as u16);
        assert_eq!(tiles[2].tile_id, b'L' as u16);
        assert_eq!(tiles[3].tile_id, b'L' as u16);
        assert_eq!(tiles[4].tile_id, b'O' as u16);

        // Position (5,0) should still be default
        assert_eq!(tiles[5].tile_id, 0);
    }

    #[test]
    fn test_dialog_engine_done_deactivates() {
        let provider = MockAsciiProvider;
        let mut engine = DialogEngine::new(provider);
        let mut buffer = TileBuffer::new(20, 18);

        engine.open_dialog(&[0x48, 0xFF]); // "H" + DONE

        engine.update(&mut buffer); // 'H'
        engine.update(&mut buffer); // DONE
        assert!(!engine.is_active());
    }

    #[test]
    fn test_advance_after_done() {
        let provider = MockAsciiProvider;
        let mut engine = DialogEngine::new(provider);
        let mut buffer = TileBuffer::new(20, 18);

        engine.open_dialog(&[0x48, 0xFF]); // "H" + DONE

        engine.update(&mut buffer); // 'H'
        engine.update(&mut buffer); // DONE

        // After DONE, engine should not be active
        assert!(!engine.is_active());
        assert_eq!(engine.stream.is_some(), true); // stream still there

        // advance() should clear the stream
        engine.advance();
        assert!(!engine.is_active());
        assert!(engine.stream.is_none());
    }

    #[test]
    fn test_tile_buffer_clear() {
        let mut buffer = TileBuffer::new(20, 18);
        buffer.set_tile(TilePos::new(5, 5), 0x42, 1);

        assert_eq!(buffer.tiles[buffer.index(5, 5)].tile_id, 0x42);
        buffer.clear();
        assert_eq!(buffer.tiles[buffer.index(5, 5)].tile_id, 0);
        assert_eq!(buffer.cursor, TilePos::new(0, 0));
    }

    #[test]
    fn test_tile_buffer_newline() {
        let mut buffer = TileBuffer::new(20, 18);
        buffer.cursor = TilePos::new(12, 5);

        buffer.newline();
        assert_eq!(buffer.cursor.x, 0);
        assert_eq!(buffer.cursor.y, 6);
    }

    #[test]
    fn test_tile_buffer_scroll() {
        let mut buffer = TileBuffer::new(20, 18);

        // Put a marker in row 1
        buffer.set_tile(TilePos::new(0, 1), 0xAB, 0);
        // Put a marker in row 0
        buffer.set_tile(TilePos::new(0, 0), 0xCD, 0);

        buffer.scroll();

        // Row 0 was discarded, row 1 moved up
        assert_eq!(buffer.tiles[buffer.index(0, 0)].tile_id, 0xAB);
        // Bottom row should be default
        assert_eq!(
            buffer.tiles[buffer.index(0, buffer.height_tiles - 1)].tile_id,
            0
        );
    }

    #[test]
    fn test_text_stream_operations() {
        let mut stream = TextStream::new(vec![1, 2, 3, 4, 5]);
        assert_eq!(stream.peek(), Some(&1));
        assert_eq!(stream.next(), Some(&1));
        assert_eq!(stream.next(), Some(&2));
        assert_eq!(stream.position(), 2);

        stream.skip(1); // skip 3
        assert_eq!(stream.peek(), Some(&4));
        assert_eq!(stream.remaining(), &[4, 5]);

        assert_eq!(stream.next(), Some(&4));
        assert_eq!(stream.next(), Some(&5));
        assert!(stream.is_at_end());
        assert_eq!(stream.next(), None);
    }

    #[test]
    fn test_control_action_equality() {
        assert_eq!(ControlAction::None, ControlAction::None);
        assert_ne!(ControlAction::Newline, ControlAction::Done);
        assert_eq!(ControlAction::SetSpeed(3), ControlAction::SetSpeed(3));
        assert_ne!(ControlAction::SetSpeed(1), ControlAction::SetSpeed(2));
        assert_eq!(
            ControlAction::MoveCursor { x: 5, y: 3 },
            ControlAction::MoveCursor { x: 5, y: 3 }
        );
        assert_ne!(
            ControlAction::MoveCursor { x: 1, y: 1 },
            ControlAction::MoveCursor { x: 2, y: 2 }
        );
        assert_eq!(ControlAction::Scroll, ControlAction::Scroll);
        assert_eq!(ControlAction::Pause(30), ControlAction::Pause(30));
        assert_ne!(ControlAction::Pause(10), ControlAction::Pause(30));
    }

    #[test]
    fn test_dialog_state_default() {
        let state = DialogState::default();
        assert_eq!(state.mode, DialogMode::Typing);
        assert_eq!(state.page_index, 0);
        assert_eq!(state.char_index, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_string_width() {
        let provider = MockAsciiProvider;
        let chars = vec![MockChar::Ascii('H'), MockChar::Ascii('I')];
        assert_eq!(provider.string_width(&chars), 16);
        assert_eq!(provider.string_width(&[]), 0);
    }

    #[test]
    fn test_decode_digit() {
        let provider = MockAsciiProvider;
        assert_eq!(provider.decode_byte(0x00), Some(MockChar::Digit(0)));
        assert_eq!(provider.decode_byte(0x05), Some(MockChar::Digit(5)));
        assert_eq!(provider.decode_byte(0x09), Some(MockChar::Digit(9)));
    }
}
