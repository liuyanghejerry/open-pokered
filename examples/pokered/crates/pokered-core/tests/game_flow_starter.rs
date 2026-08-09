mod game_flow_helpers;

use game_flow_helpers::*;
use pokered_core::battle::state::*;
use pokered_core::battle::turn::execute_turn;
use pokered_core::naming_screen::{
    NamingInput, NamingScreenResult, NamingScreenState, NamingScreenType,
};
use pokered_core::pokemon::pokedex::Pokedex;
use pokered_data::move_data::MoveData;
use pokered_data::moves::MoveEffect;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

#[test]
fn bulbasaur_starter_has_correct_stats_and_moves() {
    let pokemon = make_starter_bulbasaur();
    assert_eq!(pokemon.species, Species::Bulbasaur);
    assert_eq!(pokemon.level, 5);
    assert_eq!(pokemon.type1, PokemonType::Grass);
    assert_eq!(pokemon.type2, PokemonType::Poison);
    assert_eq!(pokemon.moves[0], MoveId::Tackle);
    assert_eq!(pokemon.moves[1], MoveId::Growl);
    assert_eq!(pokemon.moves[2], MoveId::None);
    assert_eq!(pokemon.moves[3], MoveId::None);
    assert_eq!(pokemon.hp, pokemon.max_hp);
}

#[test]
fn charmander_starter_has_correct_stats_and_moves() {
    let pokemon = make_starter_charmander();
    assert_eq!(pokemon.species, Species::Charmander);
    assert_eq!(pokemon.level, 5);
    assert_eq!(pokemon.type1, PokemonType::Fire);
    assert_eq!(pokemon.type2, PokemonType::Fire);
    assert_eq!(pokemon.moves[0], MoveId::Scratch);
    assert_eq!(pokemon.moves[1], MoveId::Growl);
    assert_eq!(pokemon.hp, pokemon.max_hp);
}

#[test]
fn squirtle_starter_has_correct_stats_and_moves() {
    let pokemon = make_starter_squirtle();
    assert_eq!(pokemon.species, Species::Squirtle);
    assert_eq!(pokemon.level, 5);
    assert_eq!(pokemon.type1, PokemonType::Water);
    assert_eq!(pokemon.type2, PokemonType::Water);
    assert_eq!(pokemon.moves[0], MoveId::Tackle);
    assert_eq!(pokemon.moves[1], MoveId::TailWhip);
    assert_eq!(pokemon.hp, pokemon.max_hp);
}

#[test]
fn starter_party_has_exactly_one_pokemon() {
    let party = party_at_start(make_starter_charmander());
    assert_eq!(party.count(), 1);
    assert!(!party.is_full());
    assert!(!party.is_empty());
    assert_eq!(party.leader().unwrap().species, Species::Charmander);
}

#[test]
fn rival_gets_type_advantage_starter() {
    let player_bulbasaur = make_starter_bulbasaur();
    let rival_charmander = make_starter_charmander();
    assert_eq!(player_bulbasaur.type1, PokemonType::Grass);
    assert_eq!(rival_charmander.type1, PokemonType::Fire);

    let player_charmander = make_starter_charmander();
    let rival_squirtle = make_starter_squirtle();
    assert_eq!(player_charmander.type1, PokemonType::Fire);
    assert_eq!(rival_squirtle.type1, PokemonType::Water);

    let player_squirtle = make_starter_squirtle();
    let rival_bulbasaur = make_starter_bulbasaur();
    assert_eq!(player_squirtle.type1, PokemonType::Water);
    assert_eq!(rival_bulbasaur.type1, PokemonType::Grass);
}

#[test]
fn rival_battle_in_oaks_lab_level5_vs_level5() {
    let player = make_starter_charmander();
    let rival = make_starter_squirtle();

    assert_eq!(player.level, 5);
    assert_eq!(rival.level, 5);

    let scratch_data = MoveData {
        id: MoveId::Scratch,
        effect: MoveEffect::NoAdditionalEffect,
        power: 40,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 35,
    };
    let tackle_data = MoveData {
        id: MoveId::Tackle,
        effect: MoveEffect::NoAdditionalEffect,
        power: 35,
        move_type: PokemonType::Normal,
        accuracy: 255,
        pp: 35,
    };

    let mut state = new_battle_state(BattleType::Trainer, vec![player], vec![rival]);
    state.player.selected_move = MoveId::Scratch;
    state.player.selected_move_index = 0;
    state.enemy.selected_move = MoveId::Tackle;
    state.enemy.selected_move_index = 0;

    let randoms = turn_randoms_player_first();
    execute_turn(&mut state, &scratch_data, &tackle_data, &randoms);

    assert!(
        state.player.active_mon().hp < state.player.active_mon().max_hp
            || state.enemy.active_mon().hp < state.enemy.active_mon().max_hp,
        "At least one side should take damage in the first rival battle"
    );
}

#[test]
fn pokedex_registers_starter_as_owned() {
    let mut dex = Pokedex::new();
    let starter = make_starter_charmander();

    dex.set_owned(starter.species);
    assert!(dex.is_owned(Species::Charmander));
    assert!(dex.is_seen(Species::Charmander));
    assert_eq!(dex.owned_count(), 1);
    assert_eq!(dex.seen_count(), 1);
}

#[test]
fn pokedex_registers_rival_pokemon_as_seen() {
    let mut dex = Pokedex::new();

    dex.set_owned(Species::Charmander);
    dex.set_seen(Species::Squirtle);

    assert!(dex.is_owned(Species::Charmander));
    assert!(dex.is_seen(Species::Squirtle));
    assert!(!dex.is_owned(Species::Squirtle));
    assert_eq!(dex.owned_count(), 1);
    assert_eq!(dex.seen_count(), 2);
}

#[test]
fn naming_screen_player_name_submit() {
    let mut screen = NamingScreenState::new(NamingScreenType::Player);
    assert_eq!(screen.max_length(), 7);

    let result = screen.update_frame(NamingInput {
        a: true,
        ..NamingInput::none()
    }, false);
    assert_eq!(result, NamingScreenResult::Editing);
    assert_eq!(screen.name().len(), 1);

    let result = screen.update_frame(NamingInput {
        start: true,
        ..NamingInput::none()
    }, false);
    assert!(matches!(result, NamingScreenResult::Submitted(_)));
}

#[test]
fn naming_screen_rival_name_max_7_chars() {
    let mut screen = NamingScreenState::new(NamingScreenType::Rival);
    assert_eq!(screen.max_length(), 7);

    for _ in 0..7 {
        screen.update_frame(NamingInput {
            a: true,
            ..NamingInput::none()
        }, false);
    }
    assert_eq!(screen.name().len(), 7);

    screen.update_frame(NamingInput {
        a: true,
        ..NamingInput::none()
    }, false);
    assert_eq!(screen.name().len(), 7);
}

#[test]
fn all_three_starters_have_distinct_types() {
    let b = make_starter_bulbasaur();
    let c = make_starter_charmander();
    let s = make_starter_squirtle();

    assert_ne!(b.type1, c.type1);
    assert_ne!(c.type1, s.type1);
    assert_ne!(b.type1, s.type1);
}
