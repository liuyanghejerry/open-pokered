use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

use crate::battle::state::{Pokemon, StatusCondition};
use crate::pokemon::move_learning::*;

fn make_pokemon(species: Species, level: u8, moves: [MoveId; 4]) -> Pokemon {
    let pp: [u8; 4] = [
        get_move_max_pp(moves[0]),
        get_move_max_pp(moves[1]),
        get_move_max_pp(moves[2]),
        get_move_max_pp(moves[3]),
    ];
    Pokemon {
        species,
        nickname: [0x50; 11],
        level,
        hp: 100,
        max_hp: 100,
        attack: 50,
        defense: 50,
        speed: 50,
        special: 50,
        type1: PokemonType::Grass,
        type2: PokemonType::Poison,
        moves,
        pp,
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

#[test]
fn is_hm_move_cut() {
    assert!(is_hm_move(MoveId::Cut));
    assert!(is_hm_move(MoveId::Fly));
    assert!(is_hm_move(MoveId::Surf));
    assert!(is_hm_move(MoveId::Strength));
    assert!(is_hm_move(MoveId::Flash));
}

#[test]
fn is_hm_move_regular_move() {
    assert!(!is_hm_move(MoveId::Tackle));
    assert!(!is_hm_move(MoveId::Thunderbolt));
}

#[test]
fn tm_to_move_valid() {
    assert_eq!(tm_to_move(1), Some(MoveId::MegaPunch));
    assert_eq!(tm_to_move(6), Some(MoveId::Toxic));
    assert_eq!(tm_to_move(50), Some(MoveId::Substitute));
}

#[test]
fn tm_to_move_invalid() {
    assert_eq!(tm_to_move(0), None);
    assert_eq!(tm_to_move(51), None);
}

#[test]
fn hm_to_move_valid() {
    assert_eq!(hm_to_move(1), Some(MoveId::Cut));
    assert_eq!(hm_to_move(5), Some(MoveId::Flash));
}

#[test]
fn hm_to_move_invalid() {
    assert_eq!(hm_to_move(0), None);
    assert_eq!(hm_to_move(6), None);
}

#[test]
fn bulbasaur_can_learn_tm06_toxic() {
    assert!(can_learn_tm(Species::Bulbasaur, 6));
}

#[test]
fn bulbasaur_cannot_learn_tm01_mega_punch() {
    assert!(!can_learn_tm(Species::Bulbasaur, 1));
}

#[test]
fn bulbasaur_can_learn_hm01_cut() {
    assert!(can_learn_hm(Species::Bulbasaur, 1));
}

#[test]
fn bulbasaur_cannot_learn_hm02_fly() {
    assert!(!can_learn_hm(Species::Bulbasaur, 2));
}

#[test]
fn can_learn_tm_invalid_range() {
    assert!(!can_learn_tm(Species::Bulbasaur, 0));
    assert!(!can_learn_tm(Species::Bulbasaur, 51));
}

#[test]
fn can_learn_hm_invalid_range() {
    assert!(!can_learn_hm(Species::Bulbasaur, 0));
    assert!(!can_learn_hm(Species::Bulbasaur, 6));
}

#[test]
fn try_learn_move_empty_slots() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    );
    let result = try_learn_move(&mut mon, MoveId::VineWhip);
    assert_eq!(result, LearnMoveResult::Learned { slot: 2 });
    assert_eq!(mon.moves[2], MoveId::VineWhip);
    assert!(mon.pp[2] > 0);
}

#[test]
fn try_learn_move_already_known() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    );
    let result = try_learn_move(&mut mon, MoveId::Tackle);
    assert_eq!(result, LearnMoveResult::AlreadyKnown);
}

#[test]
fn try_learn_move_slots_full() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        20,
        [
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::VineWhip,
            MoveId::LeechSeed,
        ],
    );
    let result = try_learn_move(&mut mon, MoveId::Poisonpowder);
    assert_eq!(result, LearnMoveResult::MoveSlotsFull);
}

#[test]
fn try_learn_move_none_is_noop() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let result = try_learn_move(&mut mon, MoveId::None);
    assert_eq!(result, LearnMoveResult::AlreadyKnown);
}

#[test]
fn replace_move_works() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        20,
        [
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::VineWhip,
            MoveId::LeechSeed,
        ],
    );
    replace_move(&mut mon, 1, MoveId::Poisonpowder);
    assert_eq!(mon.moves[1], MoveId::Poisonpowder);
    assert_eq!(mon.pp[1], get_move_max_pp(MoveId::Poisonpowder));
}

#[test]
fn replace_move_out_of_bounds_is_noop() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    replace_move(&mut mon, 5, MoveId::Growl);
    assert_eq!(mon.moves[0], MoveId::Tackle);
}

#[test]
fn teach_tm_compatible() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    );
    let result = teach_tm(&mut mon, 6);
    assert_eq!(result, Ok(LearnMoveResult::Learned { slot: 2 }));
    assert_eq!(mon.moves[2], MoveId::Toxic);
}

#[test]
fn teach_tm_incompatible() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let result = teach_tm(&mut mon, 1);
    assert_eq!(result, Err(TeachError::Incompatible));
}

#[test]
fn teach_tm_already_known() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Toxic, MoveId::None, MoveId::None, MoveId::None],
    );
    let result = teach_tm(&mut mon, 6);
    assert_eq!(result, Err(TeachError::AlreadyKnown));
}

#[test]
fn teach_tm_slots_full() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        20,
        [
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::VineWhip,
            MoveId::LeechSeed,
        ],
    );
    let result = teach_tm(&mut mon, 6);
    assert_eq!(result, Ok(LearnMoveResult::MoveSlotsFull));
}

#[test]
fn teach_tm_invalid_number() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    assert_eq!(teach_tm(&mut mon, 0), Err(TeachError::InvalidTmHm));
    assert_eq!(teach_tm(&mut mon, 51), Err(TeachError::InvalidTmHm));
}

#[test]
fn teach_hm_compatible() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let result = teach_hm(&mut mon, 1);
    assert_eq!(result, Ok(LearnMoveResult::Learned { slot: 1 }));
    assert_eq!(mon.moves[1], MoveId::Cut);
}

#[test]
fn teach_hm_incompatible() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let result = teach_hm(&mut mon, 2);
    assert_eq!(result, Err(TeachError::Incompatible));
}

#[test]
fn forget_regular_move() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::VineWhip,
            MoveId::None,
        ],
    );
    let result = forget_move(&mut mon, MoveId::Growl);
    assert_eq!(result, ForgetMoveResult::Forgotten { slot: 1 });
    assert_eq!(mon.moves[0], MoveId::Tackle);
    assert_eq!(mon.moves[1], MoveId::VineWhip);
    assert_eq!(mon.moves[2], MoveId::None);
    assert_eq!(mon.moves[3], MoveId::None);
}

#[test]
fn forget_hm_move_blocked() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Cut, MoveId::None, MoveId::None, MoveId::None],
    );
    let result = forget_move(&mut mon, MoveId::Cut);
    assert_eq!(result, ForgetMoveResult::CannotForgetHm);
    assert_eq!(mon.moves[0], MoveId::Cut);
}

#[test]
fn forget_unknown_move() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        5,
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    );
    let result = forget_move(&mut mon, MoveId::Growl);
    assert_eq!(result, ForgetMoveResult::MoveNotKnown);
}

#[test]
fn moves_at_level_bulbasaur_7() {
    let moves = moves_at_level(Species::Bulbasaur, 7);
    assert_eq!(moves, vec![MoveId::LeechSeed]);
}

#[test]
fn moves_at_level_no_moves() {
    let moves = moves_at_level(Species::Bulbasaur, 5);
    assert!(moves.is_empty());
}

#[test]
fn process_level_up_moves_bulbasaur_6_to_7() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        6,
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    );
    let result = process_level_up_moves(&mut mon, 6, 7);
    assert_eq!(result.learned.len(), 1);
    assert_eq!(result.learned[0].move_id, MoveId::LeechSeed);
    assert_eq!(result.learned[0].level, 7);
    assert!(result.pending.is_empty());
    assert_eq!(mon.moves[2], MoveId::LeechSeed);
}

#[test]
fn process_level_up_moves_slots_full_creates_pending() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        6,
        [
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::VineWhip,
            MoveId::Poisonpowder,
        ],
    );
    let result = process_level_up_moves(&mut mon, 6, 7);
    assert!(result.learned.is_empty());
    assert_eq!(result.pending.len(), 1);
    assert_eq!(result.pending[0].move_id, MoveId::LeechSeed);
    assert_eq!(result.pending[0].level, 7);
}

#[test]
fn process_level_up_moves_multi_level() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        6,
        [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
    );
    let result = process_level_up_moves(&mut mon, 6, 13);
    assert_eq!(result.learned.len(), 2);
    assert_eq!(result.learned[0].move_id, MoveId::LeechSeed);
    assert_eq!(result.learned[0].level, 7);
    assert_eq!(result.learned[1].move_id, MoveId::VineWhip);
    assert_eq!(result.learned[1].level, 13);
    assert!(result.pending.is_empty());
}

#[test]
fn process_level_up_skips_already_known() {
    let mut mon = make_pokemon(
        Species::Bulbasaur,
        6,
        [
            MoveId::Tackle,
            MoveId::Growl,
            MoveId::LeechSeed,
            MoveId::None,
        ],
    );
    let result = process_level_up_moves(&mut mon, 6, 7);
    assert!(result.learned.is_empty());
    assert!(result.pending.is_empty());
}

#[test]
fn get_move_max_pp_tackle() {
    assert_eq!(get_move_max_pp(MoveId::Tackle), 35);
}

#[test]
fn get_move_max_pp_none() {
    assert_eq!(get_move_max_pp(MoveId::None), 0);
}
