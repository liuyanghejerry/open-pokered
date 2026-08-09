use crate::battle::state::{Pokemon, StatusCondition};
use crate::items::healing::*;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_mon(hp: u16, max_hp: u16) -> Pokemon {
    Pokemon {
        species: Species::Pikachu,
        nickname: None,
        level: 25,
        hp,
        max_hp,
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
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: None,
    }
}

#[test]
fn potion_heals_20() {
    let mut mon = make_mon(50, 100);
    let result = use_healing_item(&mut mon, ItemId::Potion);
    assert_eq!(result, HealResult::Healed { hp_restored: 20 });
    assert_eq!(mon.hp, 70);
}

#[test]
fn super_potion_heals_50() {
    let mut mon = make_mon(30, 100);
    let result = use_healing_item(&mut mon, ItemId::SuperPotion);
    assert_eq!(result, HealResult::Healed { hp_restored: 50 });
    assert_eq!(mon.hp, 80);
}

#[test]
fn hyper_potion_heals_200() {
    let mut mon = make_mon(50, 300);
    let result = use_healing_item(&mut mon, ItemId::HyperPotion);
    assert_eq!(result, HealResult::Healed { hp_restored: 200 });
    assert_eq!(mon.hp, 250);
}

#[test]
fn potion_caps_at_max_hp() {
    let mut mon = make_mon(95, 100);
    let result = use_healing_item(&mut mon, ItemId::Potion);
    assert_eq!(result, HealResult::Healed { hp_restored: 5 });
    assert_eq!(mon.hp, 100);
}

#[test]
fn potion_no_effect_at_full_hp() {
    let mut mon = make_mon(100, 100);
    let result = use_healing_item(&mut mon, ItemId::Potion);
    assert_eq!(result, HealResult::AlreadyFullHp);
}

#[test]
fn potion_fails_on_fainted() {
    let mut mon = make_mon(0, 100);
    let result = use_healing_item(&mut mon, ItemId::Potion);
    assert_eq!(result, HealResult::NotFainted);
}

#[test]
fn max_potion_restores_full_hp() {
    let mut mon = make_mon(1, 200);
    let result = use_healing_item(&mut mon, ItemId::MaxPotion);
    assert_eq!(result, HealResult::Healed { hp_restored: 199 });
    assert_eq!(mon.hp, 200);
}

#[test]
fn full_restore_heals_and_cures_status() {
    let mut mon = make_mon(30, 100);
    mon.status = StatusCondition::Burn;
    let result = use_healing_item(&mut mon, ItemId::FullRestore);
    assert_eq!(result, HealResult::Healed { hp_restored: 70 });
    assert_eq!(mon.hp, 100);
    assert_eq!(mon.status, StatusCondition::None);
}

#[test]
fn full_restore_cures_status_at_full_hp() {
    let mut mon = make_mon(100, 100);
    mon.status = StatusCondition::Poison;
    let result = use_healing_item(&mut mon, ItemId::FullRestore);
    assert_eq!(result, HealResult::Healed { hp_restored: 0 });
    assert_eq!(mon.status, StatusCondition::None);
}

#[test]
fn full_restore_no_effect_when_healthy() {
    let mut mon = make_mon(100, 100);
    let result = use_healing_item(&mut mon, ItemId::FullRestore);
    assert_eq!(result, HealResult::AlreadyFullHp);
}

#[test]
fn fresh_water_heals_50() {
    let mut mon = make_mon(10, 100);
    let result = use_healing_item(&mut mon, ItemId::FreshWater);
    assert_eq!(result, HealResult::Healed { hp_restored: 50 });
    assert_eq!(mon.hp, 60);
}

#[test]
fn soda_pop_heals_60() {
    let mut mon = make_mon(10, 100);
    let result = use_healing_item(&mut mon, ItemId::SodaPop);
    assert_eq!(result, HealResult::Healed { hp_restored: 60 });
    assert_eq!(mon.hp, 70);
}

#[test]
fn lemonade_heals_80() {
    let mut mon = make_mon(10, 100);
    let result = use_healing_item(&mut mon, ItemId::Lemonade);
    assert_eq!(result, HealResult::Healed { hp_restored: 80 });
    assert_eq!(mon.hp, 90);
}

#[test]
fn revive_restores_half_hp() {
    let mut mon = make_mon(0, 100);
    let result = use_healing_item(&mut mon, ItemId::Revive);
    assert_eq!(result, HealResult::Revived { hp_restored: 50 });
    assert_eq!(mon.hp, 50);
}

#[test]
fn revive_fails_if_not_fainted() {
    let mut mon = make_mon(50, 100);
    let result = use_healing_item(&mut mon, ItemId::Revive);
    assert_eq!(result, HealResult::NotFainted);
}

#[test]
fn max_revive_restores_full_hp() {
    let mut mon = make_mon(0, 200);
    let result = use_healing_item(&mut mon, ItemId::MaxRevive);
    assert_eq!(result, HealResult::Revived { hp_restored: 200 });
    assert_eq!(mon.hp, 200);
}

#[test]
fn revive_gives_at_least_1_hp() {
    let mut mon = make_mon(0, 1);
    let result = use_healing_item(&mut mon, ItemId::Revive);
    assert_eq!(result, HealResult::Revived { hp_restored: 1 });
    assert_eq!(mon.hp, 1);
}

#[test]
fn non_healing_item_not_applicable() {
    let mut mon = make_mon(50, 100);
    let result = use_healing_item(&mut mon, ItemId::Antidote);
    assert_eq!(result, HealResult::NotApplicable);
}
