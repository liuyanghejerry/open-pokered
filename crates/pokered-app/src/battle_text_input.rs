//! Convert the existing physical input owner to the source's logical bits.
//! No previous-input or frame counter lives in this adapter.
use pokered_renderer::input::{GbButton, InputState};
use pokered_core::battle::{BattleTextFrame, BattleTextSettings};

pub(crate) fn frame(input: &InputState, settings: Option<BattleTextSettings>) -> BattleTextFrame {
    let mut held_buttons = 0;
    // e533 currently uses the same layout, but keep the boundary explicit:
    // source work must not silently depend on an engine enum's discriminants.
    for (button, source_bit) in [
        (GbButton::A, 0x01),
        (GbButton::B, 0x02),
        (GbButton::Select, 0x04),
        (GbButton::Start, 0x08),
        (GbButton::Right, 0x10),
        (GbButton::Left, 0x20),
        (GbButton::Up, 0x40),
        (GbButton::Down, 0x80),
    ] {
        if input.is_held(button) {
            held_buttons |= source_bit;
        }
    }
    BattleTextFrame {
        held_buttons,
        settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_bits_cover_every_physical_key_and_do_not_require_a_fresh_edge() {
        for (key, expected) in [
            (GbButton::A, 1),
            (GbButton::B, 2),
            (GbButton::Select, 4),
            (GbButton::Start, 8),
            (GbButton::Right, 16),
            (GbButton::Left, 32),
            (GbButton::Up, 64),
            (GbButton::Down, 128),
        ] {
            let mut input = InputState::new();
            input.press(key);
            assert_eq!(frame(&input, None).held_buttons, expected);
            input.begin_frame();
            assert!(!input.is_just_pressed(key));
            assert_eq!(frame(&input, None).held_buttons, expected);
        }
        let mut input = InputState::new();
        for key in [
            GbButton::A,
            GbButton::Select,
            GbButton::Left,
            GbButton::Down,
        ] {
            input.press(key);
        }
        assert_eq!(frame(&input, None).held_buttons, 0xa5);
        input.release(GbButton::Left);
        assert_eq!(frame(&input, None).held_buttons, 0x85);
    }
}
