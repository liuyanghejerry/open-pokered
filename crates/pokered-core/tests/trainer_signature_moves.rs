//! ReadTrainer overrides checked against original raw table offsets/branches.
use pokered_core::battle::special_moves::apply_trainer_special_moves;
use pokered_core::pokemon::stats::create_pokemon;
use pokered_data::trainer_data::{get_trainer_party_mons, gym_leader_no, lone_move, TrainerClass};
use serde_json::Value;

fn reference() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/original_trainer_signature_moves.json"
    ))
    .unwrap()
}

fn party(class: TrainerClass, index: usize) -> Vec<pokered_core::battle::state::Pokemon> {
    get_trainer_party_mons(class, index)
        .unwrap()
        .iter()
        .map(|m| create_pokemon(m.species, m.level, [0x98, 0x88]).unwrap())
        .collect()
}

#[test]
fn all_eight_leaders_use_the_original_zero_based_party_offset() {
    for row in reference()["lone_moves"].as_array().unwrap() {
        let class: TrainerClass = serde_json::from_value(row["class"].clone()).unwrap();
        let target = row["target"].as_u64().unwrap() as usize;
        let mv = serde_json::from_value(row["move"].clone()).unwrap();
        assert_eq!(
            gym_leader_no(class),
            Some(row["gym_no"].as_u64().unwrap() as u8)
        );
        assert_eq!(
            lone_move(row["gym_no"].as_u64().unwrap() as u8),
            Some((target, mv)),
            "{class:?}"
        );
        let mut mons = party(class, row["party_index"].as_u64().unwrap() as usize);
        let before: Vec<_> = mons.iter().map(|m| m.moves).collect();
        assert_eq!(
            serde_json::to_value(mons[target].species).unwrap(),
            row["species"]
        );
        apply_trainer_special_moves(class, &mut mons);
        for (index, mon) in mons.iter().enumerate() {
            let mut expected = before[index];
            if index == target {
                expected[2] = mv;
            }
            assert_eq!(mon.moves, expected, "{class:?} member {index}");
        }
    }
}

#[test]
fn all_three_champion_starters_receive_their_own_signature_move() {
    for row in reference()["champion_moves"].as_array().unwrap() {
        let mut mons = party(
            TrainerClass::Rival3,
            row["party_index"].as_u64().unwrap() as usize,
        );
        let before: Vec<_> = mons.iter().map(|m| m.moves).collect();
        assert_eq!(
            serde_json::to_value(mons[5].species).unwrap(),
            row["species"]
        );
        apply_trainer_special_moves(TrainerClass::Rival3, &mut mons);
        for (index, mon) in mons.iter().enumerate() {
            let mut expected = before[index];
            if index == 0 {
                expected[2] = pokered_data::moves::MoveId::SkyAttack;
            }
            if index == 5 {
                expected[2] = serde_json::from_value(row["move"].clone()).unwrap();
            }
            assert_eq!(mon.moves, expected, "{} member {index}", row["species"]);
        }
    }
}

#[test]
fn giovanni_hideout_and_silph_parties_do_not_gain_the_gym_signature() {
    for index in 0..2 {
        let mut mons = party(TrainerClass::Giovanni, index);
        let before: Vec<_> = mons.iter().map(|m| m.moves).collect();
        apply_trainer_special_moves(TrainerClass::Giovanni, &mut mons);
        assert_eq!(
            mons.iter().map(|m| m.moves).collect::<Vec<_>>(),
            before,
            "Giovanni party {index}"
        );
    }
}
