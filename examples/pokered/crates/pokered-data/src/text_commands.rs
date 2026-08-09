/// TextCommandJumpTable indexes (0x00-0x17) + TX_END (0x50).
/// See home/text.asm and macros/scripts/text.asm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextCommand {
    /// $00 — TX_START: Start writing text. Followed by 2-byte pointer to text data.
    TxStart = 0x00,
    /// $01 — TX_RAM: Print text from RAM address. Followed by 2-byte address.
    TxRam = 0x01,
    /// $02 — TX_BCD: Print BCD number. Followed by 2-byte address + 1-byte flags (size|digits).
    TxBcd = 0x02,
    /// $03 — TX_MOVE: Move text write location. Followed by 2-byte tilemap address.
    TxMove = 0x03,
    /// $04 — TX_BOX: Draw a text box. Followed by 2-byte address + 1-byte height + 1-byte width.
    TxBox = 0x04,
    /// $05 — TX_LOW: Move cursor to the bottom text line (coord 1,16).
    TxLow = 0x05,
    /// $06 — TX_PROMPT_BUTTON: Show ▼ arrow and wait for button press.
    TxPromptButton = 0x06,
    /// $07 — TX_SCROLL: Scroll text up one line.
    TxScroll = 0x07,
    /// $08 — TX_START_ASM: Start executing inline assembly (return from text engine).
    TxStartAsm = 0x08,
    /// $09 — TX_NUM: Print big-endian decimal number. Followed by 2-byte addr + 1-byte (bytes<<4|digits).
    TxNum = 0x09,
    /// $0A — TX_PAUSE: Pause text for 30 frames.
    TxPause = 0x0A,
    /// $0B — TX_SOUND_GET_ITEM_1: Play "got item" sound effect. Also used as TX_SOUND_LEVEL_UP.
    TxSoundGetItem1 = 0x0B,
    /// $0C — TX_DOTS: Print N dots (ellipsis). Followed by 1-byte count.
    TxDots = 0x0C,
    /// $0D — TX_WAIT_BUTTON: Wait for button press (no arrow shown).
    TxWaitButton = 0x0D,
    /// $0E — TX_SOUND_POKEDEX_RATING: Play Pokédex rating sound.
    TxSoundPokedexRating = 0x0E,
    /// $0F — TX_SOUND_GET_ITEM_1_DUPLICATE: Duplicate of "got item" sound.
    TxSoundGetItem1Duplicate = 0x0F,
    /// $10 — TX_SOUND_GET_ITEM_2: Play alternate "got item" sound.
    TxSoundGetItem2 = 0x10,
    /// $11 — TX_SOUND_GET_KEY_ITEM: Play "got key item" sound.
    TxSoundGetKeyItem = 0x11,
    /// $12 — TX_SOUND_CAUGHT_MON: Play "caught Pokémon" sound.
    TxSoundCaughtMon = 0x12,
    /// $13 — TX_SOUND_DEX_PAGE_ADDED: Play "dex page added" sound.
    TxSoundDexPageAdded = 0x13,
    /// $14 — TX_SOUND_CRY_NIDORINA: Play Nidorina cry.
    TxSoundCryNidorina = 0x14,
    /// $15 — TX_SOUND_CRY_PIDGEOT: Play Pidgeot cry.
    TxSoundCryPidgeot = 0x15,
    /// $16 — TX_SOUND_CRY_DEWGONG: Play Dewgong cry.
    TxSoundCryDewgong = 0x16,
    /// $17 — TX_FAR: Load text from another bank. Followed by 3-byte far pointer (addr + bank).
    TxFar = 0x17,
}

/// TX_END is the text terminator, value 0x50, separate from the 0x00-0x17 range.
pub const TX_END: u8 = 0x50;

/// Alias: TX_SOUND_LEVEL_UP is the same command as TX_SOUND_GET_ITEM_1.
pub const TX_SOUND_LEVEL_UP: u8 = TextCommand::TxSoundGetItem1 as u8;

/// Number of entries in TextCommandJumpTable (0x00 through 0x17).
pub const TEXT_COMMAND_COUNT: usize = 24;

impl TextCommand {
    /// Convert a byte to a TextCommand.
    /// Returns None for bytes outside 0x00-0x17 (use TX_END constant for 0x50).
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::TxStart),
            0x01 => Some(Self::TxRam),
            0x02 => Some(Self::TxBcd),
            0x03 => Some(Self::TxMove),
            0x04 => Some(Self::TxBox),
            0x05 => Some(Self::TxLow),
            0x06 => Some(Self::TxPromptButton),
            0x07 => Some(Self::TxScroll),
            0x08 => Some(Self::TxStartAsm),
            0x09 => Some(Self::TxNum),
            0x0A => Some(Self::TxPause),
            0x0B => Some(Self::TxSoundGetItem1),
            0x0C => Some(Self::TxDots),
            0x0D => Some(Self::TxWaitButton),
            0x0E => Some(Self::TxSoundPokedexRating),
            0x0F => Some(Self::TxSoundGetItem1Duplicate),
            0x10 => Some(Self::TxSoundGetItem2),
            0x11 => Some(Self::TxSoundGetKeyItem),
            0x12 => Some(Self::TxSoundCaughtMon),
            0x13 => Some(Self::TxSoundDexPageAdded),
            0x14 => Some(Self::TxSoundCryNidorina),
            0x15 => Some(Self::TxSoundCryPidgeot),
            0x16 => Some(Self::TxSoundCryDewgong),
            0x17 => Some(Self::TxFar),
            _ => None,
        }
    }

    /// Convert a TextCommand to its byte value.
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Returns true if this command is a sound effect command (0x0B-0x16).
    pub fn is_sound_command(self) -> bool {
        let b = self as u8;
        (0x0B..=0x16).contains(&b)
    }

    /// Returns the number of parameter bytes following this command.
    /// Used by the text processor to know how many bytes to read after the command byte.
    pub fn param_byte_count(self) -> usize {
        match self {
            Self::TxStart => 2, // 2-byte pointer to text
            Self::TxRam => 2,   // 2-byte RAM address
            Self::TxBcd => 3,   // 2-byte address + 1-byte flags
            Self::TxMove => 2,  // 2-byte tilemap address
            Self::TxBox => 4,   // 2-byte address + height + width
            Self::TxLow => 0,
            Self::TxPromptButton => 0,
            Self::TxScroll => 0,
            Self::TxStartAsm => 0,
            Self::TxNum => 3, // 2-byte address + 1-byte (bytes<<4|digits)
            Self::TxPause => 0,
            Self::TxSoundGetItem1 => 0,
            Self::TxDots => 1, // 1-byte count
            Self::TxWaitButton => 0,
            Self::TxSoundPokedexRating => 0,
            Self::TxSoundGetItem1Duplicate => 0,
            Self::TxSoundGetItem2 => 0,
            Self::TxSoundGetKeyItem => 0,
            Self::TxSoundCaughtMon => 0,
            Self::TxSoundDexPageAdded => 0,
            Self::TxSoundCryNidorina => 0,
            Self::TxSoundCryPidgeot => 0,
            Self::TxSoundCryDewgong => 0,
            Self::TxFar => 3, // 2-byte address + 1-byte bank
        }
    }

    /// Returns a human-readable name for the command.
    pub fn name(self) -> &'static str {
        match self {
            Self::TxStart => "TX_START",
            Self::TxRam => "TX_RAM",
            Self::TxBcd => "TX_BCD",
            Self::TxMove => "TX_MOVE",
            Self::TxBox => "TX_BOX",
            Self::TxLow => "TX_LOW",
            Self::TxPromptButton => "TX_PROMPT_BUTTON",
            Self::TxScroll => "TX_SCROLL",
            Self::TxStartAsm => "TX_START_ASM",
            Self::TxNum => "TX_NUM",
            Self::TxPause => "TX_PAUSE",
            Self::TxSoundGetItem1 => "TX_SOUND_GET_ITEM_1",
            Self::TxDots => "TX_DOTS",
            Self::TxWaitButton => "TX_WAIT_BUTTON",
            Self::TxSoundPokedexRating => "TX_SOUND_POKEDEX_RATING",
            Self::TxSoundGetItem1Duplicate => "TX_SOUND_GET_ITEM_1_DUPLICATE",
            Self::TxSoundGetItem2 => "TX_SOUND_GET_ITEM_2",
            Self::TxSoundGetKeyItem => "TX_SOUND_GET_KEY_ITEM",
            Self::TxSoundCaughtMon => "TX_SOUND_CAUGHT_MON",
            Self::TxSoundDexPageAdded => "TX_SOUND_DEX_PAGE_ADDED",
            Self::TxSoundCryNidorina => "TX_SOUND_CRY_NIDORINA",
            Self::TxSoundCryPidgeot => "TX_SOUND_CRY_PIDGEOT",
            Self::TxSoundCryDewgong => "TX_SOUND_CRY_DEWGONG",
            Self::TxFar => "TX_FAR",
        }
    }
}

/// Inline control character bytes used by PlaceString (home/text.asm).
/// These are NOT TextCommand values — they are processed inline during
/// character-by-character text rendering, not by TextCommandProcessor.
pub mod inline_control_chars {
    /// Move to next line in text box (coord same_x, y+1)
    pub const NEXT: u8 = 0x4E;
    /// Move to bottom text line (coord 1, 16)
    pub const LINE: u8 = 0x4F;
    /// Terminator — same as TX_END
    pub const TEXT_END_MARKER: u8 = 0x50;
    /// New paragraph — wait for button, clear text area, move to coord(1,14)
    pub const PARA: u8 = 0x51;
    /// Insert player's name
    pub const PLAYER: u8 = 0x52;
    /// Insert rival's name
    pub const RIVAL: u8 = 0x53;
    /// POKé (currency symbol)
    pub const POKE: u8 = 0x54;
    /// Scroll text up and continue — wait for button, scroll, move to coord(1,16)
    pub const CONT: u8 = 0x55;
    /// End of Pokédex entry — followed by @ terminator
    pub const DEXEND: u8 = 0x56;
    /// End text, mark as done
    pub const DONE: u8 = 0x57;
    /// Show ▼, wait for button, then end text
    pub const PROMPT: u8 = 0x58;
    /// Target Pokémon name (in battle context)
    pub const TARGET: u8 = 0x59;
    /// User Pokémon name (in battle context)
    pub const USER: u8 = 0x5A;
    /// Start a new Pokédex page — wait for button, clear large area, move to coord(1,11)
    pub const PAGE: u8 = 0x49;

    /// Returns true if the byte is an inline control character (not a printable char).
    pub fn is_inline_control(byte: u8) -> bool {
        matches!(
            byte,
            PAGE | NEXT | LINE | TEXT_END_MARKER..=PROMPT | TARGET | USER
        )
    }

    /// Returns a human-readable name for inline control characters.
    pub fn name(byte: u8) -> Option<&'static str> {
        match byte {
            PAGE => Some("<PAGE>"),
            NEXT => Some("<NEXT>"),
            LINE => Some("<LINE>"),
            TEXT_END_MARKER => Some("@"),
            PARA => Some("<PARA>"),
            PLAYER => Some("<PLAYER>"),
            RIVAL => Some("<RIVAL>"),
            POKE => Some("<POKé>"),
            CONT => Some("<CONT>"),
            DEXEND => Some("<DEXEND>"),
            DONE => Some("<DONE>"),
            PROMPT => Some("<PROMPT>"),
            TARGET => Some("<TARGET>"),
            USER => Some("<USER>"),
            _ => None,
        }
    }
}

/// Check if a byte is a text command (0x00-0x17) that should be processed
/// by TextCommandProcessor rather than placed as a character.
pub fn is_text_command(byte: u8) -> bool {
    byte <= 0x17
}

/// Check if a byte is the text terminator.
pub fn is_text_end(byte: u8) -> bool {
    byte == TX_END
}
