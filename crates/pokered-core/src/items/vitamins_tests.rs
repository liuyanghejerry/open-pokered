use crate::battle::state::{Pokemon, StatusCondition};
use crate::items::vitamins::*;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_mon_with_stat_exp(stat_exp: [u16; 5]) -> Pokemon {
    Pokemon {
        species: Species::Bulbasaur,
        nickname: [0x50; 11],
        level: 50,
        hp: 100,
        max_hp: 100,
        attack: 50,
        defense: 50,
        speed: 50,
        special: 50,
        type1: PokemonType::Grass,
        type2: PokemonType::Poison,
        moves: [MoveId::VineWhip, MoveId::None, MoveId::None, MoveId::None],
        pp: [10, 0, 0, 0],
        pp_ups: [0; 4],
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp,
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

#[test]
fn hp_up_adds_stat_exp() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    let result = use_vitamin(&mut mon, ItemId::HpUp);
    assert_eq!(
        result,
        VitaminResult::Applied {
            stat_index: 0,
            new_stat_exp: 2560
        }
    );
    assert_eq!(mon.stat_exp[0], 2560);
}

#[test]
fn protein_adds_attack_stat_exp() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    let result = use_vitamin(&mut mon, ItemId::Protein);
    assert_eq!(
        result,
        VitaminResult::Applied {
            stat_index: 1,
            new_stat_exp: 2560
        }
    );
}

#[test]
fn iron_adds_defense_stat_exp() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    let result = use_vitamin(&mut mon, ItemId::Iron);
    assert_eq!(
        result,
        VitaminResult::Applied {
            stat_index: 2,
            new_stat_exp: 2560
        }
    );
}

#[test]
fn carbos_adds_speed_stat_exp() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    let result = use_vitamin(&mut mon, ItemId::Carbos);
    assert_eq!(
        result,
        VitaminResult::Applied {
            stat_index: 3,
            new_stat_exp: 2560
        }
    );
}

#[test]
fn calcium_adds_special_stat_exp() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    let result = use_vitamin(&mut mon, ItemId::Calcium);
    assert_eq!(
        result,
        VitaminResult::Applied {
            stat_index: 4,
            new_stat_exp: 2560
        }
    );
}

#[test]
fn vitamin_caps_at_25600() {
    let mut mon = make_mon_with_stat_exp([24000, 0, 0, 0, 0]);
    let result = use_vitamin(&mut mon, ItemId::HpUp);
    assert_eq!(
        result,
        VitaminResult::Applied {
            stat_index: 0,
            new_stat_exp: 25600
        }
    );
}

#[test]
fn vitamin_no_effect_at_cap() {
    let mut mon = make_mon_with_stat_exp([25600, 0, 0, 0, 0]);
    let result = use_vitamin(&mut mon, ItemId::HpUp);
    assert_eq!(result, VitaminResult::NoEffect);
}

#[test]
fn vitamin_no_effect_on_fainted() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    mon.hp = 0;
    let result = use_vitamin(&mut mon, ItemId::HpUp);
    assert_eq!(result, VitaminResult::NoEffect);
}

#[test]
fn vitamin_recalculates_stats() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    let old_max_hp = mon.max_hp;
    use_vitamin(&mut mon, ItemId::HpUp);
    assert!(mon.max_hp >= old_max_hp);
}

#[test]
fn non_vitamin_not_applicable() {
    let mut mon = make_mon_with_stat_exp([0; 5]);
    let result = use_vitamin(&mut mon, ItemId::Potion);
    assert_eq!(result, VitaminResult::NotApplicable);
}

#[test]
fn rare_candy_levels_up() {
    let mut mon =
        crate::pokemon::stats::create_pokemon(Species::Bulbasaur, 10, [0xFF, 0xFF]).unwrap();
    let result = use_rare_candy(&mut mon).unwrap();
    assert_eq!(result.new_level, 11);
    assert_eq!(mon.level, 11);
}

#[test]
fn rare_candy_no_effect_at_100() {
    let mut mon =
        crate::pokemon::stats::create_pokemon(Species::Bulbasaur, 100, [0xFF, 0xFF]).unwrap();
    let result = use_rare_candy(&mut mon);
    assert!(result.is_none());
}

#[test]
fn rare_candy_no_effect_on_fainted() {
    let mut mon =
        crate::pokemon::stats::create_pokemon(Species::Bulbasaur, 10, [0xFF, 0xFF]).unwrap();
    mon.hp = 0;
    let result = use_rare_candy(&mut mon);
    assert!(result.is_none());
}

#[test]
fn rare_candy_recalculates_stats() {
    let mut mon =
        crate::pokemon::stats::create_pokemon(Species::Bulbasaur, 10, [0xFF, 0xFF]).unwrap();
    let old_max_hp = mon.max_hp;
    use_rare_candy(&mut mon);
    assert!(mon.max_hp > old_max_hp);
}

#[test]
fn rare_candy_updates_total_exp() {
    let mut mon =
        crate::pokemon::stats::create_pokemon(Species::Bulbasaur, 10, [0xFF, 0xFF]).unwrap();
    let old_exp = mon.total_exp;
    use_rare_candy(&mut mon);
    assert!(mon.total_exp > old_exp);
}
