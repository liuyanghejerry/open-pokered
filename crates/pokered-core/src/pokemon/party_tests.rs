use super::party::*;
use crate::battle::state::{Pokemon, StatusCondition};
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_pokemon(species: Species, level: u8) -> Pokemon {
    Pokemon {
        species,
        nickname: None,
        level,
        hp: 50,
        max_hp: 50,
        attack: 40,
        defense: 40,
        speed: 40,
        special: 40,
        type1: PokemonType::Normal,
        type2: PokemonType::Normal,
        moves: [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        pp: [35, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xAA, 0xAA],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: None,
    }
}

#[test]
fn new_party_is_empty() {
    let party = Party::new();
    assert!(party.is_empty());
    assert_eq!(party.count(), 0);
    assert!(!party.is_full());
    assert!(party.leader().is_none());
    assert_eq!(party.leader_level(), 0);
}

#[test]
fn default_is_empty() {
    let party = Party::default();
    assert!(party.is_empty());
}

#[test]
fn add_pokemon() {
    let mut party = Party::new();
    let idx = party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(party.count(), 1);
    assert!(!party.is_empty());

    let idx2 = party.add(make_pokemon(Species::Charmander, 10)).unwrap();
    assert_eq!(idx2, 1);
    assert_eq!(party.count(), 2);
}

#[test]
fn add_up_to_six() {
    let mut party = Party::new();
    for i in 0..PARTY_LENGTH {
        let result = party.add(make_pokemon(Species::Pidgey, (i + 1) as u8));
        assert!(result.is_ok());
    }
    assert!(party.is_full());
    assert_eq!(party.count(), PARTY_LENGTH);

    let result = party.add(make_pokemon(Species::Rattata, 5));
    assert_eq!(result, Err(PartyError::PartyFull));
}

#[test]
fn leader_is_first() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Squirtle, 8)).unwrap();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();

    assert_eq!(party.leader().unwrap().species, Species::Squirtle);
    assert_eq!(party.leader_level(), 8);
}

#[test]
fn remove_pokemon() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();
    party.add(make_pokemon(Species::Squirtle, 15)).unwrap();

    let removed = party.remove(1).unwrap();
    assert_eq!(removed.species, Species::Charmander);
    assert_eq!(party.count(), 2);
    assert_eq!(party.get(0).unwrap().species, Species::Bulbasaur);
    assert_eq!(party.get(1).unwrap().species, Species::Squirtle);
}

#[test]
fn cannot_remove_last() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();
    assert_eq!(party.remove(0), Err(PartyError::CannotRemoveLast));
}

#[test]
fn remove_out_of_bounds() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();
    assert_eq!(party.remove(5), Err(PartyError::IndexOutOfBounds));
}

#[test]
fn swap_pokemon() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();
    party.add(make_pokemon(Species::Squirtle, 15)).unwrap();

    party.swap(0, 2).unwrap();
    assert_eq!(party.get(0).unwrap().species, Species::Squirtle);
    assert_eq!(party.get(2).unwrap().species, Species::Bulbasaur);
}

#[test]
fn swap_same_index_errors() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();
    assert_eq!(party.swap(0, 0), Err(PartyError::SameIndex));
}

#[test]
fn swap_out_of_bounds() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();
    assert_eq!(party.swap(0, 5), Err(PartyError::IndexOutOfBounds));
}

#[test]
fn species_list() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();
    assert_eq!(
        party.species_list(),
        vec![Species::Bulbasaur, Species::Charmander]
    );
}

#[test]
fn find_species() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();
    party.add(make_pokemon(Species::Squirtle, 15)).unwrap();

    assert_eq!(party.find_species(Species::Charmander), Some(1));
    assert_eq!(party.find_species(Species::Pikachu), None);
}

#[test]
fn alive_count_and_fainted() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();
    party.add(make_pokemon(Species::Squirtle, 15)).unwrap();

    assert_eq!(party.alive_count(), 3);
    assert!(!party.all_fainted());

    party.get_mut(0).unwrap().hp = 0;
    assert_eq!(party.alive_count(), 2);
    assert!(!party.all_fainted());

    party.get_mut(1).unwrap().hp = 0;
    party.get_mut(2).unwrap().hp = 0;
    assert_eq!(party.alive_count(), 0);
    assert!(party.all_fainted());
}

#[test]
fn first_alive_index() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();
    party.add(make_pokemon(Species::Squirtle, 15)).unwrap();

    assert_eq!(party.first_alive_index(), Some(0));

    party.get_mut(0).unwrap().hp = 0;
    assert_eq!(party.first_alive_index(), Some(1));

    party.get_mut(1).unwrap().hp = 0;
    assert_eq!(party.first_alive_index(), Some(2));

    party.get_mut(2).unwrap().hp = 0;
    assert_eq!(party.first_alive_index(), None);
}

#[test]
fn empty_party_not_all_fainted() {
    let party = Party::new();
    assert!(!party.all_fainted());
    assert_eq!(party.first_alive_index(), None);
}

#[test]
fn heal_all() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();

    party.get_mut(0).unwrap().hp = 10;
    party.get_mut(0).unwrap().status = StatusCondition::Poison;
    party.get_mut(0).unwrap().pp[0] = 0;
    party.get_mut(1).unwrap().hp = 0;
    party.get_mut(1).unwrap().status = StatusCondition::Burn;
    party.get_mut(1).unwrap().pp[0] = 0;

    party.heal_all();

    assert_eq!(party.get(0).unwrap().hp, 50);
    assert_eq!(party.get(0).unwrap().status, StatusCondition::None);
    assert!(party.get(0).unwrap().pp[0] > 0, "PP should be restored");
    assert_eq!(party.get(1).unwrap().hp, 50);
    assert_eq!(party.get(1).unwrap().status, StatusCondition::None);
    assert!(party.get(1).unwrap().pp[0] > 0, "PP should be restored");
}

#[test]
fn from_pokemon_vec() {
    let mons = vec![
        make_pokemon(Species::Bulbasaur, 5),
        make_pokemon(Species::Charmander, 10),
    ];
    let party = Party::from_pokemon(mons).unwrap();
    assert_eq!(party.count(), 2);
}

#[test]
fn from_pokemon_too_many() {
    let mons: Vec<Pokemon> = (0..7)
        .map(|i| make_pokemon(Species::Pidgey, i + 1))
        .collect();
    assert_eq!(Party::from_pokemon(mons), Err(PartyError::PartyFull));
}

#[test]
fn from_vec_trait() {
    let mons = vec![make_pokemon(Species::Pikachu, 25)];
    let party: Party = mons.into();
    assert_eq!(party.count(), 1);
    assert_eq!(party.leader().unwrap().species, Species::Pikachu);
}

#[test]
fn into_vec_trait() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();

    let mons: Vec<Pokemon> = party.into();
    assert_eq!(mons.len(), 2);
    assert_eq!(mons[0].species, Species::Bulbasaur);
}

#[test]
fn to_vec_clones() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();

    let cloned = party.to_vec();
    assert_eq!(cloned.len(), 1);
    assert_eq!(party.count(), 1);
}

#[test]
fn iter_and_iter_mut() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Bulbasaur, 5)).unwrap();
    party.add(make_pokemon(Species::Charmander, 10)).unwrap();

    let species: Vec<Species> = party.iter().map(|p| p.species).collect();
    assert_eq!(species, vec![Species::Bulbasaur, Species::Charmander]);

    for mon in party.iter_mut() {
        mon.level += 1;
    }
    assert_eq!(party.get(0).unwrap().level, 6);
    assert_eq!(party.get(1).unwrap().level, 11);
}

#[test]
fn get_and_get_mut() {
    let mut party = Party::new();
    party.add(make_pokemon(Species::Pikachu, 25)).unwrap();

    assert!(party.get(0).is_some());
    assert!(party.get(1).is_none());

    party.get_mut(0).unwrap().level = 30;
    assert_eq!(party.get(0).unwrap().level, 30);
}
