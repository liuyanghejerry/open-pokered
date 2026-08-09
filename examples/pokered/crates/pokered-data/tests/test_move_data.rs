use pokered_data::move_data::MOVES;
use pokered_data::moves::MoveId;
use pokered_data::types::PokemonType;
use strum::IntoEnumIterator;

#[test]
fn moves_has_165_entries() {
    assert_eq!(MOVES.len(), 165);
}

#[test]
fn moves_ordered_by_id() {
    for (i, m) in MOVES.iter().enumerate() {
        let expected_id = (i + 1) as u8;
        assert_eq!(
            m.id as u8, expected_id,
            "MOVES[{}] has id {:?} ({}), expected {}",
            i, m.id, m.id as u8, expected_id
        );
    }
}

#[test]
fn every_move_id_has_data() {
    for move_id in MoveId::iter() {
        if move_id == MoveId::None {
            continue;
        }
        let idx = move_id as usize;
        assert!(idx >= 1 && idx <= 165, "MoveId {:?} out of range", move_id);
        assert_eq!(
            MOVES[idx - 1].id,
            move_id,
            "MoveId mismatch for {:?}",
            move_id
        );
    }
}

#[test]
fn move_pp_nonzero() {
    for m in MOVES.iter() {
        assert!(m.pp > 0, "{:?} has 0 PP", m.id);
    }
}

#[test]
fn move_accuracy_in_range() {
    for m in MOVES.iter() {
        assert!(
            m.accuracy <= 100,
            "{:?} has accuracy {} > 100",
            m.id,
            m.accuracy
        );
    }
}

#[test]
fn move_type_is_valid() {
    for m in MOVES.iter() {
        let _ = PokemonType::iter()
            .find(|&t| t == m.move_type)
            .unwrap_or_else(|| panic!("{:?} has invalid type {:?}", m.id, m.move_type));
    }
}

#[test]
fn spot_check_tackle() {
    let tackle = &MOVES[MoveId::Tackle as usize - 1];
    assert_eq!(tackle.power, 35);
    assert_eq!(tackle.move_type, PokemonType::Normal);
    assert_eq!(tackle.accuracy, 95);
    assert_eq!(tackle.pp, 35);
}

#[test]
fn spot_check_thunder() {
    let thunder = &MOVES[MoveId::Thunder as usize - 1];
    assert_eq!(thunder.power, 120);
    assert_eq!(thunder.move_type, PokemonType::Electric);
    assert_eq!(thunder.accuracy, 70);
    assert_eq!(thunder.pp, 10);
}

#[test]
fn spot_check_psychic() {
    let psychic = &MOVES[MoveId::PsychicM as usize - 1];
    assert_eq!(psychic.power, 90);
    assert_eq!(psychic.move_type, PokemonType::Psychic);
    assert_eq!(psychic.accuracy, 100);
    assert_eq!(psychic.pp, 10);
}
