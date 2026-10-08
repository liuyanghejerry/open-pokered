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
}

impl StatsScreenState {
    pub fn new(pokemon: Pokemon) -> Self {
        Self {
            pokemon,
            page: StatsPage::Stats,
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

    pub fn pokemon(&self) -> &Pokemon {
        &self.pokemon
    }

    pub fn page(&self) -> StatsPage {
        self.page
    }

    pub fn update(&mut self, input: StatsScreenInput) -> StatsScreenAction {
        if input.a {
            self.page = match self.page {
                StatsPage::Stats => StatsPage::Moves,
                StatsPage::Moves => StatsPage::Stats,
            };
            return StatsScreenAction::Continue;
        }

        if input.b {
            match self.page {
                StatsPage::Stats => return StatsScreenAction::BackToParty,
                StatsPage::Moves => {
                    self.page = StatsPage::Stats;
                    return StatsScreenAction::Continue;
                }
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
    fn test_a_toggles_to_moves_and_back() {
        let pokemon = make_test_pokemon(Species::Bulbasaur);
        let mut screen = StatsScreenState::new(pokemon);

        // A → Moves
        let action = screen.update(StatsScreenInput { a: true, b: false });
        assert_eq!(action, StatsScreenAction::Continue);
        assert_eq!(screen.page(), StatsPage::Moves);

        // A → Stats
        let action = screen.update(StatsScreenInput { a: true, b: false });
        assert_eq!(action, StatsScreenAction::Continue);
        assert_eq!(screen.page(), StatsPage::Stats);
    }

    #[test]
    fn test_b_on_moves_returns_to_stats_page() {
        let pokemon = make_test_pokemon(Species::Bulbasaur);
        let mut screen = StatsScreenState::new(pokemon);

        // Go to Moves page
        screen.update(StatsScreenInput { a: true, b: false });
        assert_eq!(screen.page(), StatsPage::Moves);

        // B on Moves → back to Stats
        let action = screen.update(StatsScreenInput { a: false, b: true });
        assert_eq!(action, StatsScreenAction::Continue);
        assert_eq!(screen.page(), StatsPage::Stats);
    }

    #[test]
    fn test_b_on_stats_returns_back_to_party() {
        let pokemon = make_test_pokemon(Species::Bulbasaur);
        let mut screen = StatsScreenState::new(pokemon);

        // B on Stats → BackToParty
        let action = screen.update(StatsScreenInput { a: false, b: true });
        assert_eq!(action, StatsScreenAction::BackToParty);
        assert_eq!(screen.page(), StatsPage::Stats);
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
