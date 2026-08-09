use crate::battle::state::{Pokemon, StatusCondition};
use crate::items::status_cure::*;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_mon(status: StatusCondition) -> Pokemon {
    Pokemon {
        species: Species::Pikachu,
        nickname: None,
        level: 25,
        hp: 50,
        max_hp: 100,
        attack: 55,
        defense: 30,
        speed: 90,
        special: 50,
        type1: PokemonType::Electric,
        type2: PokemonType::Electric,
        moves: [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        pp: [30, 0, 0, 0],
        pp_ups: [0; 4],
        status,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: None,
    }
}

#[test]
fn antidote_cures_poison() {
    let mut mon = make_mon(StatusCondition::Poison);
    assert_eq!(
        use_status_cure(&mut mon, ItemId::Antidote),
        StatusCureResult::Cured
    );
    assert_eq!(mon.status, StatusCondition::None);
}

#[test]
fn antidote_no_effect_on_burn() {
    let mut mon = make_mon(StatusCondition::Burn);
    assert_eq!(
        use_status_cure(&mut mon, ItemId::Antidote),
        StatusCureResult::NoEffect
    );
    assert_eq!(mon.status, StatusCondition::Burn);
}

#[test]
fn burn_heal_cures_burn() {
    let mut mon = make_mon(StatusCondition::Burn);
    assert_eq!(
        use_status_cure(&mut mon, ItemId::BurnHeal),
        StatusCureResult::Cured
    );
    assert_eq!(mon.status, StatusCondition::None);
}

#[test]
fn ice_heal_cures_freeze() {
    let mut mon = make_mon(StatusCondition::Freeze);
    assert_eq!(
        use_status_cure(&mut mon, ItemId::IceHeal),
        StatusCureResult::Cured
    );
    assert_eq!(mon.status, StatusCondition::None);
}

#[test]
fn awakening_cures_sleep() {
    let mut mon = make_mon(StatusCondition::Sleep(3));
    assert_eq!(
        use_status_cure(&mut mon, ItemId::Awakening),
        StatusCureResult::Cured
    );
    assert_eq!(mon.status, StatusCondition::None);
}

#[test]
fn parlyz_heal_cures_paralysis() {
    let mut mon = make_mon(StatusCondition::Paralysis);
    assert_eq!(
        use_status_cure(&mut mon, ItemId::ParlyzHeal),
        StatusCureResult::Cured
    );
    assert_eq!(mon.status, StatusCondition::None);
}

#[test]
fn full_heal_cures_any_status() {
    for status in [
        StatusCondition::Poison,
        StatusCondition::Burn,
        StatusCondition::Freeze,
        StatusCondition::Sleep(5),
        StatusCondition::Paralysis,
    ] {
        let mut mon = make_mon(status);
        assert_eq!(
            use_status_cure(&mut mon, ItemId::FullHeal),
            StatusCureResult::Cured
        );
        assert_eq!(mon.status, StatusCondition::None);
    }
}

#[test]
fn full_heal_no_effect_when_healthy() {
    let mut mon = make_mon(StatusCondition::None);
    assert_eq!(
        use_status_cure(&mut mon, ItemId::FullHeal),
        StatusCureResult::NoEffect
    );
}

#[test]
fn cure_fails_on_fainted() {
    let mut mon = make_mon(StatusCondition::Poison);
    mon.hp = 0;
    assert_eq!(
        use_status_cure(&mut mon, ItemId::Antidote),
        StatusCureResult::NoEffect
    );
    assert_eq!(mon.status, StatusCondition::Poison);
}

#[test]
fn non_cure_item_not_applicable() {
    let mut mon = make_mon(StatusCondition::Poison);
    assert_eq!(
        use_status_cure(&mut mon, ItemId::Potion),
        StatusCureResult::NotApplicable
    );
}
