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
    // Once the bedroom is stable, exercise player/camera redraw invalidation
    // before returning to a hands-off soak.
    (4100, 4116, 0b1000_0000), // Down
    (4200, 4216, 0b0001_0000), // Right
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

/// Original frame-indexed opening, retained to check the unchanged windows.
#[cfg(test)]
fn buttons_at(frame: u32) -> u8 {
    scripted_buttons(frame, spam_at(frame))
}

fn scripted_buttons(frame: u32, mut held: u8) -> u8 {
    for &(from, until, buttons) in SCRIPT {
        if frame >= from && frame < until {
            held = buttons;
        }
    }
    held
}

/// Stop confirming opening menus once normal play starts. All opt-in
/// diagnostic ROMs share this gate; their scenario drivers still replace
/// or add their own input afterward. Fixed SCRIPT windows remain unchanged.
#[derive(Default)]
pub struct AutopilotInput {
    reached_overworld: bool,
}

impl AutopilotInput {
    pub fn buttons_at(&mut self, frame: u32, is_overworld: bool) -> u8 {
        self.reached_overworld |= is_overworld;
        scripted_buttons(frame, if self.reached_overworld { 0 } else { spam_at(frame) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autopilot_keeps_opening_and_menu_inputs_until_overworld() {
        let mut input = AutopilotInput::default();
        for frame in 0..2600 {
            assert_eq!(input.buttons_at(frame, false), buttons_at(frame));
        }
        assert_eq!(buttons_at(2620), 1);
        assert_eq!(input.buttons_at(2620, true), 0);
    }

    #[test]
    fn autopilot_never_restarts_menu_spam_after_leaving_overworld() {
        let mut input = AutopilotInput::default();
        assert_eq!(input.buttons_at(2620, true), 0);
        assert_eq!(buttons_at(2668), 1);
        assert_eq!(input.buttons_at(2668, false), 0);
        assert_eq!(input.buttons_at(2716, false), 0);
    }

    #[test]
    fn autopilot_preserves_both_literal_movement_windows() {
        let mut input = AutopilotInput::default();
        input.buttons_at(2620, true);
        for frame in 4000..4400 {
            let expected = match frame {
                4100..=4115 => 128,
                4200..=4215 => 16,
                _ => 0,
            };
            assert_eq!(input.buttons_at(frame, true), expected, "frame {frame}");
        }
    }

    #[test]
    fn stopping_menu_spam_preserves_non_menu_intro_buttons() {
        let mut input = AutopilotInput::default();
        assert_eq!(input.buttons_at(0, true), 0);
        for frame in [560, 567, 700, 707, 900, 907, 1100, 1107, 1300, 1307] {
            assert_eq!(input.buttons_at(frame, false), 1, "frame {frame}");
        }
        assert_eq!(input.buttons_at(940, false), 0);
        assert_eq!(input.buttons_at(2668, false), 0);
    }

    #[test]
    fn opening_menu_spam_keeps_its_original_windows() {
        assert_eq!(buttons_at(940), 1);
        assert_eq!(buttons_at(947), 1);
        assert_eq!(buttons_at(948), 0);
        assert_eq!(buttons_at(2668), 1);
        assert_eq!(buttons_at(3964), 1);
        assert_eq!(buttons_at(4000), 0);
    }
}
