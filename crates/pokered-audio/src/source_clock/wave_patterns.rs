//! The nine explicitly declared Audio1/3 wave-pointer entries.
//!
//! Source `audio/wave_samples.asm` deliberately points entries 5..8 at the
//! following SFX data. Those bytes differ by bank; they are not out-of-range
//! instruments. Entries 0..4 are identical and reuse the native definitions.

const fn with_sfx_wave(wave: [u8; 16]) -> [[u8; 16]; 9] {
    [
        crate::WAVE_INSTRUMENTS[0],
        crate::WAVE_INSTRUMENTS[1],
        crate::WAVE_INSTRUMENTS[2],
        crate::WAVE_INSTRUMENTS[3],
        crate::WAVE_INSTRUMENTS[4],
        wave,
        wave,
        wave,
        wave,
    ]
}

// Audio1 02:43c3 (ROM 0x83c3): StartMenu's seven bytes, then Pokeflute.
static AUDIO1: [[u8; 16]; 9] = with_sfx_wave(crate::WAVE_INSTRUMENTS[5]);
// Audio3 1f:43c3 (ROM 0x7c3c3): StartMenu's seven bytes, then Cut.
static AUDIO3: [[u8; 16]; 9] = with_sfx_wave([
    0x21, 0xe2, 0x33, 0x28, 0xe1, 0x22, 0xff, 0x22, 0xf7, 0x24, 0x22, 0xf7, 0x34, 0x24, 0xf7, 0x44,
]);

pub(super) fn for_bank(bank: u8) -> Option<&'static [[u8; 16]; 9]> {
    match bank {
        2 => Some(&AUDIO1),
        31 => Some(&AUDIO3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::for_bank;

    #[test]
    fn all_nine_declared_entries_match_each_original_bank() {
        // Independent bytes from the pinned original ROM, SHA-256
        // 5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b.
        // Both tables are at bank:4361, with targets 4373,4383,4393,43a3,
        // 43b3,43c3,43c3,43c3,43c3. All ranges below are exactly 16 bytes.
        let common = [
            [
                0x02, 0x46, 0x8a, 0xce, 0xff, 0xfe, 0xed, 0xdc, 0xcb, 0xa9, 0x87, 0x65, 0x44, 0x33,
                0x22, 0x11,
            ],
            [
                0x02, 0x46, 0x8a, 0xce, 0xef, 0xff, 0xfe, 0xee, 0xdd, 0xcb, 0xa9, 0x87, 0x65, 0x43,
                0x22, 0x11,
            ],
            [
                0x13, 0x69, 0xbd, 0xee, 0xee, 0xff, 0xff, 0xed, 0xde, 0xff, 0xff, 0xee, 0xee, 0xdb,
                0x96, 0x31,
            ],
            [
                0x02, 0x46, 0x8a, 0xcd, 0xef, 0xfe, 0xde, 0xff, 0xee, 0xdc, 0xba, 0x98, 0x76, 0x54,
                0x32, 0x10,
            ],
            [
                0x01, 0x23, 0x45, 0x67, 0x8a, 0xcd, 0xee, 0xf7, 0x7f, 0xee, 0xdc, 0xa8, 0x76, 0x54,
                0x32, 0x10,
            ],
        ];
        for (bank, tail) in [
            (
                2,
                [
                    0x21, 0xe2, 0x33, 0x28, 0xe1, 0x22, 0xff, 0xea, 0x10, 0x14, 0xdc, 0x10, 0xe3,
                    0x41, 0x51, 0x73,
                ],
            ),
            (
                31,
                [
                    0x21, 0xe2, 0x33, 0x28, 0xe1, 0x22, 0xff, 0x22, 0xf7, 0x24, 0x22, 0xf7, 0x34,
                    0x24, 0xf7, 0x44,
                ],
            ),
        ] {
            let actual = for_bank(bank).unwrap();
            for (index, pattern) in actual.iter().enumerate() {
                let expected = if index < 5 { common[index] } else { tail };
                assert_eq!(*pattern, expected, "bank {bank}, wave {index}");
            }
        }
    }
}
