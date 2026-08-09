use crate::battle::stat_stages::StatIndex;
use crate::battle::state::*;
use crate::items::battle_items::*;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_battler() -> BattlerState {
    let mon = Pokemon {
        species: Species::Pikachu,
        nickname: None,
        level: 25,
        hp: 55,
        max_hp: 55,
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
    };
    new_battler_state(vec![mon])
}

#[test]
fn x_attack_boosts_attack() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::XAttack);
    assert_eq!(
        result,
        BattleItemResult::StatBoosted {
            stat: StatIndex::Attack
        }
    );
    assert_eq!(battler.stat_stages.attack, 1);
}

#[test]
fn x_defend_boosts_defense() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::XDefend);
    assert_eq!(
        result,
        BattleItemResult::StatBoosted {
            stat: StatIndex::Defense
        }
    );
    assert_eq!(battler.stat_stages.defense, 1);
}

#[test]
fn x_speed_boosts_speed() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::XSpeed);
    assert_eq!(
        result,
        BattleItemResult::StatBoosted {
            stat: StatIndex::Speed
        }
    );
    assert_eq!(battler.stat_stages.speed, 1);
}

#[test]
fn x_special_boosts_special() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::XSpecial);
    assert_eq!(
        result,
        BattleItemResult::StatBoosted {
            stat: StatIndex::Special
        }
    );
    assert_eq!(battler.stat_stages.special, 1);
}

#[test]
fn x_stat_no_effect_at_max_stage() {
    let mut battler = make_battler();
    battler.stat_stages.attack = 6;
    let result = use_battle_item(&mut battler, ItemId::XAttack);
    assert_eq!(result, BattleItemResult::NoEffect);
}

#[test]
fn x_accuracy_sets_flag() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::XAccuracy);
    assert_eq!(result, BattleItemResult::FlagSet);
    assert!(battler.has_status2(status2::USING_X_ACCURACY));
}

#[test]
fn x_accuracy_no_effect_if_already_set() {
    let mut battler = make_battler();
    battler.set_status2(status2::USING_X_ACCURACY);
    let result = use_battle_item(&mut battler, ItemId::XAccuracy);
    assert_eq!(result, BattleItemResult::NoEffect);
}

#[test]
fn guard_spec_sets_mist_flag() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::GuardSpec);
    assert_eq!(result, BattleItemResult::FlagSet);
    assert!(battler.has_status2(status2::PROTECTED_BY_MIST));
}

#[test]
fn guard_spec_no_effect_if_already_set() {
    let mut battler = make_battler();
    battler.set_status2(status2::PROTECTED_BY_MIST);
    let result = use_battle_item(&mut battler, ItemId::GuardSpec);
    assert_eq!(result, BattleItemResult::NoEffect);
}

#[test]
fn dire_hit_sets_pumped_flag() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::DireHit);
    assert_eq!(result, BattleItemResult::FlagSet);
    assert!(battler.has_status2(status2::GETTING_PUMPED));
}

#[test]
fn dire_hit_no_effect_if_already_set() {
    let mut battler = make_battler();
    battler.set_status2(status2::GETTING_PUMPED);
    let result = use_battle_item(&mut battler, ItemId::DireHit);
    assert_eq!(result, BattleItemResult::NoEffect);
}

#[test]
fn poke_doll_returns_escaped() {
    let result = use_poke_doll();
    assert_eq!(result, BattleItemResult::Escaped);
}

#[test]
fn non_battle_item_not_applicable() {
    let mut battler = make_battler();
    let result = use_battle_item(&mut battler, ItemId::Potion);
    assert_eq!(result, BattleItemResult::NotApplicable);
}
