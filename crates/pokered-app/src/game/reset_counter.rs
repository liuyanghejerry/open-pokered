//! The original hSoftReset byte, independent of input-owner scheduling.
use pokered_renderer::input::GbButton;

pub(super) struct SoftResetCounter {
    remaining: u8,
}

impl SoftResetCounter {
    pub(super) const fn new() -> Self {
        Self {
            remaining: super::SOFT_RESET_HOLD_FRAMES,
        }
    }

    pub(super) fn is_combo(raw: u8) -> bool {
        const BUTTONS: u8 = (1 << GbButton::A as u8)
            | (1 << GbButton::B as u8)
            | (1 << GbButton::Select as u8)
            | (1 << GbButton::Start as u8);
        raw == BUTTONS
    }

    /// A caller supplies a real raw sample. Only TrySoftReset's exact
    /// PAD_BUTTONS path decrements; a different input never reseeds Init.
    /// True means the caller must perform reset before another poll.
    pub(super) fn poll(&mut self, raw: u8) -> bool {
        if !Self::is_combo(raw) {
            return false;
        }
        self.remaining = self.remaining.saturating_sub(1);
        self.remaining == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_buttons_decrement_all_256_raw_masks() {
        for raw in 0u8..=255 {
            let mut counter = SoftResetCounter::new();
            assert!(!counter.poll(raw));
            assert_eq!(counter.remaining, if raw == 0x0f { 15 } else { 16 });
        }
    }

    #[test]
    fn eight_combo_neutral_and_eight_combo_reach_reset() {
        let mut counter = SoftResetCounter::new();
        for _ in 0..8 {
            assert!(!counter.poll(0x0f));
        }
        assert!(!counter.poll(0));
        assert!(!counter.poll(0x4f), "extra UP is not a reset poll");
        for _ in 0..7 {
            assert!(!counter.poll(0x0f));
        }
        assert!(counter.poll(0x0f));
    }

    #[test]
    fn a_new_boot_counter_requires_all_sixteen_polls_again() {
        let mut old = SoftResetCounter::new();
        for _ in 0..15 {
            assert!(!old.poll(0x0f));
        }
        assert!(old.poll(0x0f));
        let mut after_init = SoftResetCounter::new();
        for _ in 0..15 {
            assert!(!after_init.poll(0x0f));
        }
        assert!(after_init.poll(0x0f));
    }
}
