use jrpg_engine::text::{ControlAction, DialogState, TextProvider, TextStream, TileBuffer};
use pokered_data::{charmap, text_commands::TextCommand};

use super::NameBuffers;

// ── PokemonChar ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PokemonChar {
    /// Printable character tile (byte value = tile ID).
    Tile(u8),
    /// Player name substitution.
    PlayerName,
    /// Rival name substitution.
    RivalName,
    /// POKé insertion.
    Poke,
    /// Next line (move cursor down one row).
    NextLine,
    /// Move cursor to bottom text line (1, 16).
    BottomLine,
    /// Paragraph break (wait for button, clear).
    Para,
    /// Page break (wait for button, clear large area).
    Page,
    /// Scroll and continue.
    Cont,
    /// Show arrow and wait for button.
    Prompt,
    /// End of text.
    Done,
    /// Terminator char (@).
    Terminator,
    /// Target/User name placeholder (handled by battle system).
    Target,
    User,
    /// End of Pokédex entry.
    DexEnd,
    /// Text command with pre-decoded parameters.
    Command(TextCommandData),
}

#[derive(Debug, Clone)]
pub enum TextCommandData {
    /// TX_WAIT_BUTTON - wait for button press.
    WaitButton,
    /// TX_PAUSE - pause for N frames.
    Pause(u8),
    /// TX_SCROLL - scroll text up.
    Scroll,
    /// TX_PROMPT_BUTTON - show arrow and wait.
    PromptButton,
    /// TX_DOTS - print N dots.
    Dots(u8),
    /// TX_LOW - move cursor to bottom line.
    Low,
    /// TX_MOVE - move cursor to tilemap address.
    Move { addr: u16 },
    /// TX_START_ASM - start inline assembly.
    StartAsm,
    /// Other text commands (TX_START, TX_RAM, TX_BCD, TX_BOX, TX_NUM, TX_FAR, sound commands).
    /// Parameters are consumed during stream decoding but produce no visible output.
    Skipped { cmd: u8 },
}

// ── PokemonTextProvider ─────────────────────────────────────────────

pub struct PokemonTextProvider {
    pub player_name: Vec<u8>,
    pub rival_name: Vec<u8>,
}

impl PokemonTextProvider {
    pub fn new(player_name: Vec<u8>, rival_name: Vec<u8>) -> Self {
        Self {
            player_name,
            rival_name,
        }
    }
}

impl Default for PokemonTextProvider {
    fn default() -> Self {
        let names = NameBuffers::default();
        Self {
            player_name: names.player_name.to_vec(),
            rival_name: names.rival_name.to_vec(),
        }
    }
}

impl TextProvider for PokemonTextProvider {
    type Char = PokemonChar;

    fn decode_byte(&self, byte: u8) -> Option<Self::Char> {
        if byte == charmap::control_chars::CHAR_TERMINATOR || byte == pokered_data::text_commands::TX_END {
            return Some(PokemonChar::Terminator);
        }

        if TextCommand::from_byte(byte).is_some() {
            // Text commands need parameter handling in decode_stream
            return None;
        }

        match byte {
            0x49 => Some(PokemonChar::Page),
            0x4E => Some(PokemonChar::NextLine),
            0x4F => Some(PokemonChar::BottomLine),
            0x51 => Some(PokemonChar::Para),
            0x52 => Some(PokemonChar::PlayerName),
            0x53 => Some(PokemonChar::RivalName),
            0x54 => Some(PokemonChar::Poke),
            0x55 => Some(PokemonChar::Cont),
            0x57 => Some(PokemonChar::Done),
            0x58 => Some(PokemonChar::Prompt),
            0x59 => Some(PokemonChar::Target),
            0x5A => Some(PokemonChar::User),
            0x56 => Some(PokemonChar::DexEnd),
            // Control characters: PKMN(0x4A), _CONT(0x4B), SCROLL(0x4C)
            0x4A | 0x4B | 0x4C | 0x5B | 0x5C | 0x5D | 0x5E | 0x5F => {
                // These are control codes that produce empty output or are unused
                Some(PokemonChar::Tile(byte))
            }
            // Printable characters (0x60-0xFF)
            _ => {
                if charmap::decode_char(byte).is_some() {
                    Some(PokemonChar::Tile(byte))
                } else {
                    // Bytes without a charmap mapping are still valid tiles
                    Some(PokemonChar::Tile(byte))
                }
            }
        }
    }

    fn decode_stream(&self, bytes: &[u8]) -> TextStream<Self::Char> {
        let mut chars = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];

            // Text commands (0x00-0x17) — read parameters
            if let Some(cmd) = TextCommand::from_byte(byte) {
                let param_count = cmd.param_byte_count();
                i += 1; // consume command byte

                match cmd {
                    TextCommand::TxWaitButton => {
                        chars.push(PokemonChar::Command(TextCommandData::WaitButton));
                    }
                    TextCommand::TxPause => {
                        chars.push(PokemonChar::Command(TextCommandData::Pause(30)));
                    }
                    TextCommand::TxScroll => {
                        chars.push(PokemonChar::Command(TextCommandData::Scroll));
                    }
                    TextCommand::TxPromptButton => {
                        chars.push(PokemonChar::Command(TextCommandData::PromptButton));
                    }
                    TextCommand::TxDots => {
                        let count = if i < bytes.len() { bytes[i] } else { 0 };
                        chars.push(PokemonChar::Command(TextCommandData::Dots(count)));
                    }
                    TextCommand::TxLow => {
                        chars.push(PokemonChar::Command(TextCommandData::Low));
                    }
                    TextCommand::TxMove => {
                        let addr = if i + 1 < bytes.len() {
                            u16::from_le_bytes([bytes[i], bytes[i + 1]])
                        } else {
                            0
                        };
                        chars.push(PokemonChar::Command(TextCommandData::Move { addr }));
                    }
                    TextCommand::TxStartAsm => {
                        chars.push(PokemonChar::Command(TextCommandData::StartAsm));
                    }
                    _ => {
                        // TX_START, TX_RAM, TX_BCD, TX_BOX, TX_NUM, TX_FAR, sound commands
                        chars.push(PokemonChar::Command(TextCommandData::Skipped {
                            cmd: byte,
                        }));
                    }
                }

                i += param_count;
                if i > bytes.len() {
                    i = bytes.len();
                }
                continue;
            }

            // Regular byte — delegate to decode_byte
            if let Some(c) = self.decode_byte(byte) {
                chars.push(c);
            }
            i += 1;
        }

        TextStream::new(chars)
    }

    fn render_char(&self, c: &Self::Char, buffer: &mut TileBuffer) {
        match c {
            PokemonChar::Tile(byte) => {
                let pos = buffer.cursor;
                buffer.set_tile(pos, *byte as u16, 0);
                buffer.cursor.x += 1;
            }
            PokemonChar::PlayerName => {
                for &b in &self.player_name {
                    if b == charmap::CHAR_TERMINATOR {
                        break;
                    }
                    let pos = buffer.cursor;
                    buffer.set_tile(pos, b as u16, 0);
                    buffer.cursor.x += 1;
                }
            }
            PokemonChar::RivalName => {
                for &b in &self.rival_name {
                    if b == charmap::CHAR_TERMINATOR {
                        break;
                    }
                    let pos = buffer.cursor;
                    buffer.set_tile(pos, b as u16, 0);
                    buffer.cursor.x += 1;
                }
            }
            PokemonChar::Poke => {
                let poke_text = charmap::encode_string("POKé").unwrap_or_default();
                for &b in &poke_text[..poke_text.len().saturating_sub(1)] {
                    let pos = buffer.cursor;
                    buffer.set_tile(pos, b as u16, 0);
                    buffer.cursor.x += 1;
                }
            }
            // Control codes that insert text are handled above.
            // Positional and state-changing control codes are handled by process_control.
            _ => {}
        }
    }

    fn string_width(&self, text: &[Self::Char]) -> u16 {
        // Fixed-width font: 8 pixels per character.
        // For name substitutions, count their actual length.
        let mut total = 0u16;
        for c in text {
            match c {
                PokemonChar::PlayerName => {
                    total += self.player_name.iter()
                        .take_while(|&&b| b != charmap::CHAR_TERMINATOR)
                        .count() as u16 * 8;
                }
                PokemonChar::RivalName => {
                    total += self.rival_name.iter()
                        .take_while(|&&b| b != charmap::CHAR_TERMINATOR)
                        .count() as u16 * 8;
                }
                PokemonChar::Poke => {
                    total += 4 * 8; // "POKé" = 4 chars
                }
                PokemonChar::Tile(_) => {
                    total += 8;
                }
                _ => {} // Controls don't add width
            }
        }
        total
    }

    fn is_control_code(&self, c: &Self::Char) -> bool {
        match c {
            PokemonChar::Tile(_)
            | PokemonChar::PlayerName
            | PokemonChar::RivalName
            | PokemonChar::Poke => false,
            _ => true,
        }
    }

    fn process_control(&self, c: &Self::Char, _state: &mut DialogState) -> ControlAction {
        match c {
            PokemonChar::Tile(_) => ControlAction::None,
            PokemonChar::PlayerName | PokemonChar::RivalName | PokemonChar::Poke => {
                ControlAction::None
            }
            PokemonChar::NextLine => ControlAction::Newline,
            PokemonChar::BottomLine => ControlAction::MoveCursor { x: 1, y: 16 },
            PokemonChar::Para => ControlAction::WaitInput,
            PokemonChar::Page => ControlAction::PageBreak,
            PokemonChar::Cont => ControlAction::WaitInput,
            PokemonChar::Prompt => ControlAction::WaitInput,
            PokemonChar::Done => ControlAction::Done,
            PokemonChar::Terminator => ControlAction::Done,
            PokemonChar::Target | PokemonChar::User => ControlAction::None,
            PokemonChar::DexEnd => ControlAction::Done,
            PokemonChar::Command(cmd) => match cmd {
                TextCommandData::WaitButton => ControlAction::WaitInput,
                TextCommandData::Pause(frames) => ControlAction::Pause(*frames),
                TextCommandData::Scroll => ControlAction::Scroll,
                TextCommandData::PromptButton => ControlAction::WaitInput,
                TextCommandData::Dots(_count) => {
                    ControlAction::None
                }
                TextCommandData::Low => ControlAction::MoveCursor { x: 1, y: 16 },
                TextCommandData::Move { addr } => {
                    let tilemap_offset = addr.wrapping_sub(0x9800) as usize;
                    let index = tilemap_offset.min(359);
                    let x = (index % 20) as u16;
                    let y = (index / 20) as u16;
                    ControlAction::MoveCursor { x, y }
                }
                TextCommandData::StartAsm => ControlAction::Done,
                TextCommandData::Skipped { .. } => ControlAction::None,
            },
        }
    }
}

impl PartialEq for PokemonChar {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PokemonChar::Tile(a), PokemonChar::Tile(b)) => a == b,
            (PokemonChar::PlayerName, PokemonChar::PlayerName) => true,
            (PokemonChar::RivalName, PokemonChar::RivalName) => true,
            (PokemonChar::Poke, PokemonChar::Poke) => true,
            (PokemonChar::NextLine, PokemonChar::NextLine) => true,
            (PokemonChar::BottomLine, PokemonChar::BottomLine) => true,
            (PokemonChar::Para, PokemonChar::Para) => true,
            (PokemonChar::Page, PokemonChar::Page) => true,
            (PokemonChar::Cont, PokemonChar::Cont) => true,
            (PokemonChar::Prompt, PokemonChar::Prompt) => true,
            (PokemonChar::Done, PokemonChar::Done) => true,
            (PokemonChar::Terminator, PokemonChar::Terminator) => true,
            (PokemonChar::Target, PokemonChar::Target) => true,
            (PokemonChar::User, PokemonChar::User) => true,
            (PokemonChar::DexEnd, PokemonChar::DexEnd) => true,
            (PokemonChar::Command(a), PokemonChar::Command(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialEq for TextCommandData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TextCommandData::WaitButton, TextCommandData::WaitButton) => true,
            (TextCommandData::Pause(a), TextCommandData::Pause(b)) => a == b,
            (TextCommandData::Scroll, TextCommandData::Scroll) => true,
            (TextCommandData::PromptButton, TextCommandData::PromptButton) => true,
            (TextCommandData::Dots(a), TextCommandData::Dots(b)) => a == b,
            (TextCommandData::Low, TextCommandData::Low) => true,
            (TextCommandData::Move { addr: a }, TextCommandData::Move { addr: b }) => a == b,
            (TextCommandData::StartAsm, TextCommandData::StartAsm) => true,
            (TextCommandData::Skipped { cmd: a }, TextCommandData::Skipped { cmd: b }) => a == b,
            _ => false,
        }
    }
}
