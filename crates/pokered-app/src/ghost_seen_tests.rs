use super::*;
use pokered_data::{items::ItemId, maps::MapId, species::Species};

#[test]
fn encounter_records_underlying_species_as_seen_even_when_displayed_as_ghost() {
    for (map, scope, species, ghost) in [
        (MapId::PokemonTower6F, false, Species::Marowak, true),
        (MapId::PokemonTower3F, false, Species::Gastly, true),
        (MapId::PokemonTower6F, true, Species::Marowak, false),
        (MapId::Route1, false, Species::Pidgey, false),
    ] {
        let mut game = PokemonGame::new_with_options(
            GameVersion::Red, None, None, None, false, None, false, true,
            #[cfg(feature = "debug-server")]
            None,
        );
        game.overworld.state.current_map = map;
        if scope {
            game.save_data.game_data.bag.add_item(ItemId::SilphScope, 1).unwrap();
        }
        assert!(!game.save_data.game_data.pokedex.is_seen(species));
        assert!(!game.save_data.game_data.pokedex.is_owned(species));
        game.start_wild_battle(species, 30);
        assert_eq!(game.battle.is_ghost, ghost);
        assert!(game.save_data.game_data.pokedex.is_seen(species), "{map:?}/{species:?}");
        assert!(!game.save_data.game_data.pokedex.is_owned(species));
    }
}
