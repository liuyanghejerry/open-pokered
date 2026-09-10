// Minimal Game Boy charmap encoding for menu/font rendering.
// Maps ASCII/UTF-8 characters to Game Boy tile IDs (matching pokered's charmap).

/// Encode an ASCII/UTF-8 character to the Game Boy charmap byte.
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

        'é' => Some(0xBA),
        '\'' => Some(0xE0),
        '-' => Some(0xE3),
        '?' => Some(0xE6),
        '!' => Some(0xE7),
        '.' => Some(0xE8),
        '/' => Some(0xF3),
        ',' => Some(0xF4),
        ' ' => Some(0x7F),

        '×' => Some(0xF1),

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

        _ => None,
    }
}

/// Encode a string to Game Boy charmap bytes.
pub fn encode_str(s: &str) -> Vec<u8> {
    s.chars().filter_map(encode_char).collect()
}
