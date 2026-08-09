#[cfg(test)]
mod tests {
    use pokered_data::moves::MoveId;
    use pokered_data::species::Species;
    use pokered_data::types::PokemonType;

    use crate::battle::settlement::evolution::*;
    use crate::battle::state::{Pokemon, StatusCondition};

    fn make_pokemon(species: Species, level: u8) -> Pokemon {
        let base = pokered_data::pokemon_data::get_base_stats(species).unwrap();
        Pokemon {
            species,
            nickname: None,
            level,
            hp: 50,
            max_hp: 50,
            attack: 30,
            defense: 30,
            speed: 30,
            special: 30,
            type1: base.type1,
            type2: base.type2,
            moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            pp: [35, 0, 0, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    #[test]
    fn bulbasaur_evolves_at_16() {
        assert_eq!(
            check_level_evolution(Species::Bulbasaur, 16),
            Some(Species::Ivysaur)
        );
    }

    #[test]
    fn bulbasaur_no_evolve_at_15() {
        assert_eq!(check_level_evolution(Species::Bulbasaur, 15), None);
    }

    #[test]
    fn charmeleon_evolves_at_36() {
        assert_eq!(
            check_level_evolution(Species::Charmeleon, 36),
            Some(Species::Charizard)
        );
    }

    #[test]
    fn kadabra_trade_evolution() {
        assert_eq!(
            check_trade_evolution(Species::Kadabra, 1),
            Some(Species::Alakazam)
        );
    }

    #[test]
    fn pikachu_item_evolution() {
        assert_eq!(
            check_item_evolution(
                Species::Pikachu,
                1,
                pokered_data::items::ItemId::ThunderStone
            ),
            Some(Species::Raichu)
        );
    }

    #[test]
    fn mew_no_evolution() {
        assert_eq!(check_level_evolution(Species::Mew, 100), None);
    }

    #[test]
    fn apply_evolution_updates_species_and_stats() {
        let mut mon = make_pokemon(Species::Bulbasaur, 16);
        let old_species = mon.species;
        apply_evolution(&mut mon, Species::Ivysaur);

        assert_eq!(mon.species, Species::Ivysaur);
        assert_ne!(old_species, mon.species);
        assert!(mon.max_hp > 0);
    }

    #[test]
    fn apply_evolution_adjusts_hp_by_delta() {
        let mut mon = make_pokemon(Species::Bulbasaur, 32);
        mon.max_hp = 60;
        mon.hp = 40;

        apply_evolution(&mut mon, Species::Ivysaur);

        let hp_delta = mon.max_hp as i32 - 60;
        let expected_hp = (40 + hp_delta).max(0) as u16;
        assert_eq!(mon.hp, expected_hp);
    }

    #[test]
    fn apply_evolution_updates_types() {
        let mut mon = make_pokemon(Species::Charmander, 36);
        mon.type1 = PokemonType::Fire;
        mon.type2 = PokemonType::Fire;

        apply_evolution(&mut mon, Species::Charizard);

        assert_eq!(mon.type1, PokemonType::Fire);
        assert_eq!(mon.type2, PokemonType::Flying);
    }
}
