//! Party-menu selector used by scripts (e.g. the Name Rater in Lavender Town).
//!
//! Unlike [`crate::party_screen::PartyScreenState`] — which presents a
//! STATS/SWITCH/CANCEL action menu — this is a pure single-pick selector: the
//! player moves the cursor and presses A to pick a party member, or B to
//! cancel. It wraps a `PartyScreenState` purely so the existing party-screen
//! renderer can draw it unchanged.

use crate::battle::state::Pokemon;
use crate::party_screen::{PartyScreenInput, PartyScreenState};

/// Result of one frame of party-selection input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartySelectResult {
    /// Still choosing.
    Active,
    /// Player picked the party member at this 0-based index.
    Selected(usize),
    /// Player backed out (B).
    Cancelled,
}

/// Single-pick party selector.
#[derive(Debug, Clone)]
pub struct PartySelectState {
    inner: PartyScreenState,
}

impl PartySelectState {
    pub fn new(party: Vec<Pokemon>) -> Self {
        Self {
            inner: PartyScreenState::new(party),
        }
    }

    /// The wrapped party-screen state, for rendering.
    pub fn screen(&self) -> &PartyScreenState {
        &self.inner
    }

    pub fn cursor(&self) -> usize {
        self.inner.cursor()
    }

    pub fn party(&self) -> &[Pokemon] {
        self.inner.party()
    }

    /// Process one frame of input. A picks the highlighted member; B cancels.
    pub fn update_frame(&mut self, input: PartyScreenInput) -> PartySelectResult {
        if self.inner.party().is_empty() {
            // Nothing to pick — any confirm/back leaves the selector.
            if input.a || input.b {
                return PartySelectResult::Cancelled;
            }
            return PartySelectResult::Active;
        }

        if input.b {
            return PartySelectResult::Cancelled;
        }
        if input.a {
            return PartySelectResult::Selected(self.inner.cursor());
        }

        // Navigate only — never forward A/B into the wrapped browsing menu,
        // which would otherwise open its STATS/SWITCH/CANCEL sub-menu.
        let nav = PartyScreenInput {
            up: input.up,
            down: input.down,
            a: false,
            b: false,
        };
        self.inner.update_frame(nav);
        PartySelectResult::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon;
    use pokered_data::species::Species;

    fn party(n: usize) -> Vec<Pokemon> {
        let species = [
            Species::Bulbasaur,
            Species::Charmander,
            Species::Squirtle,
            Species::Pidgey,
        ];
        (0..n)
            .map(|i| create_pokemon(species[i % species.len()], 5, [0xFF, 0xFF]).unwrap())
            .collect()
    }

    fn press(state: &mut PartySelectState, up: bool, down: bool, a: bool, b: bool) -> PartySelectResult {
        state.update_frame(PartyScreenInput { up, down, a, b })
    }

    #[test]
    fn a_selects_current_cursor() {
        let mut s = PartySelectState::new(party(3));
        assert_eq!(press(&mut s, false, true, false, false), PartySelectResult::Active);
        assert_eq!(s.cursor(), 1);
        assert_eq!(press(&mut s, false, false, true, false), PartySelectResult::Selected(1));
    }

    #[test]
    fn b_cancels() {
        let mut s = PartySelectState::new(party(2));
        assert_eq!(press(&mut s, false, false, false, true), PartySelectResult::Cancelled);
    }

    #[test]
    fn a_does_not_open_action_menu() {
        // Pressing A must resolve to a selection, not enter STATS/SWITCH/CANCEL.
        let mut s = PartySelectState::new(party(2));
        let r = press(&mut s, false, false, true, false);
        assert_eq!(r, PartySelectResult::Selected(0));
    }

    #[test]
    fn cursor_clamps_to_party_bounds() {
        let mut s = PartySelectState::new(party(2));
        press(&mut s, false, true, false, false);
        press(&mut s, false, true, false, false);
        press(&mut s, false, true, false, false);
        assert_eq!(s.cursor(), 1);
        assert_eq!(press(&mut s, false, false, true, false), PartySelectResult::Selected(1));
    }
}
