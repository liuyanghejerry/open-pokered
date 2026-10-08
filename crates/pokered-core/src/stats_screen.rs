use crate::battle::state::Pokemon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsPage {
    /// Page 1: name, level, HP, status, types, OT, ID, stat box (ATTACK/DEFENSE/SPEED/SPECIAL)
    Stats,
    /// Page 2: moves with PP, EXP points, level up information
    Moves,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsScreenInput {
    pub a: bool,
    pub b: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsScreenAction {
    Continue,
    BackToParty,
}

#[derive(Debug, Clone)]
pub struct StatsScreenState {
    pub pokemon: Pokemon,
    pub page: StatsPage,
    entry_frame: Option<u16>,
    entry_cry_pending: bool,
}

impl StatsScreenState {
    pub fn new(pokemon: Pokemon) -> Self {
        Self {
            pokemon,
            page: StatsPage::Stats,
            entry_frame: None,
            entry_cry_pending: false,
        }
    }

    /// StatusScreen recalculates MON_STATS for BOX_DATA at the stored box
    /// level, without changing the saved current HP or the box itself.
    pub fn from_box(mut pokemon: Pokemon) -> Self {
        let stored_hp = pokemon.hp;
        crate::pokemon::stats::recalculate_stats(&mut pokemon);
        pokemon.hp = stored_hp;
        Self::new(pokemon)
    }

    /// Production entry keeps the previous menu for the trigger frame, then
    /// blanks while tile patterns load, then loads the selected front picture.
    pub fn start_entry(&mut self) { self.entry_frame = Some(0); }

    pub fn with_entry(mut self) -> Self { self.start_entry(); self }

    pub fn entry_frame(&self) -> Option<u16> { self.entry_frame }

    pub fn entry_cry_frame(&self) -> u16 {
        crate::stats_entry_timing::CRY_FRAME_BY_INDEX.get(self.pokemon.species.to_rom_id() as usize)
            .copied().filter(|&f| f != 0).unwrap_or(71)
    }

    pub fn entry_picture_parts(&self) -> u8 {
        let Some(frame) = self.entry_frame else { return 3; };
        let elapsed = frame.saturating_sub(self.entry_cry_frame());
        if frame <= self.entry_cry_frame() { return 0; }
        if elapsed >= 3 { return 3; }
        crate::stats_entry_timing::PICTURE_PARTS_BY_INDEX
            .get(self.pokemon.species.to_rom_id() as usize)
            .map(|p| p[usize::from(elapsed - 1)]).unwrap_or(3)
    }

    pub fn entry_blocks_input(&self) -> bool {
        self.entry_frame.is_some_and(|f| f <= self.entry_cry_frame())
    }

    pub fn take_entry_cry(&mut self) -> bool {
        core::mem::take(&mut self.entry_cry_pending)
    }

    pub fn pokemon(&self) -> &Pokemon {
        &self.pokemon
    }

    pub fn page(&self) -> StatsPage {
        self.page
    }

    pub fn update(&mut self, input: StatsScreenInput) -> StatsScreenAction {
        if let Some(frame) = self.entry_frame {
            let cry = self.entry_cry_frame();
            let next = (frame + 1).min(cry + 3);
            self.entry_frame = Some(next);
            if frame < cry && next == cry { self.entry_cry_pending = true; }
            if next <= cry { return StatsScreenAction::Continue; }
        }
        // Original callers display StatusScreen then StatusScreen2,
        // each returning on A or B (WaitForTextScrollButtonPress).
        if input.a || input.b {
            match self.page {
                StatsPage::Stats => self.page = StatsPage::Moves,
                StatsPage::Moves => return StatsScreenAction::BackToParty,
            }
        }

        StatsScreenAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::species::Species;

    #[test]
    fn box_stats_restore_derived_values_from_sram_without_healing() {
        use crate::save::ser_pokemon::{serialize_box_mon, deserialize_box_mon};
        let mut mon = crate::pokemon::stats::create_pokemon(Species::Pikachu, 12, [0x9a, 0x78]).unwrap();
        mon.hp = 7;
        mon.stat_exp = [10000, 20000, 30000, 40000, 50000];
        crate::pokemon::stats::recalculate_stats(&mut mon);
        let mut bytes = alloc::vec::Vec::new();
        serialize_box_mon(&mon, &mut bytes);
        let restored = deserialize_box_mon(&bytes).unwrap();
        assert_eq!(restored.attack, 0);
        let displayed = StatsScreenState::from_box(restored).pokemon;
        assert_eq!((displayed.max_hp, displayed.attack, displayed.defense, displayed.speed, displayed.special),
            (mon.max_hp, mon.attack, mon.defense, mon.speed, mon.special));
        assert_eq!(displayed.hp, mon.hp);
        assert_eq!(restored.attack, 0);
    }

    fn make_test_pokemon(species: Species) -> Pokemon {
        crate::pokemon::stats::create_pokemon(species, 5, [0xFF, 0xFF]).unwrap()
    }

    #[test]
    fn test_stats_screen_initial_page_is_stats() {
        let pokemon = make_test_pokemon(Species::Bulbasaur);
        let screen = StatsScreenState::new(pokemon);
        assert_eq!(screen.page(), StatsPage::Stats);
    }

    #[test]
    fn either_button_advances_both_original_status_screens_then_returns() {
        for first in [StatsScreenInput { a: true, b: false }, StatsScreenInput { a: false, b: true }] {
            for second in [StatsScreenInput { a: true, b: false }, StatsScreenInput { a: false, b: true }] {
                let mut screen = StatsScreenState::new(make_test_pokemon(Species::Bulbasaur));
                assert_eq!(screen.update(first), StatsScreenAction::Continue);
                assert_eq!(screen.page(), StatsPage::Moves);
                assert_eq!(screen.update(second), StatsScreenAction::BackToParty);
            }
        }
    }

    #[test]
    fn test_stays_on_moves_when_no_input() {
        let pokemon = make_test_pokemon(Species::Bulbasaur);
        let mut screen = StatsScreenState::new(pokemon);

        // Go to Moves page
        screen.update(StatsScreenInput { a: true, b: false });
        assert_eq!(screen.page(), StatsPage::Moves);

        // No input → stays on Moves
        let action = screen.update(StatsScreenInput { a: false, b: false });
        assert_eq!(action, StatsScreenAction::Continue);
        assert_eq!(screen.page(), StatsPage::Moves);
    }
}
