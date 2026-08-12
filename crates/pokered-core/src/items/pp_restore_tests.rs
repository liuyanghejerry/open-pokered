use crate::battle::state::{Pokemon, StatusCondition};
use crate::items::pp_restore::*;
use pokered_data::items::ItemId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;

fn make_mon_with_pp(moves: [MoveId; 4], pp: [u8; 4], pp_ups: [u8; 4]) -> Pokemon {
    Pokemon {
        species: Species::Pikachu,
        nickname: [0x50; 11],
        level: 25,
        hp: 50,
        max_hp: 100,
        attack: 55,
        defense: 30,
        speed: 90,
        special: 50,
        type1: PokemonType::Electric,
        type2: PokemonType::Electric,
        moves,
        pp,
        pp_ups,
        status: StatusCondition::None,
        dv_bytes: [0xFF, 0xFF],
        stat_exp: [0; 5],
        total_exp: 0,
        is_traded: false, ot_id: 0, ot_name: [0x50; 11],
    }
}

#[test]
fn get_max_pp_no_ups() {
    // Thundershock base PP = 30
    assert_eq!(get_max_pp_with_ups(MoveId::Thundershock, 0), 30);
}

#[test]
fn get_max_pp_with_1_up() {
    // Thundershock base 30: bonus = floor(30/5) = 6, so 30 + 6 = 36
    assert_eq!(get_max_pp_with_ups(MoveId::Thundershock, 1), 36);
}

#[test]
fn get_max_pp_with_3_ups() {
    // Thundershock base 30: bonus = 6 * 3 = 18, so 30 + 18 = 48
    assert_eq!(get_max_pp_with_ups(MoveId::Thundershock, 3), 48);
}

#[test]
fn get_max_pp_bonus_capped_at_7() {
    // Growl base PP 40: floor(40/5) = 8 -> capped at 7, so 40 + 7 = 47 per up
    assert_eq!(get_max_pp_with_ups(MoveId::Growl, 1), 47);
}

#[test]
fn get_max_pp_none_move() {
    assert_eq!(get_max_pp_with_ups(MoveId::None, 0), 0);
}

#[test]
fn ether_restores_10_pp() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [10, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::Ether, 0);
    assert_eq!(
        result,
        PpRestoreResult::Restored {
            move_index: 0,
            pp_restored: 10
        }
    );
    assert_eq!(mon.pp[0], 20);
}

#[test]
fn ether_caps_at_max_pp() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [25, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::Ether, 0);
    assert_eq!(
        result,
        PpRestoreResult::Restored {
            move_index: 0,
            pp_restored: 5
        }
    );
    assert_eq!(mon.pp[0], 30);
}

#[test]
fn ether_no_effect_at_max() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [30, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::Ether, 0);
    assert_eq!(result, PpRestoreResult::NoEffect);
}

#[test]
fn max_ether_restores_all_pp() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [5, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::MaxEther, 0);
    assert_eq!(
        result,
        PpRestoreResult::Restored {
            move_index: 0,
            pp_restored: 25
        }
    );
    assert_eq!(mon.pp[0], 30);
}

#[test]
fn ether_respects_pp_ups() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [25, 0, 0, 0],
        [1, 0, 0, 0], // 1 PP Up: max = 36
    );
    let result = use_pp_restore(&mut mon, ItemId::Ether, 0);
    assert_eq!(
        result,
        PpRestoreResult::Restored {
            move_index: 0,
            pp_restored: 10
        }
    );
    assert_eq!(mon.pp[0], 35);
}

#[test]
fn elixir_restores_all_moves() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::QuickAttack,
            MoveId::ThunderWave,
            MoveId::None,
        ],
        [10, 15, 5, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::Elixer, 0);
    // ThunderShock: 10+10=20, QuickAttack(pp=30): 15+10=25, ThunderWave(pp=20): 5+10=15
    assert_eq!(mon.pp[0], 20);
    assert_eq!(mon.pp[1], 25);
    assert_eq!(mon.pp[2], 15);
    match result {
        PpRestoreResult::AllRestored { total_pp_restored } => {
            assert_eq!(total_pp_restored, 10 + 10 + 10);
        }
        _ => panic!("expected AllRestored"),
    }
}

#[test]
fn max_elixir_fully_restores_all_moves() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::QuickAttack,
            MoveId::None,
            MoveId::None,
        ],
        [0, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::MaxElixer, 0);
    assert_eq!(mon.pp[0], 30); // Thundershock max
    assert_eq!(mon.pp[1], 30); // QuickAttack max
    match result {
        PpRestoreResult::AllRestored { total_pp_restored } => {
            assert_eq!(total_pp_restored, 60);
        }
        _ => panic!("expected AllRestored"),
    }
}

#[test]
fn elixir_no_effect_when_all_full() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [30, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::Elixer, 0);
    assert_eq!(result, PpRestoreResult::NoEffect);
}

#[test]
fn pp_up_increases_count() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [30, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::PpUp, 0);
    assert_eq!(
        result,
        PpRestoreResult::PpUpApplied {
            move_index: 0,
            new_pp_up_count: 1
        }
    );
    assert_eq!(mon.pp_ups[0], 1);
    assert_eq!(mon.pp[0], 36); // 30 + 6
}

#[test]
fn pp_up_capped_at_3() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [48, 0, 0, 0],
        [3, 0, 0, 0],
    );
    let result = use_pp_restore(&mut mon, ItemId::PpUp, 0);
    assert_eq!(result, PpRestoreResult::NoEffect);
}

#[test]
fn pp_up_on_empty_slot_no_effect() {
    let mut mon = make_mon_with_pp(
        [MoveId::None, MoveId::None, MoveId::None, MoveId::None],
        [0; 4],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::PpUp, 0);
    assert_eq!(result, PpRestoreResult::NoEffect);
}

#[test]
fn ether_on_invalid_index_no_effect() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [10, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::Ether, 5);
    assert_eq!(result, PpRestoreResult::NoEffect);
}

#[test]
fn non_pp_item_not_applicable() {
    let mut mon = make_mon_with_pp(
        [
            MoveId::Thundershock,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ],
        [10, 0, 0, 0],
        [0; 4],
    );
    let result = use_pp_restore(&mut mon, ItemId::Potion, 0);
    assert_eq!(result, PpRestoreResult::NotApplicable);
}
