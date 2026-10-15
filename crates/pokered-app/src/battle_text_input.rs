//! Convert the existing physical input owner to the source's logical bits.
//! No previous-input or frame counter lives in this adapter.
use pokered_renderer::input::{GbButton, InputState};
use pokered_core::battle::{BattleTextFrame, BattleTextSettings};

// The pinned engine stores the source's eight logical bits directly. Make
// that contract a compile-time check so a later engine remap cannot silently
// change text input, and avoid eight held-key polls on every battle frame.
const _: () = {
    assert!(GbButton::A as u8 == 0);
    assert!(GbButton::B as u8 == 1);
    assert!(GbButton::Select as u8 == 2);
    assert!(GbButton::Start as u8 == 3);
    assert!(GbButton::Right as u8 == 4);
    assert!(GbButton::Left as u8 == 5);
    assert!(GbButton::Up as u8 == 6);
    assert!(GbButton::Down as u8 == 7);
};

#[inline]
pub(crate) fn frame(input: &InputState, settings: Option<BattleTextSettings>) -> BattleTextFrame {
    BattleTextFrame {
        held_buttons: input.raw_current(),
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
