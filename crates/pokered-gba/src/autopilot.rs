//! Frame-indexed input script for automated GBA playtesting
//! (`--features autopilot`). Outside a script window the controller reads as
//! idle, so the ROM behaves like a hands-off attract run.
//!
//! Buttons are bit flags: A=1 B=2 Select=4 Start=8 Right=16 Left=32 Up=64
//! Down=128 R=256 L=512 (matches mGBA's key order).

/// (from_frame, until_frame, buttons). Evaluated in order; last match wins.
const SCRIPT: &[(u32, u32, u8)] = &[
    // LanguageSelect: confirm English (A) once the screen is up.
    (560, 568, 0b0000_0001),
    // IntroScene skips forward on any key.
    (700, 708, 0b0000_0001),
    (900, 908, 0b0000_0001),
    (1100, 1108, 0b0000_0001),
    (1300, 1308, 0b0000_0001),
];

/// Post-menu spam: confirm NEW GAME, mash A through Oak's speech and the
/// naming screens (A picks the default options), until the overworld is up.
/// Evaluated on top of SCRIPT.
const POST_MENU_SPAM: (u32, u32) = (940, 4000);

fn spam_at(frame: u32) -> u8 {
    let (from, until) = POST_MENU_SPAM;
    let t = frame.saturating_sub(from);
    if frame >= from && frame < until && t % 48 < 8 {
        return 0b0000_0001; // A
    }
    0
}

/// Returns the held-button bitmask for `frame`.
pub fn buttons_at(frame: u32) -> u8 {
    let mut held = spam_at(frame);
    for &(from, until, buttons) in SCRIPT {
        if frame >= from && frame < until {
            held = buttons;
        }
    }
    held
}
