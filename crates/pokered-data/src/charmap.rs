//! Pokémon Red/Blue character encoding — NOT ASCII.
//!
//! The Game Boy uses a custom charmap where 'A' = 0x80, '0' = 0xF6,
//! '@' = 0x50 (string terminator), and various control codes occupy 0x00-0x5F.
//! See constants/charmap.asm in the original disassembly.

/// Control character byte values (used inline in text streams)
pub mod control_chars {
    pub const CHAR_NULL: u8 = 0x00;
    pub const CHAR_PAGE: u8 = 0x49;
    pub const CHAR_PKMN: u8 = 0x4A; // "<PK><MN>"
    pub const CHAR_CONT_INTERNAL: u8 = 0x4B; // implements "<CONT>"
    pub const CHAR_SCROLL: u8 = 0x4C;
    pub const CHAR_NEXT: u8 = 0x4E;
    pub const CHAR_LINE: u8 = 0x4F;
    pub const CHAR_TERMINATOR: u8 = 0x50; // '@'
    pub const CHAR_PARA: u8 = 0x51;
    pub const CHAR_PLAYER: u8 = 0x52;
    pub const CHAR_RIVAL: u8 = 0x53;
    pub const CHAR_POKE: u8 = 0x54; // '#' → "POKé"
    pub const CHAR_CONT: u8 = 0x55;
    pub const CHAR_SIX_DOTS: u8 = 0x56; // "……"
    pub const CHAR_DONE: u8 = 0x57;
    pub const CHAR_PROMPT: u8 = 0x58;
    pub const CHAR_TARGET: u8 = 0x59;
    pub const CHAR_USER: u8 = 0x5A;
    pub const CHAR_PC: u8 = 0x5B; // "PC"
    pub const CHAR_TM: u8 = 0x5C; // "TM"
    pub const CHAR_TRAINER: u8 = 0x5D; // "TRAINER"
    pub const CHAR_ROCKET: u8 = 0x5E; // "ROCKET"
    pub const CHAR_DEXEND: u8 = 0x5F;
}

/// A named control character with its byte value and display representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCharEntry {
    pub byte: u8,
    pub name: &'static str,
    pub display: &'static str,
}

/// All control characters, indexed for lookup.
pub const CONTROL_CHARS: [ControlCharEntry; 20] = [
    ControlCharEntry {
        byte: 0x00,
        name: "NULL",
        display: "",
    },
    ControlCharEntry {
        byte: 0x49,
        name: "PAGE",
        display: "",
    },
    ControlCharEntry {
        byte: 0x4A,
        name: "PKMN",
        display: "<PK><MN>",
    },
    ControlCharEntry {
        byte: 0x4B,
        name: "_CONT",
        display: "",
    },
    ControlCharEntry {
        byte: 0x4C,
        name: "SCROLL",
        display: "",
    },
    ControlCharEntry {
        byte: 0x4E,
        name: "NEXT",
        display: "\n",
    },
    ControlCharEntry {
        byte: 0x4F,
        name: "LINE",
        display: "\n",
    },
    ControlCharEntry {
        byte: 0x50,
        name: "END",
        display: "",
    },
    ControlCharEntry {
        byte: 0x51,
        name: "PARA",
        display: "\n\n",
    },
    ControlCharEntry {
        byte: 0x52,
        name: "PLAYER",
        display: "<PLAYER>",
    },
    ControlCharEntry {
        byte: 0x53,
        name: "RIVAL",
        display: "<RIVAL>",
    },
    ControlCharEntry {
        byte: 0x54,
        name: "POKe",
        display: "POKé",
    },
    ControlCharEntry {
        byte: 0x55,
        name: "CONT",
        display: "",
    },
    ControlCharEntry {
        byte: 0x56,
        name: "SIX_DOTS",
        display: "……",
    },
    ControlCharEntry {
        byte: 0x57,
        name: "DONE",
        display: "",
    },
    ControlCharEntry {
        byte: 0x58,
        name: "PROMPT",
        display: "",
    },
    ControlCharEntry {
        byte: 0x59,
        name: "TARGET",
        display: "<TARGET>",
    },
    ControlCharEntry {
        byte: 0x5A,
        name: "USER",
        display: "<USER>",
    },
    ControlCharEntry {
        byte: 0x5B,
        name: "PC",
        display: "PC",
    },
    ControlCharEntry {
        byte: 0x5C,
        name: "TM",
        display: "TM",
    },
];

/// Decode a single byte to a character string.
/// Returns None for control characters (they need special handling).
/// Returns the character as a &str for printable characters.
pub fn decode_char(byte: u8) -> Option<&'static str> {
    match byte {
        // Control codes — return None (caller handles them)
        0x00..=0x48 => None,
        0x49..=0x5F => None,

        // Bold/special font characters
        0x60 => Some("A"),        // BOLD_A (unused in English)
        0x61 => Some("B"),        // BOLD_B (unused)
        0x62 => Some("C"),        // BOLD_C (unused)
        0x63 => Some("D"),        // BOLD_D (unused)
        0x64 => Some("E"),        // BOLD_E (unused)
        0x65 => Some("F"),        // BOLD_F (unused)
        0x66 => Some("G"),        // BOLD_G (unused)
        0x67 => Some("H"),        // BOLD_H (unused)
        0x68 => Some("I"),        // BOLD_I (unused)
        0x69 => Some("V"),        // BOLD_V
        0x6A => Some("S"),        // BOLD_S
        0x6B => Some("L"),        // BOLD_L (unused)
        0x6C => Some("M"),        // BOLD_M (unused)
        0x6D => Some(":"),        // COLON (tinier dots)
        0x6E => Some("ぃ"),       // hiragana small i (unused) / LV in battle
        0x6F => Some("ぅ"),       // hiragana small u (unused)
        0x70 => Some("\u{2018}"), // ' opening single quote
        0x71 => Some("\u{2019}"), // ' closing single quote
        0x72 => Some("\u{201C}"), // " opening double quote
        0x73 => Some("\u{201D}"), // " closing double quote / ID in battle
        0x74 => Some("·"),        // middle dot (unused) / № in battle
        0x75 => Some("…"),        // ellipsis
        0x76 => Some("_"),        // underscore (naming screen) / hiragana small a (unused in JP)
        0x77 => Some("_"), // raised underscore (naming screen) / hiragana small e (unused in JP)
        0x78 => Some("ぉ"), // hiragana small o (unused)

        // Box-drawing characters
        0x79 => Some("┌"),
        0x7A => Some("─"),
        0x7B => Some("┐"),
        0x7C => Some("│"),
        0x7D => Some("└"),
        0x7E => Some("┘"),
        0x7F => Some(" "), // space

        // Uppercase A-Z
        0x80 => Some("A"),
        0x81 => Some("B"),
        0x82 => Some("C"),
        0x83 => Some("D"),
        0x84 => Some("E"),
        0x85 => Some("F"),
        0x86 => Some("G"),
        0x87 => Some("H"),
        0x88 => Some("I"),
        0x89 => Some("J"),
        0x8A => Some("K"),
        0x8B => Some("L"),
        0x8C => Some("M"),
        0x8D => Some("N"),
        0x8E => Some("O"),
        0x8F => Some("P"),
        0x90 => Some("Q"),
        0x91 => Some("R"),
        0x92 => Some("S"),
        0x93 => Some("T"),
        0x94 => Some("U"),
        0x95 => Some("V"),
        0x96 => Some("W"),
        0x97 => Some("X"),
        0x98 => Some("Y"),
        0x99 => Some("Z"),

        // Punctuation
        0x9A => Some("("),
        0x9B => Some(")"),
        0x9C => Some(":"),
        0x9D => Some(";"),
        0x9E => Some("["),
        0x9F => Some("]"),

        // Lowercase a-z
        0xA0 => Some("a"),
        0xA1 => Some("b"),
        0xA2 => Some("c"),
        0xA3 => Some("d"),
        0xA4 => Some("e"),
        0xA5 => Some("f"),
        0xA6 => Some("g"),
        0xA7 => Some("h"),
        0xA8 => Some("i"),
        0xA9 => Some("j"),
        0xAA => Some("k"),
        0xAB => Some("l"),
        0xAC => Some("m"),
        0xAD => Some("n"),
        0xAE => Some("o"),
        0xAF => Some("p"),
        0xB0 => Some("q"),
        0xB1 => Some("r"),
        0xB2 => Some("s"),
        0xB3 => Some("t"),
        0xB4 => Some("u"),
        0xB5 => Some("v"),
        0xB6 => Some("w"),
        0xB7 => Some("x"),
        0xB8 => Some("y"),
        0xB9 => Some("z"),

        // Special characters
        0xBA => Some("é"),
        0xBB => Some("'d"),
        0xBC => Some("'l"),
        0xBD => Some("'s"),
        0xBE => Some("'t"),
        0xBF => Some("'v"),

        // 0xC0-0xDF: unused in English (Japanese hiragana in JP version)
        0xC0..=0xDF => None,

        // More special characters
        0xE0 => Some("'"),
        0xE1 => Some("<PK>"),
        0xE2 => Some("<MN>"),
        0xE3 => Some("-"),
        0xE4 => Some("'r"),
        0xE5 => Some("'m"),
        0xE6 => Some("?"),
        0xE7 => Some("!"),
        0xE8 => Some("."),

        // Japanese katakana small (unused in English)
        0xE9 => Some("ァ"),
        0xEA => Some("ゥ"),
        0xEB => Some("ェ"),

        // Symbols
        0xEC => Some("▷"),
        0xED => Some("▶"),
        0xEE => Some("▼"),
        0xEF => Some("♂"),
        0xF0 => Some("ED"), // ED tile for naming screen (also ¥ yen symbol)
        0xF1 => Some("×"),
        0xF2 => Some("."), // decimal point (DOT)
        0xF3 => Some("/"),
        0xF4 => Some(","),
        0xF5 => Some("♀"),

        // Digits 0-9
        0xF6 => Some("0"),
        0xF7 => Some("1"),
        0xF8 => Some("2"),
        0xF9 => Some("3"),
        0xFA => Some("4"),
        0xFB => Some("5"),
        0xFC => Some("6"),
        0xFD => Some("7"),
        0xFE => Some("8"),
        0xFF => Some("9"),
    }
}

/// Encode an ASCII/UTF-8 character to the Pokémon charmap byte.
/// Returns None if the character has no mapping.
pub fn encode_char(c: char) -> Option<u8> {
    match c {
        'A' => Some(0x80),
        'B' => Some(0x81),
        'C' => Some(0x82),
        'D' => Some(0x83),
        'E' => Some(0x84),
        'F' => Some(0x85),
        'G' => Some(0x86),
        'H' => Some(0x87),
        'I' => Some(0x88),
        'J' => Some(0x89),
        'K' => Some(0x8A),
        'L' => Some(0x8B),
        'M' => Some(0x8C),
        'N' => Some(0x8D),
        'O' => Some(0x8E),
        'P' => Some(0x8F),
        'Q' => Some(0x90),
        'R' => Some(0x91),
        'S' => Some(0x92),
        'T' => Some(0x93),
        'U' => Some(0x94),
        'V' => Some(0x95),
        'W' => Some(0x96),
        'X' => Some(0x97),
        'Y' => Some(0x98),
        'Z' => Some(0x99),

        'a' => Some(0xA0),
        'b' => Some(0xA1),
        'c' => Some(0xA2),
        'd' => Some(0xA3),
        'e' => Some(0xA4),
        'f' => Some(0xA5),
        'g' => Some(0xA6),
        'h' => Some(0xA7),
        'i' => Some(0xA8),
        'j' => Some(0xA9),
        'k' => Some(0xAA),
        'l' => Some(0xAB),
        'm' => Some(0xAC),
        'n' => Some(0xAD),
        'o' => Some(0xAE),
        'p' => Some(0xAF),
        'q' => Some(0xB0),
        'r' => Some(0xB1),
        's' => Some(0xB2),
        't' => Some(0xB3),
        'u' => Some(0xB4),
        'v' => Some(0xB5),
        'w' => Some(0xB6),
        'x' => Some(0xB7),
        'y' => Some(0xB8),
        'z' => Some(0xB9),

        '(' => Some(0x9A),
        ')' => Some(0x9B),
        ':' => Some(0x9C),
        ';' => Some(0x9D),
        '[' => Some(0x9E),
        ']' => Some(0x9F),

        'é' => Some(0xBA),
        '\'' => Some(0xE0),
        '-' => Some(0xE3),
        '?' => Some(0xE6),
        '!' => Some(0xE7),
        '.' => Some(0xE8),
        '/' => Some(0xF3),
        ',' => Some(0xF4),
        ' ' => Some(0x7F),

        '♂' => Some(0xEF),
        '♀' => Some(0xF5),
        '×' => Some(0xF1),
        '▷' => Some(0xEC),
        '▶' => Some(0xED),
        '▼' => Some(0xEE),

        '0' => Some(0xF6),
        '1' => Some(0xF7),
        '2' => Some(0xF8),
        '3' => Some(0xF9),
        '4' => Some(0xFA),
        '5' => Some(0xFB),
        '6' => Some(0xFC),
        '7' => Some(0xFD),
        '8' => Some(0xFE),
        '9' => Some(0xFF),

        '@' => Some(0x50), // string terminator

        _ => None,
    }
}

/// Encode a string to Pokémon charmap bytes, appending the terminator (0x50).
/// Returns None if any character cannot be encoded.
pub fn encode_string(s: &str) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(s.len() + 1);
    for c in s.chars() {
        result.push(encode_char(c)?);
    }
    result.push(control_chars::CHAR_TERMINATOR);
    Some(result)
}

/// Decode a slice of Pokémon charmap bytes to a String.
/// Stops at the terminator (0x50) or end of slice.
/// Skips control characters (returns placeholder for them).
pub fn decode_string(bytes: &[u8]) -> String {
    let mut result = String::new();
    for &b in bytes {
        if b == control_chars::CHAR_TERMINATOR {
            break;
        }
        if let Some(s) = decode_char(b) {
            result.push_str(s);
        }
        // Control characters are silently skipped in simple decode
    }
    result
}

/// Maximum name length for player/rival names (including terminator)
pub const NAME_LENGTH: usize = 11;

/// Maximum length for Pokémon nicknames (including terminator)
pub const POKEMON_NAME_LENGTH: usize = 11;

/// Screen dimensions in tiles
pub const SCREEN_WIDTH: usize = 20;
pub const SCREEN_HEIGHT: usize = 18;

/// Text box dimensions
pub const TEXT_BOX_WIDTH: usize = 18; // inner width (excluding borders)
pub const TEXT_BOX_LINES: usize = 2; // lines visible in standard text box

pub const CHAR_SPACE: u8 = 0x7F;
pub const CHAR_TERMINATOR: u8 = control_chars::CHAR_TERMINATOR;

/// Special tile IDs for naming screen (not standard character codes)
pub mod naming_tiles {
    /// ED (End/Done) tile - loaded separately into vFont tile $70
    pub const ED_TILE: u8 = 0xF0;

    /// Underscore tile - from font_battle_extra.png (loaded at vChars2 tile $76)
    pub const UNDERSCORE: u8 = 0x76;

    /// Raised underscore tile - from font_battle_extra.png (loaded at vChars2 tile $77)
    pub const RAISED_UNDERSCORE: u8 = 0x77;

    /// Right arrow cursor tile
    pub const CURSOR_ARROW: u8 = 0xED;
}
